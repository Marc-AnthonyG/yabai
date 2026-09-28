use crate::display::identity::query_displays_active_for_drawing;
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_spaces_of_display;
use crate::serialise::space::write_space_as_json_object;
use crate::space::manager::{SpaceManager, find_or_create_view_for_space};
use crate::support::handles::{DisplayId, SpaceId, WindowId};
use crate::support::response::Response;
use crate::window::manager::WindowManager;
use crate::window::model::query_every_space_holding_window;

pub(crate) fn write_space_as_json_object_followed_by_newline(
    response: &mut Response,
    space_id: SpaceId,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> bool {
    if find_view_for_query_creating_it_once_space_manager_started(
        space_manager,
        space_id,
        display_manager,
        window_manager,
    )
    .is_none()
    {
        return false;
    }

    write_space_as_json_object(
        response,
        space_manager,
        space_id,
        flags,
        display_manager,
        window_manager,
    );
    response.write(format_args!("\n"));
    true
}

pub(crate) fn write_spaces_of_window_as_json_array(
    response: &mut Response,
    window_id: WindowId,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> bool {
    let space_list = query_every_space_holding_window(window_id);
    if space_list.is_empty() {
        return false;
    }
    let space_count = space_list.len() as i32;

    response.write(format_args!("["));
    for index in 0..space_count {
        let space_id = space_list[index as usize];
        if find_view_for_query_creating_it_once_space_manager_started(
            space_manager,
            space_id,
            display_manager,
            window_manager,
        )
        .is_none()
        {
            continue;
        }

        write_space_as_json_object(
            response,
            space_manager,
            space_id,
            flags,
            display_manager,
            window_manager,
        );
        response.write(format_args!(
            "{}",
            if index < space_count - 1 { ',' } else { ']' }
        ));
    }
    response.write(format_args!("\n"));

    true
}

pub(crate) fn write_spaces_of_display_as_json_array(
    response: &mut Response,
    display_id: DisplayId,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(space_list) = query_spaces_of_display(display_id) else {
        return false;
    };
    let space_count = space_list.len() as i32;

    response.write(format_args!("["));
    for index in 0..space_count {
        let space_id = space_list[index as usize];
        if find_view_for_query_creating_it_once_space_manager_started(
            space_manager,
            space_id,
            display_manager,
            window_manager,
        )
        .is_none()
        {
            continue;
        }

        write_space_as_json_object(
            response,
            space_manager,
            space_id,
            flags,
            display_manager,
            window_manager,
        );
        response.write(format_args!(
            "{}",
            if index < space_count - 1 { ',' } else { ']' }
        ));
    }
    response.write(format_args!("\n"));

    true
}

pub(crate) fn write_spaces_of_every_display_as_json_array(
    response: &mut Response,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> bool {
    let display_list = query_displays_active_for_drawing();
    let display_count = display_list.len() as i32;

    response.write(format_args!("["));
    for index in 0..display_count {
        let Some(space_list) = query_spaces_of_display(display_list[index as usize]) else {
            continue;
        };
        let space_count = space_list.len() as i32;

        for inner_index in 0..space_count {
            let space_id = space_list[inner_index as usize];
            if find_view_for_query_creating_it_once_space_manager_started(
                space_manager,
                space_id,
                display_manager,
                window_manager,
            )
            .is_none()
            {
                continue;
            }

            write_space_as_json_object(
                response,
                space_manager,
                space_id,
                flags,
                display_manager,
                window_manager,
            );
            if inner_index < space_count - 1 {
                response.write(format_args!(","));
            }
        }

        response.write(format_args!(
            "{}",
            if index < display_count - 1 { ',' } else { ']' }
        ));
    }
    response.write(format_args!("\n"));

    true
}

pub(crate) fn find_view_for_query_creating_it_once_space_manager_started(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> Option<SpaceId> {
    if space_manager.did_begin {
        return Some(find_or_create_view_for_space(
            space_manager,
            space_id,
            display_manager,
            window_manager,
        ));
    }
    space_manager.view.find(&space_id).map(|_| space_id)
}
