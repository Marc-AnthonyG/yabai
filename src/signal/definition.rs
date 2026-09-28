use regex::Regex;

use crate::support::strings::are_both_strings_present_and_equal;

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

    DockDidChangePreferences = 26,
    DockDidRestart = 27,

    MenuBarHiddenChanged = 28,
    SystemWoke = 29,
}

#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SignalPropertyRequirement {
    #[default]
    Undefined = 0,
    Yes = 1,
    No = 2,
}

#[derive(Default)]
pub(crate) struct Signal {
    pub(crate) app: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) app_regex_exclude: bool,
    pub(crate) title_regex_exclude: bool,
    pub(crate) app_regex: Option<Regex>,
    pub(crate) title_regex: Option<Regex>,
    pub(crate) active: SignalPropertyRequirement,
    pub(crate) command: Option<String>,
    pub(crate) label: Option<String>,
}

pub(crate) const SIGNAL_TYPE_COUNT: usize = 30;

pub(crate) static SIGNAL_TYPE_NAMES: [&str; 31] = [
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

pub(crate) static SIGNAL_TYPE_BY_DISCRIMINANT: [SignalType; SIGNAL_TYPE_COUNT] = [
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
    SignalType::DockDidChangePreferences,
    SignalType::DockDidRestart,
    SignalType::MenuBarHiddenChanged,
    SignalType::SystemWoke,
];

pub(crate) fn signal_type_for_event_name(string: &[u8]) -> SignalType {
    let string = std::str::from_utf8(string).ok();

    for index in SignalType::ApplicationLaunched as usize..SIGNAL_TYPE_COUNT {
        if are_both_strings_present_and_equal(string, Some(SIGNAL_TYPE_NAMES[index])) {
            return SIGNAL_TYPE_BY_DISCRIMINANT[index];
        }
    }

    SignalType::Unknown
}

pub(crate) fn add_signal_replacing_any_with_the_same_label(
    signal_type: SignalType,
    signal: Signal,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
) {
    if let Some(label) = signal.label.as_deref() {
        remove_signal_with_label(label.as_bytes(), signal_event);
    }
    signal_event[signal_type as usize].push(signal);
}

pub(crate) fn remove_signal_at_listing_index(
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

pub(crate) fn remove_signal_with_label(
    label: &[u8],
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
) -> bool {
    let label = String::from_utf8_lossy(label);

    for index in SignalType::ApplicationLaunched as usize..SIGNAL_TYPE_COUNT {
        for inner_index in 0..signal_event[index].len() {
            if are_both_strings_present_and_equal(
                Some(&label),
                signal_event[index][inner_index].label.as_deref(),
            ) {
                signal_event[index].swap_remove(inner_index);
                return true;
            }
        }
    }

    false
}
