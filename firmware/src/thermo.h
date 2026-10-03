#pragma once

// DS18B20 probe in the reservoir. Conversions run in the background.
namespace thermo {

void begin();
void update();
float celsius();
bool present();

}  // namespace thermo
