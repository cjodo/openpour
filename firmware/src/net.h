#pragma once
#include <Arduino.h>

// Wi-Fi (station, or a fallback access point), the embedded web app, the
// REST endpoints and the /ws WebSocket. Requests arrive on the async TCP task;
// anything that changes machine state is queued and handled from loop().
namespace net {

void begin();
void update();
void broadcast(const String& json);
bool popCommand(String& out);
bool wantsStatus();  // a client just connected
bool apMode();

}  // namespace net
