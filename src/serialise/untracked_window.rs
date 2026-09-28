use core::ffi::c_void;
use std::sync::Mutex;

use crate::display::arrangement::query_arrangement_index_of_display;
use crate::display::manager::DisplayManager;
use crate::ffi::core_foundation::CGRect;
use crate::ffi::skylight::{SLSConnectionGetPID, SLSGetWindowBounds, SLSGetWindowOwner};
use crate::layout::tree::{WINDOW_NODE_CHILD_NAMES, WINDOW_NODE_SPLIT_NAMES, WindowNodeChild};
use crate::serialise::window::{
    WINDOW_PROPERTY_APP, WINDOW_PROPERTY_CAN_MOVE, WINDOW_PROPERTY_CAN_RESIZE,
    WINDOW_PROPERTY_DISPLAY, WINDOW_PROPERTY_FRAME, WINDOW_PROPERTY_HAS_AX_REFERENCE,
    WINDOW_PROPERTY_HAS_FOCUS, WINDOW_PROPERTY_HAS_FULLSCREEN_ZOOM,
    WINDOW_PROPERTY_HAS_PARENT_ZOOM, WINDOW_PROPERTY_HAS_SHADOW, WINDOW_PROPERTY_ID,
    WINDOW_PROPERTY_IS_FLOATING, WINDOW_PROPERTY_IS_FULLSCREEN, WINDOW_PROPERTY_IS_GRABBED,
    WINDOW_PROPERTY_IS_GROUPED, WINDOW_PROPERTY_IS_HIDDEN, WINDOW_PROPERTY_IS_MINIMIZED,
    WINDOW_PROPERTY_IS_STICKY, WINDOW_PROPERTY_IS_VISIBLE, WINDOW_PROPERTY_LAYER,
    WINDOW_PROPERTY_LEVEL, WINDOW_PROPERTY_OPACITY, WINDOW_PROPERTY_PID, WINDOW_PROPERTY_ROLE,
    WINDOW_PROPERTY_ROOT_WINDOW, WINDOW_PROPERTY_SCRATCHPAD, WINDOW_PROPERTY_SPACE,
    WINDOW_PROPERTY_SPLIT_CHILD, WINDOW_PROPERTY_SPLIT_TYPE, WINDOW_PROPERTY_STACK_INDEX,
    WINDOW_PROPERTY_SUB_LAYER, WINDOW_PROPERTY_SUB_LEVEL, WINDOW_PROPERTY_SUBROLE,
    WINDOW_PROPERTY_TITLE, layer_name_of_window_level,
};
use crate::space::lookup::query_mission_control_index_of_space;
use crate::space::managed_space::{is_native_fullscreen_space, query_display_holding_space};
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::{SpaceId, WindowId};
use crate::support::json::{
    escape_string_for_json_when_it_needs_escaping, json_literal_for_boolean,
};
use crate::support::printf_float_format::format_float_with_decimals_as_printf_does;
use crate::support::response::Response;
use crate::window::model::{
    is_window_on_more_than_one_space, is_window_shadow_shown_according_to_window_server,
    query_parent_window_from_window_server, query_space_holding_window,
    query_window_level_from_window_server, query_window_opacity_from_window_server,
    query_window_sub_level_from_window_server, query_window_title_from_window_server,
};
use libc::proc_name;

pub(crate) fn write_untracked_window_as_json_object(
    response: &mut Response,
    window_id: WindowId,
    flags: u64,
    display_manager: &mut DisplayManager,
) {
    let mut flags = flags;
    if flags == 0x0 {
        flags |= !flags;
    }

    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();

    let mut process_id: Option<libc::pid_t> = None;
    let mut space_id: Option<SpaceId> = None;
    let mut level: Option<i32> = None;
    let mut sub_level: Option<i32> = None;

    if (flags & WINDOW_PROPERTY_PID) != 0 || (flags & WINDOW_PROPERTY_APP) != 0 {
        let mut connection: i32 = 0;
        unsafe { SLSGetWindowOwner(connection_id, window_id.0, &mut connection) };
        let mut value: libc::pid_t = 0;
        unsafe { SLSConnectionGetPID(connection, &mut value) };
        process_id = Some(value);
    }

    if (flags & WINDOW_PROPERTY_DISPLAY) != 0
        || (flags & WINDOW_PROPERTY_SPACE) != 0
        || (flags & WINDOW_PROPERTY_IS_FULLSCREEN) != 0
    {
        space_id = Some(query_space_holding_window(window_id));
    }

    if (flags & WINDOW_PROPERTY_LEVEL) != 0 || (flags & WINDOW_PROPERTY_LAYER) != 0 {
        level = Some(query_window_level_from_window_server(window_id));
    }

    if (flags & WINDOW_PROPERTY_SUB_LEVEL) != 0 || (flags & WINDOW_PROPERTY_SUB_LAYER) != 0 {
        sub_level = Some(query_window_sub_level_from_window_server(window_id));
    }

    let mut did_output = false;
    response.write(format_args!("{{\n"));

    if (flags & WINDOW_PROPERTY_ID) != 0 {
        response.write(format_args!("\t\"id\":{}", window_id.0 as i32));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_PID) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"pid\":{}", process_id.unwrap()));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_APP) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        static PROCESS_NAME_BUFFER: Mutex<[u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize]> =
            Mutex::new([0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize]);
        let mut process_name = PROCESS_NAME_BUFFER.lock().unwrap();
        unsafe {
            proc_name(
                process_id.unwrap(),
                process_name.as_mut_ptr().cast::<c_void>(),
                process_name.len() as u32,
            )
        };

        let end = process_name
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(process_name.len());
        let application_name = String::from_utf8_lossy(&process_name[..end]).into_owned();
        drop(process_name);
        let escaped_application_name =
            escape_string_for_json_when_it_needs_escaping(&application_name);

        response.write(format_args!(
            "\t\"app\":\"{}\"",
            escaped_application_name
                .as_deref()
                .unwrap_or(&application_name)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_TITLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let title = query_window_title_from_window_server(window_id);
        let escaped_title = escape_string_for_json_when_it_needs_escaping(&title);

        response.write(format_args!(
            "\t\"title\":\"{}\"",
            escaped_title.as_deref().unwrap_or(&title)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SCRATCHPAD) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"scratchpad\":\"{}\"", ""));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_FRAME) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let mut frame = CGRect::ZERO;
        unsafe { SLSGetWindowBounds(connection_id, window_id.0, &mut frame) };

        response.write(format_args!(
            "\t\"frame\":{{\n\t\t\"x\":{},\n\t\t\"y\":{},\n\t\t\"w\":{},\n\t\t\"h\":{}\n\t}}",
            format_float_with_decimals_as_printf_does(frame.origin.x, 4),
            format_float_with_decimals_as_printf_does(frame.origin.y, 4),
            format_float_with_decimals_as_printf_does(frame.size.width, 4),
            format_float_with_decimals_as_printf_does(frame.size.height, 4)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_ROLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"role\":\"{}\"", ""));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SUBROLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"subrole\":\"{}\"", ""));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_ROOT_WINDOW) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let parent_window_id = query_parent_window_from_window_server(window_id);
        response.write(format_args!(
            "\t\"root-window\":{}",
            json_literal_for_boolean(parent_window_id == WindowId(0))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_DISPLAY) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let display = query_arrangement_index_of_display(
            query_display_holding_space(space_id.unwrap()),
            display_manager,
        );
        response.write(format_args!("\t\"display\":{}", display));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SPACE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let space = query_mission_control_index_of_space(space_id.unwrap());
        response.write(format_args!("\t\"space\":{}", space));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_LEVEL) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"level\":{}", level.unwrap()));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SUB_LEVEL) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"sub-level\":{}", sub_level.unwrap()));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_LAYER) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let layer = layer_name_of_window_level(level.unwrap());
        response.write(format_args!("\t\"layer\":\"{}\"", layer));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SUB_LAYER) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let sub_layer = layer_name_of_window_level(sub_level.unwrap());
        response.write(format_args!("\t\"sub-layer\":\"{}\"", sub_layer));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_OPACITY) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let opacity = query_window_opacity_from_window_server(window_id);
        response.write(format_args!(
            "\t\"opacity\":{}",
            format_float_with_decimals_as_printf_does(opacity as f64, 4)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SPLIT_TYPE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"split-type\":\"{}\"",
            WINDOW_NODE_SPLIT_NAMES[0]
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SPLIT_CHILD) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"split-child\":\"{}\"",
            WINDOW_NODE_CHILD_NAMES[WindowNodeChild::None as usize]
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_STACK_INDEX) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"stack-index\":{}", 0));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_CAN_MOVE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"can-move\":{}",
            json_literal_for_boolean(false)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_CAN_RESIZE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"can-resize\":{}",
            json_literal_for_boolean(false)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_FOCUS) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-focus\":{}",
            json_literal_for_boolean(false)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_SHADOW) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-shadow\":{}",
            json_literal_for_boolean(is_window_shadow_shown_according_to_window_server(window_id))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_PARENT_ZOOM) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-parent-zoom\":{}",
            json_literal_for_boolean(false)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_FULLSCREEN_ZOOM) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-fullscreen-zoom\":{}",
            json_literal_for_boolean(false)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_AX_REFERENCE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-ax-reference\":{}",
            json_literal_for_boolean(false)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_FULLSCREEN) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let is_fullscreen = is_native_fullscreen_space(space_id.unwrap());
        response.write(format_args!(
            "\t\"is-native-fullscreen\":{}",
            json_literal_for_boolean(is_fullscreen)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_VISIBLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-visible\":{}",
            json_literal_for_boolean(false)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_MINIMIZED) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-minimized\":{}",
            json_literal_for_boolean(false)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_HIDDEN) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-hidden\":{}",
            json_literal_for_boolean(false)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_FLOATING) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-floating\":{}",
            json_literal_for_boolean(false)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_STICKY) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let is_sticky = is_window_on_more_than_one_space(window_id);
        response.write(format_args!(
            "\t\"is-sticky\":{}",
            json_literal_for_boolean(is_sticky)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_GRABBED) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-grabbed\":{}",
            json_literal_for_boolean(false)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_GROUPED) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-grouped\":{}",
            json_literal_for_boolean(false)
        ));
    }

    response.write(format_args!("\n}}"));
}
