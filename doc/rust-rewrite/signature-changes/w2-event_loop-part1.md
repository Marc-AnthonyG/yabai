# Signature changes requested — `w2-event_loop-part1`

Both rows are in `src/window_manager.rs`, not in this unit's file. They were changed in this
unit's isolated crate copy only (the signature line; the body stays `todo!()`), so that
`src/event_loop.rs` compiles. Integration reconciles them.

| C location | current signature | new signature | why |
| --- | --- | --- | --- |
| `src/window_manager.c:1414` | `pub(crate) fn window_manager_remove_application(window_manager: &mut WindowManager, process_id: ProcessId)` | `pub(crate) fn window_manager_remove_application(window_manager: &mut WindowManager, process_id: ProcessId) -> Option<Application>` | `EVENT_HANDLER_APPLICATION_TERMINATED` removes the application from the table at `src/event_loop.c:258` and then runs `application_unobserve(application)` and `application_destroy(application)` on that same record at `:317-318`. With a `()` return the record is dropped inside the remove, so the unobserve step — which `THREADS.md` §5.4 requires to run synchronously on EVENTLOOP before the handles are handed over — is lost. |
| `src/window_manager.c:1399` | `pub(crate) fn window_manager_remove_window(window_manager: &mut WindowManager, window_id: WindowId)` | `pub(crate) fn window_manager_remove_window(window_manager: &mut WindowManager, window_id: WindowId) -> Option<Window>` | Same shape: `src/event_loop.c:314-315` runs `window_unobserve(window)` then `window_destroy(window)` on the record just removed at `:313`, and `EVENT_HANDLER_WINDOW_DESTROYED` repeats it at `:627-629`. The removed `Window` has to reach the caller. |
