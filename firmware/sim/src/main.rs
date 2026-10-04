//! OpenPour host simulator: the firmware's logic (pourcore) on a PC, driving
//! simulated hardware and serving the real web app and API.
//!
//!   cd web && npm run build          (or `npm run watch` while editing the app)
//!   cd firmware/sim && cargo run     then open http://localhost:8080
//!
//! Options: --port 8080, --speed 1 (4 = four times faster), --lan (listen on
//! all interfaces, to try it from a phone), --data <dir> (settings and
//! recipes, default firmware/sim/data), --web <dir> (default web/dist).
//! Faults and hardware truth: GET/POST /api/sim, see README.md.
//! Logging: RUST_LOG=debug for controller internals.

mod hardware;
mod machine;
mod server;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pourcore::app::{self, Effect, Press};
use pourcore::brew::Brew;
use serde_json::Value;
use tokio::sync::broadcast;

use crate::machine::SimMachine;
use crate::server::{Shared, Snapshot, ToSim};

const STATUS_PERIOD: Duration = Duration::from_millis(200);
/// Caps catch-up after a stall (a breakpoint, a suspended laptop).
const MAX_STEPS_PER_LOOP: u32 = 2000;

struct Options {
    port: u16,
    speed: f64,
    lan: bool,
    data: PathBuf,
    web: PathBuf,
}

fn options() -> Options {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut o = Options {
        port: 8080,
        speed: 1.0,
        lan: false,
        data: here.join("data"),
        web: here.join("../../web/dist"),
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--port" => o.port = value().parse().expect("--port takes a number"),
            "--speed" => o.speed = value().parse::<f64>().expect("--speed takes a number").clamp(0.1, 50.0),
            "--lan" => o.lan = true,
            "--data" => o.data = value().into(),
            "--web" => o.web = value().into(),
            "-h" | "--help" => {
                println!("usage: openpour-sim [--port 8080] [--speed 1] [--lan] [--data DIR] [--web DIR]");
                std::process::exit(0);
            }
            other => panic!("unknown option {other} (try --help)"),
        }
    }
    o
}

fn refresh(snapshot: &Mutex<Snapshot>, dev: &SimMachine) {
    let mut s = snapshot.lock().unwrap();
    s.settings = dev.settings_api_json();
    s.recipes = dev.recipes_json();
    s.sim = dev.plant.to_json().to_string();
}

/// The firmware's main loop (firmware/esp32/src/main.rs), stepping machine
/// time in 1 ms increments at `speed` times real time.
fn simulate(rx: Receiver<ToSim>, out: broadcast::Sender<String>, snapshot: Arc<Mutex<Snapshot>>, o: &Options) {
    let mut dev = SimMachine::new(Some(o.data.clone()));
    let mut brew = Brew::default();
    refresh(&snapshot, &dev);

    let mut machine_ms = 0.0f64;
    let mut last = Instant::now();
    let mut last_status = Instant::now();
    loop {
        let now = Instant::now();
        machine_ms += now.duration_since(last).as_secs_f64() * 1000.0 * o.speed;
        last = now;
        let due = (machine_ms as u32).wrapping_sub(dev.now).min(MAX_STEPS_PER_LOOP);
        for _ in 0..due {
            dev.step(1);
            brew.update(&mut dev);
        }
        machine_ms = machine_ms.min(dev.now as f64 + 1.0);

        let mut effects = Vec::new();
        let mut want_status = false;
        let mut changed = false;
        while let Ok(msg) = rx.try_recv() {
            changed = true;
            match msg {
                ToSim::Command(line) => effects.extend(app::handle_command(&line, &mut brew, &mut dev)),
                ToSim::Sim(v) => {
                    log::info!(target: "sim", "{v}");
                    dev.plant.apply(&v);
                    let press = match v.get("press").and_then(Value::as_str) {
                        Some("short") => Some(Press::Short),
                        Some("long") => Some(Press::Long),
                        _ => None,
                    };
                    if let Some(p) = press {
                        effects.extend(app::handle_press(p, &mut brew, &mut dev));
                    }
                }
                ToSim::WantStatus => want_status = true,
            }
        }
        if changed {
            refresh(&snapshot, &dev);
        }
        for fx in effects {
            match fx {
                Effect::Notify { kind, msg } => {
                    let _ = out.send(Effect::notify_json(kind, &msg));
                }
                Effect::Reboot => log::warn!(target: "sim", "Wi-Fi settings changed: the real machine restarts here"),
            }
        }
        if want_status || last_status.elapsed() >= STATUS_PERIOD {
            last_status = Instant::now();
            refresh(&snapshot, &dev);
            let _ = out.send(brew.status(&dev).to_string());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let o = options();

    if !o.web.join("index.html").exists() {
        eprintln!("No web app at {}. Build it first: cd web && npm ci && npm run build", o.web.display());
        std::process::exit(1);
    }
    std::fs::create_dir_all(&o.data).expect("create the data directory");

    let (to_sim, rx) = sync_channel(16);
    let (out, _) = broadcast::channel(256);
    let snapshot = Arc::new(Mutex::new(Snapshot::default()));
    let shared = Shared { to_sim, out: out.clone(), snapshot: snapshot.clone() };

    let addr = SocketAddr::from(if o.lan { ([0, 0, 0, 0], o.port) } else { ([127, 0, 0, 1], o.port) });
    let web = o.web.clone();
    log::info!(target: "sim", "speed {}x, data in {}", o.speed, o.data.display());
    std::thread::spawn(move || simulate(rx, out, snapshot, &o));

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| panic!("cannot listen on {addr}: {e}"));
    log::info!(target: "sim", "OpenPour simulator on http://localhost:{}", addr.port());
    axum::serve(listener, server::router(shared, web)).await.unwrap();
}
