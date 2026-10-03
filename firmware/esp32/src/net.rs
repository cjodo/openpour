//! Wi-Fi (station, or a fallback access point), mDNS, the embedded web app,
//! the REST endpoints and the /ws WebSocket. Requests arrive on the HTTP
//! server's task; anything that changes machine state is queued as a JSON
//! command line and handled by the main loop.

use std::ffi::CString;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex};

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::modem::Modem;
use esp_idf_svc::handle::RawHandle;
use esp_idf_svc::http::server::{Configuration as HttpConfig, EspHttpConnection, EspHttpServer, Request};
use esp_idf_svc::http::Method;
use esp_idf_svc::io::{EspIOError, Write};
use esp_idf_svc::ipv4::{self, Mask, RouterConfiguration, Subnet};
use esp_idf_svc::mdns::EspMdns;
use esp_idf_svc::netif::{EspNetif, NetifConfiguration, NetifStack};
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys::{esp_netif_set_hostname, EspError};
use esp_idf_svc::wifi::{
    AccessPointConfiguration, AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi, WifiDeviceId,
    WifiDriver,
};
use esp_idf_svc::ws::FrameType;
use esp_idf_svc::http::server::ws::EspHttpWsDetachedSender;
use pourcore::settings::Settings;
use serde_json::{json, Value};

use crate::storage;
use crate::web::{self, WebAsset};

const AP_PASSWORD: &str = "pourover";
/// The address the docs point people to on first boot.
const AP_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 4, 1);
const MAX_SETTINGS_BODY: usize = 4 * 1024;
const MAX_RECIPES_BODY: usize = 32 * 1024;

type Clients = Arc<Mutex<Vec<EspHttpWsDetachedSender>>>;

pub struct Net {
    _wifi: BlockingWifi<EspWifi<'static>>,
    _server: EspHttpServer<'static>,
    _mdns: Option<EspMdns>,
    clients: Clients,
    commands: Receiver<String>,
    status_requested: Arc<AtomicBool>,
    /// `GET /api/settings` serves this; the main loop refreshes it on change.
    settings_json: Arc<Mutex<String>>,
    ap_mode: bool,
}

impl Net {
    pub fn ap_mode(&self) -> bool {
        self.ap_mode
    }

    pub fn pop_command(&self) -> Option<String> {
        self.commands.try_recv().ok()
    }

    /// True once after a WebSocket client connects.
    pub fn wants_status(&self) -> bool {
        self.status_requested.swap(false, Ordering::Relaxed)
    }

    pub fn broadcast(&self, json: &str) {
        let mut clients = self.clients.lock().unwrap();
        clients.retain_mut(|c| !c.is_closed() && c.send(FrameType::Text(false), json.as_bytes()).is_ok());
    }

    pub fn publish_settings(&self, s: &Settings) {
        *self.settings_json.lock().unwrap() = settings_api_json(s, self.ap_mode);
    }
}

fn settings_api_json(s: &Settings, ap_mode: bool) -> String {
    let mut v = s.to_api_json();
    v["apMode"] = json!(ap_mode);
    v.to_string()
}

fn start_wifi(
    modem: Modem<'static>,
    sysloop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
    s: &Settings,
) -> Result<(BlockingWifi<EspWifi<'static>>, bool), EspError> {
    let driver = WifiDriver::new(modem, sysloop.clone(), Some(nvs))?;
    let sta = EspNetif::new(NetifStack::Sta)?;
    let ap = EspNetif::new_with_conf(&NetifConfiguration {
        ip_configuration: Some(ipv4::Configuration::Router(RouterConfiguration {
            subnet: Subnet { gateway: AP_IP, mask: Mask(24) },
            dhcp_enabled: true,
            dns: None,
            secondary_dns: None,
        })),
        ..NetifConfiguration::wifi_default_router()
    })?;
    let mut wifi = BlockingWifi::wrap(EspWifi::wrap_all(driver, sta, ap)?, sysloop)?;

    let host = CString::new(s.hostname.as_str()).unwrap_or_default();
    unsafe { esp_netif_set_hostname(wifi.wifi().sta_netif().handle(), host.as_ptr()) };

    if !s.wifi_ssid.is_empty() {
        let conf = ClientConfiguration {
            ssid: s.wifi_ssid.as_str().try_into().unwrap_or_default(),
            password: s.wifi_pass.as_str().try_into().unwrap_or_default(),
            auth_method: if s.wifi_pass.is_empty() { AuthMethod::None } else { AuthMethod::WPA2Personal },
            ..Default::default()
        };
        wifi.set_configuration(&Configuration::Client(conf))?;
        wifi.start()?;
        match wifi.connect().and_then(|_| wifi.wait_netif_up()) {
            Ok(()) => {
                let ip = wifi.wifi().sta_netif().get_ip_info()?.ip;
                log::info!("Wi-Fi connected: http://{}.local  ({ip})", s.hostname);
                return Ok((wifi, false));
            }
            Err(e) => {
                log::warn!("Wi-Fi connection failed ({e}), starting access point");
                wifi.stop()?;
            }
        }
    }

    let mac = wifi.wifi().get_mac(WifiDeviceId::Sta)?;
    let ssid = format!("OpenPour-{:02X}{:02X}", mac[4], mac[5]);
    wifi.set_configuration(&Configuration::AccessPoint(AccessPointConfiguration {
        ssid: ssid.as_str().try_into().unwrap_or_default(),
        password: AP_PASSWORD.try_into().unwrap_or_default(),
        auth_method: AuthMethod::WPA2Personal,
        channel: 1,
        max_connections: 4,
        ..Default::default()
    }))?;
    wifi.start()?;
    wifi.wait_netif_up()?;
    log::info!("Access point \"{ssid}\" (password {AP_PASSWORD}): http://{AP_IP}");
    Ok((wifi, true))
}

fn start_mdns(hostname: &str) -> Option<EspMdns> {
    let mut mdns = EspMdns::take().ok()?;
    mdns.set_hostname(hostname).ok()?;
    mdns.add_service(None, "_http", "_tcp", 80, &[]).ok()?;
    Some(mdns)
}

/// Reads the whole body, refusing anything over `max` bytes.
fn read_body(req: &mut Request<&mut EspHttpConnection>, max: usize) -> Option<Vec<u8>> {
    let len: usize = req.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(0);
    if len == 0 || len > max {
        return None;
    }
    let mut body = vec![0; len];
    let mut got = 0;
    while got < len {
        match req.read(&mut body[got..]) {
            Ok(0) | Err(_) => return None,
            Ok(n) => got += n,
        }
    }
    Some(body)
}

fn send_json(req: Request<&mut EspHttpConnection>, status: u16, body: &str) -> Result<(), EspIOError> {
    let mut res = req.into_response(status, None, &[("Content-Type", "application/json")])?;
    res.write_all(body.as_bytes())?;
    Ok(())
}

fn send_asset(req: Request<&mut EspHttpConnection>, a: &WebAsset) -> Result<(), EspIOError> {
    let headers = [("Content-Type", a.mime), ("Content-Encoding", "gzip"), ("Cache-Control", "no-cache")];
    let mut res = req.into_response(200, None, &headers)?;
    res.write_all(a.data)?;
    Ok(())
}

/// Registers a POST endpoint whose JSON body is forwarded to the main loop as
/// `{"cmd": cmd, "data": body}`.
fn forward_post(
    server: &mut EspHttpServer<'static>,
    uri: &str,
    cmd: &'static str,
    max: usize,
    want_array: bool,
    tx: SyncSender<String>,
) -> Result<(), EspError> {
    server.fn_handler(uri, Method::Post, move |mut req| {
        let Some(data) = read_body(&mut req, max).and_then(|b| serde_json::from_slice::<Value>(&b).ok()) else {
            return send_json(req, 400, r#"{"error":"Expected a JSON body"}"#);
        };
        if want_array && !data.is_array() {
            return send_json(req, 400, r#"{"error":"Expected an array of recipes"}"#);
        }
        if tx.try_send(json!({ "cmd": cmd, "data": data }).to_string()).is_err() {
            return send_json(req, 503, r#"{"error":"Busy, try again"}"#);
        }
        send_json(req, 200, r#"{"ok":true}"#)
    })?;
    Ok(())
}

pub fn start(
    modem: Modem<'static>,
    sysloop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
    settings: &Settings,
) -> anyhow::Result<Net> {
    let (wifi, ap_mode) = start_wifi(modem, sysloop, nvs, settings)?;
    let mdns = start_mdns(&settings.hostname);
    if mdns.is_none() {
        log::warn!("mDNS failed to start");
    }

    let (tx, commands) = sync_channel::<String>(16);
    let clients: Clients = Arc::new(Mutex::new(Vec::new()));
    let status_requested = Arc::new(AtomicBool::new(false));
    let settings_json = Arc::new(Mutex::new(settings_api_json(settings, ap_mode)));

    let mut server = EspHttpServer::new(&HttpConfig {
        stack_size: 12 * 1024,
        max_uri_handlers: 12,
        uri_match_wildcard: true,
        ..Default::default()
    })?;

    // Handlers match in registration order, so specific URIs come first.
    {
        let clients = clients.clone();
        let status_requested = status_requested.clone();
        let tx = tx.clone();
        server.ws_handler("/ws", None, move |ws| {
            if ws.is_new() {
                if let Ok(sender) = ws.create_detached_sender() {
                    clients.lock().unwrap().push(sender);
                }
                status_requested.store(true, Ordering::Relaxed);
                return Ok::<(), EspError>(());
            }
            if ws.is_closed() {
                let session = ws.session();
                clients.lock().unwrap().retain(|c| c.session() != session);
                return Ok(());
            }
            // Commands are small; ignore anything fragmented or oversized.
            let mut buf = [0u8; 1024];
            match ws.recv(&mut buf) {
                Ok((FrameType::Text(false), len)) => {
                    // Text frames are reported with a NUL terminator.
                    let text = buf[..len].strip_suffix(&[0]).unwrap_or(&buf[..len]);
                    if let Ok(line) = core::str::from_utf8(text) {
                        let _ = tx.try_send(line.to_owned());
                    }
                }
                Ok(_) => {}
                Err(e) => log::debug!("ws recv: {e}"),
            }
            Ok(())
        })?;
    }

    {
        let settings_json = settings_json.clone();
        server.fn_handler("/api/settings", Method::Get, move |req| {
            let body = settings_json.lock().unwrap().clone();
            send_json(req, 200, &body)
        })?;
    }
    server.fn_handler("/api/recipes", Method::Get, |req| {
        let body = storage::read_recipes().map_or_else(|| "[]".to_owned(), |v| v.to_string());
        send_json(req, 200, &body)
    })?;
    forward_post(&mut server, "/api/settings", "settings", MAX_SETTINGS_BODY, false, tx.clone())?;
    forward_post(&mut server, "/api/recipes", "saveRecipes", MAX_RECIPES_BODY, true, tx)?;
    for method in [Method::Get, Method::Post] {
        server.fn_handler("/api/*", method, |req| send_json(req, 404, r#"{"error":"Not found"}"#))?;
    }
    server.fn_handler("/*", Method::Get, |req| {
        let path = req.uri().split('?').next().unwrap_or("/");
        let path = if path == "/" { "/index.html" } else { path };
        // Unknown paths get the single-page app.
        match web::find(path).or_else(|| web::find("/index.html")) {
            Some(a) => send_asset(req, a),
            None => send_json(req, 404, r#"{"error":"Not found"}"#),
        }
    })?;

    Ok(Net {
        _wifi: wifi,
        _server: server,
        _mdns: mdns,
        clients,
        commands,
        status_requested,
        settings_json,
        ap_mode,
    })
}
