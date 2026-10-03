#include "net.h"

#include <AsyncJson.h>
#include <ESPAsyncWebServer.h>
#include <ESPmDNS.h>
#include <LittleFS.h>
#include <WiFi.h>

#include "generated/web_assets.h"
#include "recipes.h"
#include "settings.h"

namespace net {

static constexpr const char* kApPassword = "pourover";
static constexpr uint32_t kConnectTimeoutMs = 15000;

static AsyncWebServer server(80);
static AsyncWebSocket ws("/ws");
static QueueHandle_t commands;
static volatile bool statusRequested = false;
static bool inApMode = false;
static uint32_t lastCleanupMs = 0;

static void enqueue(const String& s) {
  String* copy = new String(s);
  if (xQueueSend(commands, &copy, 0) != pdTRUE) delete copy;
}

bool popCommand(String& out) {
  String* p = nullptr;
  if (xQueueReceive(commands, &p, 0) != pdTRUE) return false;
  out = *p;
  delete p;
  return true;
}

bool wantsStatus() {
  const bool w = statusRequested;
  statusRequested = false;
  return w;
}

bool apMode() { return inApMode; }

void broadcast(const String& json) {
  if (ws.count()) ws.textAll(json);
}

static void onWs(AsyncWebSocket*, AsyncWebSocketClient*, AwsEventType type, void* arg,
                 uint8_t* data, size_t len) {
  if (type == WS_EVT_CONNECT) {
    statusRequested = true;
    return;
  }
  if (type != WS_EVT_DATA) return;
  const AwsFrameInfo* info = (AwsFrameInfo*)arg;
  // Commands are small; ignore anything fragmented.
  if (!info->final || info->index != 0 || info->len != len || info->opcode != WS_TEXT) return;
  enqueue(String((const char*)data, len));
}

static void sendAsset(AsyncWebServerRequest* req, const WebAsset& a) {
  AsyncWebServerResponse* res = req->beginResponse(200, a.mime, a.data, a.len);
  res->addHeader("Content-Encoding", "gzip");
  res->addHeader("Cache-Control", "no-cache");
  req->send(res);
}

static const WebAsset* findAsset(const String& path) {
  for (size_t i = 0; i < kWebAssetCount; i++)
    if (path == kWebAssets[i].path) return &kWebAssets[i];
  return nullptr;
}

static void startWifi() {
  WiFi.setHostname(settings.hostname.c_str());
  if (settings.wifiSsid.length()) {
    WiFi.mode(WIFI_STA);
    WiFi.begin(settings.wifiSsid.c_str(), settings.wifiPass.c_str());
    const uint32_t start = millis();
    while (WiFi.status() != WL_CONNECTED && millis() - start < kConnectTimeoutMs) delay(100);
    if (WiFi.status() == WL_CONNECTED) {
      Serial.printf("Wi-Fi connected: http://%s.local  (%s)\n", settings.hostname.c_str(),
                    WiFi.localIP().toString().c_str());
      return;
    }
    Serial.println("Wi-Fi connection failed, starting access point");
  }
  inApMode = true;
  uint8_t mac[6];
  WiFi.macAddress(mac);
  char ssid[24];
  snprintf(ssid, sizeof(ssid), "OpenPour-%02X%02X", mac[4], mac[5]);
  WiFi.mode(WIFI_AP);
  WiFi.softAP(ssid, kApPassword);
  Serial.printf("Access point \"%s\" (password %s): http://%s\n", ssid, kApPassword,
                WiFi.softAPIP().toString().c_str());
}

void begin() {
  commands = xQueueCreate(16, sizeof(String*));
  startWifi();
  if (MDNS.begin(settings.hostname.c_str())) MDNS.addService("http", "tcp", 80);

  ws.onEvent(onWs);
  server.addHandler(&ws);

  server.on("/api/settings", HTTP_GET, [](AsyncWebServerRequest* req) {
    JsonDocument doc;
    settingsToJson(doc.to<JsonObject>());
    doc["apMode"] = inApMode;
    String out;
    serializeJson(doc, out);
    req->send(200, "application/json", out);
  });

  server.on("/api/recipes", HTTP_GET, [](AsyncWebServerRequest* req) {
    req->send(LittleFS, recipes::kPath, "application/json");
  });

  // Both POST bodies are forwarded to loop() as {"cmd": ..., "data": body}.
  auto* settingsHandler = new AsyncCallbackJsonWebHandler(
      "/api/settings", [](AsyncWebServerRequest* req, JsonVariant& json) {
        JsonDocument doc;
        doc["cmd"] = "settings";
        doc["data"] = json;
        String out;
        serializeJson(doc, out);
        enqueue(out);
        req->send(200, "application/json", "{\"ok\":true}");
      });
  server.addHandler(settingsHandler);

  auto* recipesHandler = new AsyncCallbackJsonWebHandler(
      "/api/recipes", [](AsyncWebServerRequest* req, JsonVariant& json) {
        if (!json.is<JsonArray>()) {
          req->send(400, "application/json", "{\"error\":\"Expected an array of recipes\"}");
          return;
        }
        JsonDocument doc;
        doc["cmd"] = "saveRecipes";
        doc["data"] = json;
        String out;
        serializeJson(doc, out);
        enqueue(out);
        req->send(200, "application/json", "{\"ok\":true}");
      });
  recipesHandler->setMaxContentLength(32 * 1024);
  server.addHandler(recipesHandler);

  server.onNotFound([](AsyncWebServerRequest* req) {
    const String path = req->url();
    if (path.startsWith("/api/")) {
      req->send(404, "application/json", "{\"error\":\"Not found\"}");
      return;
    }
    const WebAsset* a = findAsset(path == "/" ? "/index.html" : path);
    if (!a) a = findAsset("/index.html");  // single-page app
    if (a)
      sendAsset(req, *a);
    else
      req->send(404);
  });

  server.begin();
}

void update() {
  const uint32_t now = millis();
  if (now - lastCleanupMs > 1000) {
    lastCleanupMs = now;
    ws.cleanupClients();
  }
}

}  // namespace net
