use crate::display::arrangement::display_manager_display_id_arrangement;
use crate::display::identity::display_uuid;
use crate::display::labels::display_manager_get_label_for_display;
use crate::display::manager::DisplayManager;
use crate::display::spaces::display_space_list;
use crate::ffi::core_foundation::ts_cfstring_copy;
use crate::ffi::core_graphics::CGDisplayBounds;
use crate::handles::DisplayId;
use crate::space::lookup::space_manager_mission_control_index;
use crate::support::json::json_bool;
use crate::support::response::Response;

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
