// Bindings for the Wokwi custom chips API (https://wokwi.com/api/chips/wokwi-api.h),
// shared by the chips through `#[path]`.

#![allow(dead_code)]

use std::ffi::c_void;

pub type Pin = i32;
pub type Timer = u32;

pub const LOW: u32 = 0;
pub const HIGH: u32 = 1;

pub const INPUT: u32 = 0;
pub const OUTPUT: u32 = 1;
pub const INPUT_PULLUP: u32 = 2;
pub const OUTPUT_LOW: u32 = 16;
pub const OUTPUT_HIGH: u32 = 17;

pub const RISING: u32 = 1;
pub const FALLING: u32 = 2;
pub const BOTH: u32 = 3;

#[repr(C)]
pub struct PinWatchConfig {
    pub user_data: *mut c_void,
    pub edge: u32,
    pub pin_change: extern "C" fn(user_data: *mut c_void, pin: Pin, value: u32),
}

#[repr(C)]
pub struct TimerConfig {
    pub user_data: *mut c_void,
    pub callback: extern "C" fn(user_data: *mut c_void),
    pub reserved: [u32; 8],
}

extern "C" {
    #[link_name = "pinInit"]
    fn pin_init_raw(name: *const u8, mode: u32) -> Pin;
    #[link_name = "pinRead"]
    pub fn pin_read(pin: Pin) -> u32;
    #[link_name = "pinWrite"]
    pub fn pin_write(pin: Pin, value: u32);
    #[link_name = "pinMode"]
    pub fn pin_mode(pin: Pin, mode: u32);
    #[link_name = "pinWatch"]
    pub fn pin_watch(pin: Pin, config: *const PinWatchConfig) -> bool;
    #[link_name = "attrInit"]
    fn attr_init_raw(name: *const u8, default: u32) -> u32;
    #[link_name = "attrInitFloat"]
    fn attr_init_float_raw(name: *const u8, default: f32) -> u32;
    #[link_name = "attrRead"]
    pub fn attr_read(attr: u32) -> u32;
    #[link_name = "attrReadFloat"]
    pub fn attr_read_float(attr: u32) -> f32;
    #[link_name = "timerInit"]
    pub fn timer_init(config: *const TimerConfig) -> Timer;
    #[link_name = "timerStart"]
    pub fn timer_start(timer: Timer, micros: u32, repeat: bool);
    #[link_name = "getSimNanos"]
    pub fn get_sim_nanos() -> f64;
}

/// `name` must end in a NUL byte, e.g. `b"OUT\0"`.
pub fn pin_init(name: &[u8], mode: u32) -> Pin {
    debug_assert_eq!(name.last(), Some(&0));
    unsafe { pin_init_raw(name.as_ptr(), mode) }
}

pub fn attr_init(name: &[u8], default: u32) -> u32 {
    unsafe { attr_init_raw(name.as_ptr(), default) }
}

pub fn attr_init_float(name: &[u8], default: f32) -> u32 {
    unsafe { attr_init_float_raw(name.as_ptr(), default) }
}

/// Configs must outlive the chip, so they are leaked (chips live for the whole simulation).
pub fn watch(pin: Pin, edge: u32, user_data: *mut c_void, f: extern "C" fn(*mut c_void, Pin, u32)) {
    let cfg = Box::leak(Box::new(PinWatchConfig { user_data, edge, pin_change: f }));
    unsafe { pin_watch(pin, cfg) };
}

pub fn timer(user_data: *mut c_void, f: extern "C" fn(*mut c_void)) -> Timer {
    let cfg = Box::leak(Box::new(TimerConfig { user_data, callback: f, reserved: [0; 8] }));
    unsafe { timer_init(cfg) }
}

#[no_mangle]
pub extern "C" fn __wokwi_api_version_1() -> i32 {
    1
}
