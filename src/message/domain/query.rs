use crate::command::query::{
    DisplaysQueryFilter, QueryCommand, SpacesQueryFilter, WindowsQueryFilter,
};
use crate::display::identity::query_displays_active_for_drawing;
use crate::display::manager::DisplayManager;
use crate::message::selector_resolution::{
    resolve_display_selector_or_the_display_showing_the_active_menu_bar,
    resolve_space_selector_or_the_focused_space, resolve_window_selector_or_the_focused_window,
};
use crate::mouse::drag::MouseDragState;
use crate::query::spaces::{
    find_view_for_query_creating_it_once_space_manager_started, spaces_holding_window_with_a_view,
    spaces_of_display_with_a_view, spaces_of_every_display_with_a_view,
};
use crate::query::windows::{
    snapshots_of_the_windows_on_display, snapshots_of_the_windows_on_every_display,
    snapshots_of_the_windows_on_spaces,
};
use crate::serialise::display::{DisplaySnapshot, snapshot_of_display};
use crate::serialise::field_selection::pretty_json_keeping_only_the_selected_fields;
use crate::serialise::space::{SpaceSnapshot, snapshot_of_the_space_of_a_view};
use crate::serialise::window::{WindowSnapshot, snapshot_of_tracked_window};
use crate::space::managed_space::query_display_holding_space;
use crate::space::manager::SpaceManager;
use crate::support::handles::{DisplayId, SpaceId};
use crate::window::manager::WindowManager;
use crate::window::model::query_display_holding_window;

pub(crate) fn run_query_command(
    command: QueryCommand,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &MouseDragState,
) -> Result<String, String> {
    let json = match command {
        QueryCommand::Displays {
            filter,
            field_selection,
        } => pretty_json_keeping_only_the_selected_fields(
            &snapshots_of_the_displays_the_filter_covers(
                &filter,
                display_manager,
                window_manager,
                space_manager,
            )?,
            &field_selection.fields,
        ),
        QueryCommand::Spaces {
            filter,
            field_selection,
        } => pretty_json_keeping_only_the_selected_fields(
            &snapshots_of_the_spaces_the_filter_covers(
                &filter,
                display_manager,
                window_manager,
                space_manager,
            )?,
            &field_selection.fields,
        ),
        QueryCommand::Windows {
            filter,
            field_selection,
        } => pretty_json_keeping_only_the_selected_fields(
            &snapshots_of_the_windows_the_filter_covers(
                &filter,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            )?,
            &field_selection.fields,
        ),
        QueryCommand::Display {
            display,
            field_selection,
        } => {
            let display_id = resolve_display_selector_or_the_display_showing_the_active_menu_bar(
                display.as_ref(),
                display_manager,
            )?;
            pretty_json_keeping_only_the_selected_fields(
                &snapshot_of_display(display_id, display_manager),
                &field_selection.fields,
            )
        }
        QueryCommand::Space {
            space,
            field_selection,
        } => {
            let space_id = resolve_space_selector_or_the_focused_space(
                space.as_ref(),
                window_manager,
                space_manager,
            )?;
            pretty_json_keeping_only_the_selected_fields(
                &snapshot_of_one_space(space_id, display_manager, window_manager, space_manager)?,
                &field_selection.fields,
            )
        }
        QueryCommand::Window {
            window,
            field_selection,
        } => {
            let window_id = resolve_window_selector_or_the_focused_window(
                window.as_ref(),
                display_manager,
                window_manager,
                space_manager,
            )?;
            let snapshot = snapshot_of_tracked_window(
                window_id,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            )
            .ok_or_else(|| String::from("could not retrieve window details."))?;
            pretty_json_keeping_only_the_selected_fields(&snapshot, &field_selection.fields)
        }
    };
    Ok(json + "\n")
}

fn snapshots_of_the_displays_the_filter_covers(
    filter: &DisplaysQueryFilter,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Result<Vec<DisplaySnapshot>, String> {
    let display_list: Vec<DisplayId> = match (&filter.space, &filter.window) {
        (Some(space), _) => {
            let space_id = resolve_space_selector_or_the_focused_space(
                space.as_ref(),
                window_manager,
                space_manager,
            )?;
            vec![query_display_holding_space(space_id)]
        }
        (_, Some(window)) => {
            let window_id = resolve_window_selector_or_the_focused_window(
                window.as_ref(),
                display_manager,
                window_manager,
                space_manager,
            )?;
            vec![query_display_holding_window(window_id)]
        }
        (None, None) => query_displays_active_for_drawing(),
    };

    Ok(display_list
        .into_iter()
        .map(|display_id| snapshot_of_display(display_id, display_manager))
        .collect())
}

fn snapshots_of_the_spaces_the_filter_covers(
    filter: &SpacesQueryFilter,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Result<Vec<SpaceSnapshot>, String> {
    let space_list = match (&filter.display, &filter.window) {
        (Some(display), _) => {
            let display_id = resolve_display_selector_or_the_display_showing_the_active_menu_bar(
                display.as_ref(),
                display_manager,
            )?;
            spaces_of_display_with_a_view(
                display_id,
                display_manager,
                window_manager,
                space_manager,
            )
            .ok_or_else(|| String::from("could not retrieve spaces for display."))?
        }
        (_, Some(window)) => {
            let window_id = resolve_window_selector_or_the_focused_window(
                window.as_ref(),
                display_manager,
                window_manager,
                space_manager,
            )?;
            spaces_holding_window_with_a_view(
                window_id,
                display_manager,
                window_manager,
                space_manager,
            )
        }
        (None, None) => {
            spaces_of_every_display_with_a_view(display_manager, window_manager, space_manager)
        }
    };

    Ok(space_list
        .into_iter()
        .filter_map(|space_id| {
            snapshot_of_the_space_of_a_view(
                space_id,
                display_manager,
                window_manager,
                space_manager,
            )
        })
        .collect())
}

fn snapshot_of_one_space(
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> Result<SpaceSnapshot, String> {
    find_view_for_query_creating_it_once_space_manager_started(
        space_manager,
        space_id,
        display_manager,
        window_manager,
    )
    .and_then(|space_id| {
        snapshot_of_the_space_of_a_view(space_id, display_manager, window_manager, space_manager)
    })
    .ok_or_else(|| String::from("could not retrieve space details."))
}

fn snapshots_of_the_windows_the_filter_covers(
    filter: &WindowsQueryFilter,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &MouseDragState,
) -> Result<Vec<WindowSnapshot>, String> {
    Ok(match (&filter.display, &filter.space) {
        (Some(display), _) => {
            let display_id = resolve_display_selector_or_the_display_showing_the_active_menu_bar(
                display.as_ref(),
                display_manager,
            )?;
            snapshots_of_the_windows_on_display(
                display_id,
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            )
        }
        (_, Some(space)) => {
            let space_id = resolve_space_selector_or_the_focused_space(
                space.as_ref(),
                window_manager,
                space_manager,
            )?;
            snapshots_of_the_windows_on_spaces(
                &[space_id],
                display_manager,
                window_manager,
                space_manager,
                mouse_drag_state,
            )
        }
        (None, None) => snapshots_of_the_windows_on_every_display(
            display_manager,
            window_manager,
            space_manager,
            mouse_drag_state,
        ),
    })
}
