use crate::display::identity::display_uuid;
use crate::ffi::core_foundation::{
    CFArray, CFDictionary, CFEqual, CFIndex, CFNumber, CFString, as_cftype,
    cfarray_borrow_value_at_index, cfarray_count, cfdictionary_borrow_value,
    cfnumber_read_u64_widening, k_display_identifier, k_id64, k_spaces, take_create_rule_result,
};
use crate::ffi::skylight::{
    SLSCopyManagedDisplaySpaces, SLSManagedDisplayGetCurrentSpace, SLSManagedDisplayIsAnimating,
};
use crate::globals::CONNECTION;
use crate::handles::{DisplayId, SpaceId};
use crate::workspace::{
    workspace_is_macos_bigsur, workspace_is_macos_monterey, workspace_is_macos_ventura,
};

pub(crate) fn display_space_id(display_id: DisplayId) -> SpaceId {
    let uuid = display_uuid(display_id);
    let Some(uuid) = uuid else {
        return SpaceId(0);
    };

    let space_id = unsafe {
        SLSManagedDisplayGetCurrentSpace(*CONNECTION.get().unwrap(), uuid.as_ref())
    };

    SpaceId(space_id)
}

pub(crate) fn display_space_list(display_id: DisplayId) -> Option<Vec<SpaceId>> {
    let mut space_list: Option<Vec<SpaceId>> = None;

    let uuid = display_uuid(display_id);
    let Some(uuid) = uuid else {
        return space_list;
    };

    let display_spaces_ref = unsafe {
        take_create_rule_result(SLSCopyManagedDisplaySpaces(*CONNECTION.get().unwrap()))
    };
    let Some(display_spaces_ref) = display_spaces_ref else {
        return space_list;
    };

    let display_spaces_count = cfarray_count(&display_spaces_ref) as i32;
    for index in 0..display_spaces_count {
        let display_ref: &CFDictionary = match unsafe {
            cfarray_borrow_value_at_index(&display_spaces_ref, index as CFIndex)
        } {
            Some(display_ref) => display_ref,
            None => continue,
        };
        let identifier: &CFString =
            match unsafe { cfdictionary_borrow_value(display_ref, k_display_identifier()) } {
                Some(identifier) => identifier,
                None => continue,
            };
        if !CFEqual(Some(as_cftype(uuid.as_ref())), Some(as_cftype(identifier))) {
            continue;
        }

        let spaces_ref: &CFArray =
            match unsafe { cfdictionary_borrow_value(display_ref, k_spaces()) } {
                Some(spaces_ref) => spaces_ref,
                None => continue,
            };
        let spaces_count = cfarray_count(spaces_ref) as i32;

        let mut space_list_of_display: Vec<SpaceId> = Vec::with_capacity(spaces_count as usize);

        for inner_index in 0..spaces_count {
            let space_ref: &CFDictionary =
                match unsafe { cfarray_borrow_value_at_index(spaces_ref, inner_index as CFIndex) } {
                    Some(space_ref) => space_ref,
                    None => continue,
                };
            let space_id_ref: &CFNumber =
                match unsafe { cfdictionary_borrow_value(space_ref, k_id64()) } {
                    Some(space_id_ref) => space_id_ref,
                    None => continue,
                };
            space_list_of_display.push(SpaceId(cfnumber_read_u64_widening(space_id_ref)));
        }

        space_list = Some(space_list_of_display);
    }

    space_list
}

pub(crate) fn display_manager_display_is_animating(display_id: DisplayId) -> bool {
    if workspace_is_macos_bigsur()
        || workspace_is_macos_monterey()
        || workspace_is_macos_ventura()
    {
        let connection_id = *CONNECTION.get().unwrap();

        let Some(uuid) = display_uuid(display_id) else {
            return false;
        };

        let result = unsafe { SLSManagedDisplayIsAnimating(connection_id, uuid.as_ref()) };

        return result;
    }

    false // This does not return a correct result on modern macOS versions.
}
