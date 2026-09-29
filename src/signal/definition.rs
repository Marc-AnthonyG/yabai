use clap::ValueEnum;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SignalType {
    #[value(skip)]
    Unknown = 0,

    /// An application launched; sets YABAI_PROCESS_ID; filtered by --app
    ApplicationLaunched = 1,
    /// An application quit; sets YABAI_PROCESS_ID; filtered by --app and --active
    ApplicationTerminated = 2,
    /// Another application came to the front; sets YABAI_PROCESS_ID and
    /// YABAI_RECENT_PROCESS_ID
    ApplicationFrontSwitched = 3,
    /// An application was activated; sets YABAI_PROCESS_ID; filtered by --app
    ApplicationActivated = 4,
    /// An application was deactivated; sets YABAI_PROCESS_ID; filtered by --app
    ApplicationDeactivated = 5,
    /// An application was unhidden; sets YABAI_PROCESS_ID; filtered by --app
    ApplicationVisible = 6,
    /// An application was hidden; sets YABAI_PROCESS_ID; filtered by --app and --active
    ApplicationHidden = 7,

    /// A window opened, also when its application launched; sets YABAI_WINDOW_ID; filtered
    /// by --app and --title
    WindowCreated = 8,
    /// A window closed, also when its application quit; sets YABAI_WINDOW_ID; filtered by
    /// --app and --active
    WindowDestroyed = 9,
    /// A window became the key window; sets YABAI_WINDOW_ID; filtered by --app and --title
    WindowFocused = 10,
    /// A window moved; sets YABAI_WINDOW_ID; filtered by --app, --title and --active
    WindowMoved = 11,
    /// A window changed size; sets YABAI_WINDOW_ID; filtered by --app, --title and --active
    WindowResized = 12,
    /// A window was minimized; sets YABAI_WINDOW_ID; filtered by --app, --title and --active
    WindowMinimized = 13,
    /// A window was deminimized; sets YABAI_WINDOW_ID; filtered by --app and --title
    WindowDeminimized = 14,
    /// A window changed its title; sets YABAI_WINDOW_ID; filtered by --app, --title and
    /// --active
    WindowTitleChanged = 15,

    /// A space was created; sets YABAI_SPACE_ID and YABAI_SPACE_INDEX
    SpaceCreated = 16,
    /// A space was destroyed; sets YABAI_SPACE_ID
    SpaceDestroyed = 17,
    /// Another space became active; sets YABAI_SPACE_ID, YABAI_SPACE_INDEX,
    /// YABAI_RECENT_SPACE_ID and YABAI_RECENT_SPACE_INDEX
    SpaceChanged = 18,

    /// A display was connected; sets YABAI_DISPLAY_ID and YABAI_DISPLAY_INDEX
    DisplayAdded = 19,
    /// A display was disconnected; sets YABAI_DISPLAY_ID
    DisplayRemoved = 20,
    /// The display arrangement changed; sets YABAI_DISPLAY_ID and YABAI_DISPLAY_INDEX
    DisplayMoved = 21,
    /// A display changed resolution; sets YABAI_DISPLAY_ID and YABAI_DISPLAY_INDEX
    DisplayResized = 22,
    /// Another display became active; sets YABAI_DISPLAY_ID, YABAI_DISPLAY_INDEX,
    /// YABAI_RECENT_DISPLAY_ID and YABAI_RECENT_DISPLAY_INDEX
    DisplayChanged = 23,

    /// Mission control opened; sets YABAI_MISSION_CONTROL_MODE
    MissionControlEnter = 24,
    /// Mission control closed; sets YABAI_MISSION_CONTROL_MODE
    MissionControlExit = 25,

    /// The Dock preferences changed
    DockDidChangePreferences = 26,
    /// The Dock restarted, which unloads the scripting addition
    DockDidRestart = 27,

    /// The menu bar autohide setting changed
    MenuBarHiddenChanged = 28,
    /// The system woke from sleep
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

pub(crate) fn add_signal_replacing_any_with_the_same_label(
    signal_type: SignalType,
    signal: Signal,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
) {
    if let Some(label) = signal.label.as_deref() {
        remove_signal_with_label(label, signal_event);
    }
    signal_event[signal_type as usize].push(signal);
}

pub(crate) fn remove_signal_at_listing_index(
    index: usize,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
) -> bool {
    let mut signals_listed_before_this_event = 0;
    for signals in signal_event
        .iter_mut()
        .skip(SignalType::ApplicationLaunched as usize)
    {
        if index < signals_listed_before_this_event + signals.len() {
            signals.swap_remove(index - signals_listed_before_this_event);
            return true;
        }
        signals_listed_before_this_event += signals.len();
    }

    false
}

pub(crate) fn remove_signal_with_label(
    label: &str,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
) -> bool {
    for signals in signal_event
        .iter_mut()
        .skip(SignalType::ApplicationLaunched as usize)
    {
        if let Some(position) = signals
            .iter()
            .position(|signal| signal.label.as_deref() == Some(label))
        {
            signals.swap_remove(position);
            return true;
        }
    }

    false
}
