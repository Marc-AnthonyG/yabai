#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum MissionControlMode {
    #[default]
    Inactive = 0,
    Show = 1,
    ShowAllWindows = 2,
    ShowFrontWindows = 3,
    ShowDesktop = 4,
}

pub(crate) static MISSION_CONTROL_MODE_STR: [Option<&str>; 5] = [
    Some("inactive"),
    Some("show"),
    Some("show-all-windows"),
    Some("show-front-windows"),
    Some("show-desktop"),
];

pub(crate) fn mission_control_is_active(mission_control_mode: &mut MissionControlMode) -> bool {
    *mission_control_mode != MissionControlMode::Inactive
}
