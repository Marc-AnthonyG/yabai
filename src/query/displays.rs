use crate::display::identity::display_manager_active_display_list;
use crate::display::manager::DisplayManager;
use crate::serialise::display::display_serialize;
use crate::support::response::Response;

pub(crate) fn display_manager_query_displays(
    response: &mut Response,
    flags: u64,
    display_manager: &mut DisplayManager,
) -> bool {
    let display_list = display_manager_active_display_list();
    let count = display_list.len() as i32;

    response.write(format_args!("["));
    for index in 0..count {
        display_serialize(
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
