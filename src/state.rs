use crate::display_manager::DisplayManager;
use crate::event_signal::{PendingSignal, SIGNAL_TYPE_COUNT, Signal};
use crate::ffi::core_foundation::{CGPoint, CGRect};
use crate::handles::{NodeId, SpaceId, WindowId};
use crate::mission_control::MissionControlMode;
use crate::mouse_handler::MouseMode;
use crate::process_manager::ProcessManager;
use crate::space_manager::SpaceManager;
use crate::window_manager::{FfmMode, WindowManager};

pub struct MouseDragState {
    pub current_action: MouseMode,
    pub down_location: CGPoint,
    pub last_moved_time: u64,
    pub window_id: Option<WindowId>,
    pub window_frame: CGRect,
    pub ffm_window_id: WindowId,
    pub direction: u8,
    pub feedback_node: Option<(SpaceId, NodeId)>,
}

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
