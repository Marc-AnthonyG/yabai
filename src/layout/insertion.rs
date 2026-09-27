use crate::layout::area::{
    Area, area_a_window_inserted_in_direction_takes_from_node_area, cgrect_from_area,
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

pub(crate) fn area_the_insert_feedback_previews(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
) -> Option<Area> {
    let ratio = window_node_get_ratio(space_id, node_id, space_manager);
    let gap = window_node_get_gap(space_manager, space_id);

    let view = space_manager.view.find(&space_id)?;
    let node = view.find_node(node_id)?;
    let insert_direction_the_view_layout_honours =
        if view.layout == ViewType::Stack && node.insert_direction != 0 {
            STACK
        } else {
            node.insert_direction
        };
    area_a_window_inserted_in_direction_takes_from_node_area(
        insert_direction_the_view_layout_honours,
        node.area,
        ratio,
        gap,
    )
}

pub(crate) fn insert_feedback_show(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) {
    let Some(area_of_the_inserted_window) =
        area_the_insert_feedback_previews(space_id, node_id, space_manager)
    else {
        return;
    };
    let frame_of_the_inserted_window = cgrect_from_area(area_of_the_inserted_window);

    let Some(node) = space_manager
        .view
        .find(&space_id)
        .and_then(|view| view.find_node(node_id))
    else {
        return;
    };
    let node_first_window_id = node.window_order[0];

    if FeedbackWindow::window_id_or_zero(&node.feedback_window) == 0 {
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
        schedule_a_fade_in_step_unless_one_is_already_scheduled(space_manager);
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

pub(crate) fn schedule_a_fade_in_step_unless_one_is_already_scheduled(
    space_manager: &mut SpaceManager,
) {
    if space_manager.insert_feedback_fade_in_step_is_scheduled {
        return;
    }
    space_manager.insert_feedback_fade_in_step_is_scheduled = true;
    schedule_the_next_feedback_window_fade_in_step();
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

#[cfg(test)]
mod tests {
    use super::{
        area_the_insert_feedback_previews,
        clear_every_pending_insertion_point_other_than_the_window,
    };
    use crate::display::manager::DisplayManager;
    use crate::layout::area::Area;
    use crate::layout::settings::{ViewFlag, ViewType};
    use crate::layout::tree::{
        WindowNode, WindowNodeChild, WindowNodeSplit, view_add_window_node_with_insertion_point,
        window_node_split_and_child_placing_a_window_inserted_in_direction,
    };
    use crate::layout::view::View;
    use crate::mouse::drag::{MouseDragState, mouse_drag_state_without_a_drag};
    use crate::space::manager::{
        SpaceManager, space_manager_without_any_view_with_its_initial_settings,
    };
    use crate::support::direction::{DIR_EAST, DIR_NORTH, DIR_SOUTH, DIR_WEST, STACK};
    use crate::support::handles::{NodeId, ROOT_NODE_ID, SpaceId, WindowId};
    use crate::window::manager::window_manager_tracking_nothing_with_its_initial_settings;

    const WINDOW_GAP_OF_EVERY_VIEW: i32 = 11;
    const GLOBAL_SPLIT_RATIO: f32 = 0.62;

    fn area_at(x: f32, y: f32, width: f32, height: f32) -> Area {
        Area {
            x,
            y,
            width,
            height,
        }
    }

    fn x_y_width_height(area: Area) -> (f32, f32, f32, f32) {
        (area.x, area.y, area.width, area.height)
    }

    fn view_holding_one_window_in_its_root(
        space_id: SpaceId,
        layout: ViewType,
        gap_is_enabled: bool,
        window_id: WindowId,
        root_area: Area,
        root_ratio: f32,
    ) -> View {
        let mut root = WindowNode {
            area: root_area,
            ratio: root_ratio,
            window_count: 1,
            ..WindowNode::default()
        };
        root.window_list[0] = window_id;
        root.window_order[0] = window_id;

        View {
            uuid: None,
            space_id,
            nodes: vec![Some(root)],
            free_node_ids: Vec::new(),
            insertion_point: WindowId(0),
            layout,
            split_type: WindowNodeSplit::None,
            top_padding: 0,
            bottom_padding: 0,
            left_padding: 0,
            right_padding: 0,
            window_gap: WINDOW_GAP_OF_EVERY_VIEW,
            auto_balance: 0,
            flags: if gap_is_enabled {
                ViewFlag::ENABLE_GAP.0
            } else {
                0
            },
        }
    }

    fn space_manager_with_views(views: Vec<View>) -> SpaceManager {
        let mut space_manager = space_manager_without_any_view_with_its_initial_settings();
        space_manager.split_ratio = GLOBAL_SPLIT_RATIO;
        for view in views {
            space_manager.view.add(view.space_id, view);
        }
        space_manager
    }

    fn set_the_root_to_insert_in_direction_as_window_insert_does(
        space_manager: &mut SpaceManager,
        space_id: SpaceId,
        insert_direction: i32,
    ) {
        let view = space_manager.view.find_mut(&space_id).unwrap();
        let root = view.node_mut(ROOT_NODE_ID);
        if let Some((split, child)) =
            window_node_split_and_child_placing_a_window_inserted_in_direction(insert_direction)
        {
            root.split = split;
            root.child = child;
        }
        root.insert_direction = insert_direction;
        let root_first_window_id = root.window_order[0];
        view.insertion_point = root_first_window_id;
    }

    fn insert_a_new_window_through_the_insertion_point(
        space_manager: &mut SpaceManager,
        space_id: SpaceId,
        new_window_id: WindowId,
    ) {
        let mut display_manager = DisplayManager::default();
        let mut window_manager = window_manager_tracking_nothing_with_its_initial_settings();
        view_add_window_node_with_insertion_point(
            space_manager,
            space_id,
            new_window_id,
            WindowId(0),
            &mut display_manager,
            &mut window_manager,
        );
    }

    fn area_of_the_leaf_holding(
        space_manager: &SpaceManager,
        space_id: SpaceId,
        window_id: WindowId,
    ) -> Area {
        space_manager
            .view
            .find(&space_id)
            .unwrap()
            .nodes
            .iter()
            .flatten()
            .find(|node| node.window_list[..node.window_count as usize].contains(&window_id))
            .map(|node| node.area)
            .unwrap_or_else(|| panic!("window {} should be in a leaf", window_id.0))
    }

    #[test]
    fn the_preview_is_the_area_the_new_window_then_gets_through_the_insertion_point() {
        let space_id = SpaceId(7);
        let window_in_the_node = WindowId(101);
        let new_window = WindowId(202);

        for insert_direction in [DIR_NORTH, DIR_EAST, DIR_SOUTH, DIR_WEST, STACK] {
            for node_ratio in [0.3f32, 0.9, 0.0, 0.95] {
                for gap_is_enabled in [true, false] {
                    let mut space_manager =
                        space_manager_with_views(vec![view_holding_one_window_in_its_root(
                            space_id,
                            ViewType::Bsp,
                            gap_is_enabled,
                            window_in_the_node,
                            area_at(0.5, 25.25, 1511.0, 943.0),
                            node_ratio,
                        )]);
                    set_the_root_to_insert_in_direction_as_window_insert_does(
                        &mut space_manager,
                        space_id,
                        insert_direction,
                    );

                    let previewed_area = area_the_insert_feedback_previews(
                        space_id,
                        ROOT_NODE_ID,
                        &mut space_manager,
                    );
                    insert_a_new_window_through_the_insertion_point(
                        &mut space_manager,
                        space_id,
                        new_window,
                    );

                    assert_eq!(
                        previewed_area.map(x_y_width_height),
                        Some(x_y_width_height(area_of_the_leaf_holding(
                            &space_manager,
                            space_id,
                            new_window
                        ))),
                        "direction {insert_direction} node ratio {node_ratio} gap {gap_is_enabled}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_view_with_the_stack_layout_previews_the_whole_node_where_the_new_window_is_stacked() {
        let space_id = SpaceId(7);
        let window_in_the_node = WindowId(101);
        let new_window = WindowId(202);
        let root_area = area_at(-1728.0, 38.0, 1727.0, 1079.0);

        for insert_direction in [DIR_NORTH, DIR_EAST, DIR_SOUTH, DIR_WEST] {
            let mut space_manager =
                space_manager_with_views(vec![view_holding_one_window_in_its_root(
                    space_id,
                    ViewType::Stack,
                    true,
                    window_in_the_node,
                    root_area,
                    0.5,
                )]);
            space_manager
                .view
                .find_mut(&space_id)
                .unwrap()
                .node_mut(ROOT_NODE_ID)
                .insert_direction = insert_direction;

            let previewed_area =
                area_the_insert_feedback_previews(space_id, ROOT_NODE_ID, &mut space_manager);
            insert_a_new_window_through_the_insertion_point(
                &mut space_manager,
                space_id,
                new_window,
            );

            assert_eq!(
                previewed_area.map(x_y_width_height),
                Some(x_y_width_height(root_area)),
                "direction {insert_direction}"
            );
            assert_eq!(
                x_y_width_height(area_of_the_leaf_holding(
                    &space_manager,
                    space_id,
                    new_window
                )),
                x_y_width_height(root_area),
                "direction {insert_direction}"
            );
        }
    }

    #[test]
    fn a_node_without_an_insert_direction_previews_nothing() {
        let space_id = SpaceId(7);
        let mut space_manager =
            space_manager_with_views(vec![view_holding_one_window_in_its_root(
                space_id,
                ViewType::Bsp,
                true,
                WindowId(101),
                area_at(0.0, 0.0, 1000.0, 800.0),
                0.5,
            )]);

        assert!(
            area_the_insert_feedback_previews(space_id, ROOT_NODE_ID, &mut space_manager).is_none()
        );
    }

    struct TwoViewsPendingAnInsertion {
        space_manager: SpaceManager,
        first_space_id: SpaceId,
        first_window: WindowId,
        second_space_id: SpaceId,
    }

    fn two_views_each_pending_an_insertion_at_its_only_window() -> TwoViewsPendingAnInsertion {
        let first_space_id = SpaceId(1);
        let first_window = WindowId(11);
        let second_space_id = SpaceId(2);
        let second_window = WindowId(22);
        let mut space_manager = space_manager_with_views(vec![
            view_holding_one_window_in_its_root(
                first_space_id,
                ViewType::Bsp,
                true,
                first_window,
                area_at(0.0, 0.0, 1000.0, 800.0),
                0.5,
            ),
            view_holding_one_window_in_its_root(
                second_space_id,
                ViewType::Bsp,
                true,
                second_window,
                area_at(0.0, 0.0, 1000.0, 800.0),
                0.5,
            ),
        ]);
        set_the_root_to_insert_in_direction_as_window_insert_does(
            &mut space_manager,
            first_space_id,
            DIR_EAST,
        );
        set_the_root_to_insert_in_direction_as_window_insert_does(
            &mut space_manager,
            second_space_id,
            DIR_NORTH,
        );

        TwoViewsPendingAnInsertion {
            space_manager,
            first_space_id,
            first_window,
            second_space_id,
        }
    }

    fn insertion_point_and_root_split_child_and_insert_direction(
        space_manager: &SpaceManager,
        space_id: SpaceId,
    ) -> (WindowId, WindowNodeSplit, WindowNodeChild, i32) {
        let view = space_manager.view.find(&space_id).unwrap();
        let root = view.node(ROOT_NODE_ID);
        (
            view.insertion_point,
            root.split,
            root.child,
            root.insert_direction,
        )
    }

    fn mouse_drag_state_showing_its_preview_on(
        feedback_node: Option<(SpaceId, NodeId)>,
    ) -> MouseDragState {
        MouseDragState {
            feedback_node,
            ..mouse_drag_state_without_a_drag()
        }
    }

    fn clear_every_pending_insertion_point_other_than(
        space_manager: &mut SpaceManager,
        focused_window: WindowId,
        mouse_drag_state: &MouseDragState,
    ) {
        let mut window_manager = window_manager_tracking_nothing_with_its_initial_settings();
        clear_every_pending_insertion_point_other_than_the_window(
            focused_window,
            &mut window_manager,
            space_manager,
            mouse_drag_state,
        );
    }

    #[test]
    fn focusing_a_window_that_is_no_insertion_point_clears_the_insertion_point_of_every_view() {
        let mut views = two_views_each_pending_an_insertion_at_its_only_window();

        clear_every_pending_insertion_point_other_than(
            &mut views.space_manager,
            WindowId(99),
            &mouse_drag_state_showing_its_preview_on(None),
        );

        for space_id in [views.first_space_id, views.second_space_id] {
            assert!(
                insertion_point_and_root_split_child_and_insert_direction(
                    &views.space_manager,
                    space_id
                ) == (WindowId(0), WindowNodeSplit::None, WindowNodeChild::None, 0),
                "space {}",
                space_id.0
            );
        }
    }

    #[test]
    fn focusing_the_window_of_an_insertion_point_keeps_that_one_and_clears_the_others() {
        let mut views = two_views_each_pending_an_insertion_at_its_only_window();

        clear_every_pending_insertion_point_other_than(
            &mut views.space_manager,
            views.first_window,
            &mouse_drag_state_showing_its_preview_on(None),
        );

        assert!(
            insertion_point_and_root_split_child_and_insert_direction(
                &views.space_manager,
                views.first_space_id
            ) == (
                views.first_window,
                WindowNodeSplit::Y,
                WindowNodeChild::Second,
                DIR_EAST
            )
        );
        assert!(
            insertion_point_and_root_split_child_and_insert_direction(
                &views.space_manager,
                views.second_space_id
            ) == (WindowId(0), WindowNodeSplit::None, WindowNodeChild::None, 0)
        );
    }

    #[test]
    fn a_node_showing_the_mouse_drag_preview_keeps_its_insert_direction_when_its_insertion_point_is_cleared()
     {
        let mut views = two_views_each_pending_an_insertion_at_its_only_window();
        let mouse_drag_state =
            mouse_drag_state_showing_its_preview_on(Some((views.first_space_id, ROOT_NODE_ID)));

        clear_every_pending_insertion_point_other_than(
            &mut views.space_manager,
            WindowId(99),
            &mouse_drag_state,
        );

        assert!(
            insertion_point_and_root_split_child_and_insert_direction(
                &views.space_manager,
                views.first_space_id
            ) == (
                WindowId(0),
                WindowNodeSplit::None,
                WindowNodeChild::None,
                DIR_EAST
            )
        );
    }

    #[test]
    fn a_view_without_a_pending_insertion_point_is_left_alone() {
        let space_id = SpaceId(3);
        let mut space_manager =
            space_manager_with_views(vec![view_holding_one_window_in_its_root(
                space_id,
                ViewType::Bsp,
                true,
                WindowId(33),
                area_at(0.0, 0.0, 1000.0, 800.0),
                0.5,
            )]);
        {
            let root = space_manager
                .view
                .find_mut(&space_id)
                .unwrap()
                .node_mut(ROOT_NODE_ID);
            root.split = WindowNodeSplit::X;
            root.child = WindowNodeChild::First;
            root.insert_direction = DIR_SOUTH;
        }

        clear_every_pending_insertion_point_other_than(
            &mut space_manager,
            WindowId(99),
            &mouse_drag_state_showing_its_preview_on(None),
        );

        assert!(
            insertion_point_and_root_split_child_and_insert_direction(&space_manager, space_id)
                == (
                    WindowId(0),
                    WindowNodeSplit::X,
                    WindowNodeChild::First,
                    DIR_SOUTH
                )
        );
    }

    #[test]
    fn an_insertion_point_whose_window_left_the_tree_is_still_cleared() {
        let mut views = two_views_each_pending_an_insertion_at_its_only_window();
        views
            .space_manager
            .view
            .find_mut(&views.first_space_id)
            .unwrap()
            .insertion_point = WindowId(404);

        clear_every_pending_insertion_point_other_than(
            &mut views.space_manager,
            WindowId(99),
            &mouse_drag_state_showing_its_preview_on(None),
        );

        assert_eq!(
            views
                .space_manager
                .view
                .find(&views.first_space_id)
                .unwrap()
                .insertion_point
                .0,
            0
        );
    }
}
