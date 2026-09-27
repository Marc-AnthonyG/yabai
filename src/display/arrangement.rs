#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::NonNull;

use crate::display::bounds::display_center;
use crate::display::identity::{
    display_id, display_manager_active_display_count, display_manager_active_display_list,
    display_uuid,
};
use crate::display::manager::{DisplayArrangementOrder, DisplayManager};
use crate::ffi::CFStringOwned;
use crate::ffi::core_foundation::{
    CFArray, CFArrayCreateMutableCopy, CFArrayGetCount, CFArraySortValues, CFComparisonResult,
    CFEqual, CFIndex, CFRangeMake, CFRetained, CFString, CFType, SendCFRetained, as_cftype,
    cfarray_borrow_value_at_index, take_create_rule_result,
};
use crate::ffi::core_graphics::CGDisplayBounds;
use crate::ffi::skylight::SLSCopyManagedDisplays;
use crate::globals::CONNECTION;
use crate::handles::DisplayId;
use crate::layout::area::{
    area_distance_in_direction, area_from_cgrect, area_is_in_direction, area_max_point,
};
use crate::support::arithmetic::in_range_ie;

pub(crate) unsafe extern "C-unwind" fn display_manager_coordinate_comparator(
    a_display: *const c_void,
    b_display: *const c_void,
    context: *mut c_void,
) -> CFComparisonResult {
    let axis = context as usize as u32;

    let a_display_id = display_id(unsafe { &*a_display.cast::<CFString>() });
    let b_display_id = display_id(unsafe { &*b_display.cast::<CFString>() });

    let a_center = display_center(a_display_id);
    let b_center = display_center(b_display_id);

    let mut a_coordinate: f32 = if axis == DisplayArrangementOrder::Y as u32 {
        a_center.y as f32
    } else {
        a_center.x as f32
    };
    let mut b_coordinate: f32 = if axis == DisplayArrangementOrder::Y as u32 {
        b_center.y as f32
    } else {
        b_center.x as f32
    };

    if a_coordinate < b_coordinate {
        return CFComparisonResult::CompareLessThan;
    }
    if a_coordinate > b_coordinate {
        return CFComparisonResult::CompareGreaterThan;
    }

    a_coordinate = if axis == DisplayArrangementOrder::Y as u32 {
        a_center.x as f32
    } else {
        a_center.y as f32
    };
    b_coordinate = if axis == DisplayArrangementOrder::Y as u32 {
        b_center.x as f32
    } else {
        b_center.y as f32
    };

    if a_coordinate < b_coordinate {
        return CFComparisonResult::CompareLessThan;
    }
    if a_coordinate > b_coordinate {
        return CFComparisonResult::CompareGreaterThan;
    }

    CFComparisonResult::CompareEqualTo
}

pub(crate) fn display_manager_display_id_arrangement(
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
) -> i32 {
    let connection_id = *CONNECTION.get().unwrap();
    let mut result: i32 = 0;

    let Some(uuid) = display_uuid(display_id) else {
        return result;
    };

    let Some(mut displays) =
        (unsafe { take_create_rule_result(SLSCopyManagedDisplays(connection_id)) })
    else {
        return result;
    };

    let count = CFArrayGetCount(&displays) as i32;
    if count == 0 {
        return result;
    }

    if display_manager.order != DisplayArrangementOrder::Default {
        let mutable_displays =
            unsafe { CFArrayCreateMutableCopy(None, count as CFIndex, Some(&displays)) };
        if let Some(mutable_displays) = mutable_displays {
            unsafe {
                CFArraySortValues(
                    Some(&mutable_displays),
                    CFRangeMake(0, count as CFIndex),
                    Some(display_manager_coordinate_comparator),
                    display_manager.order as usize as *mut c_void,
                )
            };
            drop(displays);
            displays = unsafe { CFRetained::cast_unchecked::<CFArray>(mutable_displays) };
        }
    }

    for index in 0..count {
        if CFEqual(
            unsafe { cfarray_borrow_value_at_index::<CFType>(&displays, index as CFIndex) },
            Some(as_cftype(uuid.as_ref())),
        ) {
            result = index + 1;
            break;
        }
    }

    result
}

pub(crate) fn display_manager_arrangement_display_uuid(
    arrangement: i32,
    display_manager: &mut DisplayManager,
) -> Option<CFStringOwned> {
    let connection_id = *CONNECTION.get().unwrap();
    let mut result: Option<CFStringOwned> = None;

    let Some(mut displays) =
        (unsafe { take_create_rule_result(SLSCopyManagedDisplays(connection_id)) })
    else {
        return result;
    };

    let count = CFArrayGetCount(&displays) as i32;
    let index = arrangement - 1;

    if in_range_ie(index, 0, count) {
        if display_manager.order != DisplayArrangementOrder::Default {
            let mutable_displays =
                unsafe { CFArrayCreateMutableCopy(None, count as CFIndex, Some(&displays)) };
            if let Some(mutable_displays) = mutable_displays {
                unsafe {
                    CFArraySortValues(
                        Some(&mutable_displays),
                        CFRangeMake(0, count as CFIndex),
                        Some(display_manager_coordinate_comparator),
                        display_manager.order as usize as *mut c_void,
                    )
                };
                drop(displays);
                displays = unsafe { CFRetained::cast_unchecked::<CFArray>(mutable_displays) };
            }
        }

        result = unsafe { cfarray_borrow_value_at_index::<CFString>(&displays, index as CFIndex) }
            .map(|value| SendCFRetained(unsafe { CFRetained::retain(NonNull::from(value)) }));
    }

    result
}

pub(crate) fn display_manager_arrangement_display_id(
    arrangement: i32,
    display_manager: &mut DisplayManager,
) -> DisplayId {
    let Some(uuid) = display_manager_arrangement_display_uuid(arrangement, display_manager) else {
        return DisplayId(0);
    };

    display_id(uuid.as_ref())
}

pub(crate) fn display_manager_prev_display_id(
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
) -> DisplayId {
    let arrangement = display_manager_display_id_arrangement(display_id, display_manager);
    if arrangement <= 1 {
        return DisplayId(0);
    }

    display_manager_arrangement_display_id(arrangement - 1, display_manager)
}

pub(crate) fn display_manager_next_display_id(
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
) -> DisplayId {
    let arrangement = display_manager_display_id_arrangement(display_id, display_manager);
    if arrangement >= display_manager_active_display_count() {
        return DisplayId(0);
    }

    display_manager_arrangement_display_id(arrangement + 1, display_manager)
}

pub(crate) fn display_manager_first_display_id(display_manager: &mut DisplayManager) -> DisplayId {
    display_manager_arrangement_display_id(1, display_manager)
}

pub(crate) fn display_manager_last_display_id(display_manager: &mut DisplayManager) -> DisplayId {
    let arrangement = display_manager_active_display_count();
    display_manager_arrangement_display_id(arrangement, display_manager)
}

pub(crate) fn display_manager_find_closest_display_in_direction(
    source_display_id: DisplayId,
    direction: i32,
) -> DisplayId {
    let display_list = display_manager_active_display_list();

    let mut best_display_id = DisplayId(0);
    let mut best_distance = i32::MAX;

    let source_area = area_from_cgrect(CGDisplayBounds(source_display_id.0));
    let source_area_max_point = area_max_point(source_area);

    let display_count = display_list.len() as i32;
    for index in 0..display_count {
        let display_id = display_list[index as usize];
        if display_id == source_display_id {
            continue;
        }

        let target_area = area_from_cgrect(CGDisplayBounds(display_id.0));
        let target_area_max_point = area_max_point(target_area);

        if area_is_in_direction(
            &source_area,
            source_area_max_point,
            &target_area,
            target_area_max_point,
            direction,
        ) {
            let distance = area_distance_in_direction(
                &source_area,
                source_area_max_point,
                &target_area,
                target_area_max_point,
                direction,
            );
            if distance < best_distance {
                best_display_id = display_id;
                best_distance = distance;
            }
        }
    }

    best_display_id
}
