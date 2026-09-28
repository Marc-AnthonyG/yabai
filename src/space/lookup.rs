use crate::display::identity::query_display_under_the_cursor;
use crate::display::spaces::{query_current_space_of_display, query_spaces_of_display};
use crate::ffi::core_foundation::{
    CFArray, CFDictionary, CFIndex, CFNumber, cfarray_borrow_value_at_index, cfarray_count,
    cfdictionary_borrow_value, cfnumber_read_u64_widening, space_id_key_of_managed_space,
    spaces_key_of_managed_display_spaces, take_create_rule_result,
};
use crate::ffi::skylight::SLSCopyManagedDisplaySpaces;
use crate::space::managed_space::{is_user_space, query_display_holding_space};
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::{DisplayId, SpaceId};

pub(crate) fn query_mission_control_index_of_space(space_id: SpaceId) -> i32 {
    let mut desktop_count: i32 = 1;

    let Some(display_spaces_ref) = (unsafe {
        take_create_rule_result(SLSCopyManagedDisplaySpaces(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
        ))
    }) else {
        return 0;
    };

    let display_spaces_count = cfarray_count(&display_spaces_ref) as i32;
    'out: {
        for index in 0..display_spaces_count {
            let display_ref: &CFDictionary =
                unsafe { cfarray_borrow_value_at_index(&display_spaces_ref, index as CFIndex) }
                    .unwrap();
            let spaces_ref: &CFArray = unsafe {
                cfdictionary_borrow_value(display_ref, spaces_key_of_managed_display_spaces())
            }
            .unwrap();
            let spaces_count = cfarray_count(spaces_ref) as i32;

            for inner_index in 0..spaces_count {
                let space_ref: &CFDictionary =
                    unsafe { cfarray_borrow_value_at_index(spaces_ref, inner_index as CFIndex) }
                        .unwrap();
                let space_id_ref: &CFNumber = unsafe {
                    cfdictionary_borrow_value(space_ref, space_id_key_of_managed_space())
                }
                .unwrap();
                let result = cfnumber_read_u64_widening(space_id_ref);
                if space_id.0 == result {
                    break 'out;
                }

                desktop_count += 1;
            }
        }

        desktop_count = 0;
    }

    desktop_count
}

pub(crate) fn query_space_at_mission_control_index(desktop_id: i32) -> SpaceId {
    let mut result: u64;
    let mut desktop_count: i32 = 1;

    let Some(display_spaces_ref) = (unsafe {
        take_create_rule_result(SLSCopyManagedDisplaySpaces(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
        ))
    }) else {
        return SpaceId(0);
    };

    let display_spaces_count = cfarray_count(&display_spaces_ref) as i32;
    'out: {
        for index in 0..display_spaces_count {
            let display_ref: &CFDictionary =
                unsafe { cfarray_borrow_value_at_index(&display_spaces_ref, index as CFIndex) }
                    .unwrap();
            let spaces_ref: &CFArray = unsafe {
                cfdictionary_borrow_value(display_ref, spaces_key_of_managed_display_spaces())
            }
            .unwrap();
            let spaces_count = cfarray_count(spaces_ref) as i32;

            for inner_index in 0..spaces_count {
                let space_ref: &CFDictionary =
                    unsafe { cfarray_borrow_value_at_index(spaces_ref, inner_index as CFIndex) }
                        .unwrap();
                let space_id_ref: &CFNumber = unsafe {
                    cfdictionary_borrow_value(space_ref, space_id_key_of_managed_space())
                }
                .unwrap();
                result = cfnumber_read_u64_widening(space_id_ref);
                if desktop_count == desktop_id {
                    break 'out;
                }

                desktop_count += 1;
            }
        }

        result = 0;
    }

    SpaceId(result)
}

pub(crate) fn query_current_space_of_display_under_the_cursor() -> SpaceId {
    let display_id = query_display_under_the_cursor();
    query_current_space_of_display(display_id)
}

pub(crate) fn query_previous_space_in_mission_control_order(space_id: SpaceId) -> SpaceId {
    let mut previous_space_id: u64 = 0;
    let mut next_space_id: u64;

    let Some(display_spaces_ref) = (unsafe {
        take_create_rule_result(SLSCopyManagedDisplaySpaces(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
        ))
    }) else {
        return SpaceId(0);
    };

    let display_spaces_count = cfarray_count(&display_spaces_ref) as i32;
    'out: {
        for index in 0..display_spaces_count {
            let display_ref: &CFDictionary =
                unsafe { cfarray_borrow_value_at_index(&display_spaces_ref, index as CFIndex) }
                    .unwrap();
            let spaces_ref: &CFArray = unsafe {
                cfdictionary_borrow_value(display_ref, spaces_key_of_managed_display_spaces())
            }
            .unwrap();
            let spaces_count = cfarray_count(spaces_ref) as i32;

            for inner_index in 0..spaces_count {
                let space_ref: &CFDictionary =
                    unsafe { cfarray_borrow_value_at_index(spaces_ref, inner_index as CFIndex) }
                        .unwrap();
                let space_id_ref: &CFNumber = unsafe {
                    cfdictionary_borrow_value(space_ref, space_id_key_of_managed_space())
                }
                .unwrap();
                next_space_id = cfnumber_read_u64_widening(space_id_ref);
                if next_space_id == space_id.0 {
                    break 'out;
                }

                previous_space_id = next_space_id;
            }
        }
    }

    if previous_space_id != space_id.0 {
        SpaceId(previous_space_id)
    } else {
        SpaceId(0)
    }
}

pub(crate) fn query_next_space_in_mission_control_order(space_id: SpaceId) -> SpaceId {
    let mut next_space_id: u64 = 0;
    let mut found_space_id = false;

    let Some(display_spaces_ref) = (unsafe {
        take_create_rule_result(SLSCopyManagedDisplaySpaces(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
        ))
    }) else {
        return SpaceId(0);
    };

    let display_spaces_count = cfarray_count(&display_spaces_ref) as i32;
    'out: {
        for index in 0..display_spaces_count {
            let display_ref: &CFDictionary =
                unsafe { cfarray_borrow_value_at_index(&display_spaces_ref, index as CFIndex) }
                    .unwrap();
            let spaces_ref: &CFArray = unsafe {
                cfdictionary_borrow_value(display_ref, spaces_key_of_managed_display_spaces())
            }
            .unwrap();
            let spaces_count = cfarray_count(spaces_ref) as i32;

            for inner_index in 0..spaces_count {
                let space_ref: &CFDictionary =
                    unsafe { cfarray_borrow_value_at_index(spaces_ref, inner_index as CFIndex) }
                        .unwrap();
                let space_id_ref: &CFNumber = unsafe {
                    cfdictionary_borrow_value(space_ref, space_id_key_of_managed_space())
                }
                .unwrap();
                next_space_id = cfnumber_read_u64_widening(space_id_ref);
                if found_space_id {
                    break 'out;
                }

                found_space_id = next_space_id == space_id.0;
            }
        }
    }

    if next_space_id != space_id.0 {
        SpaceId(next_space_id)
    } else {
        SpaceId(0)
    }
}

pub(crate) fn query_first_space_in_mission_control_order() -> SpaceId {
    let Some(display_spaces_ref) = (unsafe {
        take_create_rule_result(SLSCopyManagedDisplaySpaces(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
        ))
    }) else {
        return SpaceId(0);
    };

    if cfarray_count(&display_spaces_ref) <= 0 {
        return SpaceId(0);
    }
    let display_ref: &CFDictionary =
        unsafe { cfarray_borrow_value_at_index(&display_spaces_ref, 0) }.unwrap();
    let spaces_ref: &CFArray =
        unsafe { cfdictionary_borrow_value(display_ref, spaces_key_of_managed_display_spaces()) }
            .unwrap();

    if cfarray_count(spaces_ref) <= 0 {
        return SpaceId(0);
    }
    let space_ref: &CFDictionary = unsafe { cfarray_borrow_value_at_index(spaces_ref, 0) }.unwrap();
    let space_id_ref: &CFNumber =
        unsafe { cfdictionary_borrow_value(space_ref, space_id_key_of_managed_space()) }.unwrap();
    let space_id = cfnumber_read_u64_widening(space_id_ref);

    SpaceId(space_id)
}

pub(crate) fn query_last_space_in_mission_control_order() -> SpaceId {
    let Some(display_spaces_ref) = (unsafe {
        take_create_rule_result(SLSCopyManagedDisplaySpaces(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
        ))
    }) else {
        return SpaceId(0);
    };

    let display_spaces_count = cfarray_count(&display_spaces_ref) as i32;
    if display_spaces_count <= 0 {
        return SpaceId(0);
    }
    let display_ref: &CFDictionary = unsafe {
        cfarray_borrow_value_at_index(&display_spaces_ref, (display_spaces_count - 1) as CFIndex)
    }
    .unwrap();
    let spaces_ref: &CFArray =
        unsafe { cfdictionary_borrow_value(display_ref, spaces_key_of_managed_display_spaces()) }
            .unwrap();
    let spaces_count = cfarray_count(spaces_ref) as i32;

    if spaces_count <= 0 {
        return SpaceId(0);
    }
    let space_ref: &CFDictionary =
        unsafe { cfarray_borrow_value_at_index(spaces_ref, (spaces_count - 1) as CFIndex) }
            .unwrap();
    let space_id_ref: &CFNumber =
        unsafe { cfdictionary_borrow_value(space_ref, space_id_key_of_managed_space()) }.unwrap();
    let space_id = cfnumber_read_u64_widening(space_id_ref);

    SpaceId(space_id)
}

pub(crate) fn query_first_user_space_of_display(display_id: DisplayId) -> SpaceId {
    let Some(space_list) = query_spaces_of_display(display_id) else {
        return SpaceId(0);
    };
    let count = space_list.len() as i32;

    for index in 0..count {
        let space_id = space_list[index as usize];

        if is_user_space(space_id) {
            return space_id;
        }
    }

    SpaceId(0)
}

pub(crate) fn is_space_the_last_user_space_of_its_display(space_id: SpaceId) -> bool {
    let mut result = true;

    let Some(space_list) = query_spaces_of_display(query_display_holding_space(space_id)) else {
        return true;
    };
    let count = space_list.len() as i32;

    for index in 0..count {
        let current_space_id = space_list[index as usize];
        if space_id == current_space_id {
            continue;
        }

        if is_user_space(current_space_id) {
            result = false;
            break;
        }
    }

    result
}
