use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64};

use crate::ffi::core_foundation::CFRetained;
use crate::ffi::core_graphics::{CGContext, CGImage};
use crate::handles::WindowId;
use crate::support::easing::AnimationEasingType;
use crate::support::table::Table;

pub(crate) struct WindowCapture {
    pub(crate) window_id: WindowId,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

pub(crate) struct WindowProxy {
    pub(crate) id: AtomicU32,
    pub(crate) core_graphics_objects: Mutex<WindowProxyCoreGraphicsObjects>,
    pub(crate) target_x: AtomicU32,
    pub(crate) target_y: AtomicU32,
    pub(crate) target_width: AtomicU32,
    pub(crate) target_height: AtomicU32,
    pub(crate) frame_origin_x: AtomicU64,
    pub(crate) frame_origin_y: AtomicU64,
    pub(crate) frame_size_width: AtomicU64,
    pub(crate) frame_size_height: AtomicU64,
    pub(crate) level: AtomicI32,
    pub(crate) sub_level: AtomicI32,
}

pub(crate) struct WindowProxyCoreGraphicsObjects {
    pub(crate) context: Option<CFRetained<CGContext>>,
    pub(crate) image: Option<CFRetained<CGImage>>,
}

pub(crate) struct WindowAnimation {
    pub(crate) window_id: WindowId,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) connection_id: i32,
    pub(crate) proxy: WindowProxy,
    pub(crate) skip: AtomicBool,
}

pub(crate) struct AnimationContext {
    pub(crate) animation_connection: i32,
    pub(crate) animation_easing: AnimationEasingType,
    pub(crate) animation_duration: f32,
    pub(crate) animation_clock: AtomicU64,
    pub(crate) animation_list: Vec<WindowAnimation>,
    pub(crate) animation_count: i32,
    pub(crate) window_animations_table: Arc<Mutex<Table<WindowId, (Arc<AnimationContext>, usize)>>>,
}

unsafe impl Send for AnimationContext {}
unsafe impl Sync for AnimationContext {}
