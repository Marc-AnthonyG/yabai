use crate::ffi::core_foundation::{CGPoint, CGRect, cfstring_to_string};
use crate::ffi::core_graphics::CGRectContainsPoint;
use crate::layout::area::cgrect_from_area;
use crate::layout::group::is_node_a_group;
use crate::layout::group_area::{area_of_the_header_of_a_tile, area_of_tile_including_its_zoom};
use crate::layout::group_header_window::{
    GroupHeaderTab, create_group_header_window_on_space, draw_tabs_in_group_header_window,
    hide_group_header_window, order_group_header_window_right_above_window,
};
use crate::layout::settings::ViewLayout;
use crate::layout::view::View;
use crate::mouse::tap::GROUP_HEADER_FRAMES_WHOSE_CLICKS_THE_TAP_SWALLOWS;
use crate::space::managed_space::is_space_visible_on_its_display;
use crate::space::manager::SpaceManager;
use crate::support::handles::{NodeId, SpaceId, WindowId};
use crate::window::manager::{WindowManager, space_managing_window};

struct PlannedGroupHeader {
    node_id: NodeId,
    frame: CGRect,
    tabs: Vec<GroupHeaderTab>,
    front_window: WindowId,
}

pub(crate) fn refresh_the_group_headers_of_view(
    space_id: SpaceId,
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
) {
    if !is_space_visible_on_its_display(space_id) {
        return;
    }
    let Some(view) = space_manager.view.get_mut(&space_id) else {
        return;
    };

    let mut planned_group_headers = plan_the_group_headers_of_view(view, window_manager);
    for (index, slot) in view.nodes.iter_mut().enumerate() {
        let Some(node) = slot else {
            continue;
        };
        let Some(position) = planned_group_headers
            .iter()
            .position(|planned| planned.node_id.0 as usize == index)
        else {
            node.group_header = None;
            continue;
        };

        let planned = planned_group_headers.swap_remove(position);
        let header_window = node
            .group_header
            .get_or_insert_with(|| create_group_header_window_on_space(planned.frame, space_id));
        draw_tabs_in_group_header_window(
            header_window,
            planned.frame,
            &planned.tabs,
            &window_manager.group_header_style,
        );
        if window_manager.group_headers_are_hidden_during_mission_control {
            hide_group_header_window(header_window);
        } else {
            order_group_header_window_right_above_window(header_window, planned.front_window);
        }
    }

    publish_the_frames_of_the_visible_group_headers_to_the_mouse_tap(space_manager, window_manager);
}

fn publish_the_frames_of_the_visible_group_headers_to_the_mouse_tap(
    space_manager: &SpaceManager,
    window_manager: &WindowManager,
) {
    let mut visible_header_frames = Vec::new();
    if !window_manager.group_headers_are_hidden_during_mission_control {
        for space_id in space_manager.view.keys().copied().collect::<Vec<_>>() {
            if !is_space_visible_on_its_display(space_id) {
                continue;
            }
            let Some(view) = space_manager.view.get(&space_id) else {
                continue;
            };
            for node in view.nodes.iter().flatten() {
                if let Some(header_window) = &node.group_header {
                    visible_header_frames.push(header_window.frame);
                }
            }
        }
    }
    if let Ok(mut header_frames) = GROUP_HEADER_FRAMES_WHOSE_CLICKS_THE_TAP_SWALLOWS.lock() {
        *header_frames = visible_header_frames;
    }
}

pub(crate) fn refresh_the_group_headers_of_the_view_managing_window(
    window_id: WindowId,
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
) {
    if let Some(space_id) = space_managing_window(window_manager, window_id) {
        refresh_the_group_headers_of_view(space_id, space_manager, window_manager);
    }
}

pub(crate) fn refresh_the_group_headers_of_every_view(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
) {
    for space_id in space_manager.view.keys().copied().collect::<Vec<_>>() {
        refresh_the_group_headers_of_view(space_id, space_manager, window_manager);
    }
}

pub(crate) fn keep_the_group_header_right_above_the_front_window_of_the_group_holding(
    window_id: WindowId,
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
) {
    if window_manager.group_headers_are_hidden_during_mission_control {
        return;
    }
    let Some(space_id) = space_managing_window(window_manager, window_id) else {
        return;
    };
    let Some(view) = space_manager.view.get(&space_id) else {
        return;
    };
    for node in view.nodes.iter().flatten() {
        let holds_the_window = node.window_list[..node.window_count as usize].contains(&window_id);
        if holds_the_window && let Some(header_window) = &node.group_header {
            order_group_header_window_right_above_window(header_window, node.window_order[0]);
            return;
        }
    }
}

pub(crate) fn hide_the_group_headers_of_every_view(
    space_manager: &mut SpaceManager,
    window_manager: &WindowManager,
) {
    for space_id in space_manager.view.keys().copied().collect::<Vec<_>>() {
        let Some(view) = space_manager.view.get(&space_id) else {
            continue;
        };
        for node in view.nodes.iter().flatten() {
            if let Some(header_window) = &node.group_header {
                hide_group_header_window(header_window);
            }
        }
    }
    publish_the_frames_of_the_visible_group_headers_to_the_mouse_tap(space_manager, window_manager);
}

pub(crate) fn front_window_of_the_group_whose_header_holds_point(
    point: CGPoint,
    space_manager: &SpaceManager,
) -> Option<WindowId> {
    for space_id in space_manager.view.keys().copied().collect::<Vec<_>>() {
        if !is_space_visible_on_its_display(space_id) {
            continue;
        }
        let Some(view) = space_manager.view.get(&space_id) else {
            continue;
        };
        for node in view.nodes.iter().flatten() {
            if let Some(header_window) = &node.group_header
                && CGRectContainsPoint(header_window.frame, point)
            {
                return Some(node.window_order[0]);
            }
        }
    }
    None
}

pub(crate) fn window_whose_group_header_tab_holds_point(
    point: CGPoint,
    space_manager: &SpaceManager,
) -> Option<WindowId> {
    for space_id in space_manager.view.keys().copied().collect::<Vec<_>>() {
        if !is_space_visible_on_its_display(space_id) {
            continue;
        }
        let Some(view) = space_manager.view.get(&space_id) else {
            continue;
        };
        for node in view.nodes.iter().flatten() {
            let Some(header_window) = &node.group_header else {
                continue;
            };
            if let Some((window_id, _)) = header_window
                .tab_frames
                .iter()
                .find(|(_, tab_frame)| CGRectContainsPoint(*tab_frame, point))
            {
                return Some(*window_id);
            }
        }
    }
    None
}

fn plan_the_group_headers_of_view(
    view: &View,
    window_manager: &WindowManager,
) -> Vec<PlannedGroupHeader> {
    let header_height = window_manager.group_header_style.height;
    if header_height <= 0.0 || view.layout == ViewLayout::Float {
        return Vec::new();
    }

    view.nodes
        .iter()
        .enumerate()
        .filter_map(|(index, slot)| {
            let node = slot.as_ref()?;
            let is_leaf_holding_windows =
                node.left.is_none() && node.right.is_none() && node.window_count > 0;
            if !is_leaf_holding_windows || !is_node_a_group(view, node, window_manager) {
                return None;
            }

            let header_area = area_of_the_header_of_a_tile(
                area_of_tile_including_its_zoom(view, node),
                header_height,
            );
            if header_area.height <= 0.0 {
                return None;
            }

            let front_window = node.window_order[0];
            let tabs = node.window_list[..node.window_count as usize]
                .iter()
                .map(|window_id| GroupHeaderTab {
                    window_id: *window_id,
                    title: title_shown_on_the_tab_of_window(window_manager, *window_id),
                    is_the_front_window: *window_id == front_window,
                })
                .collect();
            Some(PlannedGroupHeader {
                node_id: NodeId(index as u32),
                frame: cgrect_from_area(header_area),
                tabs,
                front_window,
            })
        })
        .collect()
}

fn title_shown_on_the_tab_of_window(window_manager: &WindowManager, window_id: WindowId) -> String {
    let Some(window) = window_manager.window.get(&window_id) else {
        return String::new();
    };
    let window_title = window
        .title
        .as_ref()
        .and_then(|title| cfstring_to_string(title.as_ref()))
        .filter(|title| !title.is_empty());
    window_title
        .or_else(|| {
            window
                .application
                .and_then(|process_id| window_manager.application.get(&process_id))
                .map(|application| application.name.to_string())
        })
        .unwrap_or_default()
}
