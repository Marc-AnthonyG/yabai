#![allow(deprecated)]
#![allow(unused_imports)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

use objc2_core_foundation::{CFType, CGPoint, CGRect};

pub use objc2_core_graphics::{
    CGAffineTransformConcat, CGAffineTransformMakeScale, CGAffineTransformMakeTranslation,
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGBitmapInfo, CGButtonCount, CGColorSpace,
    CGColorSpaceCreateDeviceRGB, CGContext, CGContextAddPath, CGContextClearRect,
    CGContextClipToRect, CGContextDrawImage, CGContextFillRect, CGContextFlush,
    CGContextResetClip, CGContextSetLineWidth, CGContextSetRGBFillColor,
    CGContextSetRGBStrokeColor, CGContextStrokePath, CGDirectDisplayID, CGDisplayBounds,
    CGDisplayChangeSummaryFlags, CGDisplayIsBuiltin, CGDisplayReconfigurationCallBack,
    CGDisplayRegisterReconfigurationCallback, CGEnableEventStateCombining, CGError, CGEvent,
    CGEventCreate, CGEventField, CGEventFlags, CGEventGetFlags, CGEventGetIntegerValueField,
    CGEventGetLocation, CGEventMask, CGEventPost, CGEventSetDoubleValueField,
    CGEventSetIntegerValueField, CGEventSource, CGEventTapCallBack, CGEventTapCreate,
    CGEventTapEnable, CGEventTapIsEnabled, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventTapPostEvent, CGEventTapProxy, CGEventType,
    CGGetActiveDisplayList, CGImage, CGImageAlphaInfo, CGImageGetHeight, CGImageGetWidth,
    CGMainDisplayID, CGPath, CGPathCreateWithRoundedRect, CGPointEqualToPoint,
    CGPreflightScreenCaptureAccess, CGRectContainsPoint, CGRectContainsRect, CGRectEqualToRect,
    CGRectGetHeight, CGRectGetMidX, CGRectGetMidY, CGRectGetWidth, CGRectInset,
    CGRequestScreenCaptureAccess, CGSetLocalEventsSuppressionInterval, CGWarpMouseCursorPosition,
    CGWindowID, CGWindowLevel, CGWindowLevelForKey, CGWindowLevelKey,
    CGWindowListCopyWindowInfo, CGWindowListOption, kCGWindowLayer, kCGWindowName,
    kCGWindowOwnerName,
};

pub const kCGErrorSuccess: CGError = CGError::Success;

pub const kCGDisplayAddFlag: CGDisplayChangeSummaryFlags = CGDisplayChangeSummaryFlags::AddFlag;
pub const kCGDisplayRemoveFlag: CGDisplayChangeSummaryFlags =
    CGDisplayChangeSummaryFlags::RemoveFlag;
pub const kCGDisplayMovedFlag: CGDisplayChangeSummaryFlags = CGDisplayChangeSummaryFlags::MovedFlag;
pub const kCGDisplayDesktopShapeChangedFlag: CGDisplayChangeSummaryFlags =
    CGDisplayChangeSummaryFlags::DesktopShapeChangedFlag;

pub const kCGBackstopMenuLevelKey: CGWindowLevelKey = CGWindowLevelKey::BackstopMenuLevelKey;
pub const kCGNormalWindowLevelKey: CGWindowLevelKey = CGWindowLevelKey::NormalWindowLevelKey;
pub const kCGFloatingWindowLevelKey: CGWindowLevelKey = CGWindowLevelKey::FloatingWindowLevelKey;

pub const kCGWindowListOptionOnScreenOnly: CGWindowListOption =
    CGWindowListOption::OptionOnScreenOnly;

pub const kCGHIDEventTap: CGEventTapLocation = CGEventTapLocation::HIDEventTap;
pub const kCGSessionEventTap: CGEventTapLocation = CGEventTapLocation::SessionEventTap;
pub const kCGHeadInsertEventTap: CGEventTapPlacement = CGEventTapPlacement::HeadInsertEventTap;
pub const kCGEventTapOptionDefault: CGEventTapOptions = CGEventTapOptions::Default;

pub const kCGSEventTypeField: CGEventField = CGEventField(55);
pub const kCGSEventDockControl: i64 = 30;
pub const kCGEventGestureHIDType: CGEventField = CGEventField(110);
pub const kCGEventGestureSwipeMotion: CGEventField = CGEventField(123);
pub const kCGEventGestureSwipeProgress: CGEventField = CGEventField(124);
pub const kCGEventGestureSwipeVelocityX: CGEventField = CGEventField(129);
pub const kCGEventGesturePhase: CGEventField = CGEventField(132);
pub const kIOHIDEventTypeDockSwipe: i64 = 23;
pub const kCGGestureMotionHorizontal: i64 = 1;
pub const kCGSGesturePhaseBegan: i64 = 1;
pub const kCGSGesturePhaseEnded: i64 = 4;
pub const kCGSGesturePhaseCancelled: i64 = 8;

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
