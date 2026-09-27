use std::thread::ScopedJoinHandle;

use crate::ffi::core_foundation::CGRect;
use crate::window::animation::WindowCapture;
use crate::window::proxy::{WindowProxy, window_manager_build_window_proxy_thread_proc};

enum WindowProxyBuilder<'scope> {
    RunningOnItsOwnThread(ScopedJoinHandle<'scope, Option<WindowProxy>>),
    FinishedInlineBecauseNoThreadCouldBeSpawned(Option<WindowProxy>),
}

pub(crate) fn window_manager_build_proxies_for_stationary_windows(
    animation_connection: i32,
    stationary_window_list: &[WindowCapture],
) -> Vec<(WindowProxy, CGRect)> {
    window_manager_build_window_proxies_in_parallel(animation_connection, stationary_window_list)
        .into_iter()
        .zip(stationary_window_list)
        .filter_map(|(proxy, stationary_window)| {
            proxy.map(|proxy| (proxy, stationary_window.target_frame()))
        })
        .collect()
}

fn window_manager_build_window_proxies_in_parallel(
    animation_connection: i32,
    stationary_window_list: &[WindowCapture],
) -> Vec<Option<WindowProxy>> {
    std::thread::scope(|scope| {
        let builder_list: Vec<WindowProxyBuilder> = stationary_window_list
            .iter()
            .map(|stationary_window| {
                let window_id = stationary_window.window_id;
                let builder_result = std::thread::Builder::new().spawn_scoped(scope, move || {
                    window_manager_build_window_proxy_thread_proc(animation_connection, window_id)
                });
                match builder_result {
                    Ok(join_handle) => WindowProxyBuilder::RunningOnItsOwnThread(join_handle),
                    Err(_) => WindowProxyBuilder::FinishedInlineBecauseNoThreadCouldBeSpawned(
                        window_manager_build_window_proxy_thread_proc(
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
