use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64};

pub static CV_HOST_CLOCK_FREQUENCY: OnceLock<f64> = OnceLock::new();
pub static LAYER_NORMAL_WINDOW_LEVEL: OnceLock<i32> = OnceLock::new();
pub static LAYER_BELOW_WINDOW_LEVEL: OnceLock<i32> = OnceLock::new();
pub static LAYER_ABOVE_WINDOW_LEVEL: OnceLock<i32> = OnceLock::new();

pub static SA_SOCKET_FILE: OnceLock<String> = OnceLock::new();
pub static SOCKET_FILE: OnceLock<String> = OnceLock::new();
pub static CONFIG_FILE: OnceLock<String> = OnceLock::new();
pub static LOCK_FILE: OnceLock<String> = OnceLock::new();

pub static BOOTSTRAP_PORT: OnceLock<libc::mach_port_t> = OnceLock::new();
pub static CONNECTION: OnceLock<i32> = OnceLock::new();
pub static VERBOSE: AtomicBool = AtomicBool::new(false);
pub static PROCESS_ID: OnceLock<i32> = OnceLock::new();

pub static PENDING_WINDOW_FOCUS: AtomicBool = AtomicBool::new(false);
pub static PENDING_GESTURE: AtomicBool = AtomicBool::new(false);
pub static LAST_GESTURE_TIME: AtomicU64 = AtomicU64::new(0);
pub static LAST_CMD_TAB_TIME: AtomicU64 = AtomicU64::new(0);
