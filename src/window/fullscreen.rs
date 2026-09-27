#![allow(deprecated)]

use std::time::{Duration, Instant};

use crate::display::bounds::display_bounds_constrained;
use crate::display::manager::DisplayManager;
use crate::display::spaces::{display_manager_display_is_animating, display_space_id};
use crate::ffi::accessibility::{AXUIElementSetAttributeValue, kAXFullscreenAttribute};
use crate::ffi::core_foundation::{as_cftype, kCFBooleanFalse, kCFBooleanTrue};
use crate::layout::settings::{ViewFlag, ViewType};
use crate::layout::tree::{view_find_window_node, window_node_flush};
use crate::scripting_addition::client::scripting_addition_scale_window;
use crate::space::focus::space_manager_active_space_of_the_display_holding_window;
use crate::space::managed_space::{space_is_user, space_is_visible};
use crate::space::manager::{SpaceManager, space_manager_find_view};
use crate::support::handles::{ROOT_NODE_ID, WindowId};
use crate::support::macos_version::{
    workspace_is_macos_monterey, workspace_is_macos_sequoia, workspace_is_macos_sonoma,
    workspace_is_macos_tahoe, workspace_is_macos_ventura,
};
use crate::window::animation::{WindowCapture, window_manager_animate_window};
use crate::window::focus::window_manager_focus_window_with_raise_resolving_its_application;
use crate::window::manager::{WindowManager, window_manager_find_managed_window};
use crate::window::model::{
    WindowFlag, window_check_flag, window_clear_flag, window_display_id, window_is_fullscreen,
    window_set_flag, window_space,
};

const LONGEST_WAIT_FOR_A_SPACE_TRANSITION_BEFORE_GIVING_UP: Duration = Duration::from_secs(2);

pub(crate) fn window_manager_wait_for_native_fullscreen_transition(window_id: WindowId) {
    if workspace_is_macos_monterey()
        || workspace_is_macos_ventura()
        || workspace_is_macos_sonoma()
        || workspace_is_macos_sequoia()
        || workspace_is_macos_tahoe()
    {
        let deadline = Instant::now() + LONGEST_WAIT_FOR_A_SPACE_TRANSITION_BEFORE_GIVING_UP;
        while !space_is_user(space_manager_active_space_of_the_display_holding_window(
            window_id,
        )) && Instant::now() < deadline
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
        let display_id = window_display_id(window_id);

        loop {
            //
            // NOTE(asmvik): Window has exited native-fullscreen mode.
            // We need to spin lock until the display is finished animating
            // because we are not actually able to interact with the window.
            //

            unsafe { libc::usleep(100000) };

            if !display_manager_display_is_animating(display_id) {
                break;
            }
        }
    }
}

pub(crate) fn window_manager_toggle_window_native_fullscreen(
    window_id: WindowId,
    window_manager: &mut WindowManager,
) {
    let space_id = window_space(window_id).0 as u32;

    //
    // NOTE(asmvik): The window must become the focused window
    // before we can change its fullscreen attribute. We focus the
    // window and spin lock until a potential space animation has finished.
    //

    window_manager_focus_window_with_raise_resolving_its_application(window_manager, window_id);
    let deadline = Instant::now() + LONGEST_WAIT_FOR_A_SPACE_TRANSITION_BEFORE_GIVING_UP;
    while space_id as u64 != space_manager_active_space_of_the_display_holding_window(window_id).0
        && Instant::now() < deadline
    {
        unsafe { libc::usleep(100000) };
    }

    if let Some(window) = window_manager.window.find(&window_id) {
        if !window_is_fullscreen(window) {
            unsafe {
                AXUIElementSetAttributeValue(
                    &*window.element_ref,
                    kAXFullscreenAttribute(),
                    as_cftype(kCFBooleanTrue()),
                )
            };
        } else {
            unsafe {
                AXUIElementSetAttributeValue(
                    &*window.element_ref,
                    kAXFullscreenAttribute(),
                    as_cftype(kCFBooleanFalse()),
                )
            };
        }
    }

    //
    // NOTE(asmvik): We toggled the fullscreen attribute and must
    // now spin lock until the post-exit space animation has finished.
    //

    window_manager_wait_for_native_fullscreen_transition(window_id);
}

pub(crate) fn window_manager_toggle_window_zoom_parent(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) {
    let view = window_manager_find_managed_window(window_manager, window_id);
    let Some(space_id) = view else {
        return;
    };
    if space_manager
        .view
        .find(&space_id)
        .is_none_or(|view| view.layout != ViewType::Bsp)
    {
        return;
    }

    let node = view_find_window_node(space_manager, space_id, window_id);
    debug_assert!(node.is_some());
    let Some(node_id) = node else {
        return;
    };

    let Some((node_parent, node_zoom)) = space_manager
        .view
        .find(&space_id)
        .and_then(|view| view.find_node(node_id))
        .map(|node| (node.parent, node.zoom))
    else {
        return;
    };

    if node_parent.is_none() {
        return;
    }

    if node_zoom == node_parent {
        if let Some(view) = space_manager.view.find_mut(&space_id)
            && let Some(node) = view.find_node_mut(node_id)
        {
            node.zoom = None;
        }
        if space_is_visible(space_id) {
            window_node_flush(space_id, node_id, window_manager, space_manager);
        } else if let Some(view) = space_manager.view.find_mut(&space_id) {
            view.set_flag(ViewFlag::IS_DIRTY);
        }
    } else {
        if let Some(view) = space_manager.view.find_mut(&space_id)
            && let Some(node) = view.find_node_mut(node_id)
        {
            node.zoom = node_parent;
        }
        if space_is_visible(space_id) {
            window_node_flush(space_id, node_id, window_manager, space_manager);
        } else if let Some(view) = space_manager.view.find_mut(&space_id) {
            view.set_flag(ViewFlag::IS_DIRTY);
        }
    }
}

pub(crate) fn window_manager_toggle_window_zoom_fullscreen(
    window_manager: &mut WindowManager,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) {
    let view = window_manager_find_managed_window(window_manager, window_id);
    let Some(space_id) = view else {
        return;
    };
    if space_manager
        .view
        .find(&space_id)
        .is_none_or(|view| view.layout != ViewType::Bsp)
    {
        return;
    }

    let node = view_find_window_node(space_manager, space_id, window_id);
    debug_assert!(node.is_some());
    let Some(node_id) = node else {
        return;
    };

    if node_id == ROOT_NODE_ID {
        return;
    }

    let Some(node_zoom) = space_manager
        .view
        .find(&space_id)
        .and_then(|view| view.find_node(node_id))
        .map(|node| node.zoom)
    else {
        return;
    };

    if node_zoom == Some(ROOT_NODE_ID) {
        if let Some(view) = space_manager.view.find_mut(&space_id)
            && let Some(node) = view.find_node_mut(node_id)
        {
            node.zoom = None;
        }
        if space_is_visible(space_id) {
            window_node_flush(space_id, node_id, window_manager, space_manager);
        } else if let Some(view) = space_manager.view.find_mut(&space_id) {
            view.set_flag(ViewFlag::IS_DIRTY);
        }
    } else {
        if let Some(view) = space_manager.view.find_mut(&space_id)
            && let Some(node) = view.find_node_mut(node_id)
        {
            node.zoom = Some(ROOT_NODE_ID);
        }
        if space_is_visible(space_id) {
            window_node_flush(space_id, node_id, window_manager, space_manager);
        } else if let Some(view) = space_manager.view.find_mut(&space_id) {
            view.set_flag(ViewFlag::IS_DIRTY);
        }
    }
}

pub(crate) fn window_manager_toggle_window_windowed_fullscreen(
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let display_id = window_display_id(window_id);
    if display_id.0 == 0 {
        return;
    }

    let Some(window) = window_manager.window.find_mut(&window_id) else {
        return;
    };

    if window_check_flag(window, WindowFlag::WINDOWED) {
        window_clear_flag(window, WindowFlag::WINDOWED);
        let windowed_frame = window.windowed_frame;
        window_manager_animate_window(
            WindowCapture {
                window_id,
                x: windowed_frame.origin.x as f32,
                y: windowed_frame.origin.y as f32,
                width: windowed_frame.size.width as f32,
                height: windowed_frame.size.height as f32,
            },
            window_manager,
        );
    } else {
        window_set_flag(window, WindowFlag::WINDOWED);
        window.windowed_frame = window.frame;
        let bounds = display_bounds_constrained(display_id, true, display_manager);
        window_manager_animate_window(
            WindowCapture {
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

pub(crate) fn window_manager_toggle_window_pip(
    space_manager: &mut SpaceManager,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
) {
    let display_id = window_display_id(window_id);
    if display_id.0 == 0 {
        return;
    }

    let space_id = display_space_id(display_id);
    let display_view =
        space_manager_find_view(space_manager, space_id, display_manager, window_manager);

    let mut bounds = display_bounds_constrained(display_id, false, display_manager);
    if let Some(view) = space_manager.view.find(&display_view)
        && view.check_flag(ViewFlag::ENABLE_PADDING)
    {
        bounds.origin.x += view.left_padding as f64;
        bounds.size.width -= (view.left_padding + view.right_padding) as f64;
        bounds.origin.y += view.top_padding as f64;
        bounds.size.height -= (view.top_padding + view.bottom_padding) as f64;
    }

    scripting_addition_scale_window(
        window_id,
        bounds.origin.x as f32,
        bounds.origin.y as f32,
        bounds.size.width as f32,
        bounds.size.height as f32,
    );
}
