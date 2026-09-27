use crate::ffi::core_foundation::{CGPoint, CGRect};
use crate::handles::{NodeId, SpaceId, WindowId};
use crate::mouse::tap::MouseMode;
use crate::window::manager::WindowManager;

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

#[derive(Default)]
pub(crate) struct MouseWindowInfo {
    pub delta_x: f32,
    pub delta_y: f32,
    pub delta_width: f32,
    pub delta_height: f32,
    pub changed_x: bool,
    pub changed_y: bool,
    pub changed_width: bool,
    pub changed_height: bool,
    pub changed_position: bool,
    pub changed_size: bool,
}

pub(crate) fn mouse_window_info_populate(
    mouse_drag_state: &mut MouseDragState,
    info: &mut MouseWindowInfo,
    window_manager: &mut WindowManager,
) {
    let Some(frame) = mouse_drag_state
        .window_id
        .and_then(|window_id| window_manager.window.find(&window_id))
        .map(|window| window.frame)
    else {
        return;
    };

    info.delta_x = (frame.origin.x - mouse_drag_state.window_frame.origin.x) as f32;
    info.delta_y = (frame.origin.y - mouse_drag_state.window_frame.origin.y) as f32;
    info.delta_width = (frame.size.width - mouse_drag_state.window_frame.size.width) as f32;
    info.delta_height = (frame.size.height - mouse_drag_state.window_frame.size.height) as f32;

    info.changed_x = info.delta_x != 0.0f32;
    info.changed_y = info.delta_y != 0.0f32;
    info.changed_width = info.delta_width != 0.0f32;
    info.changed_height = info.delta_height != 0.0f32;

    info.changed_position = info.changed_x || info.changed_y;
    info.changed_size = info.changed_width || info.changed_height;
}
