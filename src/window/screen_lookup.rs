use crate::ffi::core_foundation::CGPoint;
use crate::ffi::skylight::{SLSFindWindowAndOwner, SLSGetCurrentCursorLocation};
use crate::space::managed_space::query_windows_on_space;
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::handles::{SpaceId, WindowId};
use crate::window::janky_borders::is_window_connection_owned_by_janky_borders;
use crate::window::manager::{WindowManager, tracked_window_with_id};

pub(crate) fn rank_of_window_in_list(window_id: WindowId, window_list: &[WindowId]) -> i32 {
    let mut rank: i32 = 0;
    for index in 0..window_list.len() {
        if window_list[index] == window_id {
            return rank;
        } else {
            rank += 1;
        }
    }

    i32::MAX
}

pub(crate) fn query_tracked_window_at_rank_on_space_skipping_window(
    window_manager: &mut WindowManager,
    space_id: SpaceId,
    rank: i32,
    filter_window_id: WindowId,
) -> Option<WindowId> {
    let window_list = query_windows_on_space(space_id, false, window_manager)?;

    let mut result: Option<WindowId> = None;
    let mut inner_index: i32 = 0;
    for index in 0..window_list.len() {
        if window_list[index] == filter_window_id {
            continue;
        }

        let Some(window) = tracked_window_with_id(window_manager, window_list[index]) else {
            continue;
        };

        inner_index += 1;
        if inner_index == rank {
            result = Some(window);
            break;
        }
    }

    result
}

pub(crate) fn query_tracked_window_at_point_skipping_window(
    window_manager: &mut WindowManager,
    point: CGPoint,
    filter_window_id: WindowId,
) -> Option<WindowId> {
    let connection = *SKYLIGHT_CONNECTION_ID.get().unwrap();
    let mut point = point;
    let mut window_point = CGPoint::new(0.0, 0.0);
    let mut window_id: u32 = 0;
    let mut window_connection_id: i32 = 0;

    unsafe {
        SLSFindWindowAndOwner(
            connection,
            filter_window_id.0 as i32,
            -1,
            0,
            &mut point,
            &mut window_point,
            &mut window_id,
            &mut window_connection_id,
        )
    };
    if connection == window_connection_id {
        unsafe {
            SLSFindWindowAndOwner(
                connection,
                window_id as i32,
                -1,
                0,
                &mut point,
                &mut window_point,
                &mut window_id,
                &mut window_connection_id,
            )
        };
    }

    if is_window_connection_owned_by_janky_borders(window_connection_id) {
        unsafe {
            SLSFindWindowAndOwner(
                connection,
                window_id as i32,
                -1,
                0,
                &mut point,
                &mut window_point,
                &mut window_id,
                &mut window_connection_id,
            )
        };
        if connection == window_connection_id {
            unsafe {
                SLSFindWindowAndOwner(
                    connection,
                    window_id as i32,
                    -1,
                    0,
                    &mut point,
                    &mut window_point,
                    &mut window_id,
                    &mut window_connection_id,
                )
            };
        }
    }

    tracked_window_with_id(window_manager, WindowId(window_id))
}

pub(crate) fn query_tracked_window_at_point(
    window_manager: &mut WindowManager,
    point: CGPoint,
) -> Option<WindowId> {
    let connection = *SKYLIGHT_CONNECTION_ID.get().unwrap();
    let mut point = point;
    let mut window_point = CGPoint::new(0.0, 0.0);
    let mut window_id: u32 = 0;
    let mut window_connection_id: i32 = 0;

    unsafe {
        SLSFindWindowAndOwner(
            connection,
            0,
            1,
            0,
            &mut point,
            &mut window_point,
            &mut window_id,
            &mut window_connection_id,
        )
    };
    if connection == window_connection_id {
        unsafe {
            SLSFindWindowAndOwner(
                connection,
                window_id as i32,
                -1,
                0,
                &mut point,
                &mut window_point,
                &mut window_id,
                &mut window_connection_id,
            )
        };
    }

    if is_window_connection_owned_by_janky_borders(window_connection_id) {
        unsafe {
            SLSFindWindowAndOwner(
                connection,
                window_id as i32,
                -1,
                0,
                &mut point,
                &mut window_point,
                &mut window_id,
                &mut window_connection_id,
            )
        };
        if connection == window_connection_id {
            unsafe {
                SLSFindWindowAndOwner(
                    connection,
                    window_id as i32,
                    -1,
                    0,
                    &mut point,
                    &mut window_point,
                    &mut window_id,
                    &mut window_connection_id,
                )
            };
        }
    }

    tracked_window_with_id(window_manager, WindowId(window_id))
}

pub(crate) fn query_tracked_window_under_cursor(
    window_manager: &mut WindowManager,
) -> Option<WindowId> {
    let mut cursor = CGPoint::new(0.0, 0.0);
    unsafe { SLSGetCurrentCursorLocation(*SKYLIGHT_CONNECTION_ID.get().unwrap(), &mut cursor) };
    query_tracked_window_at_point(window_manager, cursor)
}
