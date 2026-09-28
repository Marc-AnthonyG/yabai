use crate::display::identity::query_displays_active_for_drawing;
use crate::display::manager::DisplayManager;
use crate::display::spaces::{query_current_space_of_display, query_spaces_of_display};
use crate::ffi::core_foundation::{
    CFEqual, CFRetainedAssumedSendAndSync, as_cftype, take_create_rule_result,
};
use crate::ffi::skylight::SLSSpaceCopyName;
use crate::layout::insertion::WindowInsertionPoint;
use crate::layout::settings::{ViewFlag, ViewLayout};
use crate::layout::tree::{WindowNodeChild, WindowNodeSplit};
use crate::layout::view::{
    View, create_view_for_space_from_global_settings,
    move_view_windows_into_their_areas_or_defer_until_space_is_visible,
    recompute_view_areas_from_display_bounds_and_padding,
};
use crate::mouse::drag::MouseDragState;
use crate::space::focus::query_current_space_of_the_focused_display;
use crate::space::labels::{SpaceLabel, label_of_space};
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::{DisplayId, SpaceId};
use crate::support::table::Table;
use crate::window::manager::WindowManager;

pub(crate) struct SpaceManager {
    pub(crate) view: Table<SpaceId, View>,
    pub(crate) current_space_id: SpaceId,
    pub(crate) last_space_id: SpaceId,
    pub(crate) did_begin: bool,
    pub(crate) layout: ViewLayout,
    pub(crate) top_padding: i32,
    pub(crate) bottom_padding: i32,
    pub(crate) left_padding: i32,
    pub(crate) right_padding: i32,
    pub(crate) window_gap: i32,
    pub(crate) split_ratio: f32,
    pub(crate) split_type: WindowNodeSplit,
    pub(crate) window_placement: WindowNodeChild,
    pub(crate) window_insertion_point: WindowInsertionPoint,
    pub(crate) window_zoom_persist: bool,
    pub(crate) auto_balance: u32,
    pub(crate) labels: Vec<SpaceLabel>,
    pub(crate) skip_window_focus_animation: bool,
    pub(crate) insert_feedback_fade_in_step_is_scheduled: bool,
}

pub(crate) fn hash_view_key(key: &SpaceId) -> u64 {
    key.0
}

pub(crate) fn find_or_create_view_for_space(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> SpaceId {
    if space_manager.view.find(&space_id).is_none() {
        create_view_for_space_from_global_settings(
            space_id,
            display_manager,
            window_manager,
            space_manager,
        );
    }
    space_id
}

pub(crate) fn recompute_view_areas_and_move_its_windows_into_them(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let space_id =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return;
    };
    if view.layout == ViewLayout::Float {
        return;
    }

    recompute_view_areas_from_display_bounds_and_padding(
        space_manager,
        space_id,
        display_manager,
        window_manager,
    );
    move_view_windows_into_their_areas_or_defer_until_space_is_visible(
        space_manager,
        space_id,
        window_manager,
    );
}

pub(crate) fn mark_view_areas_out_of_date(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let space_id =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    if view.layout == ViewLayout::Float {
        return;
    }

    view.clear_flag(ViewFlag::AREAS_ARE_UP_TO_DATE);
}

pub(crate) fn point_view_handles_at_rekeyed_views(
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
    space_id_of_view_after_rekeying: impl Fn(SpaceId) -> SpaceId,
) {
    for managed_view_space_id in window_manager.managed_window.values_mut() {
        *managed_view_space_id = space_id_of_view_after_rekeying(*managed_view_space_id);
    }

    for (feedback_node_space_id, _) in window_manager.insert_feedback.values_mut() {
        *feedback_node_space_id = space_id_of_view_after_rekeying(*feedback_node_space_id);
    }

    if let Some((feedback_node_space_id, _)) = mouse_drag_state.feedback_node.as_mut() {
        *feedback_node_space_id = space_id_of_view_after_rekeying(*feedback_node_space_id);
    }
}

pub(crate) fn recompute_current_view_of_display_and_mark_its_other_views_out_of_date(
    space_manager: &mut SpaceManager,
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let Some(space_list) = query_spaces_of_display(display_id) else {
        return;
    };
    let space_count = space_list.len() as i32;

    let space_id = query_current_space_of_display(display_id);
    for index in 0..space_count {
        if space_list[index as usize] == space_id {
            recompute_view_areas_and_move_its_windows_into_them(
                space_manager,
                space_id,
                display_manager,
                window_manager,
            );
        } else {
            mark_view_areas_out_of_date(
                space_manager,
                space_list[index as usize],
                display_manager,
                window_manager,
            );
        }
    }
}

pub(crate) fn recompute_current_view_of_every_display_and_mark_the_other_views_out_of_date(
    space_manager: &mut SpaceManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let display_list = query_displays_active_for_drawing();
    let display_count = display_list.len() as i32;

    for index in 0..display_count {
        recompute_current_view_of_display_and_mark_its_other_views_out_of_date(
            space_manager,
            display_list[index as usize],
            display_manager,
            window_manager,
        );
    }
}

pub(crate) fn reattach_views_to_spaces_of_added_display_by_uuid(
    space_manager: &mut SpaceManager,
    display_id: DisplayId,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let Some(space_list) = query_spaces_of_display(display_id) else {
        return;
    };
    let space_count = space_list.len() as i32;

    let mut view_list: Vec<Option<SpaceId>> = space_manager
        .view
        .keys_in_bucket_order()
        .into_iter()
        .map(Some)
        .collect();
    let list_count = view_list.len() as i32;

    for index in 0..space_count {
        let space_id = space_list[index as usize];
        let Some(uuid) = (unsafe {
            take_create_rule_result(SLSSpaceCopyName(
                *SKYLIGHT_CONNECTION_ID.get().unwrap(),
                space_id.0,
            ))
        }) else {
            continue;
        };

        for inner_index in 0..list_count {
            let Some(view_space_id) = view_list[inner_index as usize] else {
                continue;
            };
            let Some(view_uuid) = space_manager
                .view
                .find(&view_space_id)
                .and_then(|view| view.uuid.as_ref())
            else {
                continue;
            };

            if CFEqual(Some(as_cftype(view_uuid.as_ref())), Some(as_cftype(&*uuid))) {
                view_list[inner_index as usize] = None;

                let Some(mut view) = space_manager.view.remove(&view_space_id) else {
                    break;
                };
                drop(view.uuid.take());

                if let Some(label) = label_of_space(space_manager, view.space_id) {
                    label.space_id = space_id;
                }

                view.space_id = space_id;
                view.uuid = Some(CFRetainedAssumedSendAndSync(uuid.clone()));

                let view_is_kept_by_the_table = space_manager.view.find(&space_id).is_none();
                space_manager
                    .view
                    .add_unless_key_already_present(space_id, view);
                if view_is_kept_by_the_table {
                    point_view_handles_at_rekeyed_views(
                        window_manager,
                        mouse_drag_state,
                        |handle_space_id| {
                            if handle_space_id == view_space_id {
                                space_id
                            } else {
                                handle_space_id
                            }
                        },
                    );
                }
                break;
            }
        }

        drop(uuid);
    }

    space_manager.current_space_id = query_current_space_of_the_focused_display(window_manager);
    space_manager.last_space_id = space_manager.current_space_id;
}

pub(crate) fn start_space_manager_creating_a_view_for_every_space(
    space_manager: &mut SpaceManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    space_manager.layout = ViewLayout::Float;
    space_manager.split_ratio = 0.5f32;
    space_manager.auto_balance = WindowNodeSplit::None as u32;
    space_manager.split_type = WindowNodeSplit::Auto;
    space_manager.window_placement = WindowNodeChild::Second;
    space_manager.window_insertion_point = WindowInsertionPoint::Focused;
    space_manager.window_zoom_persist = true;
    space_manager.labels = Vec::new();
    space_manager.skip_window_focus_animation = false;
    space_manager.view = Table::new(23, hash_view_key);

    let display_list = query_displays_active_for_drawing();
    let display_count = display_list.len() as i32;

    for index in 0..display_count {
        let Some(space_list) = query_spaces_of_display(display_list[index as usize]) else {
            continue;
        };
        let space_count = space_list.len() as i32;

        for inner_index in 0..space_count {
            create_view_for_space_from_global_settings(
                space_list[inner_index as usize],
                display_manager,
                window_manager,
                space_manager,
            );
        }
    }

    space_manager.current_space_id = query_current_space_of_the_focused_display(window_manager);
    space_manager.last_space_id = space_manager.current_space_id;
    space_manager.did_begin = true;
}

#[cfg(test)]
pub(crate) fn create_space_manager_without_any_view_with_its_initial_settings() -> SpaceManager {
    SpaceManager {
        view: Table::new(23, hash_view_key),
        current_space_id: SpaceId(0),
        last_space_id: SpaceId(0),
        did_begin: false,
        layout: ViewLayout::Float,
        top_padding: 0,
        bottom_padding: 0,
        left_padding: 0,
        right_padding: 0,
        window_gap: 0,
        split_ratio: 0.5f32,
        split_type: WindowNodeSplit::Auto,
        window_placement: WindowNodeChild::Second,
        window_insertion_point: WindowInsertionPoint::Focused,
        window_zoom_persist: true,
        auto_balance: WindowNodeSplit::None as u32,
        labels: Vec::new(),
        skip_window_focus_animation: false,
        insert_feedback_fade_in_step_is_scheduled: false,
    }
}
