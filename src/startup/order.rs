use core::ptr::null_mut;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use objc2::MainThreadMarker;

use crate::config_file::location::locate_the_config_file_warning_when_there_is_none;
use crate::config_file::shell_run::run_config_file_in_a_forked_shell;
use crate::display::manager::{
    DisplayManager, start_display_manager_observing_display_reconfiguration,
};
use crate::error;
use crate::event::queue::create_event_loop_channel_storing_its_sender;
use crate::event::run_loop::run_event_loop_flushing_signals_after_each_event;
use crate::ffi::appkit::NSApplication;
use crate::ffi::carbon_process::ProcessSerialNumber;
use crate::ffi::core_foundation::{CGPoint, CGRect};
use crate::ffi::skylight::SLSRegisterConnectionNotifyProc;
use crate::layout::group_header_style::group_header_style_with_its_initial_settings;
use crate::layout::insertion::WindowInsertionPoint;
use crate::layout::settings::ViewLayout;
use crate::layout::tree::{WindowNodeChild, WindowNodeSplit};
use crate::message::listening_socket::start_listening_on_message_socket;
use crate::mouse::drag::MouseDragState;
use crate::mouse::tap::MouseMode;
use crate::notifications::mission_control::start_observing_mission_control_through_the_dock;
use crate::notifications::mouse::{MOUSE_EVENT_MASK_WITHOUT_MOUSE_MOVED, start_mouse_event_tap};
use crate::notifications::skylight_connection::handle_skylight_connection_notification_callback;
use crate::notifications::window::request_skylight_notifications_for_windows_that_need_them;
use crate::notifications::workspace::detect_macos_version_and_start_observing_workspace_notifications;
use crate::process::manager::{ProcessManager, start_process_manager_observing_application_events};
use crate::space::manager::{SpaceManager, start_space_manager_creating_a_view_for_every_space};
use crate::state::event_loop_owned::EventLoopOwnedState;
use crate::state::mission_control_mode::MissionControlMode;
use crate::state::process_wide::{MESSAGE_SOCKET_PATH, SKYLIGHT_CONNECTION_ID};
use crate::support::color::RgbaColor;
use crate::support::easing::AnimationEasingType;
use crate::support::handles::{ProcessId, SpaceId, WindowId};
use crate::support::macos_version::{
    is_running_on_macos_monterey, is_running_on_macos_sequoia, is_running_on_macos_sonoma,
    is_running_on_macos_tahoe, is_running_on_macos_ventura,
};
use crate::window::animator::WindowAnimator;
use crate::window::discovery::start_tracking_running_applications_and_their_windows;
use crate::window::manager::{
    FocusFollowsMouseMode, ShadowRemovalMode, WindowManager, WindowOriginDisplayMode,
    initialize_window_manager,
};

pub(crate) fn start_the_daemon_and_enter_the_main_run_loop() {
    let event_receiver = create_event_loop_channel_storing_its_sender();

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
            application: BTreeMap::new(),
            window: BTreeMap::new(),
            managed_window: BTreeMap::new(),
            window_lost_focused_event: BTreeSet::new(),
            application_lost_front_switched_event: BTreeSet::new(),
            window_animator: Arc::new(WindowAnimator::new()),
            insert_feedback: BTreeMap::new(),
            rules: Vec::new(),
            applications_to_refresh: Vec::new(),
            focused_window_id: WindowId(0),
            focused_window_process_serial_number: ProcessSerialNumber {
                high_long_of_psn: 0,
                low_long_of_psn: 0,
            },
            last_window_id: WindowId(0),
            enable_mff: false,
            focus_follows_mouse_mode: FocusFollowsMouseMode::Disabled,
            shadow_removal_mode: ShadowRemovalMode::Never,
            window_origin_display_mode: WindowOriginDisplayMode::DisplayTheWindowOpenedOn,
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
            insert_feedback_color_follows_the_system_accent_color: true,
            scratchpad_window: Vec::new(),
            group_header_style: group_header_style_with_its_initial_settings(),
            group_headers_are_hidden_during_mission_control: false,
        },
        space_manager: SpaceManager {
            view: BTreeMap::new(),
            current_space_id: SpaceId(0),
            last_space_id: SpaceId(0),
            did_begin: false,
            layout: ViewLayout::Default,
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
            insert_feedback_fade_in_step_is_scheduled: false,
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
        focus_follows_mouse_suspended_value: FocusFollowsMouseMode::Disabled,
        is_menu_open: 0,
    };

    if !detect_macos_version_and_start_observing_workspace_notifications() {
        error!("yabai: could not start workspace context! abort..\n");
    }

    if !start_process_manager_observing_application_events(
        &mut event_loop_owned_state.process_manager,
    ) {
        error!("yabai: could not start process manager! abort..\n");
    }

    if !start_display_manager_observing_display_reconfiguration(
        &mut event_loop_owned_state.display_manager,
    ) {
        error!("yabai: could not start display manager! abort..\n");
    }

    if !start_mouse_event_tap(MOUSE_EVENT_MASK_WITHOUT_MOUSE_MOVED) {
        error!("yabai: could not start mouse handler! abort..\n");
    }

    let connection = *SKYLIGHT_CONNECTION_ID.get().unwrap();

    if is_running_on_macos_monterey()
        || is_running_on_macos_ventura()
        || is_running_on_macos_sonoma()
        || is_running_on_macos_sequoia()
        || is_running_on_macos_tahoe()
    {
        start_observing_mission_control_through_the_dock();

        if is_running_on_macos_ventura()
            || is_running_on_macos_sonoma()
            || is_running_on_macos_sequoia()
            || is_running_on_macos_tahoe()
        {
            unsafe {
                SLSRegisterConnectionNotifyProc(
                    connection,
                    handle_skylight_connection_notification_callback,
                    1327,
                    null_mut(),
                )
            };
            unsafe {
                SLSRegisterConnectionNotifyProc(
                    connection,
                    handle_skylight_connection_notification_callback,
                    1328,
                    null_mut(),
                )
            };
        }
    } else {
        unsafe {
            SLSRegisterConnectionNotifyProc(
                connection,
                handle_skylight_connection_notification_callback,
                1204,
                null_mut(),
            )
        };
    }

    unsafe {
        SLSRegisterConnectionNotifyProc(
            connection,
            handle_skylight_connection_notification_callback,
            808,
            null_mut(),
        )
    };
    unsafe {
        SLSRegisterConnectionNotifyProc(
            connection,
            handle_skylight_connection_notification_callback,
            1202,
            null_mut(),
        )
    };

    if is_running_on_macos_sequoia() || is_running_on_macos_tahoe() {
        unsafe {
            SLSRegisterConnectionNotifyProc(
                connection,
                handle_skylight_connection_notification_callback,
                804,
                null_mut(),
            )
        };
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
    initialize_window_manager(window_manager);
    start_space_manager_creating_a_view_for_every_space(
        space_manager,
        display_manager,
        window_manager,
    );
    start_tracking_running_applications_and_their_windows(
        space_manager,
        window_manager,
        process_manager,
        display_manager,
        mouse_drag_state,
        mission_control_mode,
    );

    if is_running_on_macos_sequoia() || is_running_on_macos_tahoe() {
        request_skylight_notifications_for_windows_that_need_them(window_manager, space_manager);
    }

    let _ = std::thread::Builder::new()
        .name(String::from("yabai-event-loop"))
        .spawn(move || {
            run_event_loop_flushing_signals_after_each_event(event_receiver, event_loop_owned_state)
        });

    if !start_listening_on_message_socket(Path::new(
        MESSAGE_SOCKET_PATH.get().map_or("", String::as_str),
    )) {
        error!("yabai: could not start message loop! abort..\n");
    }

    if let Some(config_file) = locate_the_config_file_warning_when_there_is_none() {
        run_config_file_in_a_forked_shell(&config_file);
    }

    if let Some(main_thread_marker) = MainThreadMarker::new() {
        NSApplication::sharedApplication(main_thread_marker).run();
    }
}
