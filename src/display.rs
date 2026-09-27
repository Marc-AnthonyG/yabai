use core::ffi::c_void;

use crate::display_manager::{
    DOCK_ORIENTATION_BOTTOM, DOCK_ORIENTATION_LEFT, DOCK_ORIENTATION_RIGHT, DisplayManager,
    ExternalBarMode, display_manager_display_id_arrangement, display_manager_dock_display_id,
    display_manager_dock_hidden, display_manager_dock_orientation, display_manager_dock_rect,
    display_manager_get_label_for_display, display_manager_main_display_id,
    display_manager_menu_bar_hidden, display_manager_menu_bar_rect,
};
use crate::event_loop::{Event, event_loop_post};
use crate::ffi::CFStringOwned;
use crate::ffi::color_sync::{CGDisplayCreateUUIDFromDisplayID, CGDisplayGetDisplayIDFromUUID};
use crate::ffi::core_foundation::{
    CFArray, CFDictionary, CFEqual, CFIndex, CFNumber, CFString, CFUUIDCreateFromString,
    CFUUIDCreateString, CGPoint, CGRect, SendCFRetained, as_cftype,
    cfarray_borrow_value_at_index, cfarray_count, cfdictionary_borrow_value,
    cfnumber_read_u64_widening, k_display_identifier, k_id64, k_spaces,
    take_create_rule_result, ts_cfstring_copy,
};
use crate::ffi::core_graphics::{
    CGDirectDisplayID, CGDisplayBounds, CGDisplayChangeSummaryFlags, kCGDisplayAddFlag,
    kCGDisplayDesktopShapeChangedFlag, kCGDisplayMovedFlag, kCGDisplayRemoveFlag,
};
use crate::ffi::skylight::{SLSCopyManagedDisplaySpaces, SLSManagedDisplayGetCurrentSpace};
use crate::globals::CONNECTION;
use crate::handles::{DisplayId, SpaceId};
use crate::misc::helpers::json_bool;
use crate::misc::response::Response;
use crate::space_manager::space_manager_mission_control_index;
use crate::workspace::workspace_display_notch_height;

pub(crate) type DisplayCallback = unsafe extern "C-unwind" fn(
    display_id: CGDirectDisplayID,
    flags: CGDisplayChangeSummaryFlags,
    context: *mut c_void,
);

macro_rules! display_property_list {
    ($display_property_entry:ident) => {
        $display_property_entry! {
            ("id", DISPLAY_PROPERTY_ID, 0x01),
            ("uuid", DISPLAY_PROPERTY_UUID, 0x02),
            ("index", DISPLAY_PROPERTY_INDEX, 0x04),
            ("label", DISPLAY_PROPERTY_LABEL, 0x08),
            ("frame", DISPLAY_PROPERTY_FRAME, 0x10),
            ("spaces", DISPLAY_PROPERTY_SPACES, 0x20),
            ("has-focus", DISPLAY_PROPERTY_HAS_FOCUS, 0x40),
        }
    };
}

macro_rules! define_display_property_list {
    ($(($name:literal, $identifier:ident, $value:literal)),* $(,)?) => {
        $(pub(crate) const $identifier: u64 = $value;)*

        pub(crate) static DISPLAY_PROPERTY_VAL: [u64; 7] = [$($value),*];

        pub(crate) static DISPLAY_PROPERTY_STR: [&str; 7] = [$($name),*];
    };
}

display_property_list!(define_display_property_list);

pub(crate) unsafe extern "C-unwind" fn display_handler(
    display_id: CGDirectDisplayID,
    flags: CGDisplayChangeSummaryFlags,
    context: *mut c_void,
) {
    if flags.contains(kCGDisplayAddFlag) {
        event_loop_post(Event::DisplayAdded(DisplayId(display_id)));
    } else if flags.contains(kCGDisplayRemoveFlag) {
        event_loop_post(Event::DisplayRemoved(DisplayId(display_id)));
    } else if flags.contains(kCGDisplayMovedFlag) {
        event_loop_post(Event::DisplayMoved(DisplayId(display_id)));
    } else if flags.contains(kCGDisplayDesktopShapeChangedFlag) {
        event_loop_post(Event::DisplayResized(DisplayId(display_id)));
    }
}

pub(crate) fn display_serialize(
    response: &mut Response,
    display_id: DisplayId,
    flags: u64,
    display_manager: &mut DisplayManager,
) {
    let mut flags = flags;
    if flags == 0x0 {
        flags |= !flags;
    }

    let mut did_output = false;
    response.write(format_args!("{{\n"));

    if flags & DISPLAY_PROPERTY_ID != 0 {
        response.write(format_args!("\t\"id\":{}", display_id.0 as i32));
        did_output = true;
    }

    if flags & DISPLAY_PROPERTY_UUID != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let mut uuid: Option<String> = None;
        let uuid_ref = display_uuid(display_id);
        if let Some(uuid_ref) = uuid_ref {
            uuid = ts_cfstring_copy(uuid_ref.as_ref());
        }

        response.write(format_args!(
            "\t\"uuid\":\"{}\"",
            uuid.as_deref().unwrap_or("<unknown>")
        ));
        did_output = true;
    }

    if flags & DISPLAY_PROPERTY_INDEX != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"index\":{}",
            display_manager_display_id_arrangement(display_id, display_manager)
        ));
        did_output = true;
    }

    if flags & DISPLAY_PROPERTY_LABEL != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let display_label = display_manager_get_label_for_display(display_manager, display_id);
        response.write(format_args!(
            "\t\"label\":\"{}\"",
            match &display_label {
                Some(display_label) => display_label.label.as_str(),
                None => "",
            }
        ));
        did_output = true;
    }

    if flags & DISPLAY_PROPERTY_FRAME != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let frame = CGDisplayBounds(display_id.0);
        response.write(format_args!(
            "\t\"frame\":{{\n\t\t\"x\":{:.4},\n\t\t\"y\":{:.4},\n\t\t\"w\":{:.4},\n\t\t\"h\":{:.4}\n\t}}",
            frame.origin.x, frame.origin.y, frame.size.width, frame.size.height
        ));
        did_output = true;
    }

    if flags & DISPLAY_PROPERTY_SPACES != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let space_list = display_space_list(display_id);

        response.write(format_args!("\t\"spaces\":["));
        if let Some(space_list) = space_list {
            let count = space_list.len() as i32;
            if let Some(first_space_id) = space_list.first() {
                let first_mission_control_index =
                    space_manager_mission_control_index(*first_space_id);
                for index in 0..count {
                    if index < count - 1 {
                        response.write(format_args!("{}, ", first_mission_control_index + index));
                    } else {
                        response.write(format_args!("{}", first_mission_control_index + index));
                    }
                }
            }
        }
        response.write(format_args!("]"));
        did_output = true;
    }

    if flags & DISPLAY_PROPERTY_HAS_FOCUS != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-focus\":{}",
            json_bool(display_id == display_manager.current_display_id)
        ));
    }

    response.write(format_args!("\n}}"));
}

pub(crate) fn display_uuid(display_id: DisplayId) -> Option<CFStringOwned> {
    let uuid_ref = unsafe { take_create_rule_result(CGDisplayCreateUUIDFromDisplayID(display_id.0)) };
    let Some(uuid_ref) = uuid_ref else {
        return None;
    };

    let uuid_string = CFUUIDCreateString(None, Some(&uuid_ref));

    uuid_string.map(SendCFRetained)
}

pub(crate) fn display_id(uuid: &CFString) -> DisplayId {
    let uuid_ref = CFUUIDCreateFromString(None, Some(uuid));
    let Some(uuid_ref) = uuid_ref else {
        return DisplayId(0);
    };

    let display_id = unsafe { CGDisplayGetDisplayIDFromUUID(&*uuid_ref) };

    DisplayId(display_id)
}

pub(crate) fn display_bounds_constrained(
    display_id: DisplayId,
    ignore_external_bar: bool,
    display_manager: &mut DisplayManager,
) -> CGRect {
    let mut frame = CGDisplayBounds(display_id.0);
    let mut effective_external_top_padding: i32 = 0;

    if !ignore_external_bar {
        if (display_manager.mode == ExternalBarMode::Main
            && display_id == display_manager_main_display_id())
            || (display_manager.mode == ExternalBarMode::All)
        {
            effective_external_top_padding = display_manager.top_padding;

            frame.origin.y += effective_external_top_padding as f64;
            frame.size.height -= effective_external_top_padding as f64;
            frame.size.height -= display_manager.bottom_padding as f64;
        }
    }

    if display_manager_menu_bar_hidden() {
        let notch_height = workspace_display_notch_height(display_id);
        if notch_height > effective_external_top_padding {
            frame.origin.y += (notch_height - effective_external_top_padding) as f64;
            frame.size.height -= (notch_height - effective_external_top_padding) as f64;
        }
    } else {
        let menu = display_manager_menu_bar_rect(display_id);
        frame.origin.y += menu.size.height;
        frame.size.height -= menu.size.height;
    }

    if !display_manager_dock_hidden() {
        if display_id == display_manager_dock_display_id() {
            let dock = display_manager_dock_rect();
            match display_manager_dock_orientation() {
                DOCK_ORIENTATION_LEFT => {
                    frame.origin.x += dock.size.width;
                    frame.size.width -= dock.size.width;
                }
                DOCK_ORIENTATION_RIGHT => {
                    frame.size.width -= dock.size.width;
                }
                DOCK_ORIENTATION_BOTTOM => {
                    frame.size.height -= dock.size.height;
                }
                _ => {}
            }
        }
    }

    frame
}

pub(crate) fn display_center(display_id: DisplayId) -> CGPoint {
    let bounds = CGDisplayBounds(display_id.0);
    CGPoint {
        x: bounds.origin.x + bounds.size.width / 2.0,
        y: bounds.origin.y + bounds.size.height / 2.0,
    }
}

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
