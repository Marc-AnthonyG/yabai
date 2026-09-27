# Deviations — `w2c-review-window_manager` (`src/window_manager.rs`, `src/sa.rs`, `src/workspace.rs`)

`src/window_manager.c:254-255` | `CGPoint cursor` was read uninitialised by `CGRectContainsPoint` when `SLSGetCurrentCursorLocation` failed | `cursor` starts at the origin

`src/window_manager.c:523-525` | `CFArrayGetValueAtIndex(image_array, 0)` on an empty array returned by `SLSHWCaptureWindowList` is undefined | the proxy image stays NULL, so `window_manager_create_window_proxy` returns early, as it does when `image_array` is NULL

`src/window_manager.c:859-867` | `check_list[check_count++]` wrote past the `window_count`-entry VLA as soon as more than `window_count` windows were queued, and a parent/child cycle kept it writing | `check_list` stops growing at `window_count` entries, while `scripting_addition_set_layer` is still called for every matching row; every defined C execution is unchanged and the walk always ends. This replaces the `check_list` clause of the `:849-868` row of `deviations/w2b-window_manager.md`, whose unbounded `push` could loop forever on such a cycle

This withdraws the `src/window_manager.c:935-941` entry of `deviations/w2b-window_manager.md`. `process_name` is a function-local `static` in C, and `proc_name` leaves it untouched on failure, which is defined behaviour: a failing lookup compares the name the previous call stored, and the empty string before any call succeeded. `window_manager_window_connection_is_jankyborders` keeps that buffer again, as a function-local `static PROCESS_NAME: Mutex<[u8; PROC_PIDPATHINFO_MAXSIZE]>`, overriding the "stack local" row of `GLOSSARY.md` §8.2 as `deviations/w2c-review-event_loop.md` does for `src/window.c:173`

This withdraws the `src/yabai.c:47` entry of `deviations/w1-workspace.md`. The static on disk is `WORKSPACE_CONTEXT: OnceLock<Retained<WorkspaceContext>>`, the `GLOSSARY.md` §8.1 type, not `OnceLock<SendRetained<WorkspaceContext>>`: `mpsc::Sender<Event>` has been `Sync` since Rust 1.72, so the ivars no longer keep `Retained<WorkspaceContext>` from being `Sync`
