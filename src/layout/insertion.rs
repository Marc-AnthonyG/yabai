use crate::layout::area::{
    area_a_window_inserted_in_direction_takes_from_node_area, cgrect_from_area,
};
use crate::layout::feedback_window::{
    FeedbackWindow, feedback_window_advance_fade_in,
    feedback_window_create_transparent_above_window, feedback_window_draw_ghost_of_frame,
    feedback_window_is_fading_in, schedule_the_next_feedback_window_fade_in_step,
};
use crate::layout::settings::{ViewType, window_node_get_gap, window_node_get_ratio};
use crate::layout::tree::{WindowNodeChild, WindowNodeSplit, view_find_window_node};
use crate::mouse::drag::MouseDragState;
use crate::notifications::window::update_window_notifications;
use crate::space::manager::SpaceManager;
use crate::support::direction::STACK;
use crate::support::handles::{NodeId, SpaceId, WindowId};
use crate::support::macos_version::{workspace_is_macos_sequoia, workspace_is_macos_tahoe};
use crate::window::manager::WindowManager;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub(crate) enum WindowInsertionPoint {
    Focused = 0,
    First = 1,
    Last = 2,
}

pub(crate) static WINDOW_INSERTION_POINT_STR: [&str; 3] = ["focused", "first", "last"];

pub(crate) fn insert_feedback_show(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) {
    let ratio = window_node_get_ratio(space_id, node_id, space_manager);
    let gap = window_node_get_gap(space_manager, space_id);

    let Some(view) = space_manager.view.find(&space_id) else {
        return;
    };
    let Some(node) = view.find_node(node_id) else {
        return;
    };
    let insert_direction_the_view_layout_honours =
        if view.layout == ViewType::Stack && node.insert_direction != 0 {
            STACK
        } else {
            node.insert_direction
        };
    let Some(area_of_the_inserted_window) =
        area_a_window_inserted_in_direction_takes_from_node_area(
            insert_direction_the_view_layout_honours,
            node.area,
            ratio,
            gap,
        )
    else {
        return;
    };
    let frame_of_the_inserted_window = cgrect_from_area(area_of_the_inserted_window);
    let node_first_window_id = node.window_order[0];

    if FeedbackWindow::window_id_or_zero(&node.feedback_window) == 0 {
        let a_fade_in_is_already_stepping = a_feedback_window_is_still_fading_in(space_manager);
        let feedback_window = feedback_window_create_transparent_above_window(
            frame_of_the_inserted_window,
            node_first_window_id,
        );
        let Some(node) = space_manager
            .view
            .find_mut(&space_id)
            .and_then(|view| view.find_node_mut(node_id))
        else {
            return;
        };
        node.feedback_window = Some(feedback_window);
        if !a_fade_in_is_already_stepping {
            schedule_the_next_feedback_window_fade_in_step();
        }
        window_manager
            .insert_feedback
            .add(node_first_window_id, (space_id, node_id));
        if !workspace_is_macos_sequoia() && !workspace_is_macos_tahoe() {
            update_window_notifications(window_manager, space_manager);
        }
    }

    let Some(feedback_window) = space_manager
        .view
        .find(&space_id)
        .and_then(|view| view.find_node(node_id))
        .and_then(|node| node.feedback_window.as_ref())
    else {
        return;
    };
    feedback_window_draw_ghost_of_frame(
        feedback_window,
        frame_of_the_inserted_window,
        window_manager.insert_feedback_color,
    );
}

pub(crate) fn insert_feedback_destroy(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) {
    let Some(view) = space_manager.view.find_mut(&space_id) else {
        return;
    };
    let Some(node) = view.find_node_mut(node_id) else {
        return;
    };

    if FeedbackWindow::window_id_or_zero(&node.feedback_window) != 0 {
        window_manager.insert_feedback.remove(&node.window_order[0]);

        if !workspace_is_macos_sequoia() && !workspace_is_macos_tahoe() {
            update_window_notifications(window_manager, space_manager);
        }

        let Some(node) = space_manager
            .view
            .find_mut(&space_id)
            .and_then(|view| view.find_node_mut(node_id))
        else {
            return;
        };
        drop(node.feedback_window.take());
    }
}

pub(crate) fn a_feedback_window_is_still_fading_in(space_manager: &SpaceManager) -> bool {
    space_manager.view.values().any(|view| {
        view.nodes.iter().flatten().any(|node| {
            node.feedback_window
                .as_ref()
                .is_some_and(feedback_window_is_fading_in)
        })
    })
}

pub(crate) fn insert_feedback_advance_every_fade_in(space_manager: &mut SpaceManager) {
    for view in space_manager.view.values_mut() {
        for node in view.nodes.iter_mut().flatten() {
            if let Some(feedback_window) = node.feedback_window.as_mut() {
                feedback_window_advance_fade_in(feedback_window);
            }
        }
    }
}

pub(crate) fn clear_every_pending_insertion_point_other_than_the_window(
    window_id: WindowId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &MouseDragState,
) {
    let space_ids_of_views_pending_an_insertion_at_another_window: Vec<SpaceId> = space_manager
        .view
        .iter()
        .filter(|(_, view)| view.insertion_point.0 != 0 && view.insertion_point != window_id)
        .map(|(space_id, _)| *space_id)
        .collect();

    for space_id in space_ids_of_views_pending_an_insertion_at_another_window {
        clear_the_pending_insertion_point_of_view(
            space_id,
            window_manager,
            space_manager,
            mouse_drag_state,
        );
    }
}

fn clear_the_pending_insertion_point_of_view(
    space_id: SpaceId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &MouseDragState,
) {
    let Some(insertion_point) = space_manager
        .view
        .find(&space_id)
        .map(|view| view.insertion_point)
    else {
        return;
    };

    if let Some(insert_node_id) = view_find_window_node(space_manager, space_id, insertion_point) {
        let the_mouse_drag_preview_is_shown_on_the_insert_node =
            mouse_drag_state.feedback_node == Some((space_id, insert_node_id));
        if !the_mouse_drag_preview_is_shown_on_the_insert_node {
            insert_feedback_destroy(space_id, insert_node_id, window_manager, space_manager);
        }

        if let Some(insert_node) = space_manager
            .view
            .find_mut(&space_id)
            .and_then(|view| view.find_node_mut(insert_node_id))
        {
            insert_node.split = WindowNodeSplit::None;
            insert_node.child = WindowNodeChild::None;
            if !the_mouse_drag_preview_is_shown_on_the_insert_node {
                insert_node.insert_direction = 0;
            }
        }
    }

    if let Some(view) = space_manager.view.find_mut(&space_id) {
        view.insertion_point = WindowId(0);
    }
}
