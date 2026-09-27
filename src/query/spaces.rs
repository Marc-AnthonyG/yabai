use crate::display::identity::display_manager_active_display_list;
use crate::display::manager::DisplayManager;
use crate::display::spaces::display_space_list;
use crate::serialise::space::view_serialize;
use crate::space::manager::{SpaceManager, space_manager_find_view};
use crate::support::handles::{DisplayId, SpaceId, WindowId};
use crate::support::response::Response;
use crate::window::manager::WindowManager;
use crate::window::model::window_space_list;

pub(crate) fn space_manager_query_space(
    response: &mut Response,
    space_id: SpaceId,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> bool {
    if space_manager_query_view(space_manager, space_id, display_manager, window_manager).is_none()
    {
        return false;
    }

    view_serialize(
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

pub(crate) fn space_manager_query_spaces_for_window(
    response: &mut Response,
    window_id: WindowId,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> bool {
    let space_list = window_space_list(window_id);
    if space_list.is_empty() {
        return false;
    }
    let space_count = space_list.len() as i32;

    response.write(format_args!("["));
    for index in 0..space_count {
        let space_id = space_list[index as usize];
        if space_manager_query_view(space_manager, space_id, display_manager, window_manager)
            .is_none()
        {
            continue;
        }

        view_serialize(
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

pub(crate) fn space_manager_query_spaces_for_display(
    response: &mut Response,
    display_id: DisplayId,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> bool {
    let Some(space_list) = display_space_list(display_id) else {
        return false;
    };
    let space_count = space_list.len() as i32;

    response.write(format_args!("["));
    for index in 0..space_count {
        let space_id = space_list[index as usize];
        if space_manager_query_view(space_manager, space_id, display_manager, window_manager)
            .is_none()
        {
            continue;
        }

        view_serialize(
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

pub(crate) fn space_manager_query_spaces_for_displays(
    response: &mut Response,
    flags: u64,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> bool {
    let display_list = display_manager_active_display_list();
    let display_count = display_list.len() as i32;

    response.write(format_args!("["));
    for index in 0..display_count {
        let Some(space_list) = display_space_list(display_list[index as usize]) else {
            continue;
        };
        let space_count = space_list.len() as i32;

        for inner_index in 0..space_count {
            let space_id = space_list[inner_index as usize];
            if space_manager_query_view(space_manager, space_id, display_manager, window_manager)
                .is_none()
            {
                continue;
            }

            view_serialize(
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

pub(crate) fn space_manager_query_view(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> Option<SpaceId> {
    if space_manager.did_begin {
        return Some(space_manager_find_view(
            space_manager,
            space_id,
            display_manager,
            window_manager,
        ));
    }
    space_manager.view.find(&space_id).map(|_| space_id)
}
