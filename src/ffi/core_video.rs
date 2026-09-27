#![allow(deprecated)]

pub use objc2_core_video::{
    CVDisplayLink, CVDisplayLinkCreateWithActiveCGDisplays, CVDisplayLinkSetOutputCallback,
    CVDisplayLinkStart, CVDisplayLinkStop, CVGetHostClockFrequency, CVOptionFlags, CVReturn,
    CVTimeStamp, kCVReturnDisplayLinkAlreadyRunning, kCVReturnSuccess,
};
