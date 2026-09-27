use crate::display_manager::{DisplayManager, display_manager_display_id_arrangement};
use crate::ffi::core_foundation::ts_cfstring_copy;
use crate::handles::{ROOT_NODE_ID, SpaceId, WindowId};
use crate::layout::settings::VIEW_TYPE_STR;
use crate::layout::tree::{window_node_find_first_leaf, window_node_find_last_leaf};
use crate::space::{space_display_id, space_is_fullscreen, space_is_visible, space_window_list};
use crate::space_manager::{
    SpaceManager, space_manager_get_label_for_space, space_manager_mission_control_index,
};
use crate::support::json::json_bool;
use crate::support::response::Response;
use crate::window_manager::WindowManager;

macro_rules! space_property_list {
    ($space_property_entry:ident) => {
        $space_property_entry! {
            ("id", SPACE_PROPERTY_ID, 0x001),
            ("uuid", SPACE_PROPERTY_UUID, 0x002),
            ("index", SPACE_PROPERTY_INDEX, 0x004),
            ("label", SPACE_PROPERTY_LABEL, 0x008),
            ("type", SPACE_PROPERTY_TYPE, 0x010),
            ("display", SPACE_PROPERTY_DISPLAY, 0x020),
            ("windows", SPACE_PROPERTY_WINDOWS, 0x040),
            ("first-window", SPACE_PROPERTY_FIRST_WINDOW, 0x080),
            ("last-window", SPACE_PROPERTY_LAST_WINDOW, 0x100),
            ("has-focus", SPACE_PROPERTY_HAS_FOCUS, 0x200),
            ("is-visible", SPACE_PROPERTY_IS_VISIBLE, 0x400),
            ("is-native-fullscreen", SPACE_PROPERTY_IS_FULLSCREEN, 0x800),
        }
    };
}

macro_rules! define_space_property_list {
    ($(($name:literal, $identifier:ident, $value:literal)),* $(,)?) => {
        $(pub(crate) const $identifier: u64 = $value;)*

        pub(crate) static SPACE_PROPERTY_VAL: [u64; 12] = [$($value),*];

        pub(crate) static SPACE_PROPERTY_STR: [&str; 12] = [$($name),*];
    };
}

space_property_list!(define_space_property_list);

pub(crate) fn view_serialize(
    response: &mut Response,
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let Some(view) = space_manager.view.find(&space_id) else {
        return;
    };
    let layout = view.layout;

    let mut flags = flags;
    if flags == 0x0 {
        flags |= !flags;
    }

    let mut did_output = false;
    response.write(format_args!("{{\n"));

    if (flags & SPACE_PROPERTY_ID) != 0 {
        response.write(format_args!("\t\"id\":{}", space_id.0 as i64));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_UUID) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let uuid = space_manager
            .view
            .find(&space_id)
            .and_then(|view| view.uuid.as_ref())
            .and_then(|uuid| ts_cfstring_copy(uuid.as_ref()));
        response.write(format_args!(
            "\t\"uuid\":\"{}\"",
            uuid.as_deref().unwrap_or("<unknown>")
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_INDEX) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"index\":{}",
            space_manager_mission_control_index(space_id)
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_LABEL) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let space_label = space_manager_get_label_for_space(space_manager, space_id);
        response.write(format_args!(
            "\t\"label\":\"{}\"",
            match &space_label {
                Some(space_label) => space_label.label.as_str(),
                None => "",
            }
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_TYPE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"type\":\"{}\"",
            VIEW_TYPE_STR[layout as usize]
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_DISPLAY) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"display\":{}",
            display_manager_display_id_arrangement(space_display_id(space_id), display_manager)
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_WINDOWS) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let window_list = space_window_list(space_id, true, window_manager).unwrap_or_default();
        let window_count = window_list.len() as i32;

        response.write(format_args!("\t\"windows\":["));
        for index in 0..window_count {
            if index < window_count - 1 {
                response.write(format_args!("{}, ", window_list[index as usize].0 as i32));
            } else {
                response.write(format_args!("{}", window_list[index as usize].0 as i32));
            }
        }
        response.write(format_args!("]"));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_FIRST_WINDOW) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let first_leaf = window_node_find_first_leaf(space_id, ROOT_NODE_ID, space_manager);
        let first_window_id = space_manager
            .view
            .find(&space_id)
            .map_or(WindowId(0), |view| view.node(first_leaf).window_order[0]);
        response.write(format_args!(
            "\t\"first-window\":{}",
            first_window_id.0 as i32
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_LAST_WINDOW) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let last_leaf = window_node_find_last_leaf(space_id, ROOT_NODE_ID, space_manager);
        let last_window_id = space_manager
            .view
            .find(&space_id)
            .map_or(WindowId(0), |view| view.node(last_leaf).window_order[0]);
        response.write(format_args!(
            "\t\"last-window\":{}",
            last_window_id.0 as i32
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_HAS_FOCUS) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"has-focus\":{}",
            json_bool(space_id == space_manager.current_space_id)
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_IS_VISIBLE) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-visible\":{}",
            json_bool(space_is_visible(space_id))
        ));
        did_output = true;
    }

    if (flags & SPACE_PROPERTY_IS_FULLSCREEN) != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        response.write(format_args!(
            "\t\"is-native-fullscreen\":{}",
            json_bool(space_is_fullscreen(space_id))
        ));
    }

    response.write(format_args!("\n}}"));
}
