use crate::display::identity::copy_uuid_of_display;
use crate::ffi::core_foundation::{
    CFArray, CFDictionary, CFEqual, CFIndex, CFNumber, CFString, as_cftype,
    cfarray_borrow_value_at_index, cfarray_count, cfdictionary_borrow_value,
    cfnumber_read_u64_widening, display_identifier_key_of_managed_display_spaces,
    space_id_key_of_managed_space, spaces_key_of_managed_display_spaces, take_create_rule_result,
};
use crate::ffi::skylight::{
    SLSCopyManagedDisplaySpaces, SLSManagedDisplayGetCurrentSpace, SLSManagedDisplayIsAnimating,
};
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::{DisplayId, SpaceId};
use crate::support::macos_version::{
    is_running_on_macos_big_sur, is_running_on_macos_monterey, is_running_on_macos_ventura,
};

pub(crate) fn query_current_space_of_display(display_id: DisplayId) -> SpaceId {
    let uuid = copy_uuid_of_display(display_id);
    let Some(uuid) = uuid else {
        return SpaceId(0);
    };

    let space_id = unsafe {
        SLSManagedDisplayGetCurrentSpace(*SKYLIGHT_CONNECTION_ID.get().unwrap(), uuid.as_ref())
    };

    SpaceId(space_id)
}

pub(crate) fn query_spaces_of_display(display_id: DisplayId) -> Option<Vec<SpaceId>> {
    let mut space_list: Option<Vec<SpaceId>> = None;

    let uuid = copy_uuid_of_display(display_id);
    let Some(uuid) = uuid else {
        return space_list;
    };

    let display_spaces_ref = unsafe {
        take_create_rule_result(SLSCopyManagedDisplaySpaces(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
        ))
    };
    let Some(display_spaces_ref) = display_spaces_ref else {
        return space_list;
    };

    let display_spaces_count = cfarray_count(&display_spaces_ref) as i32;
    for index in 0..display_spaces_count {
        let display_ref: &CFDictionary =
            match unsafe { cfarray_borrow_value_at_index(&display_spaces_ref, index as CFIndex) } {
                Some(display_ref) => display_ref,
                None => continue,
            };
        let identifier: &CFString = match unsafe {
            cfdictionary_borrow_value(
                display_ref,
                display_identifier_key_of_managed_display_spaces(),
            )
        } {
            Some(identifier) => identifier,
            None => continue,
        };
        if !CFEqual(Some(as_cftype(uuid.as_ref())), Some(as_cftype(identifier))) {
            continue;
        }

        let spaces_ref: &CFArray = match unsafe {
            cfdictionary_borrow_value(display_ref, spaces_key_of_managed_display_spaces())
        } {
            Some(spaces_ref) => spaces_ref,
            None => continue,
        };
        let spaces_count = cfarray_count(spaces_ref) as i32;

        let mut space_list_of_display: Vec<SpaceId> = Vec::with_capacity(spaces_count as usize);

        for inner_index in 0..spaces_count {
            let space_ref: &CFDictionary = match unsafe {
                cfarray_borrow_value_at_index(spaces_ref, inner_index as CFIndex)
            } {
                Some(space_ref) => space_ref,
                None => continue,
            };
            let space_id_ref: &CFNumber = match unsafe {
                cfdictionary_borrow_value(space_ref, space_id_key_of_managed_space())
            } {
                Some(space_id_ref) => space_id_ref,
                None => continue,
            };
            space_list_of_display.push(SpaceId(cfnumber_read_u64_widening(space_id_ref)));
        }

        space_list = Some(space_list_of_display);
    }

    space_list
}

pub(crate) fn is_display_animating_a_space_transition(display_id: DisplayId) -> bool {
    if is_running_on_macos_big_sur()
        || is_running_on_macos_monterey()
        || is_running_on_macos_ventura()
    {
        let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();

        let Some(uuid) = copy_uuid_of_display(display_id) else {
            return false;
        };

        let result = unsafe { SLSManagedDisplayIsAnimating(connection_id, uuid.as_ref()) };

        return result;
    }

    false // This does not return a correct result on modern macOS versions.
}
