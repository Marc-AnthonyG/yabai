use crate::ffi::core_foundation::{cfarray_count, take_create_rule_result};
use crate::ffi::skylight::{
    SLSCopyAssociatedWindows, SLSWindowIteratorAdvance, SLSWindowIteratorGetParentID,
    SLSWindowIteratorGetWindowID, SLSWindowQueryResultCopyWindows, SLSWindowQueryWindows,
};
use crate::globals::CONNECTION;
use crate::handles::WindowId;
use crate::scripting_addition::client::scripting_addition_set_layer;
use crate::support::layer::{LAYER_AUTO, LAYER_BELOW, LAYER_NORMAL};
use crate::window::manager::{WindowManager, window_manager_find_managed_window};

pub(crate) fn window_manager_adjust_layer(
    window_id: WindowId,
    layer: i32,
    window_manager: &mut WindowManager,
) {
    let Some(window) = window_manager.window.find(&window_id) else {
        return;
    };
    if window.layer != LAYER_AUTO {
        return;
    }

    scripting_addition_set_layer(window_id, layer);
}

pub(crate) fn window_manager_set_window_layer(
    window_id: WindowId,
    layer: i32,
    window_manager: &mut WindowManager,
) -> bool {
    let mut parent_layer = layer;
    let mut child_layer = layer;

    if layer == LAYER_AUTO {
        parent_layer = if window_manager_find_managed_window(window_manager, window_id).is_some() {
            LAYER_BELOW
        } else {
            LAYER_NORMAL
        };
        child_layer = LAYER_NORMAL;
    }

    let Some(window) = window_manager.window.find_mut(&window_id) else {
        return false;
    };
    window.layer = layer;
    let result = scripting_addition_set_layer(window_id, parent_layer);
    if !result {
        return false;
    }

    let connection = *CONNECTION.get().unwrap();
    let Some(window_list) =
        (unsafe { take_create_rule_result(SLSCopyAssociatedWindows(connection, window_id.0)) })
    else {
        return result;
    };

    let window_count = cfarray_count(&window_list) as i32;
    let Some(query) = (unsafe {
        take_create_rule_result(SLSWindowQueryWindows(
            connection,
            &*window_list,
            window_count,
        ))
    }) else {
        return result;
    };
    let Some(iterator) =
        (unsafe { take_create_rule_result(SLSWindowQueryResultCopyWindows(&*query)) })
    else {
        return result;
    };

    let mut relation_count: i32 = 0;
    let mut parent_list: Vec<u32> = vec![0; window_count as usize];
    let mut child_list: Vec<u32> = vec![0; window_count as usize];

    while unsafe { SLSWindowIteratorAdvance(&*iterator) } {
        let parent_window_id = unsafe { SLSWindowIteratorGetParentID(&*iterator) };
        let child_window_id = unsafe { SLSWindowIteratorGetWindowID(&*iterator) };
        if relation_count < window_count {
            parent_list[relation_count as usize] = parent_window_id;
            child_list[relation_count as usize] = child_window_id;
        }
        relation_count += 1;
    }

    let mut check_list: Vec<u32> = Vec::with_capacity(window_count as usize);
    check_list.push(window_id.0);

    let mut index = 0;
    while index < check_list.len() {
        for inner_index in 0..window_count as usize {
            if parent_list[inner_index] != check_list[index] {
                continue;
            }
            scripting_addition_set_layer(WindowId(child_list[inner_index]), child_layer);
            if check_list.len() < window_count as usize {
                check_list.push(child_list[inner_index]);
            }
        }
        index += 1;
    }

    drop(query);
    drop(iterator);
    drop(window_list);

    result
}
