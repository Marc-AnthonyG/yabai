#![allow(deprecated)]

use core::ffi::c_void;
use core::ptr::NonNull;

use crate::display::bounds::query_center_of_display;
use crate::display::identity::{
    copy_uuid_of_display, query_count_of_displays_active_for_drawing, query_display_with_uuid,
    query_displays_active_for_drawing,
};
use crate::display::manager::{DisplayArrangementOrder, DisplayManager};
use crate::ffi::CFStringOwned;
use crate::ffi::core_foundation::{
    CFArray, CFArrayCreateMutableCopy, CFArrayGetCount, CFArraySortValues, CFComparisonResult,
    CFEqual, CFIndex, CFRange, CFRetained, CFRetainedAssumedSendAndSync, CFString, CFType,
    as_cftype, cfarray_borrow_value_at_index, take_create_rule_result,
};
use crate::ffi::core_graphics::CGDisplayBounds;
use crate::ffi::skylight::SLSCopyManagedDisplays;
use crate::layout::area::{
    area_from_cgrect, bottom_right_pixel_inside_area,
    distance_from_source_area_to_target_area_in_direction,
    is_target_area_in_direction_of_source_area_and_facing_it,
};
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::arithmetic::is_within_range_including_low_excluding_high;
use crate::support::handles::DisplayId;

pub(crate) unsafe extern "C-unwind" fn compare_display_uuids_by_center_along_arrangement_axis(
    a_display: *const c_void,
    b_display: *const c_void,
    context: *mut c_void,
) -> CFComparisonResult {
    let axis = context as usize as u32;

    let a_display_id = query_display_with_uuid(unsafe { &*a_display.cast::<CFString>() });
    let b_display_id = query_display_with_uuid(unsafe { &*b_display.cast::<CFString>() });

    let a_center = query_center_of_display(a_display_id);
    let b_center = query_center_of_display(b_display_id);

    let mut a_coordinate: f32 = if axis == DisplayArrangementOrder::Vertical as u32 {
        a_center.y as f32
    } else {
        a_center.x as f32
    };
    let mut b_coordinate: f32 = if axis == DisplayArrangementOrder::Vertical as u32 {
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

    a_coordinate = if axis == DisplayArrangementOrder::Vertical as u32 {
        a_center.x as f32
    } else {
        a_center.y as f32
    };
    b_coordinate = if axis == DisplayArrangementOrder::Vertical as u32 {
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

pub(crate) fn query_arrangement_index_of_display(
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
) -> i32 {
    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();
    let mut result: i32 = 0;

    let Some(uuid) = copy_uuid_of_display(display_id) else {
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
                    CFRange::new(0, count as CFIndex),
                    Some(compare_display_uuids_by_center_along_arrangement_axis),
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

pub(crate) fn copy_uuid_of_display_at_arrangement_index(
    arrangement: i32,
    display_manager: &mut DisplayManager,
) -> Option<CFStringOwned> {
    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();
    let mut result: Option<CFStringOwned> = None;

    let Some(mut displays) =
        (unsafe { take_create_rule_result(SLSCopyManagedDisplays(connection_id)) })
    else {
        return result;
    };

    let count = CFArrayGetCount(&displays) as i32;
    let index = arrangement - 1;

    if is_within_range_including_low_excluding_high(index, 0, count) {
        if display_manager.order != DisplayArrangementOrder::Default {
            let mutable_displays =
                unsafe { CFArrayCreateMutableCopy(None, count as CFIndex, Some(&displays)) };
            if let Some(mutable_displays) = mutable_displays {
                unsafe {
                    CFArraySortValues(
                        Some(&mutable_displays),
                        CFRange::new(0, count as CFIndex),
                        Some(compare_display_uuids_by_center_along_arrangement_axis),
                        display_manager.order as usize as *mut c_void,
                    )
                };
                drop(displays);
                displays = unsafe { CFRetained::cast_unchecked::<CFArray>(mutable_displays) };
            }
        }

        result = unsafe { cfarray_borrow_value_at_index::<CFString>(&displays, index as CFIndex) }
            .map(|value| {
                CFRetainedAssumedSendAndSync(unsafe { CFRetained::retain(NonNull::from(value)) })
            });
    }

    result
}

pub(crate) fn query_display_at_arrangement_index(
    arrangement: i32,
    display_manager: &mut DisplayManager,
) -> DisplayId {
    let Some(uuid) = copy_uuid_of_display_at_arrangement_index(arrangement, display_manager) else {
        return DisplayId(0);
    };

    query_display_with_uuid(uuid.as_ref())
}

pub(crate) fn query_previous_display_in_arrangement(
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
) -> DisplayId {
    let arrangement = query_arrangement_index_of_display(display_id, display_manager);
    if arrangement <= 1 {
        return DisplayId(0);
    }

    query_display_at_arrangement_index(arrangement - 1, display_manager)
}

pub(crate) fn query_next_display_in_arrangement(
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
) -> DisplayId {
    let arrangement = query_arrangement_index_of_display(display_id, display_manager);
    if arrangement >= query_count_of_displays_active_for_drawing() {
        return DisplayId(0);
    }

    query_display_at_arrangement_index(arrangement + 1, display_manager)
}

pub(crate) fn query_first_display_in_arrangement(
    display_manager: &mut DisplayManager,
) -> DisplayId {
    query_display_at_arrangement_index(1, display_manager)
}

pub(crate) fn query_last_display_in_arrangement(display_manager: &mut DisplayManager) -> DisplayId {
    let arrangement = query_count_of_displays_active_for_drawing();
    query_display_at_arrangement_index(arrangement, display_manager)
}

pub(crate) fn closest_display_in_direction_of_display(
    source_display_id: DisplayId,
    direction: i32,
) -> DisplayId {
    let display_list = query_displays_active_for_drawing();

    let mut best_display_id = DisplayId(0);
    let mut best_distance = i32::MAX;

    let source_area = area_from_cgrect(CGDisplayBounds(source_display_id.0));
    let source_area_max_point = bottom_right_pixel_inside_area(source_area);

    let display_count = display_list.len() as i32;
    for index in 0..display_count {
        let display_id = display_list[index as usize];
        if display_id == source_display_id {
            continue;
        }

        let target_area = area_from_cgrect(CGDisplayBounds(display_id.0));
        let target_area_max_point = bottom_right_pixel_inside_area(target_area);

        if is_target_area_in_direction_of_source_area_and_facing_it(
            &source_area,
            source_area_max_point,
            &target_area,
            target_area_max_point,
            direction,
        ) {
            let distance = distance_from_source_area_to_target_area_in_direction(
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
