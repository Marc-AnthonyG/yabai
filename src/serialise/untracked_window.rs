use core::ffi::c_void;
use std::sync::Mutex;

use crate::display::arrangement::display_manager_display_id_arrangement;
use crate::display::manager::DisplayManager;
use crate::ffi::core_foundation::CGRect;
use crate::ffi::libsystem::{PROC_PIDPATHINFO_MAXSIZE, proc_name};
use crate::ffi::skylight::{SLSConnectionGetPID, SLSGetWindowBounds, SLSGetWindowOwner};
use crate::layout::tree::{WINDOW_NODE_CHILD_STR, WINDOW_NODE_SPLIT_STR, WindowNodeChild};
use crate::serialise::window::{
    WINDOW_PROPERTY_APP, WINDOW_PROPERTY_CAN_MOVE, WINDOW_PROPERTY_CAN_RESIZE,
    WINDOW_PROPERTY_DISPLAY, WINDOW_PROPERTY_FRAME, WINDOW_PROPERTY_HAS_AX_REFERENCE,
    WINDOW_PROPERTY_HAS_FOCUS, WINDOW_PROPERTY_HAS_FULLSCREEN_ZOOM,
    WINDOW_PROPERTY_HAS_PARENT_ZOOM, WINDOW_PROPERTY_HAS_SHADOW, WINDOW_PROPERTY_ID,
    WINDOW_PROPERTY_IS_FLOATING, WINDOW_PROPERTY_IS_FULLSCREEN, WINDOW_PROPERTY_IS_GRABBED,
    WINDOW_PROPERTY_IS_HIDDEN, WINDOW_PROPERTY_IS_MINIMIZED, WINDOW_PROPERTY_IS_STICKY,
    WINDOW_PROPERTY_IS_VISIBLE, WINDOW_PROPERTY_LAYER, WINDOW_PROPERTY_LEVEL,
    WINDOW_PROPERTY_OPACITY, WINDOW_PROPERTY_PID, WINDOW_PROPERTY_ROLE,
    WINDOW_PROPERTY_ROOT_WINDOW, WINDOW_PROPERTY_SCRATCHPAD, WINDOW_PROPERTY_SPACE,
    WINDOW_PROPERTY_SPLIT_CHILD, WINDOW_PROPERTY_SPLIT_TYPE, WINDOW_PROPERTY_STACK_INDEX,
    WINDOW_PROPERTY_SUB_LAYER, WINDOW_PROPERTY_SUB_LEVEL, WINDOW_PROPERTY_SUBROLE,
    WINDOW_PROPERTY_TITLE, window_layer,
};
use crate::space::lookup::space_manager_mission_control_index;
use crate::space::managed_space::{space_display_id, space_is_fullscreen};
use crate::state::process_wide::CONNECTION;
use crate::support::handles::{SpaceId, WindowId};
use crate::support::json::{json_bool, ts_string_escape};
use crate::support::response::Response;
use crate::window::model::{
    window_is_sticky, window_level, window_opacity, window_parent, window_property_title_ts,
    window_shadow, window_space, window_sub_level,
};

pub(crate) fn window_nonax_serialize(
    response: &mut Response,
    window_id: WindowId,
    flags: u64,
    display_manager: &mut DisplayManager,
) {
    let mut flags = flags;
    if flags == 0x0 {
        flags |= !flags;
    }

    let connection_id = *CONNECTION.get().unwrap();

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
        space_id = Some(window_space(window_id));
    }

    if (flags & WINDOW_PROPERTY_LEVEL) != 0 || (flags & WINDOW_PROPERTY_LAYER) != 0 {
        level = Some(window_level(window_id));
    }

    if (flags & WINDOW_PROPERTY_SUB_LEVEL) != 0 || (flags & WINDOW_PROPERTY_SUB_LAYER) != 0 {
        sub_level = Some(window_sub_level(window_id));
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

        static PROCESS_NAME: Mutex<[u8; PROC_PIDPATHINFO_MAXSIZE]> =
            Mutex::new([0u8; PROC_PIDPATHINFO_MAXSIZE]);
        let mut process_name = PROCESS_NAME.lock().unwrap();
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
        let escaped_application_name = ts_string_escape(&application_name);

        response.write(format_args!(
            "\t\"app\":\"{}\"",
            escaped_application_name.as_deref().unwrap_or(&application_name)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_TITLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let title = window_property_title_ts(window_id);
        let escaped_title = ts_string_escape(&title);

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
            "\t\"frame\":{{\n\t\t\"x\":{:.4},\n\t\t\"y\":{:.4},\n\t\t\"w\":{:.4},\n\t\t\"h\":{:.4}\n\t}}",
            frame.origin.x, frame.origin.y, frame.size.width, frame.size.height
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

        let parent_window_id = window_parent(window_id);
        response.write(format_args!(
            "\t\"root-window\":{}",
            json_bool(parent_window_id == WindowId(0))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_DISPLAY) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let display =
            display_manager_display_id_arrangement(space_display_id(space_id.unwrap()), display_manager);
        response.write(format_args!("\t\"display\":{}", display));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SPACE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let space = space_manager_mission_control_index(space_id.unwrap());
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

        let layer = window_layer(level.unwrap());
        response.write(format_args!("\t\"layer\":\"{}\"", layer));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SUB_LAYER) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let sub_layer = window_layer(sub_level.unwrap());
        response.write(format_args!("\t\"sub-layer\":\"{}\"", sub_layer));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_OPACITY) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let opacity = window_opacity(window_id);
        response.write(format_args!("\t\"opacity\":{:.4}", opacity as f64));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SPLIT_TYPE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"split-type\":\"{}\"",
            WINDOW_NODE_SPLIT_STR[0]
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SPLIT_CHILD) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"split-child\":\"{}\"",
            WINDOW_NODE_CHILD_STR[WindowNodeChild::None as usize]
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

        response.write(format_args!("\t\"can-move\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_CAN_RESIZE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"can-resize\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_FOCUS) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"has-focus\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_SHADOW) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-shadow\":{}",
            json_bool(window_shadow(window_id))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_PARENT_ZOOM) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"has-parent-zoom\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_FULLSCREEN_ZOOM) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"has-fullscreen-zoom\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_AX_REFERENCE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"has-ax-reference\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_FULLSCREEN) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let is_fullscreen = space_is_fullscreen(space_id.unwrap());
        response.write(format_args!(
            "\t\"is-native-fullscreen\":{}",
            json_bool(is_fullscreen)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_VISIBLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"is-visible\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_MINIMIZED) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"is-minimized\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_HIDDEN) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"is-hidden\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_FLOATING) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"is-floating\":{}", json_bool(false)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_STICKY) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let is_sticky = window_is_sticky(window_id);
        response.write(format_args!("\t\"is-sticky\":{}", json_bool(is_sticky)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_GRABBED) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"is-grabbed\":{}", json_bool(false)));
    }

    response.write(format_args!("\n}}"));
}
