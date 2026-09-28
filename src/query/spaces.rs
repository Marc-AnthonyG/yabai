use crate::display::identity::query_displays_active_for_drawing;
use crate::display::manager::DisplayManager;
use crate::display::spaces::query_spaces_of_display;
use crate::space::manager::{SpaceManager, find_or_create_view_for_space};
use crate::support::handles::{DisplayId, SpaceId, WindowId};
use crate::window::manager::WindowManager;
use crate::window::model::query_every_space_holding_window;

pub(crate) fn spaces_of_display_with_a_view(
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Option<Vec<SpaceId>> {
    let space_list = query_spaces_of_display(display_id)?;
    Some(keep_the_spaces_with_a_view(
        space_list,
        display_manager,
        window_manager,
        space_manager,
    ))
}

pub(crate) fn spaces_of_every_display_with_a_view(
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Vec<SpaceId> {
    let space_list = query_displays_active_for_drawing()
        .into_iter()
        .filter_map(query_spaces_of_display)
        .flatten()
        .collect();
    keep_the_spaces_with_a_view(space_list, display_manager, window_manager, space_manager)
}

pub(crate) fn spaces_holding_window_with_a_view(
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Vec<SpaceId> {
    keep_the_spaces_with_a_view(
        query_every_space_holding_window(window_id),
        display_manager,
        window_manager,
        space_manager,
    )
}

fn keep_the_spaces_with_a_view(
    space_list: Vec<SpaceId>,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Vec<SpaceId> {
    space_list
        .into_iter()
        .filter_map(|space_id| {
            find_view_for_query_creating_it_once_space_manager_started(
                space_manager,
                space_id,
                display_manager,
                window_manager,
            )
        })
        .collect()
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
    space_manager.view.get(&space_id).map(|_| space_id)
}
