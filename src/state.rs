use crate::display_manager::DisplayManager;
use crate::mission_control::MissionControlMode;
use crate::mouse::drag::MouseDragState;
use crate::process_manager::ProcessManager;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal};
use crate::signal::queue::PendingSignal;
use crate::space_manager::SpaceManager;
use crate::window_manager::{FfmMode, WindowManager};

pub struct EventLoopOwnedState {
    pub signal_event: [Vec<Signal>; SIGNAL_TYPE_COUNT],
    pub process_manager: ProcessManager,
    pub display_manager: DisplayManager,
    pub window_manager: WindowManager,
    pub space_manager: SpaceManager,
    pub signal_storage: Vec<PendingSignal>,
    pub mouse_drag_state: MouseDragState,
    pub mission_control_mode: MissionControlMode,
    pub focus_follows_mouse_suspended_value: FfmMode,
    pub is_menu_open: i32,
}

unsafe impl Send for EventLoopOwnedState {}
