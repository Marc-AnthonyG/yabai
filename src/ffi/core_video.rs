#![allow(deprecated)]
#![allow(unused_imports)]

pub use objc2_core_video::{
    CVDisplayLink, CVDisplayLinkCreateWithActiveCGDisplays, CVDisplayLinkOutputCallback,
    CVDisplayLinkSetOutputCallback, CVDisplayLinkStart, CVDisplayLinkStop, CVGetHostClockFrequency,
    CVOptionFlags, CVReturn, CVTimeStamp, kCVReturnSuccess,
};
