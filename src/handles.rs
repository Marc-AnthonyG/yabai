#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct WindowId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ProcessId(pub i32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SpaceId(pub u64);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DisplayId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct NodeId(pub u32);

pub const ROOT_NODE_ID: NodeId = NodeId(0);
