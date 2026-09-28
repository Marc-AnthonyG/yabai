#![allow(deprecated)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

use objc2_core_foundation::{CFType, CGPoint, CGRect};

pub use objc2_core_graphics::{
    CGAffineTransformConcat, CGAffineTransformIdentity, CGAffineTransformMakeScale,
    CGAffineTransformMakeTranslation, CGBitmapContextCreate, CGBitmapContextCreateImage,
    CGBitmapInfo, CGButtonCount, CGColorSpaceCreateDeviceRGB, CGContext, CGContextAddPath,
    CGContextClearRect, CGContextDrawImage, CGContextDrawPath, CGContextFlush,
    CGContextSetLineWidth, CGContextSetRGBFillColor, CGContextSetRGBStrokeColor,
    CGContextSetTextMatrix, CGContextSetTextPosition, CGDirectDisplayID, CGDisplayBounds,
    CGDisplayChangeSummaryFlags, CGDisplayIsBuiltin, CGDisplayRegisterReconfigurationCallback,
    CGEnableEventStateCombining, CGError, CGEvent, CGEventCreate, CGEventField, CGEventFlags,
    CGEventGetFlags, CGEventGetIntegerValueField, CGEventGetLocation, CGEventMask, CGEventPost,
    CGEventSetDoubleValueField, CGEventSetIntegerValueField, CGEventTapCreate, CGEventTapEnable,
    CGEventTapIsEnabled, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventTapPostEvent, CGEventTapProxy, CGEventType, CGGetActiveDisplayList, CGImage,
    CGImageAlphaInfo, CGImageGetHeight, CGImageGetWidth, CGMainDisplayID,
    CGPathCreateWithRoundedRect, CGPathDrawingMode, CGPointEqualToPoint,
    CGPreflightScreenCaptureAccess, CGRectContainsPoint, CGRectContainsRect, CGRectEqualToRect,
    CGRectGetMidX, CGRectGetMidY, CGRequestScreenCaptureAccess,
    CGSetLocalEventsSuppressionInterval, CGWarpMouseCursorPosition, CGWindowLevelForKey,
    CGWindowLevelKey, CGWindowListCopyWindowInfo, CGWindowListOption, kCGWindowLayer,
    kCGWindowName, kCGWindowOwnerName,
};

pub const kCGErrorSuccess: CGError = CGError::Success;

pub const kCGDisplayAddFlag: CGDisplayChangeSummaryFlags = CGDisplayChangeSummaryFlags::AddFlag;
pub const kCGDisplayRemoveFlag: CGDisplayChangeSummaryFlags =
    CGDisplayChangeSummaryFlags::RemoveFlag;
pub const kCGDisplayMovedFlag: CGDisplayChangeSummaryFlags = CGDisplayChangeSummaryFlags::MovedFlag;
pub const kCGDisplayDesktopShapeChangedFlag: CGDisplayChangeSummaryFlags =
    CGDisplayChangeSummaryFlags::DesktopShapeChangedFlag;

pub const kCGWindowListOptionOnScreenOnly: CGWindowListOption =
    CGWindowListOption::OptionOnScreenOnly;

pub const kCGHIDEventTap: CGEventTapLocation = CGEventTapLocation::HIDEventTap;
pub const kCGSessionEventTap: CGEventTapLocation = CGEventTapLocation::SessionEventTap;
pub const kCGHeadInsertEventTap: CGEventTapPlacement = CGEventTapPlacement::HeadInsertEventTap;
pub const kCGEventTapOptionDefault: CGEventTapOptions = CGEventTapOptions::Default;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    pub fn CGPostMouseEvent(
        mouse_cursor_position: CGPoint,
        update_mouse_cursor_position: libc::boolean_t,
        button_count: CGButtonCount,
        mouse_button_down: libc::boolean_t,
        ...
    ) -> CGError;
    pub fn CGRegionCreateEmptyRegion() -> *mut CFType;
    pub fn CGSNewRegionWithRect(rect: *mut CGRect, region: *mut *mut CFType) -> CGError;
}
