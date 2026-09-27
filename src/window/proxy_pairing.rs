use crate::support::handles::WindowId;

#[derive(Clone, Copy)]
pub(crate) struct WindowProxyPairing {
    pub(crate) real_window_id: WindowId,
    pub(crate) proxy_window_id: u32,
}
