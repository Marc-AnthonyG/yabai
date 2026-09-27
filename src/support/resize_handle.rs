#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResizeHandle(pub u8);

impl ResizeHandle {
    pub(crate) const TOP: ResizeHandle = ResizeHandle(0x01);
    pub(crate) const BOTTOM: ResizeHandle = ResizeHandle(0x02);
    pub(crate) const LEFT: ResizeHandle = ResizeHandle(0x04);
    pub(crate) const RIGHT: ResizeHandle = ResizeHandle(0x08);
    pub(crate) const ABS: ResizeHandle = ResizeHandle(0x10);
}
