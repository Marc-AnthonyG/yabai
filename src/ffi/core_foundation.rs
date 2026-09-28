#![allow(deprecated)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::OnceLock;

pub use objc2_core_foundation::{
    CFArray, CFBoolean, CFComparisonResult, CFData, CFDictionary, CFIndex, CFMachPort,
    CFMutableData, CFNumber, CFNumberType, CFRange, CFRetained, CFRunLoopSource, CFString, CFType,
    CGAffineTransform, CGFloat, CGPoint, CGRect, CGSize, Type,
};

pub use objc2_core_foundation::{
    CFArrayCreateMutableCopy, CFArrayGetCount, CFArrayGetValueAtIndex, CFArraySortValues,
    CFDataCreateMutable, CFDataGetMutableBytePtr, CFDataIncreaseLength, CFDictionaryGetValue,
    CFEqual, CFMachPortCreateRunLoopSource, CFMachPortInvalidate, CFNumberGetType,
    CFNumberGetValue, CFRunLoopAddSource, CFRunLoopGetMain, CFRunLoopRemoveSource,
    CFRunLoopSourceInvalidate, CFUUIDCreateFromString, CFUUIDCreateString, kCFRunLoopCommonModes,
    kCFRunLoopDefaultMode,
};

use crate::ffi::skylight::SLSWindowSetShadowProperties;

pub struct CFRetainedAssumedSendAndSync<T: ?Sized>(pub CFRetained<T>);

unsafe impl<T: ?Sized> Send for CFRetainedAssumedSendAndSync<T> {}
unsafe impl<T: ?Sized> Sync for CFRetainedAssumedSendAndSync<T> {}

impl<T: ?Sized> CFRetainedAssumedSendAndSync<T> {
    pub fn as_ref(&self) -> &T {
        &self.0
    }
}

macro_rules! define_cached_cfstring_constants {
    ($($accessor_name:ident => $string_value:literal,)*) => {
        $(
            pub fn $accessor_name() -> &'static CFString {
                static CACHED_CFSTRING: OnceLock<CFRetainedAssumedSendAndSync<CFString>> =
                    OnceLock::new();
                CACHED_CFSTRING
                    .get_or_init(|| CFRetainedAssumedSendAndSync(CFString::from_str($string_value)))
                    .as_ref()
            }
        )*
    };
}

define_cached_cfstring_constants! {
    window_shadow_density_option_key                 => "com.apple.WindowShadowDensity",
    display_identifier_key_of_managed_display_spaces => "Display Identifier",
    spaces_key_of_managed_display_spaces             => "Spaces",
    space_id_key_of_managed_space                    => "id64",
    accessibility_fence_attribute_name               => "__fence",
    dock_window_owner_name                           => "Dock",
    window_title_property_key                        => "kCGSWindowTitle",
    show_all_windows_dock_notification_name          => "com.apple.expose.awake",
    show_desktop_dock_notification_name              => "com.apple.showdesktop.awake",
    show_front_windows_dock_notification_name        => "com.apple.expose.front.awake",
}

pub unsafe fn take_create_rule_result<T: ?Sized + Type>(
    pointer: *const T,
) -> Option<CFRetained<T>> {
    NonNull::new(pointer.cast_mut()).map(|non_null| unsafe { CFRetained::from_raw(non_null) })
}

pub fn cfarray_count(array: &CFArray) -> CFIndex {
    CFArrayGetCount(array)
}

pub unsafe fn cfarray_borrow_value_at_index<T>(array: &CFArray, index: CFIndex) -> Option<&T> {
    if index < 0 || index >= CFArrayGetCount(array) {
        return None;
    }
    let pointer: *const c_void = unsafe { CFArrayGetValueAtIndex(array, index) };
    unsafe { pointer.cast::<T>().as_ref() }
}

pub unsafe fn cfdictionary_borrow_value<'dictionary, T>(
    dictionary: &'dictionary CFDictionary,
    key: &CFString,
) -> Option<&'dictionary T> {
    let pointer: *const c_void =
        unsafe { CFDictionaryGetValue(dictionary, (key as *const CFString).cast()) };
    unsafe { pointer.cast::<T>().as_ref() }
}

pub fn cfnumber_read_u64_widening(number: &CFNumber) -> u64 {
    let mut destination: u64 = 0;
    let number_type: CFNumberType = CFNumberGetType(number);
    unsafe {
        CFNumberGetValue(
            number,
            number_type,
            (&mut destination as *mut u64).cast::<c_void>(),
        );
    }
    destination
}

pub fn as_cftype<T>(object: &T) -> &CFType {
    unsafe { &*((object as *const T).cast::<CFType>()) }
}

pub fn disable_window_shadow_through_skylight(id: u32) {
    let density = CFNumber::new_i32(0);
    let options = CFDictionary::from_slices(&[window_shadow_density_option_key()], &[&*density]);
    unsafe { SLSWindowSetShadowProperties(id, options.as_opaque()) };
}

pub fn create_cfarray_of_window_ids(window_ids: &[u32]) -> CFRetained<CFArray> {
    let numbers: Vec<CFRetained<CFNumber>> = window_ids
        .iter()
        .map(|window_id| CFNumber::new_i32(*window_id as i32))
        .collect();
    CFArray::from_retained_objects(&numbers)
        .as_opaque()
        .retain()
}

pub fn create_cfarray_of_space_ids(space_ids: &[u64]) -> CFRetained<CFArray> {
    let numbers: Vec<CFRetained<CFNumber>> = space_ids
        .iter()
        .map(|space_id| CFNumber::new_i64(*space_id as i64))
        .collect();
    CFArray::from_retained_objects(&numbers)
        .as_opaque()
        .retain()
}
