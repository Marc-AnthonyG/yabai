use crate::display::identity::display_manager_active_display_list;
use crate::display::manager::DisplayManager;
use crate::display::spaces::{display_space_id, display_space_list};
use crate::ffi::core_foundation::{CFEqual, SendCFRetained, as_cftype, take_create_rule_result};
use crate::ffi::skylight::SLSSpaceCopyName;
use crate::layout::insertion::WindowInsertionPoint;
use crate::layout::settings::{ViewFlag, ViewType};
use crate::layout::tree::{WindowNodeChild, WindowNodeSplit};
use crate::layout::view::{View, view_create, view_flush, view_update};
use crate::mouse::drag::MouseDragState;
use crate::space::focus::space_manager_active_space;
use crate::space::labels::{SpaceLabel, space_manager_get_label_for_space};
use crate::state::process_wide::CONNECTION;
use crate::support::handles::{DisplayId, SpaceId};
use crate::support::table::Table;
use crate::window::manager::WindowManager;

pub(crate) struct SpaceManager {
    pub(crate) view: Table<SpaceId, View>,
    pub(crate) current_space_id: SpaceId,
    pub(crate) last_space_id: SpaceId,
    pub(crate) did_begin: bool,
    pub(crate) layout: ViewType,
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

pub(crate) fn space_manager_find_view(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) -> SpaceId {
    if space_manager.view.find(&space_id).is_none() {
        view_create(space_id, display_manager, window_manager, space_manager);
    }
    space_id
}

pub(crate) fn space_manager_refresh_view(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find(&space_id) else {
        return;
    };
    if view.layout == ViewType::Float {
        return;
    }

    view_update(space_manager, space_id, display_manager, window_manager);
    view_flush(space_manager, space_id, window_manager);
}

pub(crate) fn space_manager_mark_view_invalid(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let space_id =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    if view.layout == ViewType::Float {
        return;
    }

    view.clear_flag(ViewFlag::IS_VALID);
}

pub(crate) fn space_manager_point_view_handles_at_rekeyed_views(
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

pub(crate) fn space_manager_mark_spaces_invalid_for_display(
    space_manager: &mut SpaceManager,
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let Some(space_list) = display_space_list(display_id) else {
        return;
    };
    let space_count = space_list.len() as i32;

    let space_id = display_space_id(display_id);
    for index in 0..space_count {
        if space_list[index as usize] == space_id {
            space_manager_refresh_view(space_manager, space_id, display_manager, window_manager);
        } else {
            space_manager_mark_view_invalid(
                space_manager,
                space_list[index as usize],
                display_manager,
                window_manager,
            );
        }
    }
}

pub(crate) fn space_manager_mark_spaces_invalid(
    space_manager: &mut SpaceManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let display_list = display_manager_active_display_list();
    let display_count = display_list.len() as i32;

    for index in 0..display_count {
        space_manager_mark_spaces_invalid_for_display(
            space_manager,
            display_list[index as usize],
            display_manager,
            window_manager,
        );
    }
}

pub(crate) fn space_manager_handle_display_add(
    space_manager: &mut SpaceManager,
    display_id: DisplayId,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let Some(space_list) = display_space_list(display_id) else {
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
            take_create_rule_result(SLSSpaceCopyName(*CONNECTION.get().unwrap(), space_id.0))
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

                if let Some(label) = space_manager_get_label_for_space(space_manager, view.space_id)
                {
                    label.space_id = space_id;
                }

                view.space_id = space_id;
                view.uuid = Some(SendCFRetained(uuid.clone()));

                let view_is_kept_by_the_table = space_manager.view.find(&space_id).is_none();
                space_manager.view.add(space_id, view);
                if view_is_kept_by_the_table {
                    space_manager_point_view_handles_at_rekeyed_views(
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

    space_manager.current_space_id = space_manager_active_space(window_manager);
    space_manager.last_space_id = space_manager.current_space_id;
}

pub(crate) fn space_manager_begin(
    space_manager: &mut SpaceManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    space_manager.layout = ViewType::Float;
    space_manager.split_ratio = 0.5f32;
    space_manager.auto_balance = WindowNodeSplit::None as u32;
    space_manager.split_type = WindowNodeSplit::Auto;
    space_manager.window_placement = WindowNodeChild::Second;
    space_manager.window_insertion_point = WindowInsertionPoint::Focused;
    space_manager.window_zoom_persist = true;
    space_manager.labels = Vec::new();
    space_manager.skip_window_focus_animation = false;
    space_manager.view = Table::new(23, hash_view_key);

    let display_list = display_manager_active_display_list();
    let display_count = display_list.len() as i32;

    for index in 0..display_count {
        let Some(space_list) = display_space_list(display_list[index as usize]) else {
            continue;
        };
        let space_count = space_list.len() as i32;

        for inner_index in 0..space_count {
            view_create(
                space_list[inner_index as usize],
                display_manager,
                window_manager,
                space_manager,
            );
        }
    }

    space_manager.current_space_id = space_manager_active_space(window_manager);
    space_manager.last_space_id = space_manager.current_space_id;
    space_manager.did_begin = true;
}

#[cfg(test)]
pub(crate) fn space_manager_without_any_view_with_its_initial_settings() -> SpaceManager {
    SpaceManager {
        view: Table::new(23, hash_view_key),
        current_space_id: SpaceId(0),
        last_space_id: SpaceId(0),
        did_begin: false,
        layout: ViewType::Float,
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
