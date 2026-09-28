use serde::Serialize;

use crate::ffi::core_foundation::CGRect;

#[derive(Serialize, Clone, Copy, PartialEq, Debug, Default)]
pub(crate) struct FrameSnapshot {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) w: f64,
    pub(crate) h: f64,
}

pub(crate) fn snapshot_of_frame(frame: CGRect) -> FrameSnapshot {
    FrameSnapshot {
        x: frame.origin.x,
        y: frame.origin.y,
        w: frame.size.width,
        h: frame.size.height,
    }
}
