//! Persistent, user-tunable settings. Stored as /settings.json and edited
//! from the web app. Defaults match the reference build in docs/BOM.md.
//!
//! Fields are read one by one rather than with serde's derive: a missing or
//! mistyped field keeps its value without discarding the others, and it
//! avoids an Xtensa LLVM crash in serde's f32 deserializer.

use serde_json::{Map, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub wifi_ssid: String,
    pub wifi_pass: String,
    pub hostname: String,
    pub last_recipe: String,

    // Flow meter between pump and nozzle. Calibrate from the app.
    pub flow_pulses_per_litre: f32,

    // Pump. pump_gps_at_full is measured by the pump calibration.
    pub pump_gps_at_full: f32,
    pub pump_min_duty: f32,
    /// Meter latency + pump coast-down when stopping.
    pub pump_lag_s: f32,

    // Motion. Direct-drive theta (NEMA17, 1/16 microstep), GT2 20T radial belt.
    pub theta_steps_per_deg: f32,
    pub radial_steps_per_mm: f32,
    pub invert_theta: bool,
    pub invert_radial: bool,
    /// Arm angle when the theta endstop trips.
    pub theta_home_deg: f32,
    /// Carriage radius when the radial endstop trips.
    pub radial_home_mm: f32,
    pub theta_min_deg: f32,
    /// Beyond this the beam can reach the column.
    pub theta_max_deg: f32,
    pub radial_min_mm: f32,
    pub radial_max_mm: f32,
    /// Pose over the dripper centre (set by calibration).
    pub center_r: f32,
    pub center_theta_deg: f32,
    pub park_r: f32,
    pub park_theta_deg: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            wifi_ssid: String::new(),
            wifi_pass: String::new(),
            hostname: "openpour".into(),
            last_recipe: String::new(),
            flow_pulses_per_litre: 1925.0,
            pump_gps_at_full: 6.0,
            pump_min_duty: 0.25,
            pump_lag_s: 0.15,
            theta_steps_per_deg: 200.0 * 16.0 / 360.0,
            radial_steps_per_mm: 200.0 * 16.0 / 40.0,
            invert_theta: false,
            invert_radial: false,
            theta_home_deg: -75.0,
            radial_home_mm: 66.0,
            theta_min_deg: -78.0,
            theta_max_deg: 25.0,
            radial_min_mm: 66.0,
            radial_max_mm: 175.0,
            center_r: 110.0,
            center_theta_deg: 0.0,
            park_r: 80.0,
            park_theta_deg: -60.0,
        }
    }
}

trait Field: Sized {
    fn from_json(v: &Value) -> Option<Self>;
    fn to_json(&self) -> Value;
}

impl Field for f32 {
    fn from_json(v: &Value) -> Option<Self> {
        v.as_f64().map(|n| n as f32)
    }
    fn to_json(&self) -> Value {
        Value::from(*self)
    }
}

impl Field for bool {
    fn from_json(v: &Value) -> Option<Self> {
        v.as_bool()
    }
    fn to_json(&self) -> Value {
        Value::Bool(*self)
    }
}

impl Field for String {
    fn from_json(v: &Value) -> Option<Self> {
        v.as_str().map(str::to_owned)
    }
    fn to_json(&self) -> Value {
        Value::String(self.clone())
    }
}

/// Every field exposed to the app, except the Wi-Fi credentials.
macro_rules! app_fields {
    ($($field:ident => $key:literal),* $(,)?) => {
        impl Settings {
            fn write_app_fields(&self, o: &mut Map<String, Value>) {
                $( o.insert($key.into(), self.$field.to_json()); )*
            }

            fn read_app_fields(&mut self, o: &Map<String, Value>) {
                $( if let Some(v) = o.get($key).and_then(Field::from_json) { self.$field = v; } )*
            }
        }
    };
}

app_fields! {
    hostname => "hostname",
    last_recipe => "lastRecipe",
    flow_pulses_per_litre => "flowPulsesPerLitre",
    pump_gps_at_full => "pumpGpsAtFull",
    pump_min_duty => "pumpMinDuty",
    pump_lag_s => "pumpLagS",
    theta_steps_per_deg => "thetaStepsPerDeg",
    radial_steps_per_mm => "radialStepsPerMm",
    invert_theta => "invertTheta",
    invert_radial => "invertRadial",
    theta_home_deg => "thetaHomeDeg",
    radial_home_mm => "radialHomeMm",
    theta_min_deg => "thetaMinDeg",
    theta_max_deg => "thetaMaxDeg",
    radial_min_mm => "radialMinMm",
    radial_max_mm => "radialMaxMm",
    center_r => "centerR",
    center_theta_deg => "centerThetaDeg",
    park_r => "parkR",
    park_theta_deg => "parkThetaDeg",
}

const SSID: &str = "wifiSsid";
const PASS: &str = "wifiPass";

impl Settings {
    /// Everything the app may see. Never includes the Wi-Fi password.
    pub fn to_api_json(&self) -> Value {
        let mut o = Map::new();
        self.write_app_fields(&mut o);
        o.insert(SSID.into(), Value::String(self.wifi_ssid.clone()));
        Value::Object(o)
    }

    /// The full record for the settings file, password included.
    pub fn to_storage_json(&self) -> Value {
        let mut v = self.to_api_json();
        v[PASS] = Value::String(self.wifi_pass.clone());
        v
    }

    /// Reads a stored file. Missing or malformed fields keep their defaults.
    pub fn from_storage_json(v: &Value) -> Settings {
        let mut s = Settings::default();
        s.apply_json(v);
        s
    }

    /// Merges the fields present in `patch`. Unknown keys, nulls and values of
    /// the wrong type are ignored one by one. Returns true if the Wi-Fi
    /// credentials changed (the caller should reboot).
    pub fn apply_json(&mut self, patch: &Value) -> bool {
        let Some(patch) = patch.as_object() else { return false };
        self.read_app_fields(patch);

        let mut wifi_changed = false;
        if let Some(ssid) = patch.get(SSID).and_then(Value::as_str) {
            if ssid != self.wifi_ssid {
                self.wifi_ssid = ssid.to_owned();
                wifi_changed = true;
            }
        }
        if let Some(pass) = patch.get(PASS).and_then(Value::as_str) {
            self.wifi_pass = pass.to_owned();
            wifi_changed = true;
        }
        wifi_changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn api_json_hides_password() {
        let s = Settings { wifi_pass: "secret".into(), ..Default::default() };
        let v = s.to_api_json();
        assert!(v.get("wifiPass").is_none());
        assert_eq!(v["hostname"], "openpour");
        assert_eq!(s.to_storage_json()["wifiPass"], "secret");
    }

    #[test]
    fn partial_update_skips_bad_fields() {
        let mut s = Settings::default();
        let changed = s.apply_json(&json!({
            "centerR": 120,
            "pumpLagS": "oops",
            "invertTheta": true,
            "apMode": true,
            "parkR": null
        }));
        assert!(!changed);
        assert_eq!(s.center_r, 120.0);
        assert_eq!(s.pump_lag_s, 0.15);
        assert!(s.invert_theta);
        assert_eq!(s.park_r, 80.0);
    }

    #[test]
    fn wifi_changes_are_reported() {
        let mut s = Settings::default();
        assert!(s.apply_json(&json!({ "wifiSsid": "home" })));
        assert!(!s.apply_json(&json!({ "wifiSsid": "home" })));
        assert!(s.apply_json(&json!({ "wifiPass": "pw" })));
        assert_eq!(s.wifi_pass, "pw");
    }

    #[test]
    fn storage_round_trip() {
        let s = Settings { wifi_pass: "pw".into(), center_r: 101.5, ..Default::default() };
        assert_eq!(Settings::from_storage_json(&s.to_storage_json()), s);
    }
}
