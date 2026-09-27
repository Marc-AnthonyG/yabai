use crate::display::arrangement::display_manager_display_id_arrangement;
use crate::display::manager::DisplayManager;
use crate::ffi::skylight::SLSWindowIsOrderedIn;
use crate::layout::tree::{
    WINDOW_NODE_CHILD_STR, WINDOW_NODE_SPLIT_STR, WindowNodeChild, view_find_window_node,
    window_node_index_of_window, window_node_is_left_child,
};
use crate::mouse::drag::MouseDragState;
use crate::space::lookup::space_manager_mission_control_index;
use crate::space::managed_space::{space_display_id, space_is_visible};
use crate::space::manager::SpaceManager;
use crate::state::process_wide::{
    CONNECTION, LAYER_ABOVE_WINDOW_LEVEL, LAYER_BELOW_WINDOW_LEVEL, LAYER_NORMAL_WINDOW_LEVEL,
};
use crate::support::handles::{NodeId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::support::json::{json_bool, ts_string_escape};
use crate::support::layer::{LAYER_ABOVE, LAYER_BELOW, LAYER_NORMAL, LAYER_STR};
use crate::support::response::Response;
use crate::window::manager::{WindowManager, window_manager_find_managed_window};
use crate::window::model::{
    WindowFlag, window_can_move, window_can_resize, window_check_flag, window_is_sticky,
    window_level, window_opacity, window_role_ts, window_shadow, window_space, window_sub_level,
    window_subrole_ts, window_title_ts,
};

macro_rules! window_property_list {
    ($window_property_entry:ident) => {
        $window_property_entry! {
            ("id", WINDOW_PROPERTY_ID, 0x000000001),
            ("pid", WINDOW_PROPERTY_PID, 0x000000002),
            ("app", WINDOW_PROPERTY_APP, 0x000000004),
            ("title", WINDOW_PROPERTY_TITLE, 0x000000008),
            ("scratchpad", WINDOW_PROPERTY_SCRATCHPAD, 0x000000010),
            ("frame", WINDOW_PROPERTY_FRAME, 0x000000020),
            ("role", WINDOW_PROPERTY_ROLE, 0x000000040),
            ("subrole", WINDOW_PROPERTY_SUBROLE, 0x000000080),
            ("root-window", WINDOW_PROPERTY_ROOT_WINDOW, 0x000000100),
            ("display", WINDOW_PROPERTY_DISPLAY, 0x000000200),
            ("space", WINDOW_PROPERTY_SPACE, 0x000000400),
            ("level", WINDOW_PROPERTY_LEVEL, 0x000000800),
            ("sub-level", WINDOW_PROPERTY_SUB_LEVEL, 0x000001000),
            ("layer", WINDOW_PROPERTY_LAYER, 0x000002000),
            ("sub-layer", WINDOW_PROPERTY_SUB_LAYER, 0x000004000),
            ("opacity", WINDOW_PROPERTY_OPACITY, 0x000008000),
            ("split-type", WINDOW_PROPERTY_SPLIT_TYPE, 0x000010000),
            ("split-child", WINDOW_PROPERTY_SPLIT_CHILD, 0x000020000),
            ("stack-index", WINDOW_PROPERTY_STACK_INDEX, 0x000040000),
            ("can-move", WINDOW_PROPERTY_CAN_MOVE, 0x000080000),
            ("can-resize", WINDOW_PROPERTY_CAN_RESIZE, 0x000100000),
            ("has-focus", WINDOW_PROPERTY_HAS_FOCUS, 0x000200000),
            ("has-shadow", WINDOW_PROPERTY_HAS_SHADOW, 0x000400000),
            ("has-parent-zoom", WINDOW_PROPERTY_HAS_PARENT_ZOOM, 0x000800000),
            ("has-fullscreen-zoom", WINDOW_PROPERTY_HAS_FULLSCREEN_ZOOM, 0x001000000),
            ("has-ax-reference", WINDOW_PROPERTY_HAS_AX_REFERENCE, 0x002000000),
            ("is-native-fullscreen", WINDOW_PROPERTY_IS_FULLSCREEN, 0x004000000),
            ("is-visible", WINDOW_PROPERTY_IS_VISIBLE, 0x008000000),
            ("is-minimized", WINDOW_PROPERTY_IS_MINIMIZED, 0x010000000),
            ("is-hidden", WINDOW_PROPERTY_IS_HIDDEN, 0x020000000),
            ("is-floating", WINDOW_PROPERTY_IS_FLOATING, 0x040000000),
            ("is-sticky", WINDOW_PROPERTY_IS_STICKY, 0x080000000),
            ("is-grabbed", WINDOW_PROPERTY_IS_GRABBED, 0x100000000),
        }
    };
}

macro_rules! define_window_property_list {
    ($(($name:literal, $identifier:ident, $value:literal)),* $(,)?) => {
        $(pub(crate) const $identifier: u64 = $value;)*

        pub(crate) static WINDOW_PROPERTY_VAL: [u64; 33] = [$($value),*];

        pub(crate) static WINDOW_PROPERTY_STR: [&str; 33] = [$($name),*];
    };
}

window_property_list!(define_window_property_list);

pub(crate) fn window_layer(level: i32) -> &'static str {
    if level == *LAYER_BELOW_WINDOW_LEVEL.get().unwrap() {
        return LAYER_STR[LAYER_BELOW as usize].unwrap();
    }
    if level == *LAYER_NORMAL_WINDOW_LEVEL.get().unwrap() {
        return LAYER_STR[LAYER_NORMAL as usize].unwrap();
    }
    if level == *LAYER_ABOVE_WINDOW_LEVEL.get().unwrap() {
        return LAYER_STR[LAYER_ABOVE as usize].unwrap();
    }
    "unknown"
}

pub(crate) fn window_serialize(
    response: &mut Response,
    window_id: WindowId,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let mut flags = flags;
    if flags == 0x0 {
        flags |= !flags;
    }

    let connection_id = *CONNECTION.get().unwrap();

    let mut space_id: Option<SpaceId> = None;
    let mut level: Option<i32> = None;
    let mut sub_level: Option<i32> = None;
    let mut view: Option<SpaceId> = None;
    let mut node: Option<NodeId> = None;
    let mut is_minimized: Option<bool> = None;
    let mut is_sticky: Option<bool> = None;

    if (flags & WINDOW_PROPERTY_DISPLAY) != 0
        || (flags & WINDOW_PROPERTY_SPACE) != 0
        || (flags & WINDOW_PROPERTY_IS_VISIBLE) != 0
    {
        space_id = Some(window_space(window_id));
    }

    if (flags & WINDOW_PROPERTY_LEVEL) != 0 || (flags & WINDOW_PROPERTY_LAYER) != 0 {
        level = Some(window_level(window_id));
    }

    if (flags & WINDOW_PROPERTY_SUB_LEVEL) != 0 || (flags & WINDOW_PROPERTY_SUB_LAYER) != 0 {
        sub_level = Some(window_sub_level(window_id));
    }

    if (flags & WINDOW_PROPERTY_SPLIT_TYPE) != 0
        || (flags & WINDOW_PROPERTY_SPLIT_CHILD) != 0
        || (flags & WINDOW_PROPERTY_STACK_INDEX) != 0
        || (flags & WINDOW_PROPERTY_HAS_PARENT_ZOOM) != 0
        || (flags & WINDOW_PROPERTY_HAS_FULLSCREEN_ZOOM) != 0
    {
        view = window_manager_find_managed_window(window_manager, window_id);
        node = match view {
            Some(view) => view_find_window_node(space_manager, view, window_id),
            None => None,
        };
    }

    let Some(window) = window_manager.window.find(&window_id) else {
        return;
    };

    if (flags & WINDOW_PROPERTY_IS_VISIBLE) != 0 || (flags & WINDOW_PROPERTY_IS_MINIMIZED) != 0 {
        is_minimized = Some(window_check_flag(window, WindowFlag::MINIMIZE));
    }

    if (flags & WINDOW_PROPERTY_IS_VISIBLE) != 0 || (flags & WINDOW_PROPERTY_IS_STICKY) != 0 {
        is_sticky =
            Some(window_check_flag(window, WindowFlag::STICKY) || window_is_sticky(window_id));
    }

    let application = window
        .application
        .and_then(|process_id| window_manager.application.find(&process_id));
    let application_is_hidden = application.is_some_and(|application| application.is_hidden);

    let mut did_output = false;
    response.write(format_args!("{{\n"));

    if (flags & WINDOW_PROPERTY_ID) != 0 {
        response.write(format_args!("\t\"id\":{}", window.id.0 as i32));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_PID) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"pid\":{}",
            application.map_or(0, |application| application.process_id.0)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_APP) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let application_name = application.map_or("(null)", |application| &application.name);
        let escaped_application_name = ts_string_escape(application_name);

        response.write(format_args!(
            "\t\"app\":\"{}\"",
            escaped_application_name.as_deref().unwrap_or(application_name)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_TITLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let title = window_title_ts(window);
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

        response.write(format_args!(
            "\t\"scratchpad\":\"{}\"",
            window.scratchpad.as_deref().unwrap_or("")
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_FRAME) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"frame\":{{\n\t\t\"x\":{:.4},\n\t\t\"y\":{:.4},\n\t\t\"w\":{:.4},\n\t\t\"h\":{:.4}\n\t}}",
            window.frame.origin.x,
            window.frame.origin.y,
            window.frame.size.width,
            window.frame.size.height
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_ROLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let role = window_role_ts(window);
        response.write(format_args!("\t\"role\":\"{}\"", role));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SUBROLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let subrole = window_subrole_ts(window);
        response.write(format_args!("\t\"subrole\":\"{}\"", subrole));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_ROOT_WINDOW) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"root-window\":{}",
            json_bool(window.is_root)
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

        let opacity = window_opacity(window.id);
        response.write(format_args!("\t\"opacity\":{:.4}", opacity as f64));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SPLIT_TYPE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let mut split_type: usize = 0;
        if let (Some(view), Some(node)) = (view, node) {
            if let Some(view) = space_manager.view.find(&view) {
                if let Some(parent) = view.find_node(node).and_then(|node| node.parent) {
                    if let Some(parent) = view.find_node(parent) {
                        split_type = parent.split as usize;
                    }
                }
            }
        }

        response.write(format_args!(
            "\t\"split-type\":\"{}\"",
            WINDOW_NODE_SPLIT_STR[split_type]
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SPLIT_CHILD) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let split_child = match (view, node) {
            (Some(view), Some(node)) => {
                if window_node_is_left_child(view, node, space_manager) {
                    WindowNodeChild::First as usize
                } else {
                    WindowNodeChild::Second as usize
                }
            }
            _ => WindowNodeChild::None as usize,
        };

        response.write(format_args!(
            "\t\"split-child\":\"{}\"",
            WINDOW_NODE_CHILD_STR[split_child]
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_STACK_INDEX) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let mut stack_index: i32 = 0;
        if let (Some(view), Some(node)) = (view, node) {
            let window_count = space_manager
                .view
                .find(&view)
                .and_then(|view| view.find_node(node))
                .map_or(0, |node| node.window_count);
            if window_count > 1 {
                stack_index =
                    window_node_index_of_window(view, node, window.id, space_manager) + 1;
            }
        }

        response.write(format_args!("\t\"stack-index\":{}", stack_index));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_CAN_MOVE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"can-move\":{}",
            json_bool(window_can_move(window))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_CAN_RESIZE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"can-resize\":{}",
            json_bool(window_can_resize(window))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_FOCUS) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-focus\":{}",
            json_bool(window.id == window_manager.focused_window_id)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_SHADOW) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-shadow\":{}",
            json_bool(window_shadow(window.id))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_PARENT_ZOOM) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let mut zoom_parent = false;
        if let (Some(view), Some(node)) = (view, node) {
            if let Some(node) = space_manager
                .view
                .find(&view)
                .and_then(|view| view.find_node(node))
            {
                zoom_parent = node.zoom.is_some() && node.zoom == node.parent;
            }
        }

        response.write(format_args!(
            "\t\"has-parent-zoom\":{}",
            json_bool(zoom_parent)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_FULLSCREEN_ZOOM) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let mut zoom_fullscreen = false;
        if let (Some(view), Some(node)) = (view, node) {
            if let Some(node) = space_manager
                .view
                .find(&view)
                .and_then(|view| view.find_node(node))
            {
                zoom_fullscreen = node.zoom.is_some() && node.zoom == Some(ROOT_NODE_ID);
            }
        }

        response.write(format_args!(
            "\t\"has-fullscreen-zoom\":{}",
            json_bool(zoom_fullscreen)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_AX_REFERENCE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!("\t\"has-ax-reference\":{}", json_bool(true)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_FULLSCREEN) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-native-fullscreen\":{}",
            json_bool(window_check_flag(window, WindowFlag::FULLSCREEN))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_VISIBLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let mut ordered_in: u8 = 0;
        unsafe { SLSWindowIsOrderedIn(connection_id, window.id.0, &mut ordered_in) };

        let visible = ordered_in != 0
            && !is_minimized.unwrap()
            && !application_is_hidden
            && (is_sticky.unwrap() || space_is_visible(space_id.unwrap()));
        response.write(format_args!("\t\"is-visible\":{}", json_bool(visible)));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_MINIMIZED) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-minimized\":{}",
            json_bool(is_minimized.unwrap())
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_HIDDEN) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-hidden\":{}",
            json_bool(application_is_hidden)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_FLOATING) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-floating\":{}",
            json_bool(window_check_flag(window, WindowFlag::FLOAT))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_STICKY) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-sticky\":{}",
            json_bool(is_sticky.unwrap())
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_GRABBED) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let grabbed = mouse_drag_state.window_id == Some(window.id);
        response.write(format_args!("\t\"is-grabbed\":{}", json_bool(grabbed)));
    }

    response.write(format_args!("\n}}"));
}
