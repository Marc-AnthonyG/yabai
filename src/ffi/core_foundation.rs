#![allow(deprecated)]
#![allow(unused_imports)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

use core::ffi::{c_char, c_void};
use core::ptr::NonNull;
use std::sync::OnceLock;

pub use objc2_core_foundation::{
    CFAllocator, CFArray, CFArrayCallBacks, CFBoolean, CFComparatorFunction, CFComparisonResult,
    CFData, CFDictionary, CFDictionaryKeyCallBacks, CFDictionaryValueCallBacks, CFIndex, CFMachPort,
    CFMutableArray, CFMutableData, CFNumber, CFNumberType, CFRange, CFRetained, CFRunLoop,
    CFRunLoopMode, CFRunLoopSource, CFString, CFStringEncoding, CFTimeInterval, CFType, CFUUID,
    CGAffineTransform, CGFloat, CGPoint, CGRect, CGSize, Type,
};

pub use objc2_core_foundation::{
    CFArrayCreate, CFArrayCreateMutableCopy, CFArrayGetCount, CFArrayGetValueAtIndex,
    CFArraySortValues, CFBooleanGetValue, CFDataCreateMutable, CFDataGetMutableBytePtr,
    CFDataIncreaseLength, CFDictionaryCreate, CFDictionaryGetValue, CFEqual,
    CFMachPortCreateRunLoopSource, CFMachPortInvalidate, CFNumberCreate, CFNumberGetType,
    CFNumberGetValue, CFRunLoopAddSource, CFRunLoopGetMain, CFRunLoopRemoveSource,
    CFRunLoopSourceInvalidate, CFStringCreateWithCString, CFStringGetCString, CFStringGetLength,
    CFStringGetMaximumSizeForEncoding, CFUUIDCreateFromString, CFUUIDCreateString,
    kCFAllocatorDefault, kCFCopyStringDictionaryKeyCallBacks, kCFRunLoopCommonModes,
    kCFRunLoopDefaultMode, kCFTypeArrayCallBacks, kCFTypeDictionaryKeyCallBacks,
    kCFTypeDictionaryValueCallBacks,
};

use crate::ffi::skylight::SLSWindowSetShadowProperties;

pub const kCFNumberSInt32Type: CFNumberType = CFNumberType::SInt32Type;
pub const kCFNumberSInt64Type: CFNumberType = CFNumberType::SInt64Type;

pub const K_CF_STRING_ENCODING_UTF8: CFStringEncoding = 0x0800_0100;
pub const K_CF_STRING_ENCODING_MAC_ROMAN: CFStringEncoding = 0;

pub struct SendCFRetained<T: ?Sized>(pub CFRetained<T>);

unsafe impl<T: ?Sized> Send for SendCFRetained<T> {}
unsafe impl<T: ?Sized> Sync for SendCFRetained<T> {}

impl<T: ?Sized> SendCFRetained<T> {
    pub fn as_ref(&self) -> &T {
        &self.0
    }
}

macro_rules! cfstring_constants {
    ($($accessor_name:ident => $string_value:literal,)*) => {
        $(
            pub fn $accessor_name() -> &'static CFString {
                static CACHED: OnceLock<SendCFRetained<CFString>> = OnceLock::new();
                CACHED
                    .get_or_init(|| SendCFRetained(CFString::from_str($string_value)))
                    .as_ref()
            }
        )*
    };
}

cfstring_constants! {
    k_com_apple_window_shadow_density => "com.apple.WindowShadowDensity",
    k_display_identifier              => "Display Identifier",
    k_spaces                          => "Spaces",
    k_id64                            => "id64",
    k_fence                           => "__fence",
    k_dock                            => "Dock",
    k_cgs_window_title                => "kCGSWindowTitle",
    k_com_apple_expose_awake          => "com.apple.expose.awake",
    k_com_apple_showdesktop_awake     => "com.apple.showdesktop.awake",
    k_com_apple_expose_front_awake    => "com.apple.expose.front.awake",
}

pub fn kCFBooleanTrue() -> &'static CFBoolean {
    unsafe { objc2_core_foundation::kCFBooleanTrue }
        .expect("kCFBooleanTrue is a CoreFoundation constant")
}

pub fn kCFBooleanFalse() -> &'static CFBoolean {
    unsafe { objc2_core_foundation::kCFBooleanFalse }
        .expect("kCFBooleanFalse is a CoreFoundation constant")
}

pub fn CFRangeMake(location: CFIndex, length: CFIndex) -> CFRange {
    CFRange::new(location, length)
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

pub fn cfnumber_read_i32(number: &CFNumber) -> i32 {
    let mut destination: i32 = 0;
    unsafe {
        CFNumberGetValue(
            number,
            kCFNumberSInt32Type,
            (&mut destination as *mut i32).cast::<c_void>(),
        );
    }
    destination
}

pub fn cfboolean_get_value(boolean: &CFBoolean) -> bool {
    CFBooleanGetValue(boolean)
}

pub fn cfstring_to_string(string: &CFString) -> Option<String> {
    let maximum_size_in_bytes =
        CFStringGetMaximumSizeForEncoding(CFStringGetLength(string), K_CF_STRING_ENCODING_UTF8);
    let mut buffer: Vec<u8> = vec![0; (maximum_size_in_bytes + 1).max(0) as usize];
    let converted = unsafe {
        CFStringGetCString(
            string,
            buffer.as_mut_ptr().cast::<c_char>(),
            maximum_size_in_bytes + 1,
            K_CF_STRING_ENCODING_UTF8,
        )
    };

    if !converted {
        return None;
    }

    let nul_position = buffer
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(buffer.len());
    buffer.truncate(nul_position);
    Some(String::from_utf8_lossy(&buffer).into_owned())
}

pub fn cfstring_from_str(text: &str) -> CFRetained<CFString> {
    CFString::from_str(text)
}

pub fn as_cftype<T>(object: &T) -> &CFType {
    unsafe { &*((object as *const T).cast::<CFType>()) }
}

pub fn ts_cfstring_copy(string: &CFString) -> Option<String> {
    cfstring_to_string(string)
}

pub fn cfstring_copy(string: &CFString) -> Option<String> {
    cfstring_to_string(string)
}

pub fn CFNUM32(number: i32) -> CFRetained<CFNumber> {
    unsafe {
        CFNumberCreate(
            None,
            kCFNumberSInt32Type,
            (&number as *const i32).cast::<c_void>(),
        )
    }
    .unwrap()
}

pub fn sls_window_disable_shadow(id: u32) {
    let density = CFNUM32(0);
    let mut keys: [*const c_void; 1] = [(k_com_apple_window_shadow_density() as *const CFString)
        .cast::<c_void>()];
    let mut values: [*const c_void; 1] = [(&*density as *const CFNumber).cast::<c_void>()];
    let options = unsafe {
        CFDictionaryCreate(
            None,
            keys.as_mut_ptr(),
            values.as_mut_ptr(),
            1,
            &raw const kCFTypeDictionaryKeyCallBacks,
            &raw const kCFTypeDictionaryValueCallBacks,
        )
    }
    .unwrap();
    unsafe { SLSWindowSetShadowProperties(id, &*options) };
}

pub fn cfarray_of_cfnumbers<T: Copy>(
    values: &[T],
    number_type: CFNumberType,
) -> CFRetained<CFArray> {
    let numbers: Vec<CFRetained<CFNumber>> = values
        .iter()
        .map(|value| {
            unsafe {
                CFNumberCreate(None, number_type, (value as *const T).cast::<c_void>())
            }
            .unwrap()
        })
        .collect();
    let mut pointers: Vec<*const c_void> = numbers
        .iter()
        .map(|number| (&**number as *const CFNumber).cast::<c_void>())
        .collect();
    unsafe {
        CFArrayCreate(
            None,
            pointers.as_mut_ptr(),
            pointers.len() as CFIndex,
            &raw const kCFTypeArrayCallBacks,
        )
    }
    .unwrap()
}
