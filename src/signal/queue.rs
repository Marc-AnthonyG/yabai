use crate::display::arrangement::query_arrangement_index_of_display;
use crate::display::manager::DisplayManager;
use crate::ffi::carbon_events::GetCurrentEventTime;
use crate::process::manager::ProcessManager;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal, SignalType};
use crate::space::lookup::query_mission_control_index_of_space;
use crate::space::manager::SpaceManager;
use crate::state::mission_control_mode::{MISSION_CONTROL_MODE_NAMES, MissionControlMode};
use crate::support::handles::{DisplayId, ProcessId, SpaceId, WindowId};
use crate::support::log::text_or_printf_null_placeholder;
use crate::window::manager::WindowManager;
use crate::window::model::window_title_as_string;

pub(crate) struct PendingSignal {
    pub(crate) signal_type: SignalType,
    pub(crate) arguments: [Option<(String, String)>; 4],
    pub(crate) app: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) active: i32,
}

pub(crate) enum SignalContext {
    None,
    Application(ProcessId),
    Window(WindowId),
    Space(SpaceId),
    Display(DisplayId),
    MissionControl(MissionControlMode),
}

pub(crate) fn queue_pending_signal_for_its_subscribers(
    signal_type: SignalType,
    context: SignalContext,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    signal_storage: &mut Vec<PendingSignal>,
) {
    let signal_count = signal_event[signal_type as usize].len() as i32;
    if signal_count == 0 {
        return;
    }

    let mut event_signal = PendingSignal {
        signal_type,
        arguments: [None, None, None, None],
        app: None,
        title: None,
        active: 0,
    };

    match signal_type {
        SignalType::ApplicationLaunched
        | SignalType::ApplicationActivated
        | SignalType::ApplicationDeactivated
        | SignalType::ApplicationVisible => {
            let SignalContext::Application(process_id) = context else {
                return;
            };
            let Some(application) = window_manager.application.find(&process_id) else {
                return;
            };

            event_signal.arguments[0] = Some((
                String::from("YABAI_PROCESS_ID"),
                format!("{}", process_id.0),
            ));

            event_signal.app = Some(application.name.to_string());
        }
        SignalType::ApplicationTerminated => {
            let SignalContext::Application(process_id) = context else {
                return;
            };
            let Some(application) = window_manager.application.find(&process_id) else {
                return;
            };

            event_signal.arguments[0] = Some((
                String::from("YABAI_PROCESS_ID"),
                format!("{}", process_id.0),
            ));

            event_signal.app = Some(application.name.to_string());

            //
            // NOTE(asmvik): We always receive an application_front_switched event *before* an application_terminated
            // event. We need to know the difference between a front_switched + application_terminated sequence and a regular
            // application switch followed by a user-initiated termination of the previously focused application. The system
            // events are triggered within an interval that appear to be impossible to match even with a user automated sequence.
            // The dt threshold below is triple the average interval computed, to allow for some leeway.
            //

            let delta_time = unsafe { GetCurrentEventTime() } - process_manager.switch_event_time;
            if delta_time >= f64::from(0.05f32) {
                event_signal.active = i32::from(process_manager.front_process_id == process_id);
            } else {
                event_signal.active =
                    i32::from(process_manager.last_front_process_id == process_id);
            }
        }
        SignalType::ApplicationFrontSwitched => {
            event_signal.arguments[0] = Some((
                String::from("YABAI_PROCESS_ID"),
                format!("{}", process_manager.front_process_id.0),
            ));
            event_signal.arguments[1] = Some((
                String::from("YABAI_RECENT_PROCESS_ID"),
                format!("{}", process_manager.last_front_process_id.0),
            ));
        }
        SignalType::ApplicationHidden => {
            let SignalContext::Application(process_id) = context else {
                return;
            };
            let Some(application) = window_manager.application.find(&process_id) else {
                return;
            };

            event_signal.arguments[0] = Some((
                String::from("YABAI_PROCESS_ID"),
                format!("{}", process_id.0),
            ));

            event_signal.app = Some(application.name.to_string());
            event_signal.active = i32::from(process_manager.front_process_id == process_id);
        }
        SignalType::WindowCreated | SignalType::WindowFocused | SignalType::WindowDeminimized => {
            let SignalContext::Window(window_id) = context else {
                return;
            };
            let Some(window) = window_manager.window.find(&window_id) else {
                return;
            };

            event_signal.arguments[0] = Some((
                String::from("YABAI_WINDOW_ID"),
                format!("{}", window.id.0 as i32),
            ));

            let Some(application_process_id) = window.application else {
                return;
            };
            let Some(application) = window_manager.application.find(&application_process_id) else {
                return;
            };

            event_signal.app = Some(application.name.to_string());
            event_signal.title = Some(window_title_as_string(window));
        }
        SignalType::WindowDestroyed => {
            let SignalContext::Window(window_id) = context else {
                return;
            };
            let Some(window) = window_manager.window.find(&window_id) else {
                return;
            };

            event_signal.arguments[0] = Some((
                String::from("YABAI_WINDOW_ID"),
                format!("{}", window.id.0 as i32),
            ));

            let application = window.application.and_then(|application_process_id| {
                window_manager.application.find(&application_process_id)
            });
            event_signal.app = match application {
                Some(application) => Some(application.name.to_string()),
                None => Some(String::from("<unknown>")),
            };
            event_signal.active = i32::from(window_manager.focused_window_id == window.id);
        }
        SignalType::WindowMoved
        | SignalType::WindowResized
        | SignalType::WindowMinimized
        | SignalType::WindowTitleChanged => {
            let SignalContext::Window(window_id) = context else {
                return;
            };
            let Some(window) = window_manager.window.find(&window_id) else {
                return;
            };

            event_signal.arguments[0] = Some((
                String::from("YABAI_WINDOW_ID"),
                format!("{}", window.id.0 as i32),
            ));

            let Some(application_process_id) = window.application else {
                return;
            };
            let Some(application) = window_manager.application.find(&application_process_id) else {
                return;
            };

            event_signal.app = Some(application.name.to_string());
            event_signal.title = Some(window_title_as_string(window));
            event_signal.active = i32::from(window_manager.focused_window_id == window.id);
        }
        SignalType::SpaceCreated => {
            let SignalContext::Space(space_id) = context else {
                return;
            };
            let index = query_mission_control_index_of_space(space_id);

            event_signal.arguments[0] = Some((
                String::from("YABAI_SPACE_ID"),
                format!("{}", space_id.0 as i64),
            ));
            event_signal.arguments[1] =
                Some((String::from("YABAI_SPACE_INDEX"), format!("{}", index)));
        }
        SignalType::SpaceDestroyed => {
            let SignalContext::Space(space_id) = context else {
                return;
            };

            event_signal.arguments[0] = Some((
                String::from("YABAI_SPACE_ID"),
                format!("{}", space_id.0 as i64),
            ));
        }
        SignalType::SpaceChanged => {
            let space_id = space_manager.current_space_id;
            let recent_space_id = space_manager.last_space_id;

            let index = query_mission_control_index_of_space(space_id);
            let recent_index = query_mission_control_index_of_space(recent_space_id);

            event_signal.arguments[0] = Some((
                String::from("YABAI_SPACE_ID"),
                format!("{}", space_id.0 as i64),
            ));
            event_signal.arguments[1] = Some((
                String::from("YABAI_RECENT_SPACE_ID"),
                format!("{}", recent_space_id.0 as i64),
            ));

            event_signal.arguments[2] =
                Some((String::from("YABAI_SPACE_INDEX"), format!("{}", index)));
            event_signal.arguments[3] = Some((
                String::from("YABAI_RECENT_SPACE_INDEX"),
                format!("{}", recent_index),
            ));
        }
        SignalType::DisplayAdded | SignalType::DisplayMoved | SignalType::DisplayResized => {
            let SignalContext::Display(display_id) = context else {
                return;
            };
            let index = query_arrangement_index_of_display(display_id, display_manager);

            event_signal.arguments[0] = Some((
                String::from("YABAI_DISPLAY_ID"),
                format!("{}", display_id.0 as i32),
            ));
            event_signal.arguments[1] =
                Some((String::from("YABAI_DISPLAY_INDEX"), format!("{}", index)));
        }
        SignalType::DisplayRemoved => {
            let SignalContext::Display(display_id) = context else {
                return;
            };

            event_signal.arguments[0] = Some((
                String::from("YABAI_DISPLAY_ID"),
                format!("{}", display_id.0 as i32),
            ));
        }
        SignalType::DisplayChanged => {
            let display_id = display_manager.current_display_id;
            let recent_display_id = display_manager.last_display_id;

            let index = query_arrangement_index_of_display(display_id, display_manager);
            let recent_index =
                query_arrangement_index_of_display(recent_display_id, display_manager);

            event_signal.arguments[0] = Some((
                String::from("YABAI_DISPLAY_ID"),
                format!("{}", display_id.0 as i32),
            ));
            event_signal.arguments[1] = Some((
                String::from("YABAI_RECENT_DISPLAY_ID"),
                format!("{}", recent_display_id.0 as i32),
            ));

            event_signal.arguments[2] =
                Some((String::from("YABAI_DISPLAY_INDEX"), format!("{}", index)));
            event_signal.arguments[3] = Some((
                String::from("YABAI_RECENT_DISPLAY_INDEX"),
                format!("{}", recent_index),
            ));
        }
        SignalType::MissionControlEnter | SignalType::MissionControlExit => {
            let SignalContext::MissionControl(mode) = context else {
                return;
            };

            event_signal.arguments[0] = Some((
                String::from("YABAI_MISSION_CONTROL_MODE"),
                format!(
                    "{}",
                    text_or_printf_null_placeholder(MISSION_CONTROL_MODE_NAMES[mode as usize])
                ),
            ));
        }
        _ => {}
    }

    signal_storage.push(event_signal);
}
