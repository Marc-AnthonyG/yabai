use core::ffi::c_void;

use crate::display::arrangement::query_arrangement_index_of_display;
use crate::display::manager::DisplayManager;
use crate::ffi::core_foundation::CGRect;
use crate::ffi::skylight::{SLSConnectionGetPID, SLSGetWindowBounds, SLSGetWindowOwner};
use crate::serialise::frame::snapshot_of_frame;
use crate::serialise::window::{WindowSnapshot, layer_name_of_window_level};
use crate::space::lookup::query_mission_control_index_of_space;
use crate::space::managed_space::{is_native_fullscreen_space, query_display_holding_space};
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::WindowId;
use crate::window::model::{
    is_window_on_more_than_one_space, is_window_shadow_shown_according_to_window_server,
    query_parent_window_from_window_server, query_space_holding_window,
    query_window_level_from_window_server, query_window_opacity_from_window_server,
    query_window_sub_level_from_window_server, query_window_title_from_window_server,
};

pub(crate) fn snapshot_of_untracked_window(
    window_id: WindowId,
    display_manager: &mut DisplayManager,
) -> WindowSnapshot {
    let space_id = query_space_holding_window(window_id);
    let process_id = process_id_of_the_owner_of_window(window_id);
    let level = query_window_level_from_window_server(window_id);
    let sub_level = query_window_sub_level_from_window_server(window_id);

    WindowSnapshot {
        id: window_id.0,
        pid: process_id,
        app: name_of_process(process_id),
        title: query_window_title_from_window_server(window_id),
        frame: snapshot_of_frame(bounds_of_window_according_to_the_window_server(window_id)),
        root_window: query_parent_window_from_window_server(window_id) == WindowId(0),
        display: query_arrangement_index_of_display(
            query_display_holding_space(space_id),
            display_manager,
        ),
        space: query_mission_control_index_of_space(space_id),
        level,
        sub_level,
        layer: layer_name_of_window_level(level),
        sub_layer: layer_name_of_window_level(sub_level),
        opacity: query_window_opacity_from_window_server(window_id),
        has_shadow: is_window_shadow_shown_according_to_window_server(window_id),
        is_native_fullscreen: is_native_fullscreen_space(space_id),
        is_sticky: is_window_on_more_than_one_space(window_id),
        ..WindowSnapshot::default()
    }
}

fn process_id_of_the_owner_of_window(window_id: WindowId) -> libc::pid_t {
    let connection_id = *SKYLIGHT_CONNECTION_ID.get().unwrap();
    let mut owner_connection: i32 = 0;
    unsafe { SLSGetWindowOwner(connection_id, window_id.0, &mut owner_connection) };
    let mut process_id: libc::pid_t = 0;
    unsafe { SLSConnectionGetPID(owner_connection, &mut process_id) };
    process_id
}

fn name_of_process(process_id: libc::pid_t) -> String {
    let mut process_name = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    unsafe {
        libc::proc_name(
            process_id,
            process_name.as_mut_ptr().cast::<c_void>(),
            process_name.len() as u32,
        )
    };
    let end = process_name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(process_name.len());
    String::from_utf8_lossy(&process_name[..end]).into_owned()
}

fn bounds_of_window_according_to_the_window_server(window_id: WindowId) -> CGRect {
    let mut bounds = CGRect::ZERO;
    unsafe {
        SLSGetWindowBounds(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
            window_id.0,
            &mut bounds,
        )
    };
    bounds
}
