use crate::display::arrangement::query_arrangement_index_of_display;
use crate::display::manager::DisplayManager;
use crate::ffi::skylight::SLSWindowIsOrderedIn;
use crate::layout::group::is_window_in_a_group;
use crate::layout::tree::{
    WINDOW_NODE_CHILD_NAMES, WINDOW_NODE_SPLIT_NAMES, WindowNodeChild,
    is_node_the_left_child_of_its_parent, leaf_holding_window, stack_index_of_window_in_node,
};
use crate::mouse::drag::MouseDragState;
use crate::space::lookup::query_mission_control_index_of_space;
use crate::space::managed_space::{is_space_visible_on_its_display, query_display_holding_space};
use crate::space::manager::SpaceManager;
use crate::state::process_wide::{
    LAYER_ABOVE_WINDOW_LEVEL, LAYER_BELOW_WINDOW_LEVEL, LAYER_NORMAL_WINDOW_LEVEL,
    SKYLIGHT_CONNECTION_ID,
};
use crate::support::handles::{NodeId, ROOT_NODE_ID, SpaceId, WindowId};
use crate::support::json::{
    escape_string_for_json_when_it_needs_escaping, json_literal_for_boolean,
};
use crate::support::layer::{LAYER_ABOVE, LAYER_BELOW, LAYER_NAMES, LAYER_NORMAL};
use crate::support::printf_float_format::format_float_with_decimals_as_printf_does;
use crate::support::response::Response;
use crate::window::manager::{WindowManager, space_managing_window};
use crate::window::model::{
    WindowFlag, is_window_movable, is_window_on_more_than_one_space, is_window_resizable,
    is_window_shadow_shown_according_to_window_server, query_space_holding_window,
    query_window_level_from_window_server, query_window_opacity_from_window_server,
    query_window_sub_level_from_window_server, window_role_as_string, window_subrole_as_string,
    window_title_as_string,
};

macro_rules! with_every_window_property {
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
            ("is-grouped", WINDOW_PROPERTY_IS_GROUPED, 0x200000000),
        }
    };
}

macro_rules! define_window_property_bits_and_names {
    ($(($name:literal, $identifier:ident, $value:literal)),* $(,)?) => {
        $(pub(crate) const $identifier: u64 = $value;)*

        pub(crate) static WINDOW_PROPERTY_SELECTION_BITS: [u64; 34] = [$($value),*];

        pub(crate) static WINDOW_PROPERTY_NAMES: [&str; 34] = [$($name),*];
    };
}

with_every_window_property!(define_window_property_bits_and_names);

pub(crate) fn layer_name_of_window_level(level: i32) -> &'static str {
    if level == *LAYER_BELOW_WINDOW_LEVEL.get().unwrap() {
        return LAYER_NAMES[LAYER_BELOW as usize].unwrap();
    }
    if level == *LAYER_NORMAL_WINDOW_LEVEL.get().unwrap() {
        return LAYER_NAMES[LAYER_NORMAL as usize].unwrap();
    }
    if level == *LAYER_ABOVE_WINDOW_LEVEL.get().unwrap() {
        return LAYER_NAMES[LAYER_ABOVE as usize].unwrap();
    }
    "unknown"
}

pub(crate) fn write_tracked_window_as_json_object(
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

    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();

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
        space_id = Some(query_space_holding_window(window_id));
    }

    if (flags & WINDOW_PROPERTY_LEVEL) != 0 || (flags & WINDOW_PROPERTY_LAYER) != 0 {
        level = Some(query_window_level_from_window_server(window_id));
    }

    if (flags & WINDOW_PROPERTY_SUB_LEVEL) != 0 || (flags & WINDOW_PROPERTY_SUB_LAYER) != 0 {
        sub_level = Some(query_window_sub_level_from_window_server(window_id));
    }

    if (flags & WINDOW_PROPERTY_SPLIT_TYPE) != 0
        || (flags & WINDOW_PROPERTY_SPLIT_CHILD) != 0
        || (flags & WINDOW_PROPERTY_STACK_INDEX) != 0
        || (flags & WINDOW_PROPERTY_HAS_PARENT_ZOOM) != 0
        || (flags & WINDOW_PROPERTY_HAS_FULLSCREEN_ZOOM) != 0
    {
        view = space_managing_window(window_manager, window_id);
        node = match view {
            Some(view) => leaf_holding_window(space_manager, view, window_id),
            None => None,
        };
    }

    let mut is_grouped: Option<bool> = None;
    if (flags & WINDOW_PROPERTY_IS_GROUPED) != 0 {
        let space_managing_the_window = space_managing_window(window_manager, window_id);
        is_grouped = Some(space_managing_the_window.is_some_and(|space_id| {
            is_window_in_a_group(space_manager, space_id, window_id, window_manager)
        }));
    }

    let Some(window) = window_manager.window.get(&window_id) else {
        return;
    };

    if (flags & WINDOW_PROPERTY_IS_VISIBLE) != 0 || (flags & WINDOW_PROPERTY_IS_MINIMIZED) != 0 {
        is_minimized = Some(window.flags.contains(WindowFlag::MINIMIZED));
    }

    if (flags & WINDOW_PROPERTY_IS_VISIBLE) != 0 || (flags & WINDOW_PROPERTY_IS_STICKY) != 0 {
        is_sticky = Some(
            window.flags.contains(WindowFlag::STICKY)
                || is_window_on_more_than_one_space(window_id),
        );
    }

    let application = window
        .application
        .and_then(|process_id| window_manager.application.get(&process_id));
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
        let escaped_application_name =
            escape_string_for_json_when_it_needs_escaping(application_name);

        response.write(format_args!(
            "\t\"app\":\"{}\"",
            escaped_application_name
                .as_deref()
                .unwrap_or(application_name)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_TITLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let title = window_title_as_string(window);
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
            "\t\"frame\":{{\n\t\t\"x\":{},\n\t\t\"y\":{},\n\t\t\"w\":{},\n\t\t\"h\":{}\n\t}}",
            format_float_with_decimals_as_printf_does(window.frame.origin.x, 4),
            format_float_with_decimals_as_printf_does(window.frame.origin.y, 4),
            format_float_with_decimals_as_printf_does(window.frame.size.width, 4),
            format_float_with_decimals_as_printf_does(window.frame.size.height, 4)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_ROLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let role = window_role_as_string(window);
        response.write(format_args!("\t\"role\":\"{}\"", role));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SUBROLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let subrole = window_subrole_as_string(window);
        response.write(format_args!("\t\"subrole\":\"{}\"", subrole));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_ROOT_WINDOW) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"root-window\":{}",
            json_literal_for_boolean(window.is_root)
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

        let opacity = query_window_opacity_from_window_server(window.id);
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

        let mut split_type: usize = 0;
        if let (Some(view), Some(node)) = (view, node) {
            if let Some(view) = space_manager.view.get(&view) {
                if let Some(parent) = view.find_node(node).and_then(|node| node.parent) {
                    if let Some(parent) = view.find_node(parent) {
                        split_type = parent.split as usize;
                    }
                }
            }
        }

        response.write(format_args!(
            "\t\"split-type\":\"{}\"",
            WINDOW_NODE_SPLIT_NAMES[split_type]
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_SPLIT_CHILD) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let split_child = match (view, node) {
            (Some(view), Some(node)) => {
                if is_node_the_left_child_of_its_parent(view, node, space_manager) {
                    WindowNodeChild::First as usize
                } else {
                    WindowNodeChild::Second as usize
                }
            }
            _ => WindowNodeChild::None as usize,
        };

        response.write(format_args!(
            "\t\"split-child\":\"{}\"",
            WINDOW_NODE_CHILD_NAMES[split_child]
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
                .get(&view)
                .and_then(|view| view.find_node(node))
                .map_or(0, |node| node.window_count);
            if window_count > 1 {
                stack_index =
                    stack_index_of_window_in_node(view, node, window.id, space_manager) + 1;
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
            json_literal_for_boolean(is_window_movable(window))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_CAN_RESIZE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"can-resize\":{}",
            json_literal_for_boolean(is_window_resizable(window))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_FOCUS) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-focus\":{}",
            json_literal_for_boolean(window.id == window_manager.focused_window_id)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_SHADOW) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-shadow\":{}",
            json_literal_for_boolean(is_window_shadow_shown_according_to_window_server(window.id))
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
                .get(&view)
                .and_then(|view| view.find_node(node))
            {
                zoom_parent = node.zoom.is_some() && node.zoom == node.parent;
            }
        }

        response.write(format_args!(
            "\t\"has-parent-zoom\":{}",
            json_literal_for_boolean(zoom_parent)
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
                .get(&view)
                .and_then(|view| view.find_node(node))
            {
                zoom_fullscreen = node.zoom.is_some() && node.zoom == Some(ROOT_NODE_ID);
            }
        }

        response.write(format_args!(
            "\t\"has-fullscreen-zoom\":{}",
            json_literal_for_boolean(zoom_fullscreen)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_HAS_AX_REFERENCE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-ax-reference\":{}",
            json_literal_for_boolean(true)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_FULLSCREEN) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-native-fullscreen\":{}",
            json_literal_for_boolean(window.flags.contains(WindowFlag::IN_NATIVE_FULLSCREEN))
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
            && (is_sticky.unwrap() || is_space_visible_on_its_display(space_id.unwrap()));
        response.write(format_args!(
            "\t\"is-visible\":{}",
            json_literal_for_boolean(visible)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_MINIMIZED) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-minimized\":{}",
            json_literal_for_boolean(is_minimized.unwrap())
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_HIDDEN) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-hidden\":{}",
            json_literal_for_boolean(application_is_hidden)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_FLOATING) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-floating\":{}",
            json_literal_for_boolean(window.flags.contains(WindowFlag::FLOATING))
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_STICKY) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-sticky\":{}",
            json_literal_for_boolean(is_sticky.unwrap())
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_GRABBED) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let grabbed = mouse_drag_state.window_id == Some(window.id);
        response.write(format_args!(
            "\t\"is-grabbed\":{}",
            json_literal_for_boolean(grabbed)
        ));
        did_output = true;
    }

    if (flags & WINDOW_PROPERTY_IS_GROUPED) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-grouped\":{}",
            json_literal_for_boolean(is_grouped.unwrap_or(false))
        ));
    }

    response.write(format_args!("\n}}"));
}
