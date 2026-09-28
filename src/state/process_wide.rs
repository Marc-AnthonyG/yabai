use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64};

pub static CORE_VIDEO_HOST_CLOCK_FREQUENCY: OnceLock<f64> = OnceLock::new();
pub static LAYER_NORMAL_WINDOW_LEVEL: OnceLock<i32> = OnceLock::new();
pub static LAYER_BELOW_WINDOW_LEVEL: OnceLock<i32> = OnceLock::new();
pub static LAYER_ABOVE_WINDOW_LEVEL: OnceLock<i32> = OnceLock::new();

pub static SCRIPTING_ADDITION_SOCKET_PATH: OnceLock<String> = OnceLock::new();
pub static MESSAGE_SOCKET_PATH: OnceLock<String> = OnceLock::new();
pub static CONFIG_FILE_PATH: OnceLock<String> = OnceLock::new();
pub static LOCK_FILE_PATH: OnceLock<String> = OnceLock::new();

pub static BOOTSTRAP_PORT: OnceLock<libc::mach_port_t> = OnceLock::new();
pub static SKYLIGHT_CONNECTION_ID: OnceLock<i32> = OnceLock::new();
pub static VERBOSE_DEBUG_OUTPUT_ENABLED: AtomicBool = AtomicBool::new(false);
pub static RELOAD_CONFIG_FILE_ON_CHANGE_ENABLED: AtomicBool = AtomicBool::new(false);
pub static DAEMON_PROCESS_ID: OnceLock<i32> = OnceLock::new();

pub static WINDOW_FOCUS_NOTIFICATION_IS_PENDING: AtomicBool = AtomicBool::new(false);
pub static DOCK_SWIPE_GESTURE_IS_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
pub static LAST_DOCK_SWIPE_GESTURE_END_TIME: AtomicU64 = AtomicU64::new(0);
pub static LAST_COMMAND_TAB_TIME: AtomicU64 = AtomicU64::new(0);
