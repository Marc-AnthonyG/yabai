use core::ptr::null_mut;
use std::path::Path;
use std::sync::{Arc, Mutex};

use objc2::MainThreadMarker;

use crate::display::manager::{DisplayManager, display_manager_begin};
use crate::error;
use crate::event::queue::event_loop_begin;
use crate::event::run_loop::event_loop_run;
use crate::ffi::appkit::NSApplication;
use crate::ffi::carbon_process::ProcessSerialNumber;
use crate::ffi::core_foundation::{CGPoint, CGRect};
use crate::ffi::skylight::SLSRegisterConnectionNotifyProc;
use crate::layout::insertion::WindowInsertionPoint;
use crate::layout::settings::ViewType;
use crate::layout::tree::{WindowNodeChild, WindowNodeSplit};
use crate::message::listening_socket::message_loop_begin;
use crate::mouse::drag::MouseDragState;
use crate::mouse::tap::MouseMode;
use crate::notifications::mission_control::mission_control_observe;
use crate::notifications::mouse::{MOUSE_EVENT_MASK, mouse_handler_begin};
use crate::notifications::skylight_connection::connection_handler;
use crate::notifications::window::update_window_notifications;
use crate::notifications::workspace::workspace_event_handler_begin;
use crate::process::manager::{ProcessManager, process_manager_begin};
use crate::space::manager::{SpaceManager, hash_view_key, space_manager_begin};
use crate::startup::config_file::exec_config_file;
use crate::state::event_loop_owned::EventLoopOwnedState;
use crate::state::mission_control_mode::MissionControlMode;
use crate::state::process_wide::{CONFIG_FILE, CONNECTION, SOCKET_FILE};
use crate::support::color::RgbaColor;
use crate::support::easing::AnimationEasingType;
use crate::support::handles::{ProcessId, SpaceId, WindowId};
use crate::support::macos_version::{
    workspace_is_macos_monterey, workspace_is_macos_sequoia, workspace_is_macos_sonoma,
    workspace_is_macos_tahoe, workspace_is_macos_ventura,
};
use crate::support::table::Table;
use crate::window::discovery::window_manager_begin;
use crate::window::manager::{
    FfmMode, PurifyMode, WindowManager, WindowOriginMode, hash_wm_process_id, hash_wm_window_id,
    window_manager_init,
};

pub(crate) fn start_the_daemon_and_enter_the_main_run_loop() {
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
