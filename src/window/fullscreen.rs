#![allow(deprecated)]

use std::time::{Duration, Instant};

use crate::display::bounds::query_bounds_of_display_left_for_windows;
use crate::display::manager::DisplayManager;
use crate::display::spaces::{
    is_display_animating_a_space_transition, query_current_space_of_display,
};
use crate::ffi::accessibility::{AXUIElementSetAttributeValue, kAXFullscreenAttribute};
use crate::ffi::core_foundation::CFBoolean;
use crate::layout::settings::{ViewFlag, ViewLayout};
use crate::layout::tree::{leaf_holding_window, move_windows_below_node_into_their_areas};
use crate::scripting_addition::client::scale_window_through_scripting_addition;
use crate::space::focus::query_current_space_of_display_holding_window_or_else_of_the_active_menu_bar_display;
use crate::space::managed_space::{is_space_visible_on_its_display, is_user_space};
use crate::space::manager::{SpaceManager, find_or_create_view_for_space};
use crate::support::handles::{ROOT_NODE_ID, WindowId};
use crate::support::macos_version::{
    is_running_on_macos_monterey, is_running_on_macos_sequoia, is_running_on_macos_sonoma,
    is_running_on_macos_tahoe, is_running_on_macos_ventura,
};
use crate::window::animation::{
    WindowWithTargetFrame, move_window_to_its_target_frame_animating_if_enabled,
};
use crate::window::focus::focus_and_raise_tracked_window;
use crate::window::manager::{WindowManager, space_managing_window};
use crate::window::model::{
    WindowFlag, is_window_in_native_fullscreen_according_to_accessibility,
    query_display_holding_window, query_space_holding_window,
};

const LONGEST_WAIT_FOR_A_SPACE_TRANSITION_BEFORE_GIVING_UP: Duration = Duration::from_secs(2);

pub(crate) fn wait_until_native_fullscreen_transition_finishes(window_id: WindowId) {
    if is_running_on_macos_monterey()
        || is_running_on_macos_ventura()
        || is_running_on_macos_sonoma()
        || is_running_on_macos_sequoia()
        || is_running_on_macos_tahoe()
    {
        let deadline = Instant::now() + LONGEST_WAIT_FOR_A_SPACE_TRANSITION_BEFORE_GIVING_UP;
        while !is_user_space(
            query_current_space_of_display_holding_window_or_else_of_the_active_menu_bar_display(
                window_id,
            ),
        ) && Instant::now() < deadline
        {
            //
            // NOTE(asmvik): Window has exited native-fullscreen mode.
            // We need to spin lock until the display is finished animating
            // because we are not actually able to interact with the window.
            //
            // The display_manager API does not work on macOS Monterey.
            //

            unsafe { libc::usleep(100000) };
        }
    } else {
        let display_id = query_display_holding_window(window_id);

        loop {
            //
            // NOTE(asmvik): Window has exited native-fullscreen mode.
            // We need to spin lock until the display is finished animating
            // because we are not actually able to interact with the window.
            //

            unsafe { libc::usleep(100000) };

            if !is_display_animating_a_space_transition(display_id) {
                break;
            }
        }
    }
}

pub(crate) fn toggle_window_native_fullscreen(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) {
    let space_id = query_space_holding_window(window_id).0 as u32;

    //
    // NOTE(asmvik): The window must become the focused window
    // before we can change its fullscreen attribute. We focus the
    // window and spin lock until a potential space animation has finished.
    //

    focus_and_raise_tracked_window(window_manager, window_id);
    let deadline = Instant::now() + LONGEST_WAIT_FOR_A_SPACE_TRANSITION_BEFORE_GIVING_UP;
    while space_id as u64
        != query_current_space_of_display_holding_window_or_else_of_the_active_menu_bar_display(
            window_id,
        )
        .0
        && Instant::now() < deadline
    {
        unsafe { libc::usleep(100000) };
    }

    if let Some(window) = window_manager.window.get(&window_id) {
        if !is_window_in_native_fullscreen_according_to_accessibility(window) {
            unsafe {
                AXUIElementSetAttributeValue(
                    &*window.element_ref,
                    kAXFullscreenAttribute(),
                    CFBoolean::new(true),
                )
            };
        } else {
            unsafe {
                AXUIElementSetAttributeValue(
                    &*window.element_ref,
                    kAXFullscreenAttribute(),
                    CFBoolean::new(false),
                )
            };
        }
    }

    //
    // NOTE(asmvik): We toggled the fullscreen attribute and must
    // now spin lock until the post-exit space animation has finished.
    //

    wait_until_native_fullscreen_transition_finishes(window_id);
}

pub(crate) fn toggle_managed_window_zoom_parent(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) {
    let view = space_managing_window(window_manager, window_id);
    let Some(space_id) = view else {
        return;
    };
    if space_manager
        .view
        .get(&space_id)
        .is_none_or(|view| view.layout != ViewLayout::BinarySpacePartitioning)
    {
        return;
    }

    let node = leaf_holding_window(space_manager, space_id, window_id);
    debug_assert!(node.is_some());
    let Some(node_id) = node else {
        return;
    };

    let Some((node_parent, node_zoom)) = space_manager
        .find_node_in_view_of_space(space_id, node_id)
        .map(|node| (node.parent, node.zoom))
    else {
        return;
    };

    if node_parent.is_none() {
        return;
    }

    if node_zoom == node_parent {
        if let Some(node) = space_manager.find_node_mut_in_view_of_space(space_id, node_id) {
            node.zoom = None;
        }
        if is_space_visible_on_its_display(space_id) {
            move_windows_below_node_into_their_areas(
                space_id,
                node_id,
                window_manager,
                space_manager,
            );
        } else if let Some(view) = space_manager.view.get_mut(&space_id) {
            view.flags.insert(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
        }
    } else {
        if let Some(node) = space_manager.find_node_mut_in_view_of_space(space_id, node_id) {
            node.zoom = node_parent;
        }
        if is_space_visible_on_its_display(space_id) {
            move_windows_below_node_into_their_areas(
                space_id,
                node_id,
                window_manager,
                space_manager,
            );
        } else if let Some(view) = space_manager.view.get_mut(&space_id) {
            view.flags.insert(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
        }
    }
}

pub(crate) fn toggle_managed_window_zoom_fullscreen(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) {
    let view = space_managing_window(window_manager, window_id);
    let Some(space_id) = view else {
        return;
    };
    if space_manager
        .view
        .get(&space_id)
        .is_none_or(|view| view.layout != ViewLayout::BinarySpacePartitioning)
    {
        return;
    }

    let node = leaf_holding_window(space_manager, space_id, window_id);
    debug_assert!(node.is_some());
    let Some(node_id) = node else {
        return;
    };

    if node_id == ROOT_NODE_ID {
        return;
    }

    let Some(node_zoom) = space_manager
        .find_node_in_view_of_space(space_id, node_id)
        .map(|node| node.zoom)
    else {
        return;
    };

    if node_zoom == Some(ROOT_NODE_ID) {
        if let Some(node) = space_manager.find_node_mut_in_view_of_space(space_id, node_id) {
            node.zoom = None;
        }
        if is_space_visible_on_its_display(space_id) {
            move_windows_below_node_into_their_areas(
                space_id,
                node_id,
                window_manager,
                space_manager,
            );
        } else if let Some(view) = space_manager.view.get_mut(&space_id) {
            view.flags.insert(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
        }
    } else {
        if let Some(node) = space_manager.find_node_mut_in_view_of_space(space_id, node_id) {
            node.zoom = Some(ROOT_NODE_ID);
        }
        if is_space_visible_on_its_display(space_id) {
            move_windows_below_node_into_their_areas(
                space_id,
                node_id,
                window_manager,
                space_manager,
            );
        } else if let Some(view) = space_manager.view.get_mut(&space_id) {
            view.flags.insert(ViewFlag::WINDOWS_AWAIT_THEIR_AREAS);
        }
    }
}

pub(crate) fn toggle_window_windowed_fullscreen(
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let display_id = query_display_holding_window(window_id);
    if display_id.0 == 0 {
        return;
    }

    let Some(window) = window_manager.window.get_mut(&window_id) else {
        return;
    };

    if window.flags.contains(WindowFlag::IN_WINDOWED_FULLSCREEN) {
        window.flags.remove(WindowFlag::IN_WINDOWED_FULLSCREEN);
        let windowed_frame = window.windowed_frame;
        move_window_to_its_target_frame_animating_if_enabled(
            WindowWithTargetFrame {
                window_id,
                x: windowed_frame.origin.x as f32,
                y: windowed_frame.origin.y as f32,
                width: windowed_frame.size.width as f32,
                height: windowed_frame.size.height as f32,
            },
            window_manager,
        );
    } else {
        window.flags.insert(WindowFlag::IN_WINDOWED_FULLSCREEN);
        window.windowed_frame = window.frame;
        let bounds = query_bounds_of_display_left_for_windows(display_id, true, display_manager);
        move_window_to_its_target_frame_animating_if_enabled(
            WindowWithTargetFrame {
                window_id,
                x: bounds.origin.x as f32,
                y: bounds.origin.y as f32,
                width: bounds.size.width as f32,
                height: bounds.size.height as f32,
            },
            window_manager,
        );
    }
}

pub(crate) fn toggle_window_picture_in_picture(
    space_manager: &mut SpaceManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let display_id = query_display_holding_window(window_id);
    if display_id.0 == 0 {
        return;
    }

    let space_id = query_current_space_of_display(display_id);
    let display_view =
        find_or_create_view_for_space(space_manager, space_id, display_manager, window_manager);

    let mut bounds = query_bounds_of_display_left_for_windows(display_id, false, display_manager);
    if let Some(view) = space_manager.view.get(&display_view)
        && view.flags.contains(ViewFlag::PADDING_IS_ENABLED)
    {
        bounds.origin.x += view.left_padding as f64;
        bounds.size.width -= (view.left_padding + view.right_padding) as f64;
        bounds.origin.y += view.top_padding as f64;
        bounds.size.height -= (view.top_padding + view.bottom_padding) as f64;
    }

    scale_window_through_scripting_addition(
        window_id,
        bounds.origin.x as f32,
        bounds.origin.y as f32,
        bounds.size.width as f32,
        bounds.size.height as f32,
    );
}
