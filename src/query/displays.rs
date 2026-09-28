use crate::display::identity::query_displays_active_for_drawing;
use crate::display::manager::DisplayManager;
use crate::serialise::display::write_display_as_json_object;
use crate::support::response::Response;

pub(crate) fn write_every_display_as_json_array(
    response: &mut Response,
    flags: u64,
    display_manager: &mut DisplayManager,
) -> bool {
    let display_list = query_displays_active_for_drawing();
    let count = display_list.len() as i32;

    response.write(format_args!("["));
    for index in 0..count {
        write_display_as_json_object(
            response,
            display_list[index as usize],
            flags,
            display_manager,
        );
        response.write(format_args!(
            "{}",
            if index < count - 1 { ',' } else { ']' }
        ));
    }
    response.write(format_args!("\n"));

    true
}
