use core::ffi::c_char;
use std::ffi::{CStr, CString};

use crate::display_manager::{DisplayManager, display_manager_display_id_arrangement};
use crate::ffi::carbon_events::GetCurrentEventTime;
use crate::handles::{DisplayId, ProcessId, SpaceId, WindowId};
use crate::misc::helpers::{json_optional_bool, string_equals, ts_string_escape};
use crate::misc::log::or_null;
use crate::misc::regex::{PosixRegex, RegexMatch, regex_match};
use crate::misc::response::Response;
use crate::mission_control::{MISSION_CONTROL_MODE_STR, MissionControlMode};
use crate::process_manager::ProcessManager;
use crate::space_manager::{SpaceManager, space_manager_mission_control_index};
use crate::window::window_title_ts;
use crate::window_manager::WindowManager;

#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SignalType {
    Unknown = 0,

    ApplicationLaunched = 1,
    ApplicationTerminated = 2,
    ApplicationFrontSwitched = 3,
    ApplicationActivated = 4,
    ApplicationDeactivated = 5,
    ApplicationVisible = 6,
    ApplicationHidden = 7,

    WindowCreated = 8,
    WindowDestroyed = 9,
    WindowFocused = 10,
    WindowMoved = 11,
    WindowResized = 12,
    WindowMinimized = 13,
    WindowDeminimized = 14,
    WindowTitleChanged = 15,

    SpaceCreated = 16,
    SpaceDestroyed = 17,
    SpaceChanged = 18,

    DisplayAdded = 19,
    DisplayRemoved = 20,
    DisplayMoved = 21,
    DisplayResized = 22,
    DisplayChanged = 23,

    MissionControlEnter = 24,
    MissionControlExit = 25,

    DockDidChangePref = 26,
    DockDidRestart = 27,

    MenuBarHiddenChanged = 28,
    SystemWoke = 29,
}

#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SignalProp {
    #[default]
    Undefined = 0,
    Yes = 1,
    No = 2,
}

pub(crate) struct PendingSignal {
    pub(crate) signal_type: SignalType,
    pub(crate) arguments: [Option<(String, String)>; 4],
    pub(crate) app: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) active: i32,
}

pub(crate) struct Signal {
    pub(crate) app: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) app_regex_exclude: bool,
    pub(crate) title_regex_exclude: bool,
    pub(crate) app_regex: Option<PosixRegex>,
    pub(crate) title_regex: Option<PosixRegex>,
    pub(crate) active: SignalProp,
    pub(crate) command: Option<String>,
    pub(crate) label: Option<String>,
}

pub(crate) enum SignalContext {
    None,
    Application(ProcessId),
    Window(WindowId),
    Space(SpaceId),
    Display(DisplayId),
    MissionControl(MissionControlMode),
}

pub(crate) struct PreparedSignalCommand {
    arguments: Vec<CString>,
    environment: Vec<CString>,
}

pub(crate) const SIGNAL_TYPE_COUNT: usize = 30;

pub(crate) static SIGNAL_TYPE_STR: [&str; 31] = [
    "signal_type_unknown",
    "application_launched",
    "application_terminated",
    "application_front_switched",
    "application_activated",
    "application_deactivated",
    "application_visible",
    "application_hidden",
    "window_created",
    "window_destroyed",
    "window_focused",
    "window_moved",
    "window_resized",
    "window_minimized",
    "window_deminimized",
    "window_title_changed",
    "space_created",
    "space_destroyed",
    "space_changed",
    "display_added",
    "display_removed",
    "display_moved",
    "display_resized",
    "display_changed",
    "mission_control_enter",
    "mission_control_exit",
    "dock_did_change_pref",
    "dock_did_restart",
    "menu_bar_hidden_changed",
    "system_woke",
    "signal_type_count",
];

static SIGNAL_TYPE_BY_DISCRIMINANT: [SignalType; SIGNAL_TYPE_COUNT] = [
    SignalType::Unknown,
    SignalType::ApplicationLaunched,
    SignalType::ApplicationTerminated,
    SignalType::ApplicationFrontSwitched,
    SignalType::ApplicationActivated,
    SignalType::ApplicationDeactivated,
    SignalType::ApplicationVisible,
    SignalType::ApplicationHidden,
    SignalType::WindowCreated,
    SignalType::WindowDestroyed,
    SignalType::WindowFocused,
    SignalType::WindowMoved,
    SignalType::WindowResized,
    SignalType::WindowMinimized,
    SignalType::WindowDeminimized,
    SignalType::WindowTitleChanged,
    SignalType::SpaceCreated,
    SignalType::SpaceDestroyed,
    SignalType::SpaceChanged,
    SignalType::DisplayAdded,
    SignalType::DisplayRemoved,
    SignalType::DisplayMoved,
    SignalType::DisplayResized,
    SignalType::DisplayChanged,
    SignalType::MissionControlEnter,
    SignalType::MissionControlExit,
    SignalType::DockDidChangePref,
    SignalType::DockDidRestart,
    SignalType::MenuBarHiddenChanged,
    SignalType::SystemWoke,
];

fn regex_subject_truncated_at_the_first_null(subject: Option<&str>) -> CString {
    let bytes = subject.unwrap_or("").as_bytes();
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    CString::new(&bytes[..end]).unwrap_or_default()
}

pub(crate) fn event_signal_filter(event_signal: &PendingSignal, signal: &Signal) -> bool {
    match event_signal.signal_type {
        SignalType::ApplicationLaunched
        | SignalType::ApplicationActivated
        | SignalType::ApplicationDeactivated
        | SignalType::ApplicationVisible => {
            let regex_match_app = if signal.app_regex_exclude {
                RegexMatch::Yes
            } else {
                RegexMatch::No
            };
            regex_match(
                signal.app_regex.as_ref(),
                &regex_subject_truncated_at_the_first_null(event_signal.app.as_deref()),
            ) == regex_match_app
        }
        SignalType::ApplicationTerminated
        | SignalType::ApplicationHidden
        | SignalType::WindowDestroyed => {
            let regex_match_app = if signal.app_regex_exclude {
                RegexMatch::Yes
            } else {
                RegexMatch::No
            };
            let app_no_match = regex_match(
                signal.app_regex.as_ref(),
                &regex_subject_truncated_at_the_first_null(event_signal.app.as_deref()),
            ) == regex_match_app;

            let mut active = signal.active == SignalProp::Undefined;
            if !active {
                active = event_signal.active == i32::from(signal.active == SignalProp::Yes);
            }

            app_no_match || !active
        }
        SignalType::WindowCreated
        | SignalType::WindowFocused
        | SignalType::WindowDeminimized => {
            let regex_match_app = if signal.app_regex_exclude {
                RegexMatch::Yes
            } else {
                RegexMatch::No
            };
            let app_no_match = regex_match(
                signal.app_regex.as_ref(),
                &regex_subject_truncated_at_the_first_null(event_signal.app.as_deref()),
            ) == regex_match_app;

            let regex_match_title = if signal.title_regex_exclude {
                RegexMatch::Yes
            } else {
                RegexMatch::No
            };
            let title_no_match = regex_match(
                signal.title_regex.as_ref(),
                &regex_subject_truncated_at_the_first_null(event_signal.title.as_deref()),
            ) == regex_match_title;

            app_no_match || title_no_match
        }
        SignalType::WindowMoved
        | SignalType::WindowResized
        | SignalType::WindowMinimized
        | SignalType::WindowTitleChanged => {
            let regex_match_app = if signal.app_regex_exclude {
                RegexMatch::Yes
            } else {
                RegexMatch::No
            };
            let app_no_match = regex_match(
                signal.app_regex.as_ref(),
                &regex_subject_truncated_at_the_first_null(event_signal.app.as_deref()),
            ) == regex_match_app;

            let regex_match_title = if signal.title_regex_exclude {
                RegexMatch::Yes
            } else {
                RegexMatch::No
            };
            let title_no_match = regex_match(
                signal.title_regex.as_ref(),
                &regex_subject_truncated_at_the_first_null(event_signal.title.as_deref()),
            ) == regex_match_title;

            let mut active = signal.active == SignalProp::Undefined;
            if !active {
                active = event_signal.active == i32::from(signal.active == SignalProp::Yes);
            }

            app_no_match || title_no_match || !active
        }
        _ => false,
    }
}

pub(crate) fn event_signal_prepare_commands(
    signal_event: &[Vec<Signal>; SIGNAL_TYPE_COUNT],
    signal_storage: &[PendingSignal],
) -> Vec<PreparedSignalCommand> {
    let mut inherited_environment: Vec<CString> = Vec::new();
    unsafe {
        let mut entry = *libc::_NSGetEnviron();
        while !(*entry).is_null() {
            inherited_environment.push(CStr::from_ptr(*entry).to_owned());
            entry = entry.add(1);
        }
    }

    let mut prepared_commands: Vec<PreparedSignalCommand> = Vec::new();

    let count = signal_storage.len() as i32;

    for index in 0..count {
        let event_signal = &signal_storage[index as usize];

        let signal_count = signal_event[event_signal.signal_type as usize].len() as i32;
        crate::debug!(
            "{}: transmitting {} to {} subscriber(s)\n",
            "event_signal_flush",
            SIGNAL_TYPE_STR[event_signal.signal_type as usize],
            signal_count
        );

        for inner_index in 0..signal_count {
            let signal = &signal_event[event_signal.signal_type as usize][inner_index as usize];
            if event_signal_filter(event_signal, signal) {
                continue;
            }

            let mut environment = inherited_environment.clone();
            for argument in event_signal.arguments.iter() {
                if let Some((name, value)) = argument {
                    let entry = CString::new(format!("{}={}", name, value)).unwrap();
                    let prefix = format!("{}=", name);
                    let existing = environment
                        .iter()
                        .position(|entry| entry.as_bytes().starts_with(prefix.as_bytes()));
                    match existing {
                        Some(existing) => environment[existing] = entry,
                        None => environment.push(entry),
                    }
                }
            }

            let mut arguments = vec![
                CString::new("/usr/bin/env").unwrap(),
                CString::new("sh").unwrap(),
                CString::new("-c").unwrap(),
            ];
            if let Some(command) = signal.command.as_deref() {
                arguments.push(CString::new(command).unwrap());
            }

            prepared_commands.push(PreparedSignalCommand {
                arguments,
                environment,
            });
        }
    }

    prepared_commands
}

pub(crate) fn null_terminated_pointer_list(strings: &[CString]) -> Vec<*const c_char> {
    let mut pointers: Vec<*const c_char> = strings.iter().map(|string| string.as_ptr()).collect();
    pointers.push(core::ptr::null());
    pointers
}

pub(crate) fn event_signal_flush(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    signal_storage: &mut Vec<PendingSignal>,
) {
    if signal_storage.is_empty() {
        return;
    }

    let prepared_commands = event_signal_prepare_commands(signal_event, signal_storage.as_slice());
    let prepared_command_pointers: Vec<(Vec<*const c_char>, Vec<*const c_char>)> =
        prepared_commands
            .iter()
            .map(|prepared_command| {
                (
                    null_terminated_pointer_list(&prepared_command.arguments),
                    null_terminated_pointer_list(&prepared_command.environment),
                )
            })
            .collect();

    let process_id = unsafe { libc::fork() };
    if process_id != 0 {
        signal_storage.clear();
        return;
    }

    for (argument_pointers, environment_pointers) in prepared_command_pointers.iter() {
        let process_id = unsafe { libc::fork() };
        if process_id != 0 {
            continue;
        }

        unsafe {
            *libc::_NSGetEnviron() = environment_pointers.as_ptr() as *mut *mut c_char;
            libc::_exit(libc::execvp(
                argument_pointers[0],
                argument_pointers.as_ptr(),
            ));
        }
    }

    unsafe { libc::_exit(libc::EXIT_SUCCESS) }
}

pub(crate) fn event_signal_push(
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

            let delta_time =
                unsafe { GetCurrentEventTime() } - process_manager.switch_event_time;
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
        SignalType::WindowCreated
        | SignalType::WindowFocused
        | SignalType::WindowDeminimized => {
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
            event_signal.title = Some(window_title_ts(window));
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

            let application = window
                .application
                .and_then(|application_process_id| {
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
            event_signal.title = Some(window_title_ts(window));
            event_signal.active = i32::from(window_manager.focused_window_id == window.id);
        }
        SignalType::SpaceCreated => {
            let SignalContext::Space(space_id) = context else {
                return;
            };
            let index = space_manager_mission_control_index(space_id);

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

            let index = space_manager_mission_control_index(space_id);
            let recent_index = space_manager_mission_control_index(recent_space_id);

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
            let index = display_manager_display_id_arrangement(display_id, display_manager);

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

            let index = display_manager_display_id_arrangement(display_id, display_manager);
            let recent_index =
                display_manager_display_id_arrangement(recent_display_id, display_manager);

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
                format!("{}", or_null(MISSION_CONTROL_MODE_STR[mode as usize])),
            ));
        }
        _ => {}
    }

    signal_storage.push(event_signal);
}

pub(crate) fn signal_type_from_string(string: &[u8]) -> SignalType {
    let string = std::str::from_utf8(string).ok();

    for index in SignalType::ApplicationLaunched as usize..SIGNAL_TYPE_COUNT {
        if string_equals(string, Some(SIGNAL_TYPE_STR[index])) {
            return SIGNAL_TYPE_BY_DISCRIMINANT[index];
        }
    }

    SignalType::Unknown
}

pub(crate) fn event_signal_add(
    signal_type: SignalType,
    signal: Signal,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
) {
    if let Some(label) = signal.label.as_deref() {
        event_signal_remove(label.as_bytes(), signal_event);
    }
    signal_event[signal_type as usize].push(signal);
}

impl Drop for Signal {
    fn drop(&mut self) {
        drop(self.app_regex.take());
        drop(self.title_regex.take());
        drop(self.command.take());
        drop(self.label.take());
        drop(self.app.take());
        drop(self.title.take());
    }
}

pub(crate) fn event_signal_remove_by_index(
    index: i32,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
) -> bool {
    let mut signal_index: i32 = 0;
    for signal_type_index in SignalType::ApplicationLaunched as usize..SIGNAL_TYPE_COUNT {
        for inner_index in 0..signal_event[signal_type_index].len() {
            if signal_index == index {
                signal_event[signal_type_index].swap_remove(inner_index);
                return true;
            }
            signal_index += 1;
        }
    }

    false
}

pub(crate) fn event_signal_remove(
    label: &[u8],
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
) -> bool {
    let label = String::from_utf8_lossy(label);

    for index in SignalType::ApplicationLaunched as usize..SIGNAL_TYPE_COUNT {
        for inner_index in 0..signal_event[index].len() {
            if string_equals(Some(&label), signal_event[index][inner_index].label.as_deref()) {
                signal_event[index].swap_remove(inner_index);
                return true;
            }
        }
    }

    false
}

pub(crate) fn event_signal_serialize(
    response: &mut Response,
    signal: &Signal,
    signal_type: SignalType,
    index: i32,
) {
    let app = signal.app.as_deref();
    let title = signal.title.as_deref();
    let command = signal.command.as_deref();

    let escaped_app = app.and_then(ts_string_escape);
    let escaped_title = title.and_then(ts_string_escape);
    let escaped_command = command.and_then(ts_string_escape);

    response.write(format_args!(
        "{{\n\t\"index\":{},\n\t\"label\":\"{}\",\n\t\"app\":\"{}\",\n\t\"title\":\"{}\",\n\t\"active\":{},\n\t\"event\":\"{}\",\n\t\"action\":\"{}\"\n}}",
        index,
        signal.label.as_deref().unwrap_or(""),
        escaped_app.as_deref().or(app).unwrap_or(""),
        escaped_title.as_deref().or(title).unwrap_or(""),
        json_optional_bool(signal.active as i32),
        SIGNAL_TYPE_STR[signal_type as usize],
        escaped_command.as_deref().or(command).unwrap_or("")
    ));
}

pub(crate) fn event_signal_list(
    response: &mut Response,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
) {
    response.write(format_args!("["));
    let mut signal_index: i32 = 0;
    let mut event_did_output = false;
    for index in SignalType::ApplicationLaunched as usize..SIGNAL_TYPE_COUNT {
        let count = signal_event[index].len() as i32;

        if count > 0 && event_did_output {
            response.write(format_args!(","));
        }

        for inner_index in 0..count {
            event_signal_serialize(
                response,
                &signal_event[index][inner_index as usize],
                SIGNAL_TYPE_BY_DISCRIMINANT[index],
                signal_index,
            );
            if inner_index < signal_event[index].len() as i32 - 1 {
                response.write(format_args!(","));
            }
            signal_index += 1;
        }

        if !event_did_output {
            event_did_output = count > 0;
        }
    }
    response.write(format_args!("]\n"));
}
