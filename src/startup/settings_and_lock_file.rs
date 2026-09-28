use std::ffi::CString;

use crate::error;
use crate::ffi::core_graphics::{CGWindowLevelForKey, CGWindowLevelKey};
use crate::ffi::core_video::CVGetHostClockFrequency;
use crate::ffi::mach_port::{TASK_BOOTSTRAP_PORT, task_get_special_port};
use crate::ffi::skylight::SLSMainConnectionID;
use crate::ffi::skylight_dynamic::resolve_dynamic_skylight_symbols;
use crate::mouse::tap::set_default_mouse_modifier_and_actions;
use crate::protocol::socket_path::message_socket_path_of_user;
use crate::state::process_wide::{
    BOOTSTRAP_PORT, CORE_VIDEO_HOST_CLOCK_FREQUENCY, DAEMON_PROCESS_ID, LAYER_ABOVE_WINDOW_LEVEL,
    LAYER_BELOW_WINDOW_LEVEL, LAYER_NORMAL_WINDOW_LEVEL, LOCK_FILE_PATH, MESSAGE_SOCKET_PATH,
    SCRIPTING_ADDITION_SOCKET_PATH, SKYLIGHT_CONNECTION_ID,
};
use crate::support::layer::{LAYER_ABOVE, LAYER_BELOW, LAYER_NORMAL};

pub(crate) const SCRIPTING_ADDITION_SOCKET_PATH_FORMAT: &str = "/tmp/yabai-sa_%s.socket";
pub(crate) const LOCK_FILE_PATH_FORMAT: &str = "/tmp/yabai_%s.lock";

#[allow(deprecated)]
pub(crate) fn configure_settings_and_acquire_lock() -> bool {
    let Some(user) = std::env::var_os("USER") else {
        error!("yabai: 'env USER' not set! abort..\n");
    };
    let user = user.to_string_lossy();

    let _ = SCRIPTING_ADDITION_SOCKET_PATH
        .set(SCRIPTING_ADDITION_SOCKET_PATH_FORMAT.replacen("%s", &user, 1));
    let _ = MESSAGE_SOCKET_PATH.set(message_socket_path_of_user(&user));
    let _ = LOCK_FILE_PATH.set(LOCK_FILE_PATH_FORMAT.replacen("%s", &user, 1));

    crate::ffi::appkit::NSApplicationLoad();
    let _ = DAEMON_PROCESS_ID.set(unsafe { libc::getpid() });
    let _ = SKYLIGHT_CONNECTION_ID.set(unsafe { SLSMainConnectionID() });
    let _ = CORE_VIDEO_HOST_CLOCK_FREQUENCY.set(CVGetHostClockFrequency());
    let _ = LAYER_NORMAL_WINDOW_LEVEL.set(CGWindowLevelForKey(CGWindowLevelKey(LAYER_NORMAL)));
    let _ = LAYER_BELOW_WINDOW_LEVEL.set(CGWindowLevelForKey(CGWindowLevelKey(LAYER_BELOW)));
    let _ = LAYER_ABOVE_WINDOW_LEVEL.set(CGWindowLevelForKey(CGWindowLevelKey(LAYER_ABOVE)));
    resolve_dynamic_skylight_symbols();

    unsafe { libc::signal(libc::SIGCHLD, libc::SIG_IGN) };
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN) };
    crate::ffi::core_graphics::CGSetLocalEventsSuppressionInterval(0.0f32 as f64);
    crate::ffi::core_graphics::CGEnableEventStateCombining(false);
    set_default_mouse_modifier_and_actions();
    let mut bootstrap_port: libc::mach_port_t = 0;
    unsafe {
        task_get_special_port(
            libc::mach_task_self(),
            TASK_BOOTSTRAP_PORT,
            &mut bootstrap_port,
        )
    };
    let _ = BOOTSTRAP_PORT.set(bootstrap_port);

    let lock_file =
        CString::new(LOCK_FILE_PATH.get().map_or("", String::as_str)).unwrap_or_default();
    let handle = unsafe {
        libc::open(
            lock_file.as_ptr(),
            libc::O_CREAT | libc::O_WRONLY | libc::O_CLOEXEC,
            0o600 as libc::c_uint,
        )
    };
    if handle == -1 {
        error!("yabai: could not create lock-file! abort..\n");
    }

    let lock_file_description = libc::flock {
        l_start: 0,
        l_len: 0,
        l_pid: *DAEMON_PROCESS_ID.get().unwrap_or(&0),
        l_type: libc::F_WRLCK as i16,
        l_whence: libc::SEEK_SET as i16,
    };

    unsafe { libc::fcntl(handle, libc::F_SETLK, &lock_file_description) != -1 }
}

pub(crate) fn configure_settings_and_acquire_lock_or_exit() {
    if !configure_settings_and_acquire_lock() {
        error!("yabai: could not acquire lock-file! abort..\n");
    }
}
