use crate::display::arrangement::query_arrangement_index_of_display;
use crate::display::identity::copy_uuid_of_display;
use crate::display::labels::label_of_display;
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_spaces_of_display;
use crate::ffi::core_foundation::cfstring_to_string;
use crate::ffi::core_graphics::CGDisplayBounds;
use crate::space::lookup::query_mission_control_index_of_space;
use crate::support::handles::DisplayId;
use crate::support::json::json_literal_for_boolean;
use crate::support::printf_float_format::format_float_with_decimals_as_printf_does;
use crate::support::response::Response;

macro_rules! with_every_display_property {
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

macro_rules! define_display_property_bits_and_names {
    ($(($name:literal, $identifier:ident, $value:literal)),* $(,)?) => {
        $(pub(crate) const $identifier: u64 = $value;)*

        pub(crate) static DISPLAY_PROPERTY_SELECTION_BITS: [u64; 7] = [$($value),*];

        pub(crate) static DISPLAY_PROPERTY_NAMES: [&str; 7] = [$($name),*];
    };
}

with_every_display_property!(define_display_property_bits_and_names);

pub(crate) fn write_display_as_json_object(
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
        let uuid_ref = copy_uuid_of_display(display_id);
        if let Some(uuid_ref) = uuid_ref {
            uuid = cfstring_to_string(uuid_ref.as_ref());
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
            query_arrangement_index_of_display(display_id, display_manager)
        ));
        did_output = true;
    }

    if flags & DISPLAY_PROPERTY_LABEL != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let display_label = label_of_display(display_manager, display_id);
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
            "\t\"frame\":{{\n\t\t\"x\":{},\n\t\t\"y\":{},\n\t\t\"w\":{},\n\t\t\"h\":{}\n\t}}",
            format_float_with_decimals_as_printf_does(frame.origin.x, 4),
            format_float_with_decimals_as_printf_does(frame.origin.y, 4),
            format_float_with_decimals_as_printf_does(frame.size.width, 4),
            format_float_with_decimals_as_printf_does(frame.size.height, 4)
        ));
        did_output = true;
    }

    if flags & DISPLAY_PROPERTY_SPACES != 0 {
        if did_output {
            response.write(format_args!(",\n"));
        }

        let space_list = query_spaces_of_display(display_id);

        response.write(format_args!("\t\"spaces\":["));
        if let Some(space_list) = space_list {
            let count = space_list.len() as i32;
            if let Some(first_space_id) = space_list.first() {
                let first_mission_control_index =
                    query_mission_control_index_of_space(*first_space_id);
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
            json_literal_for_boolean(display_id == display_manager.current_display_id)
        ));
    }

    response.write(format_args!("\n}}"));
}
