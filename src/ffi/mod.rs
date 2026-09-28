pub mod accessibility;
pub mod appkit;
pub mod carbon_core;
pub mod carbon_events;
pub mod carbon_process;
pub mod color_sync;
pub mod core_foundation;
pub mod core_graphics;
pub mod core_text;
pub mod core_video;
pub mod dispatch;
pub mod foundation;
pub mod libsystem;
pub mod mach_port;
pub mod macho;
pub mod skylight;
pub mod skylight_dynamic;

pub type CFStringOwned = core_foundation::CFRetainedAssumedSendAndSync<core_foundation::CFString>;
