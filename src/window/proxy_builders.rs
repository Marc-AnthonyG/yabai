use std::thread::ScopedJoinHandle;

use crate::ffi::core_foundation::CGRect;
use crate::window::animation::WindowWithTargetFrame;
use crate::window::proxy::{WindowProxy, build_window_proxy_from_a_capture_of_the_window};

enum WindowProxyBuilder<'scope> {
    RunningOnItsOwnThread(ScopedJoinHandle<'scope, Option<WindowProxy>>),
    FinishedInlineBecauseNoThreadCouldBeSpawned(Option<WindowProxy>),
}

pub(crate) fn build_proxies_for_stationary_windows(
    animation_connection: i32,
    stationary_window_list: &[WindowWithTargetFrame],
) -> Vec<(WindowProxy, CGRect)> {
    build_window_proxies_in_parallel(animation_connection, stationary_window_list)
        .into_iter()
        .zip(stationary_window_list)
        .filter_map(|(proxy, stationary_window)| {
            proxy.map(|proxy| (proxy, stationary_window.target_frame()))
        })
        .collect()
}

fn build_window_proxies_in_parallel(
    animation_connection: i32,
    stationary_window_list: &[WindowWithTargetFrame],
) -> Vec<Option<WindowProxy>> {
    std::thread::scope(|scope| {
        let builder_list: Vec<WindowProxyBuilder> = stationary_window_list
            .iter()
            .map(|stationary_window| {
                let window_id = stationary_window.window_id;
                let builder_result = std::thread::Builder::new().spawn_scoped(scope, move || {
                    build_window_proxy_from_a_capture_of_the_window(animation_connection, window_id)
                });
                match builder_result {
                    Ok(join_handle) => WindowProxyBuilder::RunningOnItsOwnThread(join_handle),
                    Err(_) => WindowProxyBuilder::FinishedInlineBecauseNoThreadCouldBeSpawned(
                        build_window_proxy_from_a_capture_of_the_window(
                            animation_connection,
                            window_id,
                        ),
                    ),
                }
            })
            .collect();

        builder_list
            .into_iter()
            .map(|builder| match builder {
                WindowProxyBuilder::RunningOnItsOwnThread(join_handle) => {
                    join_handle.join().ok().flatten()
                }
                WindowProxyBuilder::FinishedInlineBecauseNoThreadCouldBeSpawned(proxy) => proxy,
            })
            .collect()
    })
}
