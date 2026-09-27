# Deviations — `w2-event_loop-part1` (`src/event_loop.h`, `src/event_loop.c:1-600`)

One line each: C location | what C did | what Rust does.

`src/event_loop.c:19,21-30` | filled a fixed `uint32_t window_list[1024]` with one entry per tracked window (Sequoia/Tahoe) or per insert-feedback node, with no bound on `window_count`, overrunning the stack buffer once more than 1024 entries exist | collects the ids into a `Vec<u32>` that grows with the table, so the subscription list is complete and nothing overruns.

`src/event_loop.c:59-60` | `view_find_window_node(view, window->id)` result dereferenced (`node->window_count`) with no NULL check | the `Option<NodeId>` is tested and `window_did_receive_focus` returns when the lookup misses (`patterns/idioms-and-conventions.md` §9.3).

`src/event_loop.c:36-71,250-345,349-423,426-488,490-549,551-601` | held `struct window *`, `struct application *` and `struct view *` pointers across a handler and dereferenced them after calls that can free or rehash the owning table | every handle is resolved at each use through `WindowManager`/`SpaceManager`, and a lookup miss skips that step instead of dereferencing a dangling pointer (`DECISIONS.md` 14).

`src/event_loop.c:53` | `wm->focused_window_psn = window->application->psn` dereferenced `window->application`, which `EVENT_HANDLER_APPLICATION_TERMINATED` (`:281`) sets to NULL on windows it could not claim | `focused_window_process_serial_number` is left unchanged when the window, or its application, is no longer in the table.

`src/event_loop.c:180` | `uint64_t sid;` was read only after an assignment on every reachable path, but declared uninitialised | initialised to `SpaceId(0)`, which Rust requires; no read of the initial value is reachable.

`src/event_loop.c:366-367` | `AXUIElementCopyAttributeValue(application->ref, CFSTR("__fence"), &dummy)` never released `dummy`, leaking the returned value whenever the fence call succeeded | the create-rule result is taken and dropped, releasing it.

`src/event_loop.c:110-115,129-134` | `@try { [application removeObserver:g_workspace_context forKeyPath:@"..." context:process] } @catch {}`, with the `struct process` refcon kept alive by the process table | `remove_observer_swallowing_exception`, and, when the removal returned without throwing, `release_kvo_refcon_on_main_queue` drops the one strong count the observation held (`DECISIONS.md` 20, `THREADS.md` §6.3).
