
mod ffi;
mod service;
mod support;
mod handles;
mod globals;
mod state;
mod layout;
mod scripting_addition;
mod event_loop;
mod signal;
mod workspace;
mod message;
mod display;
mod space;
mod window;
mod process;
mod application;
mod mouse;
mod mission_control;
mod query;
mod serialise;

use core::ffi::{c_int, c_void};
use core::ptr::null_mut;
use std::ffi::CString;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

use objc2::MainThreadMarker;

use crate::display::manager::{DisplayManager, display_manager_begin};
use crate::event_loop::{event_loop_begin, event_loop_run, update_window_notifications};
use crate::ffi::accessibility::ax_privilege;
use crate::ffi::appkit::NSApplication;
use crate::ffi::carbon_process::ProcessSerialNumber;
use crate::ffi::core_foundation::{CGPoint, CGRect};
use crate::ffi::core_graphics::{CGWindowLevelForKey, CGWindowLevelKey};
use crate::ffi::core_video::CVGetHostClockFrequency;
use crate::ffi::mach_port::{TASK_BOOTSTRAP_PORT, task_get_special_port};
use crate::ffi::skylight::{
    SLSGetSpaceManagementMode, SLSMainConnectionID, SLSRegisterConnectionNotifyProc,
};
use crate::ffi::skylight_dynamic::resolve_dynamic_skylight_symbols;
use crate::globals::{
    BOOTSTRAP_PORT, CONFIG_FILE, CONNECTION, CV_HOST_CLOCK_FREQUENCY, LAYER_ABOVE_WINDOW_LEVEL,
    LAYER_BELOW_WINDOW_LEVEL, LAYER_NORMAL_WINDOW_LEVEL, LOCK_FILE, PROCESS_ID, SA_SOCKET_FILE,
    SOCKET_FILE,
};
use crate::handles::{ProcessId, SpaceId, WindowId};
use crate::layout::insertion::WindowInsertionPoint;
use crate::layout::settings::ViewType;
use crate::layout::tree::{WindowNodeChild, WindowNodeSplit};
use crate::message::message_loop_begin;
use crate::mission_control::{MissionControlMode, connection_handler, mission_control_observe};
use crate::mouse::drag::MouseDragState;
use crate::mouse::tap::{MOUSE_EVENT_MASK, MouseMode, mouse_handler_begin, mouse_state_init};
use crate::process::manager::{ProcessManager, process_manager_begin};
use crate::scripting_addition::installer::{scripting_addition_load, scripting_addition_uninstall};
use crate::service::launchctl::{service_restart, service_start, service_stop};
use crate::service::plist::{service_install, service_uninstall};
use crate::space::manager::{SpaceManager, hash_view_key, space_manager_begin};
use crate::state::EventLoopOwnedState;
use crate::support::color::RgbaColor;
use crate::support::config_file::exec_config_file;
use crate::support::easing::AnimationEasingType;
use crate::support::layer::{LAYER_ABOVE, LAYER_BELOW, LAYER_NORMAL};
use crate::support::log::set_g_verbose;
use crate::support::privilege::is_root;
use crate::support::response::FAILURE_MESSAGE;
use crate::support::sockets::{socket_close, socket_connect, socket_open};
use crate::support::strings::string_equals;
use crate::support::table::Table;
use crate::window::discovery::window_manager_begin;
use crate::window::manager::{
    FfmMode, PurifyMode, WindowManager, WindowOriginMode, hash_wm_process_id, hash_wm_window_id,
    window_manager_init,
};
use crate::workspace::{
    workspace_event_handler_begin, workspace_is_macos_monterey, workspace_is_macos_sequoia,
    workspace_is_macos_sonoma, workspace_is_macos_tahoe, workspace_is_macos_ventura,
};

pub(crate) const SA_SOCKET_PATH_FMT: &str = "/tmp/yabai-sa_%s.socket";
pub(crate) const SOCKET_PATH_FMT: &str = "/tmp/yabai_%s.socket";
pub(crate) const LCFILE_PATH_FMT: &str = "/tmp/yabai_%s.lock";

pub(crate) const SCRPT_ADD_LOAD_OPT: &str = "--load-sa";
pub(crate) const SCRPT_ADD_UNINSTALL_OPT: &str = "--uninstall-sa";
pub(crate) const SERVICE_INSTALL_OPT: &str = "--install-service";
pub(crate) const SERVICE_UNINSTALL_OPT: &str = "--uninstall-service";
pub(crate) const SERVICE_START_OPT: &str = "--start-service";
pub(crate) const SERVICE_RESTART_OPT: &str = "--restart-service";
pub(crate) const SERVICE_STOP_OPT: &str = "--stop-service";
pub(crate) const CLIENT_OPT_LONG: &str = "--message";
pub(crate) const CLIENT_OPT_SHRT: &str = "-m";
pub(crate) const CONFIG_OPT_LONG: &str = "--config";
pub(crate) const CONFIG_OPT_SHRT: &str = "-c";
pub(crate) const DEBUG_VERBOSE_OPT_LONG: &str = "--verbose";
pub(crate) const DEBUG_VERBOSE_OPT_SHRT: &str = "-V";
pub(crate) const VERSION_OPT_LONG: &str = "--version";
pub(crate) const VERSION_OPT_SHRT: &str = "-v";
pub(crate) const HELP_OPT_LONG: &str = "--help";
pub(crate) const HELP_OPT_SHRT: &str = "-h";

pub(crate) const MAJOR: i32 = 7;
pub(crate) const MINOR: i32 = 1;
pub(crate) const PATCH: i32 = 25;

pub(crate) fn client_send_message(arguments: &[std::ffi::OsString]) -> i32 {
    let argument_count = arguments.len() as i32;

    if argument_count <= 1 {
        error!("yabai-msg: no arguments given! abort..\n");
    }

    let Some(user) = std::env::var_os("USER") else {
        error!("yabai-msg: 'env USER' not set! abort..\n");
    };
    let user = user.to_string_lossy();

    let mut message_length: c_int = argument_count;
    let mut argument_lengths: Vec<c_int> = vec![0; argument_count as usize];

    for index in 1..argument_count as usize {
        argument_lengths[index] = std::os::unix::ffi::OsStrExt::as_bytes(arguments[index].as_os_str()).len() as c_int;
        message_length += argument_lengths[index];
    }

    let mut message: Vec<u8> = Vec::with_capacity(size_of::<c_int>() + message_length as usize);

    message.extend_from_slice(&message_length.to_ne_bytes());
    for index in 1..argument_count as usize {
        message.extend_from_slice(
            &std::os::unix::ffi::OsStrExt::as_bytes(arguments[index].as_os_str())[..argument_lengths[index] as usize],
        );
        message.push(b'\0');
    }
    message.push(b'\0');

    let mut socket_file_descriptor: c_int = 0;
    let socket_file = SOCKET_PATH_FMT.replacen("%s", &user, 1);

    if !socket_open(&mut socket_file_descriptor) {
        error!("yabai-msg: failed to open socket..\n");
    }

    if !socket_connect(socket_file_descriptor, &socket_file) {
        error!("yabai-msg: failed to connect to socket..\n");
    }

    if unsafe {
        libc::send(
            socket_file_descriptor,
            message.as_ptr().cast::<c_void>(),
            size_of::<c_int>() + message_length as usize,
            0,
        )
    } == -1
    {
        error!("yabai-msg: failed to send data..\n");
    }

    unsafe { libc::shutdown(socket_file_descriptor, libc::SHUT_WR) };
    drop(message);

    let mut result = libc::EXIT_SUCCESS;
    let mut standard_output = std::io::stdout();
    let mut standard_error = std::io::stderr();
    let mut output: &mut dyn Write = &mut standard_output;
    let mut bytes_read: isize;
    let mut response = [0u8; libc::BUFSIZ as usize];

    loop {
        bytes_read = unsafe {
            libc::read(
                socket_file_descriptor,
                response.as_mut_ptr().cast::<c_void>(),
                response.len() - 1,
            )
        };
        if !(bytes_read > 0) {
            break;
        }

        response[bytes_read as usize] = b'\0';

        if response[0] == FAILURE_MESSAGE[0] {
            result = libc::EXIT_FAILURE;
            output = &mut standard_error;
            let text = &response[1..];
            let length = text
                .iter()
                .position(|byte| *byte == b'\0')
                .unwrap_or(text.len());
            let _ = output.write_all(&text[..length]);
            let _ = output.flush();
        } else {
            let text = &response[..];
            let length = text
                .iter()
                .position(|byte| *byte == b'\0')
                .unwrap_or(text.len());
            let _ = output.write_all(&text[..length]);
            let _ = output.flush();
        }
    }

    socket_close(socket_file_descriptor);
    result
}

#[allow(deprecated)]
pub(crate) fn configure_settings_and_acquire_lock() -> bool {
    let Some(user) = std::env::var_os("USER") else {
        error!("yabai: 'env USER' not set! abort..\n");
    };
    let user = user.to_string_lossy();

    let _ = SA_SOCKET_FILE.set(SA_SOCKET_PATH_FMT.replacen("%s", &user, 1));
    let _ = SOCKET_FILE.set(SOCKET_PATH_FMT.replacen("%s", &user, 1));
    let _ = LOCK_FILE.set(LCFILE_PATH_FMT.replacen("%s", &user, 1));

    crate::ffi::appkit::NSApplicationLoad();
    let _ = PROCESS_ID.set(unsafe { libc::getpid() });
    let _ = CONNECTION.set(unsafe { SLSMainConnectionID() });
    let _ = CV_HOST_CLOCK_FREQUENCY.set(CVGetHostClockFrequency());
    let _ = LAYER_NORMAL_WINDOW_LEVEL.set(CGWindowLevelForKey(CGWindowLevelKey(LAYER_NORMAL)));
    let _ = LAYER_BELOW_WINDOW_LEVEL.set(CGWindowLevelForKey(CGWindowLevelKey(LAYER_BELOW)));
    let _ = LAYER_ABOVE_WINDOW_LEVEL.set(CGWindowLevelForKey(CGWindowLevelKey(LAYER_ABOVE)));
    resolve_dynamic_skylight_symbols();

    unsafe { libc::signal(libc::SIGCHLD, libc::SIG_IGN) };
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN) };
    crate::ffi::core_graphics::CGSetLocalEventsSuppressionInterval(0.0f32 as f64);
    crate::ffi::core_graphics::CGEnableEventStateCombining(false);
    mouse_state_init();
    let mut bootstrap_port: libc::mach_port_t = 0;
    unsafe {
        task_get_special_port(
            libc::mach_task_self(),
            TASK_BOOTSTRAP_PORT,
            &mut bootstrap_port,
        )
    };
    let _ = BOOTSTRAP_PORT.set(bootstrap_port);

    let lock_file = CString::new(LOCK_FILE.get().map_or("", String::as_str)).unwrap_or_default();
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
        l_pid: *PROCESS_ID.get().unwrap_or(&0),
        l_type: libc::F_WRLCK as i16,
        l_whence: libc::SEEK_SET as i16,
    };

    unsafe { libc::fcntl(handle, libc::F_SETLK, &lock_file_description) != -1 }
}

pub(crate) fn parse_arguments(
    arguments: &[String],
    arguments_as_given: &[std::ffi::OsString],
) -> Option<String> {
    let argument_count = arguments.len();
    let mut config_file: Option<String> = None;

    if (string_equals(Some(&arguments[1]), Some(HELP_OPT_LONG)))
        || (string_equals(Some(&arguments[1]), Some(HELP_OPT_SHRT)))
    {
        print!(
            "Usage: yabai [option]\n\
             Options:\n    \
             --load-sa              Install and load the scripting-addition.\n    \
             --uninstall-sa         Uninstall the scripting-addition.\n    \
             --install-service      Write launchd service file to disk.\n    \
             --uninstall-service    Remove launchd service file from disk.\n    \
             --start-service        Enable, load, and start the launchd service.\n    \
             --restart-service      Attempts to restart the service instance.\n    \
             --stop-service         Stops a running instance of the service.\n    \
             --message, -m <msg>    Send message to a running instance of yabai.\n    \
             --config, -c <config>  Use the specified configuration file.\n    \
             --verbose, -V          Output debug information to stdout.\n    \
             --version, -v          Print version to stdout and exit.\n    \
             --help, -h             Print options to stdout and exit.\n\
             Type `man yabai` for more information, or visit: \
             https://github.com/asmvik/yabai/blob/v{}.{}.{}/doc/yabai.asciidoc\n",
            MAJOR, MINOR, PATCH
        );
        std::process::exit(libc::EXIT_SUCCESS);
    }

    if (string_equals(Some(&arguments[1]), Some(VERSION_OPT_LONG)))
        || (string_equals(Some(&arguments[1]), Some(VERSION_OPT_SHRT)))
    {
        print!("yabai-v{}.{}.{}\n", MAJOR, MINOR, PATCH);
        std::process::exit(libc::EXIT_SUCCESS);
    }

    if (string_equals(Some(&arguments[1]), Some(CLIENT_OPT_LONG)))
        || (string_equals(Some(&arguments[1]), Some(CLIENT_OPT_SHRT)))
    {
        std::process::exit(client_send_message(&arguments_as_given[1..]));
    }

    if string_equals(Some(&arguments[1]), Some(SCRPT_ADD_UNINSTALL_OPT)) {
        std::process::exit(scripting_addition_uninstall());
    }

    if string_equals(Some(&arguments[1]), Some(SCRPT_ADD_LOAD_OPT)) {
        std::process::exit(scripting_addition_load());
    }

    if string_equals(Some(&arguments[1]), Some(SERVICE_INSTALL_OPT)) {
        std::process::exit(service_install());
    }

    if string_equals(Some(&arguments[1]), Some(SERVICE_UNINSTALL_OPT)) {
        std::process::exit(service_uninstall());
    }

    if string_equals(Some(&arguments[1]), Some(SERVICE_START_OPT)) {
        std::process::exit(service_start());
    }

    if string_equals(Some(&arguments[1]), Some(SERVICE_RESTART_OPT)) {
        std::process::exit(service_restart());
    }

    if string_equals(Some(&arguments[1]), Some(SERVICE_STOP_OPT)) {
        std::process::exit(service_stop());
    }

    let mut index = 1;
    while index < argument_count {
        let option = &arguments[index];

        if (string_equals(Some(option), Some(DEBUG_VERBOSE_OPT_LONG)))
            || (string_equals(Some(option), Some(DEBUG_VERBOSE_OPT_SHRT)))
        {
            set_g_verbose(true);
        } else if (string_equals(Some(option), Some(CONFIG_OPT_LONG)))
            || (string_equals(Some(option), Some(CONFIG_OPT_SHRT)))
        {
            let value = if index < argument_count - 1 {
                index += 1;
                Some(&arguments[index])
            } else {
                None
            };
            let Some(value) = value else {
                error!(
                    "yabai: option '{}|{}' requires an argument!\n",
                    CONFIG_OPT_LONG, CONFIG_OPT_SHRT
                );
            };
            config_file = Some(value.clone());
        } else {
            error!("yabai: '{}' is not a valid option!\n", option);
        }

        index += 1;
    }

    config_file
}

fn main() {
    std::panic::set_hook(Box::new(|panic_information| {
        eprintln!("{panic_information}");
        std::process::abort();
    }));
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };

    let arguments_as_given: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let arguments: Vec<String> = arguments_as_given
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let argument_count = arguments.len();

    let mut config_file: Option<String> = None;
    if argument_count > 1 {
        config_file = parse_arguments(&arguments, &arguments_as_given);
    }
    let _ = CONFIG_FILE.set(config_file.unwrap_or_default());

    if is_root() {
        require!("yabai: running as root is not allowed! abort..\n");
    }

    if !ax_privilege() {
        require!("yabai: could not access accessibility features! abort..\n");
    }

    if !(unsafe { SLSGetSpaceManagementMode(SLSMainConnectionID()) } == 1) {
        require!("yabai: 'display has separate spaces' is disabled! abort..\n");
    }

    if !configure_settings_and_acquire_lock() {
        error!("yabai: could not acquire lock-file! abort..\n");
    }

    let event_receiver = event_loop_begin();

    let mut event_loop_owned_state = EventLoopOwnedState {
        signal_event: std::array::from_fn(|_| Vec::new()),
        process_manager: ProcessManager {
            front_process_id: ProcessId(0),
            last_front_process_id: ProcessId(0),
            switch_event_time: 0.0,
            finder_process_serial_number: ProcessSerialNumber {
                high_long_of_psn: 0,
                low_long_of_psn: 0,
            },
        },
        display_manager: DisplayManager::default(),
        window_manager: WindowManager {
            system_element: core::ptr::null(),
            application: Table::new(0, hash_wm_process_id),
            window: Table::new(0, hash_wm_window_id),
            managed_window: Table::new(0, hash_wm_window_id),
            window_lost_focused_event: Table::new(0, hash_wm_window_id),
            application_lost_front_switched_event: Table::new(0, hash_wm_process_id),
            window_animations_table: Arc::new(Mutex::new(Table::new(0, hash_wm_window_id))),
            insert_feedback: Table::new(0, hash_wm_window_id),
            rules: Vec::new(),
            applications_to_refresh: Vec::new(),
            focused_window_id: WindowId(0),
            focused_window_process_serial_number: ProcessSerialNumber {
                high_long_of_psn: 0,
                low_long_of_psn: 0,
            },
            last_window_id: WindowId(0),
            enable_mff: false,
            ffm_mode: FfmMode::Disabled,
            purify_mode: PurifyMode::Disabled,
            window_origin_mode: WindowOriginMode::Default,
            enable_window_opacity: false,
            menubar_opacity: 0.0,
            active_window_opacity: 0.0,
            normal_window_opacity: 0.0,
            window_opacity_duration: 0.0,
            window_animation_duration: 0.0,
            window_animation_easing: AnimationEasingType::EaseInSine,
            insert_feedback_color: RgbaColor {
                packed: 0,
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 0.0,
            },
            scratchpad_window: Vec::new(),
        },
        space_manager: SpaceManager {
            view: Table::new(0, hash_view_key),
            current_space_id: SpaceId(0),
            last_space_id: SpaceId(0),
            did_begin: false,
            layout: ViewType::Default,
            top_padding: 0,
            bottom_padding: 0,
            left_padding: 0,
            right_padding: 0,
            window_gap: 0,
            split_ratio: 0.0,
            split_type: WindowNodeSplit::None,
            window_placement: WindowNodeChild::None,
            window_insertion_point: WindowInsertionPoint::Focused,
            window_zoom_persist: false,
            auto_balance: 0,
            labels: Vec::new(),
            skip_window_focus_animation: false,
        },
        signal_storage: Vec::new(),
        mouse_drag_state: MouseDragState {
            current_action: MouseMode::None,
            down_location: CGPoint { x: 0.0, y: 0.0 },
            last_moved_time: 0,
            window_id: None,
            window_frame: CGRect::default(),
            ffm_window_id: WindowId(0),
            direction: 0,
            feedback_node: None,
        },
        mission_control_mode: MissionControlMode::Inactive,
        focus_follows_mouse_suspended_value: FfmMode::Disabled,
        is_menu_open: 0,
    };

    if !workspace_event_handler_begin() {
        error!("yabai: could not start workspace context! abort..\n");
    }

    if !process_manager_begin(&mut event_loop_owned_state.process_manager) {
        error!("yabai: could not start process manager! abort..\n");
    }

    if !display_manager_begin(&mut event_loop_owned_state.display_manager) {
        error!("yabai: could not start display manager! abort..\n");
    }

    if !mouse_handler_begin(MOUSE_EVENT_MASK) {
        error!("yabai: could not start mouse handler! abort..\n");
    }

    let connection = *CONNECTION.get().unwrap();

    if workspace_is_macos_monterey()
        || workspace_is_macos_ventura()
        || workspace_is_macos_sonoma()
        || workspace_is_macos_sequoia()
        || workspace_is_macos_tahoe()
    {
        mission_control_observe();

        if workspace_is_macos_ventura()
            || workspace_is_macos_sonoma()
            || workspace_is_macos_sequoia()
            || workspace_is_macos_tahoe()
        {
            unsafe {
                SLSRegisterConnectionNotifyProc(connection, connection_handler, 1327, null_mut())
            };
            unsafe {
                SLSRegisterConnectionNotifyProc(connection, connection_handler, 1328, null_mut())
            };
        }
    } else {
        unsafe { SLSRegisterConnectionNotifyProc(connection, connection_handler, 1204, null_mut()) };
    }

    unsafe { SLSRegisterConnectionNotifyProc(connection, connection_handler, 808, null_mut()) };
    unsafe { SLSRegisterConnectionNotifyProc(connection, connection_handler, 1202, null_mut()) };

    if workspace_is_macos_sequoia() || workspace_is_macos_tahoe() {
        unsafe { SLSRegisterConnectionNotifyProc(connection, connection_handler, 804, null_mut()) };
    }

    let EventLoopOwnedState {
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        mouse_drag_state,
        mission_control_mode,
        ..
    } = &mut event_loop_owned_state;
    window_manager_init(window_manager);
    space_manager_begin(space_manager, display_manager, window_manager);
    window_manager_begin(
        space_manager,
        window_manager,
        process_manager,
        display_manager,
        mouse_drag_state,
        mission_control_mode,
    );

    if workspace_is_macos_sequoia() || workspace_is_macos_tahoe() {
        update_window_notifications(window_manager, space_manager);
    }

    let _ = std::thread::Builder::new()
        .name(String::from("yabai-event-loop"))
        .spawn(move || event_loop_run(event_receiver, event_loop_owned_state));

    if !message_loop_begin(Path::new(SOCKET_FILE.get().map_or("", String::as_str))) {
        error!("yabai: could not start message loop! abort..\n");
    }

    exec_config_file(CONFIG_FILE.get().cloned().unwrap_or_default());

    if let Some(main_thread_marker) = MainThreadMarker::new() {
        NSApplication::sharedApplication(main_thread_marker).run();
    }
}
