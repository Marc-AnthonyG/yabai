# Final Rust signatures — `window.h`, `window.c`, `application.h`, `application.c`, `process_manager.h`, `process_manager.c`

Work unit W0b-1. One row per C function, in C source order, per file. Wave 1 pastes the
**Rust signature** column verbatim; nothing in a body may change a parameter list.

Derived from `DECISIONS.md` (13, 14, 20, 22, 28, 32, 37), `patterns/state-and-ownership.md`
§2-§4, `GLOSSARY.md`, `THREADS.md` §5, `patterns/memory-text-and-os-objects.md` §1,
`patterns/message-and-serialisation.md` §7, and `state-access.tsv`, with every row checked
against the C.

71 functions are defined across the six files: 6 in `window.h`, 44 in `window.c`, 10 in
`application.c`, 11 in `process_manager.c`. `application.h` and `process_manager.h` define none.
One of the 71 is dead (§5). `window.h:140` declares a function that has no definition and no
caller and is also listed in §5.

---

## 1. How each signature was built

Applied in this order, from `patterns/state-and-ownership.md` §2.2:

1. C parameters, in C order, pointers rewritten to handles (§3.1: `struct window *` →
   `WindowId`, `struct application *` → `ProcessId`, `struct view *` → `&mut SpaceManager` plus
   `SpaceId`, `struct process *` → `Arc<Process>` per `DECISIONS.md` 22).
2. A manager the C declares keeps the position the C gave it.
3. Every remaining manager of `Managers(f)` is appended after the whole C parameter list, in the
   `src/yabai.c:27-35` order: `process_manager` (`:28`), `display_manager` (`:29`),
   `window_manager` (`:30`), `space_manager` (`:31`), `mouse_drag_state` (`:33`).

Every manager parameter is `&mut` (§2.1). Statics are never parameters — `CONNECTION`,
`VERBOSE`, the three window levels, `PROCESS_TABLE`, `CARBON_PROCESS_EVENT_INSTALLATION`,
`WORKSPACE_CONTEXT`, `EVENT_SENDER`, `AX_WINDOW_NOTIFICATION`, `AX_APPLICATION_NOTIFICATION`.
An out-parameter `int *count` paired with a returned buffer collapses into the returned `Vec`.
`FILE *rsp` becomes `response: &mut Response` and keeps its C position, which is first.

Return types: a nullable pointer becomes `Option` (`DECISIONS.md` 32); a returned list becomes
`Vec`; `bool` stays `bool`; observable sentinels stay integers — `window_level`/`window_sub_level`
keep `i32`, `process_handler` keeps `OSStatus` because Carbon reads `noErr` and `-1`,
`application_focused_window` keeps the `WindowId(0)` miss that nine callers test.
`CFStringOwned` is the crate's spelling for an owned `CFStringRef` (`GLOSSARY.md` §2.5); it is
the `CFRetained<CFString>` of `patterns/memory-text-and-os-objects.md` §1.1 under the name the
glossary fixes, and is therefore not a third wrapper type.

**The one shape this file settles.** `patterns/state-and-ownership.md` §2.2 step 1 rewrites every
`struct window *` / `struct application *` parameter to a handle, but the C calls most of the
functions in these two files on a record that is **not in its manager's table at that moment**,
so no handle could be resolved:

* `window_create` (`window.c:1093-1134`) populates a freshly allocated `struct window` by calling
  `window_ax_frame`, `window_ax_role`, `window_ax_subrole`, `window_title`, `window_is_root`,
  `window_is_minimized`, `window_ax_can_move`, `window_ax_can_resize` and `window_is_fullscreen`
  on it. `window_manager_add_window` does not run until `window_manager.c:1471`.
* `window_manager_create_and_add_window` (`window_manager.c:1440-1471`) calls `window_title_ts`,
  `window_role_ts`, `window_subrole_ts`, `window_is_unknown`, `window_observe`,
  `window_unobserve` and `window_destroy` on that same pre-insertion local, and returns `NULL`
  at `:1451` and `:1463` after destroying it.
* `EVENT_HANDLER(APPLICATION_LAUNCHED)` (`event_loop.c:146-152`) calls `application_create`,
  `application_observe`, `application_unobserve` and `application_destroy` before
  `window_manager_add_application` at `:172`.
* `event_loop.c:313-315` and `:627-629` call `window_manager_remove_window` **first**, so
  `window_unobserve` and `window_destroy` always see a record that is already out of the table
  (recipe R5, `patterns/state-and-ownership.md` §4.5).

So those parameters stay the record: `&Window` / `&mut Window` / `Window` by value, and
`&Application` / `&mut Application` / `Application` by value. This is the shape `THREADS.md` §5.4
already fixes literally for `window_observe` and `window_unobserve`
("the parameter set decision 13 computes for both … is `(window_manager, window)`") and that
`patterns/idioms-and-conventions.md` §6.2 fixes for the six `window.h` flag accessors. The handle
form is used wherever the C *does* reach a record through the manager —
`window_serialize` (`patterns/state-and-ownership.md` §2.3 example 2) and the `uint32_t wid`
functions. §4 lists this and every other judgement call.

`WindowManager` appears as a parameter only where the body reaches it for something **other than
the record the caller already holds**: `window->application->observer_ref`
(`window_observe`/`window_unobserve`), `window->application->ref` (`window_is_root`,
`window_create`), `wm->managed_window` and `wm->focused_window_id` (`window_serialize`).

---

## 2. `src/window.h` and `src/window.c`

| C name | file:line | thread context | event-loop-owned managers touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `window_check_flag` | `window.h:128` | any | — | `pub(crate) fn window_check_flag(window: &Window, flag: WindowFlag) -> bool` | `pub(crate)` |
| `window_clear_flag` | `window.h:129` | any | — | `pub(crate) fn window_clear_flag(window: &mut Window, flag: WindowFlag)` | `pub(crate)` |
| `window_set_flag` | `window.h:130` | any | — | `pub(crate) fn window_set_flag(window: &mut Window, flag: WindowFlag)` | `pub(crate)` |
| `window_check_rule_flag` | `window.h:132` | any | — | `pub(crate) fn window_check_rule_flag(window: &Window, flag: WindowRuleFlag) -> bool` | `pub(crate)` |
| `window_clear_rule_flag` | `window.h:133` | any | — | `pub(crate) fn window_clear_rule_flag(window: &mut Window, flag: WindowRuleFlag)` | `pub(crate)` |
| `window_set_rule_flag` | `window.h:134` | any | — | `pub(crate) fn window_set_rule_flag(window: &mut Window, flag: WindowRuleFlag)` | `pub(crate)` |
| `window_observe` | `window.c:7` | event loop; also main run loop at start-up | `WindowManager` | `pub(crate) fn window_observe(window: &mut Window, window_manager: &mut WindowManager) -> bool` | `pub(crate)` |
| `window_unobserve` | `window.c:21` | event loop; also main run loop at start-up | `WindowManager` | `pub(crate) fn window_unobserve(window: &mut Window, window_manager: &mut WindowManager)` | `pub(crate)` |
| `window_display_uuid` | `window.c:31` | event loop; also main run loop at start-up | — | `fn window_display_uuid(window_id: WindowId) -> Option<CFStringOwned>` | private |
| `window_display_id` | `window.c:42` | event loop | — | `pub(crate) fn window_display_id(window_id: WindowId) -> DisplayId` | `pub(crate)` |
| `window_display_space` | `window.c:56` | event loop; also main run loop at start-up | — | `fn window_display_space(window_id: WindowId) -> SpaceId` | private |
| `window_space` | `window.c:67` | event loop; also main run loop at start-up | — | `pub(crate) fn window_space(window_id: WindowId) -> SpaceId` | `pub(crate)` |
| `window_space_list` | `window.c:89` | event loop | — | `pub(crate) fn window_space_list(window_id: WindowId) -> Vec<SpaceId>` | `pub(crate)` |
| `window_layer` | `window.c:113` | event loop | — | `fn window_layer(level: i32) -> &'static str` | private |
| `window_nonax_serialize` | `window.c:121` | event loop | `DisplayManager` | `pub(crate) fn window_nonax_serialize(response: &mut Response, window_id: WindowId, flags: u64, display_manager: &mut DisplayManager)` | `pub(crate)` |
| `window_serialize` | `window.c:409` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager`, `MouseDragState` | `pub(crate) fn window_serialize(response: &mut Response, window_id: WindowId, flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState)` | `pub(crate)` |
| `window_property_title_ts` | `window.c:713` | event loop | — | `fn window_property_title_ts(window_id: WindowId) -> String` | private |
| `window_title_ts` | `window.c:724` | event loop; also main run loop at start-up | — | `pub(crate) fn window_title_ts(window: &Window) -> String` | `pub(crate)` |
| `window_title` | `window.c:729` | event loop; also main run loop at start-up | — | `pub(crate) fn window_title(window: &Window) -> Option<CFStringOwned>` | `pub(crate)` |
| `window_ax_origin` | `window.c:736` | event loop | — | `pub(crate) fn window_ax_origin(window: &Window) -> CGPoint` | `pub(crate)` |
| `window_ax_frame` | `window.c:751` | event loop; also main run loop at start-up | — | `pub(crate) fn window_ax_frame(window: &Window) -> CGRect` | `pub(crate)` |
| `window_ax_can_move` | `window.c:773` | event loop; also main run loop at start-up | — | `pub(crate) fn window_ax_can_move(window: &Window) -> bool` | `pub(crate)` |
| `window_can_move` | `window.c:782` | event loop; also main run loop at start-up | — | `pub(crate) fn window_can_move(window: &Window) -> bool` | `pub(crate)` |
| `window_ax_can_resize` | `window.c:787` | event loop; also main run loop at start-up | — | `pub(crate) fn window_ax_can_resize(window: &Window) -> bool` | `pub(crate)` |
| `window_can_resize` | `window.c:796` | event loop; also main run loop at start-up | — | `pub(crate) fn window_can_resize(window: &Window) -> bool` | `pub(crate)` |
| `window_can_minimize` | `window.c:801` | event loop | — | `pub(crate) fn window_can_minimize(window: &Window) -> bool` | `pub(crate)` |
| `window_is_undersized` | `window.c:810` | event loop; also main run loop at start-up | — | `pub(crate) fn window_is_undersized(window: &Window) -> bool` | `pub(crate)` |
| `window_is_minimized` | `window.c:817` | event loop; also main run loop at start-up | — | `fn window_is_minimized(window: &Window) -> bool` | private |
| `window_is_fullscreen` | `window.c:830` | event loop; also main run loop at start-up | — | `pub(crate) fn window_is_fullscreen(window: &Window) -> bool` | `pub(crate)` |
| `window_is_sticky` | `window.c:843` | event loop; also main run loop at start-up | — | `pub(crate) fn window_is_sticky(window_id: WindowId) -> bool` | `pub(crate)` |
| `window_shadow` | `window.c:859` | event loop; also main run loop at start-up | — | `fn window_shadow(window_id: WindowId) -> bool` | private |
| `window_opacity` | `window.c:865` | event loop | — | `fn window_opacity(window_id: WindowId) -> f32` | private |
| `window_parent` | `window.c:872` | event loop; also main run loop at start-up | — | `fn window_parent(window_id: WindowId) -> WindowId` | private |
| `window_level` | `window.c:899` | event loop; also main run loop at start-up | — | `pub(crate) fn window_level(window_id: WindowId) -> i32` | `pub(crate)` |
| `SLSGetWindowSubLevel__Internal` | `window.c:930` | event loop | — | `fn SLSGetWindowSubLevel__Internal(connection_id: i32, window_id: WindowId) -> i32` | private |
| `window_sub_level` | `window.c:954` | event loop | — | `pub(crate) fn window_sub_level(window_id: WindowId) -> i32` | `pub(crate)` |
| `window_tags` | `window.c:963` | event loop; also main run loop at start-up | — | `fn window_tags(window_id: WindowId) -> u64` | private |
| `window_ax_role` | `window.c:989` | event loop; also main run loop at start-up | — | `pub(crate) fn window_ax_role(window: &Window) -> Option<CFStringOwned>` | `pub(crate)` |
| `window_role` | `window.c:996` | event loop; also main run loop at start-up | — | `pub(crate) fn window_role(window: &Window) -> Option<&CFString>` | `pub(crate)` |
| `window_role_ts` | `window.c:1001` | event loop; also main run loop at start-up | — | `pub(crate) fn window_role_ts(window: &Window) -> String` | `pub(crate)` |
| `window_ax_subrole` | `window.c:1010` | event loop; also main run loop at start-up | — | `pub(crate) fn window_ax_subrole(window: &Window) -> Option<CFStringOwned>` | `pub(crate)` |
| `window_subrole` | `window.c:1017` | event loop; also main run loop at start-up | — | `fn window_subrole(window: &Window) -> Option<&CFString>` | private |
| `window_subrole_ts` | `window.c:1022` | event loop; also main run loop at start-up | — | `pub(crate) fn window_subrole_ts(window: &Window) -> String` | `pub(crate)` |
| `window_is_root` | `window.c:1031` | event loop; also main run loop at start-up | `WindowManager` | `fn window_is_root(window: &Window, window_manager: &mut WindowManager) -> bool` | private |
| `window_is_real` | `window.c:1044` | event loop; also main run loop at start-up | — | `pub(crate) fn window_is_real(window: &Window) -> bool` | `pub(crate)` |
| `window_is_standard` | `window.c:1062` | event loop; also main run loop at start-up | — | `pub(crate) fn window_is_standard(window: &Window) -> bool` | `pub(crate)` |
| `window_level_is_standard` | `window.c:1078` | event loop; also main run loop at start-up | — | `pub(crate) fn window_level_is_standard(window: &Window) -> bool` | `pub(crate)` |
| `window_is_unknown` | `window.c:1084` | event loop; also main run loop at start-up | — | `pub(crate) fn window_is_unknown(window: &Window) -> bool` | `pub(crate)` |
| `window_create` | `window.c:1093` | event loop; also main run loop at start-up | `WindowManager` | `pub(crate) fn window_create(application: ProcessId, window_ref: AXUIElementRef, window_id: WindowId, window_manager: &mut WindowManager) -> Window` | `pub(crate)` |
| `window_destroy` | `window.c:1136` | event loop; also main run loop at start-up | — | `pub(crate) fn window_destroy(mut window: Window)` | `pub(crate)` |

Notes on individual rows:

* **`window_observe` / `window_unobserve`.** `&mut Window` because both write `window.notification`
  and the `liveness_reference_held_by_the_observation` slot of `GLOSSARY.md` §3.2;
  `&mut WindowManager` because `window->application->observer_ref` (`window.c:10`, `:26`) is now
  the lookup `window_manager_find_application_observer(window_manager, window.application)`
  (`THREADS.md` §5.4). `state-access.tsv` shows only `g_verbose` for `window_observe` and nothing
  for `window_unobserve`; the `WindowManager` comes from the handle-resolution clause of
  `patterns/state-and-ownership.md` §2.1, which the textual extraction cannot see.
* **`window_display_uuid`.** `SLSCopyManagedDisplayForWindow` / `SLSCopyBestManagedDisplayForRect`
  are copy-rule, so the return is `+1` and nullable.
* **`window_space_list`.** The `int *count` out-parameter collapses into the returned `Vec`. C
  returns `NULL` without writing `*count` when `SLSCopySpacesForWindows` fails (`window.c:94`);
  both that path and `count == 0` become an empty `Vec`. One `DEVIATIONS.md` line.
* **`window_layer`.** Returns one of `layer_str[LAYER_BELOW|NORMAL|ABOVE]` or the literal
  `"unknown"`, all `'static`.
* **`window_nonax_serialize` / `window_serialize`.** `DisplayManager` is in both sets because
  `display_manager_display_id_arrangement` (`display_manager.c:178`, `:180`) reads
  `g_display_manager.order`; the `window_serialize` example in
  `patterns/state-and-ownership.md` §2.3 (2) omits it — see §4.
  `SpaceManager` is in the `window_serialize` set because `view_find_window_node` (`window.c:445`)
  resolves a view handle and `window.c:645` reads `view->root`. `MouseDragState` because of
  `window == g_mouse_state.window` (`window.c:706`).
  Inside the body the window record is resolved with recipe R1: read `id`, `frame`, `is_root`,
  `flags`, `scratchpad`, the application `pid`/`name`/`is_hidden` and the role/subrole strings into
  locals before the first call that takes `&mut WindowManager` or `&mut SpaceManager`.
* **`window_role` / `window_subrole`.** The C returns the cached field at `+0`; the Rust returns a
  borrow of the same field, so no retain is added. `window_is_real` and `window_is_standard` need
  both at once and therefore read `window.role` and `window.subrole` off the one `&Window` they
  already hold rather than calling through twice.
* **`window_level_is_standard`.** Reads `window.id`, then `window_level(window_id)`;
  `g_layer_normal_window_level` is the `LAYER_NORMAL_WINDOW_LEVEL` static, not a parameter.
* **`SLSGetWindowSubLevel__Internal`.** Name kept verbatim (`DECISIONS.md` 37,
  `TRANSLATION_PLAN.md` W2-window), so the item carries `#[allow(non_snake_case)]`. `cid` →
  `connection_id` (`GLOSSARY.md` §10.2). The packed message struct is `#[repr(C, packed(4))]`
  with a `const` size assertion (`DECISIONS.md` 35).
* **`window_create`.** `application` is the `ProcessId` the C pointer becomes, because
  `GLOSSARY.md` §3.2 stores it as `Window::application: Option<ProcessId>`; `&mut WindowManager`
  is needed to resolve it for `window_is_root`. The application **is** in
  `WindowManager::application` at both call sites (`event_loop.c:172` precedes `:175`,
  `window_manager.c:2751` precedes the window pass). `window_ref` keeps its C name and is adopted
  at `+1` (`THREADS.md` §5.1). The returned `Window` is never `None`: C's `malloc` cannot fail
  here and no branch returns `NULL`.
* **`window_destroy`.** Takes the `Window` by value, which is what C's `free` becomes; `mut`
  because `window.id = 0` (`window.c:1138`) must still happen before the CF releases, and the
  releases are the `Drop` of `role`, `subrole`, `title` and `element_ref`. It no longer frees
  `scratchpad`, which is an owned `Option<String>` clone (`patterns/state-and-ownership.md` §3.2).

---

## 3. `src/application.h`, `src/application.c`, `src/process_manager.h`, `src/process_manager.c`

`application.h` and `process_manager.h` define no functions; they hold types, the two X-macro
notification tables and the `observer_callback` / `process_event_handler` typedefs
(`GLOSSARY.md` §2.4).

| C name | file:line | thread context | event-loop-owned managers touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `application_notification_handler` | `application.c:6` | main run loop | — | `unsafe extern "C-unwind" fn application_notification_handler(observer: NonNull<AXObserver>, element: NonNull<AXUIElement>, notification: NonNull<CFString>, context: *mut c_void)` | private |
| `application_observe` | `application.c:43` | event loop; also main run loop at start-up | — | `pub(crate) fn application_observe(application: &mut Application) -> bool` | `pub(crate)` |
| `application_unobserve` | `application.c:63` | event loop; also main run loop at start-up | — | `pub(crate) fn application_unobserve(application: &mut Application)` | `pub(crate)` |
| `application_main_window` | `application.c:79` | — | — | **not translated** — no caller (§5) | — |
| `application_focused_window` | `application.c:91` | event loop | — | `pub(crate) fn application_focused_window(application: &Application) -> WindowId` | `pub(crate)` |
| `application_is_frontmost` | `application.c:103` | event loop | — | `pub(crate) fn application_is_frontmost(application: &Application) -> bool` | `pub(crate)` |
| `application_is_hidden` | `application.c:112` | event loop; also main run loop at start-up | — | `fn application_is_hidden(application: &Application) -> bool` | private |
| `application_window_list` | `application.c:118` | event loop; also main run loop at start-up | — | `pub(crate) fn application_window_list(application: &Application) -> Option<CFRetained<CFArray>>` | `pub(crate)` |
| `application_create` | `application.c:125` | event loop; also main run loop at start-up | — | `pub(crate) fn application_create(process: &Arc<Process>) -> Application` | `pub(crate)` |
| `application_destroy` | `application.c:140` | event loop; also main run loop at start-up | — | `pub(crate) fn application_destroy(application: Application)` | `pub(crate)` |
| `hash_psn` | `process_manager.c:4` | any | — | `fn hash_process_serial_number(key: &ProcessSerialNumber) -> u64` | private |
| `compare_psn` | `process_manager.c:9` | any | — | **no function** — `impl PartialEq for ProcessSerialNumber` comparing both longs | private |
| `process_pid_for_psn` | `process_manager.c:24` | main run loop | — | `fn process_pid_for_psn(process_serial_number: ProcessSerialNumber) -> ProcessId` | private |
| `process_create` | `process_manager.c:31` | main run loop | — | `fn process_create(process_serial_number: ProcessSerialNumber, process_id: ProcessId) -> Option<Arc<Process>>` | private |
| `process_is_being_debugged` | `process_manager.c:70` | main run loop | — | `fn process_is_being_debugged(process_id: ProcessId) -> bool` | private |
| `process_manager_active_space_for_psn` | `process_manager.c:82` | event loop | — | `pub(crate) fn process_manager_active_space_for_psn(connection: i32) -> SpaceId` | `pub(crate)` |
| `process_handler` | `process_manager.c:151` | main run loop | — | `unsafe extern "C-unwind" fn process_handler(handler_call_ref: EventHandlerCallRef, event: EventRef, context: *mut c_void) -> OSStatus` | private |
| `process_manager_add_running_processes` | `process_manager.c:208` | start-up only (main run loop) | `ProcessManager` | `fn process_manager_add_running_processes(process_manager: &mut ProcessManager)` | private |
| `process_manager_begin` | `process_manager.c:230` | start-up only (main run loop) | `ProcessManager` | `pub(crate) fn process_manager_begin(process_manager: &mut ProcessManager) -> bool` | `pub(crate)` |
| `process_manager_find_process` | `process_manager.c:255` | main run loop | — | `pub(crate) fn process_manager_find_process(process_serial_number: &ProcessSerialNumber) -> Option<Arc<Process>>` | `pub(crate)` |
| `process_destroy` | `process_manager.c:260` | event loop | — | `pub(crate) fn process_destroy(process: Arc<Process>)` | `pub(crate)` |

Notes on individual rows:

* **`application_notification_handler`.** Signature copied from `THREADS.md` §5.3. It is an
  `AXObserverCreate` callback, so it keeps the C ABI and all four parameters even though
  `observer` is unused. `context` is the raw `*const WindowLivenessCell` of `DECISIONS.md` 21,
  not a `struct window *`. No manager: `DECISIONS.md` 20.
* **`application_observe` / `application_unobserve`.** `&mut Application` — both write
  `notification` and `is_observing`, `application_observe` also writes `observer_ref` and
  `ax_retry`. `state-access.tsv` lists only `g_verbose`, a static. Per `DECISIONS.md` 20 and
  `THREADS.md` §5.4 the `AXObserverRemoveNotification` / `CFRunLoopSourceInvalidate` / `CFRelease`
  half of `application_unobserve` moves to a main-queue trampoline; the trampoline is a Rust-only
  helper and does not change this signature.
* **`application_window_list`.** `AXUIElementCopyAttributeValue(kAXWindowsAttribute)` is copy-rule
  and nullable. The C declares the result `CFArrayRef` without checking the type; the Rust casts
  the same way rather than validating.
* **`application_create`.** `&Arc<Process>` rather than a `ProcessId`: the process lives in the
  `PROCESS_TABLE` static (`DECISIONS.md` 22), not in an event-loop-owned manager, and the C
  reaches `process->pid`, `process->psn` and `process->name` through the pointer it is given.
  `Application::name` is the `Arc<str>` clone of `Process::name` (`GLOSSARY.md` §3.3), which is
  what `application.c:133`'s alias becomes. `g_connection` at `:135` is the `CONNECTION` static.
  Never `None`: no branch in the C returns `NULL`.
* **`application_destroy`.** Takes the `Application` by value; the `CFRelease` at `:142` and the
  `free` at `:143` are both its `Drop`.
* **`hash_psn` / `compare_psn`.** `patterns/state-and-ownership.md` §1.3: the hash keeps
  `lowLongOfPSN` and is renamed `hash_process_serial_number`; the comparator vanishes into a
  hand-written `PartialEq` that compares both longs, matching `SameProcess`.
* **`process_create`.** Returns `None` on the three C `NULL` paths plus the new one: a `NULL`
  `cfstring_copy` makes it return `None` instead of letting `string_equals(NULL, ..)` at
  `process_manager.c:54` dereference it (`GLOSSARY.md` §3.4). One `DEVIATIONS.md` line.
  `Process::policy` is explicitly initialised where C leaves it uninitialised — one more
  `DEVIATIONS.md` line.
* **`process_manager_active_space_for_psn`.** Parameter keeps its C name `connection`
  (`GLOSSARY.md` §12); it is *not* the `CONNECTION` static, which the body reads separately at
  `:113` and `:119`. No manager: `display_manager_active_display_list`, `display_space_list` and
  `window_space` all reach only `g_connection`.
* **`process_handler`.** `ref` is a Rust keyword, so the first parameter is `handler_call_ref`
  (`GLOSSARY.md` §11). The C `context` is `&g_process_manager`, used only for `pm->process`
  (`:162`, `:182`, `:186`, `:190`, `:197`), which is now the `PROCESS_TABLE` static — so the
  parameter survives as the unused ABI slot and no manager appears. `OSStatus` stays an integer
  because Carbon reads `noErr` and `-1` (`DECISIONS.md` 32).
* **`process_manager_add_running_processes` / `process_manager_begin`.** Both run on the main
  thread, but at start-up, which `patterns/state-and-ownership.md` §1.4 step 3 runs **against
  `&mut state`** before the state is moved into the event-loop thread. `DECISIONS.md` 20 forbids
  a manager in a main-thread *callback*, not in start-up code, so `&mut ProcessManager` is
  correct: `finder_psn` (`:223`), `front_pid`/`last_front_pid`/`switch_event_time` (`:248-250`)
  are `ProcessManager` fields. `target`/`handler`/`type`/`ref` (`:232-239`, `:251`) go into the
  `CARBON_PROCESS_EVENT_INSTALLATION` static and `table_init` (`:240`) into `PROCESS_TABLE`, so
  neither contributes a parameter.
* **`process_manager_find_process`.** The declared `struct process_manager *pm` covers only
  `pm->process`, now a static, so the parameter disappears entirely — statics are never
  parameters (`patterns/state-and-ownership.md` §2.1). Returns a cloned `Arc<Process>` because a
  reference cannot leave the `Mutex` guard.
* **`process_destroy`.** Takes the `Arc<Process>` by value: the C `free(process->name)` +
  `free(process)` is the `Drop` of the last strong count. What is left in the body is the
  `workspace_application_destroy_running_ns_application` call against the `WORKSPACE_CONTEXT`
  static (`patterns/state-and-ownership.md` §1.2).

---

## 4. Judgement calls

Every place the mechanical rule was ambiguous or two contract documents disagreed, and what this
file chose.

1. **`struct window *` / `struct application *` parameters do not become handles in these two
   files.** `patterns/state-and-ownership.md` §2.2 step 1 says they do. Chosen: the record
   (`&Window` / `&mut Window` / `Window`, `&Application` / `&mut Application` / `Application`),
   because at the C call sites listed in §1 the record is not in `WindowManager::window` /
   `WindowManager::application`, so a handle would resolve to a miss and `window_create` could not
   populate the record it is building at all. `THREADS.md` §5.4 fixes this shape literally for
   `window_observe`/`window_unobserve` and `patterns/idioms-and-conventions.md` §6.2 for the six
   flag accessors. `window_serialize` keeps the handle form, which is the shape
   `patterns/state-and-ownership.md` §2.3 (2) fixes and the only one that works there.
2. **`window_serialize` and `window_nonax_serialize` take `&mut DisplayManager`.**
   `patterns/state-and-ownership.md` §2.3 (2) gives `window_serialize` as
   `(response, window_id, flags, window_manager, space_manager, mouse_drag_state)` — no
   `DisplayManager`. Its `Direct`/callee enumeration skips `window.c:533`, where
   `display_manager_display_id_arrangement` reads `g_display_manager.order`
   (`display_manager.c:178`, `:180`). The `state-access.tsv` rows for both functions list
   `g_display_manager` transitively, and the C confirms it. `DisplayManager` is therefore in both
   sets and, by the `src/yabai.c:27-35` order, sits **before** `window_manager`. Wave 1 must use
   the signature in §2, not the one in §2.3 (2).
3. **Manager parameters stay `&mut`, and are appended, where `THREADS.md` §5.4 writes
   `(window_manager: &WindowManager, window: &mut Window)`.**
   `patterns/state-and-ownership.md` §2.1 ("every manager parameter is `&mut`, no exceptions, no
   read-only variants") and §2.2 step 3 (managers are appended, never a prefix) are the binding
   rule; `THREADS.md` is scoped to thread context and the `Event` enum. So
   `window_observe(window, window_manager)`, not `window_observe(window_manager, window)`.
4. **`FILE *rsp` is `response: &mut Response`, not `&mut dyn std::io::Write`.**
   `patterns/state-and-ownership.md` §2.2 says `&mut dyn std::io::Write`; `DECISIONS.md` 28,
   `GLOSSARY.md` §10.4 and `patterns/message-and-serialisation.md` §7 all say one `Response` type
   that owns the failure prefix and the silent case. `Response` wins.
5. **`CFStringOwned` and `CFRetained<T>` are the same thing under two names.** `GLOSSARY.md` §2.5
   names an owned `CFStringRef` `CFStringOwned`; `patterns/memory-text-and-os-objects.md` §1.1
   says there are exactly two wrapper types and no hand-rolled owned type. Read as: `CFStringOwned`
   is the crate alias for `CFRetained<CFString>` in `crate::ffi`. Owned `CFString` returns are
   spelled `CFStringOwned` (the glossary is the spelling authority); the one owned `CFArray`
   return, `application_window_list`, is spelled `CFRetained<CFArray>`.
6. **`window_role` / `window_subrole` return a borrow, not a retained clone.** The C returns the
   cached field at `+0`. `Option<&CFString>` keeps that exactly; the alternative,
   `Option<CFStringOwned>` via `CFRetained::retain`, would add a retain/release pair per call on
   a path that runs per window per event. The cost is that `window_is_real` and
   `window_is_standard`, which need role and subrole simultaneously, read the two fields off the
   one `&Window` they hold instead of calling through twice (recipe R1).
7. **`compare_psn` gets a row but no signature.** It has a definition and a caller
   (`table_init`, `process_manager.c:240`), so it is not dead, but
   `patterns/state-and-ownership.md` §1.3 dissolves it into `K: PartialEq`. The row records where
   it went rather than omitting it.
8. **`window_create` returns `Window`, not `Option<Window>`.** The "returned record pointer becomes
   `Option` of its handle" rule is for lookups into a collection. `window_create` and
   `application_create` are constructors whose C `malloc` result is never checked and never
   `NULL`; they return the owned value the caller inserts into the table. `process_create` keeps
   the `Option` because its three C `NULL` returns are real.
9. **Thread context for start-up.** "event loop; also main run loop at start-up" means the
   function is reachable from `window_manager_begin` (`yabai.c:338`), which
   `patterns/state-and-ownership.md` §1.4 step 3 runs on the main thread against `&mut state`
   before the state is moved. It is not a `DECISIONS.md` 20 main-thread callback and may take a
   manager. The only true main-thread callbacks here are `application_notification_handler`,
   `process_handler` and, through them, `process_pid_for_psn`, `process_create`,
   `process_is_being_debugged` and `process_manager_find_process` — none of which takes a manager.
   `process_manager_find_process` is also called from the two `dispatch_get_main_queue` blocks at
   `event_loop.c:94` and `:158`, which is still MAIN.
10. **`hash_psn` and the six flag accessors are "any".** They are called from both threads and
    reach nothing but their arguments.
11. **`window_display_id` is marked "event loop" only.** Its `window_manager.c` call sites
    (`:258`, `:2131`, `:2281`, `:2388`, `:2414`) are message commands and `space_manager.c:658`
    and `message.c:2470` are both reached from `EVENT_HANDLER(DAEMON_MESSAGE)`. If a start-up path
    to it is found later the column changes; the signature does not.

---

## 5. Not translated

| C name | file:line | why |
| --- | --- | --- |
| `window_unknown_serialize` | `window.h:140` | declaration with no definition and no caller (`DECISIONS.md` 5). The definition that exists is `window_nonax_serialize` (`window.c:121`), under a different name. One `DEVIATIONS.md` line |
| `application_main_window` | `application.c:79` | defined, declared at `application.h:84`, **no caller anywhere in `src/`** (`DECISIONS.md` 5). One `DEVIATIONS.md` line |

`TIME_FUNCTION` at `window.c:123` and `:411` is the `PROFILE` machinery of `DECISIONS.md` 5 and
disappears from both bodies; it is not a function of these files.

---

## 6. Functions of other modules these three call

The shape wave 1 must expect at the call site. Anything here that another W0b-1 sibling document
contradicts, that sibling wins for its own module.

**From `window.c`**

* `window_manager_find_managed_window(window_manager: &mut WindowManager, window_id: WindowId) -> Option<SpaceId>` — `window.c:444`. C returns `struct view *`; `patterns/state-and-ownership.md` §3.2 makes `WindowManager::managed_window` a `Table<WindowId, SpaceId>`, so the caller resolves the `SpaceId` through `SpaceManager::view`.
* `view_find_window_node(space_manager: &mut SpaceManager, space_id: SpaceId, window_id: WindowId) -> Option<NodeId>` — `window.c:445`, per §2.2 step 2 (`struct view *` contributes `&mut SpaceManager` plus the `SpaceId`). `patterns/state-and-ownership.md` §4.1 writes the same call as `view_find_window_node(view: &mut View, window_id)`; either form resolves the same handle, and the W0b `view.c` document settles which.
* `window_node_is_left_child(space_id: SpaceId, node_id: NodeId, space_manager: &mut SpaceManager) -> bool` — `window.c:594`.
* `window_node_index_of_window(space_id: SpaceId, node_id: NodeId, window_id: WindowId, space_manager: &mut SpaceManager) -> i32` — `window.c:601`.
* `display_manager_display_id_arrangement(display_id: DisplayId, display_manager: &mut DisplayManager) -> i32` — `window.c:235`, `:533`.
* `space_display_id(space_id: SpaceId) -> DisplayId` — `window.c:235`, `:533`.
* `space_manager_mission_control_index(space_id: SpaceId) -> i32` — `window.c:243`, `:541`. No manager.
* `space_is_fullscreen(space_id: SpaceId) -> bool` — `window.c:359`, `:1125`.
* `space_is_visible(space_id: SpaceId) -> bool` — `window.c:670`.
* `workspace_is_macos_ventura() / _sonoma() / _sequoia() / _tahoe() -> bool` — `window.c:903`, `:948`.
* `string_escape(text: &str) -> Option<String>` — `ts_string_escape`, `window.c:177`, `:187`, `:477`, `:487`; keeps the "`None` means nothing was escaped" contract of `patterns/memory-text-and-os-objects.md` §2.3.
* `ts_cfstring_copy(string_ref: &CFString) -> String` and `ts_string_copy(text: &str) -> String` — `window.c:717-726`, `:1004-1027`; both become owned `String` (`DECISIONS.md` 17).
* `cfarray_of_cfnumbers(values: &[T], number_type: CFNumberType) -> CFRetained<CFArray>` — `window.c:71`, `:92`, `:847`, `:876`, `:904`, `:966`; the `(pointer, element size, count)` triple collapses into one slice.
* `json_bool(value: bool) -> &'static str` — 18 sites.
* `CGS_GET_CONNECTION_PORT_BY_ID` — the `OnceLock<Option<extern "C" fn(i32) -> mach_port_t>>` static of `patterns/state-and-ownership.md` §1.3; its `None` drives the branch at `window.c:956`.

**From `application.c`**

* `event_loop_post(event: Event)` — `application.c:9-38`, ten sites. `DECISIONS.md` 19: the whole typed `Event` replaces `(&g_event_loop, type, context, 0)`; the variants are `Event::WindowCreated`, `WindowFocused`, `WindowMoved`, `WindowResized`, `WindowTitleChanged`, `MenuOpened`, `MenuClosed`, `WindowMinimized`, `WindowDeminimized`, `WindowDestroyed` (`GLOSSARY.md` §4.2). `WindowCreated` carries the element at `+1` as `SendCFRetained`; the rest carry a `WindowId`, and `MenuClosed` carries nothing.
* `ax_window_id(element: AXUIElementRef) -> u32` — `application.c:12-20`, `:85`, `:97`. Keeps the `0` sentinel.
* `psn_equals(first: &ProcessSerialNumber, second: &ProcessSerialNumber) -> bool` — `application.c:107`.

**From `process_manager.c`**

* `workspace_application_create_running_ns_application(process: &Arc<Process>) -> *mut c_void` — `process_manager.c:66`; stored into `Process::ns_application` (`AtomicPtr<c_void>`).
* `workspace_application_destroy_running_ns_application(workspace_context: &WorkspaceContext, process: &Arc<Process>)` — `process_manager.c:262`; `workspace_context` comes from the `WORKSPACE_CONTEXT` static, not a parameter of `process_destroy`.
* `workspace_application_unobserve(workspace_context: &WorkspaceContext, process: &Arc<Process>)` — `process_manager.c:191`.
* `display_manager_active_display_list() -> Vec<DisplayId>` — `process_manager.c:87`; the `int *count` out-parameter collapses into the `Vec`.
* `display_space_list(display_id: DisplayId) -> Option<Vec<SpaceId>>` — `process_manager.c:95`; same collapse. The C's "one linear allocator, track the first list and the total count" idiom at `:98-105` becomes one `Vec<SpaceId>` extended per display (`DECISIONS.md` 17).
* `window_space(window_id: WindowId) -> SpaceId` — `process_manager.c:132`.
* `cfstring_copy(string_ref: &CFString) -> Option<String>` — `process_manager.c:44`; the `None` is what makes `process_create` return `None`.
* `string_equals(first: &str, second: &str) -> bool` — `process_manager.c:54`, `:221`.
* `event_loop_post(event: Event)` — `process_manager.c:183`, `:194`, `:200`, carrying `Event::ApplicationLaunched(Arc<Process>)`, `Event::ApplicationTerminated(Arc<Process>)`, `Event::ApplicationFrontSwitched(Arc<Process>)`.
* `PROCESS_TABLE.lock()` with `Table::add` / `Table::remove` / `Table::find` — `process_manager.c:182`, `:190`, `:226`, `:240`, `:257`. `DECISIONS.md` 16 and 22: `add` does not overwrite, and the lock is never held across an ObjC or AX call.
