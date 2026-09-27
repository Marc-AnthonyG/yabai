# State access — `src/event_loop.h`, `src/event_loop.c`, `src/mission_control.c`

Wave 0b unit `W0b-1`, module `event_loop`. One row per C function defined in these three files,
in C source order, with the final Rust signature wave 1 pastes verbatim.

`src/event_loop.h` defines no function: it holds the `EVENT_HANDLER` macro (`:4`), the
`EVENT_TYPE_LIST` X-macro (`:6-46`), `enum event_type`, `struct event`, `struct event_loop` and
two declarations whose definitions live in `src/event_loop.c`. `src/event_loop.c` defines 45
functions (40 `EVENT_HANDLER` bodies plus `update_window_notifications`,
`window_did_receive_focus`, `event_loop_run`, `event_loop_post`, `event_loop_begin`).
`src/mission_control.c` defines 5. **50 functions, 50 rows.**

---

## 1. The rule as applied here

`patterns/state-and-ownership.md` §2.2, unchanged:

1. the C parameter list, in C order, with every pointer rewritten to its handle (§3);
2. a manager the C declares keeps its C position — `struct mouse_state *` becomes
   `&mut MouseDragState`, `struct view *` becomes `&mut SpaceManager` immediately followed by the
   `SpaceId`, `struct event_loop *` disappears into the `EVENT_SENDER` static;
3. every remaining event-loop-owned member of `Touches(f)` is **appended after the whole C
   parameter list**, in the `src/yabai.c:27-37` declaration order, which is the
   `EventLoopOwnedState` field order of `GLOSSARY.md` §3.31:

| # | parameter | type |
| --- | --- | --- |
| 1 | `signal_event` | `&mut [Vec<Signal>; SIGNAL_TYPE_COUNT]` |
| 2 | `process_manager` | `&mut ProcessManager` |
| 3 | `display_manager` | `&mut DisplayManager` |
| 4 | `window_manager` | `&mut WindowManager` |
| 5 | `space_manager` | `&mut SpaceManager` |
| 6 | `signal_storage` | `&mut Vec<PendingSignal>` |
| 7 | `mouse_drag_state` | `&mut MouseDragState` |
| 8 | `mission_control_mode` | `&mut MissionControlMode` |
| 9 | `focus_follows_mouse_suspended_value` | `&mut FfmMode` |
| 10 | `is_menu_open` | `&mut i32` |

Rows 2-5 and 7 are the five managers `patterns/state-and-ownership.md` §2.2 step 3 enumerates,
in exactly its order. Rows 1, 6, 8, 9 and 10 are the remaining `EventLoopOwnedState` fields;
§2.2 step 3 does not name them because §2.1 defines `Managers(f)` over managers only, and
`TRANSLATION_PLAN.md` §3.2 step 3 requires them to be scanned for and step 3's row format
requires them to be appended in this same order. See §8, judgement call 1.

Everything else is a static and is never a parameter (`DECISIONS.md` 18, 22, 23):
`CONNECTION`, `VERBOSE`, `BOOTSTRAP_PORT`, the three window levels, `EVENT_SENDER`,
`WORKSPACE_CONTEXT`, `PROCESS_TABLE`, `MOUSE_TAP_STATE`, `MISSION_CONTROL_OBSERVER`, the four
`kAXExpose*` strings and the four `__pending_*` / `__last_*` atomics.

Every state parameter is `&mut`, with no read-only variant (`patterns/state-and-ownership.md`
§2.1).

**Handler payloads.** The C `EVENT_HANDLER(t)` macro expands to
`void EVENT_HANDLER_t(void *context, int param1)`. `DECISIONS.md` 19 replaces the pair with the
`Event` variant's owned payload (`THREADS.md` §3.1), so the handler's C parameter list is that
payload, field for field, in variant order. Handler names come from `GLOSSARY.md` §4.2.

**Return types.** `bool` stays `bool` (`DECISIONS.md` 32). No function in these files returns a
record pointer, a list, or a `FILE *rsp`-driven response, so those clauses of the rule are
vacuous here.

---

## 2. `src/event_loop.c` — 45 rows

Thread contexts are the `THREADS.md` §1.1 names: **MAIN** = main run loop, **EVENTLOOP** = the
event-loop thread, **MSGLOOP** = the message accept thread.

| C name | file:line | thread context | event-loop-owned state touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `update_window_notifications` | `src/event_loop.c:16` | MAIN (start-up only, `yabai.c:341`) + EVENTLOOP | `window_manager` | `pub(crate) fn update_window_notifications(window_manager: &mut WindowManager)` | `pub(crate)` |
| `window_did_receive_focus` | `src/event_loop.c:36` | EVENTLOOP | `window_manager`, `space_manager`, `mouse_drag_state` | `fn window_did_receive_focus(window_manager: &mut WindowManager, mouse_drag_state: &mut MouseDragState, window_id: WindowId, space_manager: &mut SpaceManager)` | private |
| `EVENT_HANDLER_APPLICATION_LAUNCHED` | `src/event_loop.c:75` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state`, `mission_control_mode` | `fn event_handler_application_launched(process: Arc<Process>, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | private |
| `EVENT_HANDLER_APPLICATION_TERMINATED` | `src/event_loop.c:250` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state` | `fn event_handler_application_terminated(process: Arc<Process>, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState)` | private |
| `EVENT_HANDLER_APPLICATION_FRONT_SWITCHED` | `src/event_loop.c:349` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state`, `mission_control_mode` | `fn event_handler_application_front_switched(process: Arc<Process>, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | private |
| `EVENT_HANDLER_APPLICATION_VISIBLE` | `src/event_loop.c:426` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage` | `fn event_handler_application_visible(process_id: ProcessId, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>)` | private |
| `EVENT_HANDLER_APPLICATION_HIDDEN` | `src/event_loop.c:490` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state` | `fn event_handler_application_hidden(process_id: ProcessId, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState)` | private |
| `EVENT_HANDLER_WINDOW_CREATED` | `src/event_loop.c:551` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state`, `mission_control_mode` | `fn event_handler_window_created(element_ref: SendCFRetained<AXUIElement>, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | private |
| `EVENT_HANDLER_WINDOW_DESTROYED` | `src/event_loop.c:603` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state` | `fn event_handler_window_destroyed(window_id: WindowId, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState)` | private |
| `EVENT_HANDLER_WINDOW_FOCUSED` | `src/event_loop.c:636` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state` | `fn event_handler_window_focused(window_id: WindowId, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState)` | private |
| `EVENT_HANDLER_WINDOW_MOVED` | `src/event_loop.c:675` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state` | `fn event_handler_window_moved(window_id: WindowId, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState)` | private |
| `EVENT_HANDLER_WINDOW_RESIZED` | `src/event_loop.c:725` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state` | `fn event_handler_window_resized(window_id: WindowId, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState)` | private |
| `EVENT_HANDLER_WINDOW_MINIMIZED` | `src/event_loop.c:829` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state` | `fn event_handler_window_minimized(window_id: WindowId, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState)` | private |
| `EVENT_HANDLER_WINDOW_DEMINIMIZED` | `src/event_loop.c:873` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage` | `fn event_handler_window_deminimized(window_id: WindowId, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>)` | private |
| `EVENT_HANDLER_WINDOW_TITLE_CHANGED` | `src/event_loop.c:924` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage` | `fn event_handler_window_title_changed(window_id: WindowId, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>)` | private |
| `EVENT_HANDLER_SLS_WINDOW_ORDERED` | `src/event_loop.c:944` | EVENTLOOP | `window_manager`, `space_manager` | `fn event_handler_sls_window_ordered(window_id: WindowId, window_manager: &mut WindowManager, space_manager: &mut SpaceManager)` | private |
| `EVENT_HANDLER_SLS_WINDOW_DESTROYED` | `src/event_loop.c:952` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state` | `fn event_handler_sls_window_destroyed(window_id: WindowId, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState)` | private |
| `EVENT_HANDLER_SLS_SPACE_CREATED` | `src/event_loop.c:968` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage` | `fn event_handler_sls_space_created(space_id: SpaceId, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>)` | private |
| `EVENT_HANDLER_SLS_SPACE_DESTROYED` | `src/event_loop.c:980` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state` | `fn event_handler_sls_space_destroyed(space_id: SpaceId, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState)` | private |
| `EVENT_HANDLER_SPACE_CHANGED` | `src/event_loop.c:994` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state`, `mission_control_mode` | `fn event_handler_space_changed(signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | private |
| `EVENT_HANDLER_DISPLAY_CHANGED` | `src/event_loop.c:1031` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state`, `mission_control_mode` | `fn event_handler_display_changed(signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | private |
| `EVENT_HANDLER_DISPLAY_ADDED` | `src/event_loop.c:1083` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state` | `fn event_handler_display_added(display_id: DisplayId, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState)` | private |
| `EVENT_HANDLER_DISPLAY_REMOVED` | `src/event_loop.c:1092` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state` | `fn event_handler_display_removed(display_id: DisplayId, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState)` | private |
| `EVENT_HANDLER_DISPLAY_MOVED` | `src/event_loop.c:1101` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage` | `fn event_handler_display_moved(display_id: DisplayId, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>)` | private |
| `EVENT_HANDLER_DISPLAY_RESIZED` | `src/event_loop.c:1109` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage` | `fn event_handler_display_resized(display_id: DisplayId, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>)` | private |
| `EVENT_HANDLER_MOUSE_DOWN` | `src/event_loop.c:1117` | EVENTLOOP | `window_manager`, `mouse_drag_state`, `mission_control_mode` | `fn event_handler_mouse_down(event: SendCFRetained<CGEvent>, event_modifier: MouseMod, window_manager: &mut WindowManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | private |
| `EVENT_HANDLER_MOUSE_UP` | `src/event_loop.c:1154` | EVENTLOOP | `display_manager`, `window_manager`, `space_manager`, `mouse_drag_state`, `mission_control_mode` | `fn event_handler_mouse_up(event: SendCFRetained<CGEvent>, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | private |
| `EVENT_HANDLER_MOUSE_DRAGGED` | `src/event_loop.c:1235` | EVENTLOOP | `display_manager`, `window_manager`, `space_manager`, `mouse_drag_state`, `mission_control_mode` | `fn event_handler_mouse_dragged(event: SendCFRetained<CGEvent>, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | private |
| `EVENT_HANDLER_MOUSE_MOVED` | `src/event_loop.c:1345` | EVENTLOOP | `display_manager`, `window_manager`, `space_manager`, `mouse_drag_state`, `mission_control_mode` | `fn event_handler_mouse_moved(event: SendCFRetained<CGEvent>, event_modifier: MouseMod, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | private |
| `EVENT_HANDLER_MISSION_CONTROL_SHOW_ALL_WINDOWS` | `src/event_loop.c:1452` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mission_control_mode` | `fn event_handler_mission_control_show_all_windows(signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mission_control_mode: &mut MissionControlMode)` | private |
| `EVENT_HANDLER_MISSION_CONTROL_SHOW_FRONT_WINDOWS` | `src/event_loop.c:1459` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mission_control_mode` | `fn event_handler_mission_control_show_front_windows(signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mission_control_mode: &mut MissionControlMode)` | private |
| `EVENT_HANDLER_MISSION_CONTROL_SHOW_DESKTOP` | `src/event_loop.c:1466` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mission_control_mode` | `fn event_handler_mission_control_show_desktop(signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mission_control_mode: &mut MissionControlMode)` | private |
| `EVENT_HANDLER_MISSION_CONTROL_ENTER` | `src/event_loop.c:1473` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mission_control_mode` | `fn event_handler_mission_control_enter(signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mission_control_mode: &mut MissionControlMode)` | private |
| `EVENT_HANDLER_MISSION_CONTROL_CHECK_FOR_EXIT` | `src/event_loop.c:1485` | EVENTLOOP | `mission_control_mode` | `fn event_handler_mission_control_check_for_exit(mission_control_mode: &mut MissionControlMode)` | private |
| `EVENT_HANDLER_MISSION_CONTROL_EXIT` | `src/event_loop.c:1528` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage`, `mouse_drag_state`, `mission_control_mode` | `fn event_handler_mission_control_exit(signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | private |
| `EVENT_HANDLER_DOCK_DID_RESTART` | `src/event_loop.c:1545` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage` | `fn event_handler_dock_did_restart(signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>)` | private |
| `EVENT_HANDLER_MENU_OPENED` | `src/event_loop.c:1564` | EVENTLOOP | `window_manager`, `focus_follows_mouse_suspended_value`, `is_menu_open` | `fn event_handler_menu_opened(window_id: WindowId, window_manager: &mut WindowManager, focus_follows_mouse_suspended_value: &mut FfmMode, is_menu_open: &mut i32)` | private |
| `EVENT_HANDLER_MENU_CLOSED` | `src/event_loop.c:1575` | EVENTLOOP | `window_manager`, `focus_follows_mouse_suspended_value`, `is_menu_open` | `fn event_handler_menu_closed(window_manager: &mut WindowManager, focus_follows_mouse_suspended_value: &mut FfmMode, is_menu_open: &mut i32)` | private |
| `EVENT_HANDLER_MENU_BAR_HIDDEN_CHANGED` | `src/event_loop.c:1587` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage` | `fn event_handler_menu_bar_hidden_changed(signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>)` | private |
| `EVENT_HANDLER_DOCK_DID_CHANGE_PREF` | `src/event_loop.c:1594` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage` | `fn event_handler_dock_did_change_pref(signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>)` | private |
| `EVENT_HANDLER_SYSTEM_WOKE` | `src/event_loop.c:1601` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage` | `fn event_handler_system_woke(signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>)` | private |
| `EVENT_HANDLER_DAEMON_MESSAGE` | `src/event_loop.c:1614` | EVENTLOOP | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `mouse_drag_state`, `mission_control_mode` | `fn event_handler_daemon_message(stream: UnixStream, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | private |
| `event_loop_run` | `src/event_loop.c:1647` | EVENTLOOP (it is this thread's entry point) | the whole `EventLoopOwnedState`, by value | `pub(crate) fn event_loop_run(event_receiver: Receiver<Event>, mut event_loop_owned_state: EventLoopOwnedState)` | `pub(crate)` |
| `event_loop_post` | `src/event_loop.c:1684` | any (MAIN, EVENTLOOP, MSGLOOP, CVLINK) | none | `pub(crate) fn event_loop_post(event: Event)` | `pub(crate)` |
| `event_loop_begin` | `src/event_loop.c:1705` | MAIN, start-up only | none | `pub(crate) fn event_loop_begin() -> (Sender<Event>, Receiver<Event>)` | `pub(crate)` |

---

## 3. `src/mission_control.c` — 5 rows

| C name | file:line | thread context | event-loop-owned state touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `connection_handler` | `src/mission_control.c:7` | MAIN | none (decision 20) | `pub(crate) unsafe extern "C-unwind" fn connection_handler(notification_type: u32, data: *mut c_void, data_length: usize, context: *mut c_void, connection_id: i32)` | `pub(crate)` |
| `mission_control_notification_handler` | `src/mission_control.c:59` | MAIN | none (decision 20) | `unsafe extern "C-unwind" fn mission_control_notification_handler(observer: NonNull<AXObserver>, element: NonNull<AXUIElement>, notification: NonNull<CFString>, context: *mut c_void)` | private |
| `mission_control_observe` | `src/mission_control.c:73` | MAIN (start-up, `yabai.c:316`) + EVENTLOOP (`event_loop.c:1555`) | none — `MISSION_CONTROL_OBSERVER` static only | `pub(crate) fn mission_control_observe()` | `pub(crate)` |
| `mission_control_unobserve` | `src/mission_control.c:93` | EVENTLOOP (`event_loop.c:1554`) | none — `MISSION_CONTROL_OBSERVER` static only | `pub(crate) fn mission_control_unobserve()` | `pub(crate)` |
| `mission_control_is_active` | `src/mission_control.c:108` | EVENTLOOP | `mission_control_mode` | `pub(crate) fn mission_control_is_active(mission_control_mode: &mut MissionControlMode) -> bool` | `pub(crate)` |

---

## 4. Dead functions — not translated

| C name | file:line | why |
| --- | --- | --- |
| — | — | none |

Every function in these three files has a definition and a live caller. The 40 handlers are
reached through the `EVENT_TYPE_LIST` dispatch at `src/event_loop.c:1665-1667`, which a textual
grep does not see; `connection_handler` is reached through the six
`SLSRegisterConnectionNotifyProc` registrations at `src/yabai.c:322-333` and
`mission_control_notification_handler` through `AXObserverCreate` at `src/mission_control.c:80`.
There is no `#if 0` in any of the three files.

What is dropped from inside a surviving function, each one line in `DEVIATIONS.md`:

| dropped | site | why |
| --- | --- | --- |
| `struct event_loop::is_running`, `::thread`, `::semaphore`, `::pool`, `::head`, `::tail` | `src/event_loop.h:63-71` | `DECISIONS.md` 19; `GLOSSARY.md` §3.25 |
| `memory_pool_init` / `sem_open` / `sem_unlink` / `pthread_create` in `event_loop_begin` | `src/event_loop.c:1707-1718` | the pool and the semaphore are gone; the spawn moves to `main` (`DECISIONS.md` 12) |
| `memory_pool_push` and the two CAS loops in `event_loop_post` | `src/event_loop.c:1688-1701` | `mpsc` owns the queue |
| `profile_begin` / `profile_end_and_print` in `event_loop_run` | `src/event_loop.c:1657`, `:1674` | `DECISIONS.md` 5, the `PROFILE` machinery |
| `TIME_FUNCTION` in `EVENT_HANDLER_DAEMON_MESSAGE` | `src/event_loop.c:1616` | same |
| `ts_reset()` in `event_loop_run` | `src/event_loop.c:1671` | `DECISIONS.md` 17, the `ts` arena is gone |
| `sem_wait` and the empty-queue churn | `src/event_loop.c:1679` | `Receiver::recv` blocks instead |

---

## 5. Derivations that are not a straight read of `state-access.tsv`

The tsv closure is textual and does not model `DECISIONS.md` 14. Five rows differ from it, and
each difference is forced by `patterns/state-and-ownership.md` §2.1 or §3.2.

1. **`window_did_receive_focus` gains `space_manager`.** The tsv row is
   `g_connection,g_window_manager`. In C, `window_manager_find_managed_window` returns the
   `struct view *` stored directly in `g_window_manager.managed_window`
   (`window_manager.c:283`), so no space manager is reached. §3.2 makes `managed_window` a
   `Table<WindowId, SpaceId>` whose value "the caller resolves through `SpaceManager::view`", and
   `view_find_window_node` at `src/event_loop.c:59` is that resolve. §2.1 counts it. This is the
   same correction the binding worked example (2) makes for `window_serialize`
   (`patterns/state-and-ownership.md` §2.2), whose tsv row is missing `g_space_manager` for
   exactly this reason.

2. **`EVENT_HANDLER_SLS_WINDOW_ORDERED` gains `space_manager`.** Its tsv row is
   `g_connection,g_verbose,g_window_manager`. `g_window_manager.insert_feedback` becomes
   `Table<WindowId, (SpaceId, NodeId)>` (§3.2), and `node->feedback_window.id`
   (`src/event_loop.c:948`) reads a field of the node, which is a resolve through
   `SpaceManager::view`. `node->window_order[0]` in the same statement is **not** a resolve — §3.2
   fixes the table key as `node->window_order[0]`, so it is the lookup key already in hand.

3. **`update_window_notifications` does *not* gain `space_manager`.** Its loop body
   (`src/event_loop.c:28`) reads only `node->window_order[0]`, which by the same §3.2 sentence is
   the key, so the node is never resolved. `Managers = { WindowManager }`, matching the binding
   worked example (1) and `THREADS.md` §4.2 step 14.

4. **Every handler that calls `event_signal_push` gains the same six members.**
   `event_signal_push` (`src/event_signal.c:99`) names `g_signal_event` (`:101`),
   `g_signal_storage` (`:107-108`), `g_process_manager.switch_event_time` / `.front_pid` /
   `.last_front_pid` (`:156-160`, `:170-172`, `:184`), `g_window_manager.focused_window_id`
   (`:210`, `:226`), `g_space_manager.current_space_id` / `.last_space_id` (`:252-253`) and
   `g_display_manager.current_display_id` / `.last_display_id` (`:304-305`). `Managers(f)` is
   whole-function, not per-`switch`-arm, so all six travel to all 35 call sites in
   `src/event_loop.c`. This is what makes `signal_event … signal_storage` the common prefix of
   most rows above, and it agrees with the tsv's transitive column on every one of them.

5. **`g_process_manager` reached only as the process table contributes nothing.**
   `process_manager_find_process` (`src/event_loop.c:92`, `:159`) reaches
   `PROCESS_TABLE`, a static (`DECISIONS.md` 22). `EVENT_HANDLER_APPLICATION_LAUNCHED` gets
   `process_manager` from `event_signal_push`, not from those two calls. Likewise every
   `g_mouse_state` that resolves to `mouse_handler_begin` / `mouse_handler_end`
   (`window_manager.c:221`, `:224`, `:226`) reaches `MOUSE_TAP_STATE`, a static
   (`DECISIONS.md` 23), and contributes nothing;
   `window_manager_set_focus_follows_mouse` is the only function whose entire `g_mouse_state`
   use is of that kind.

Spot-checks of the remaining `g_mouse_state` entries in the tsv: for
`EVENT_HANDLER_APPLICATION_LAUNCHED`, `_WINDOW_CREATED`, `_DAEMON_MESSAGE` and the two
`space_manager_refresh_application_windows` callers it arrives through
`window_manager_create_and_add_window` → `window_serialize` (`src/window_manager.c:1523`,
`:1537`), which reads `g_mouse_state.window` (`src/window.c:706`) — the drag half, so
`mouse_drag_state` is real in every one of those rows.

---

## 6. Cross-module calls this module makes

Shapes this module passes. Managers listed in the order this module writes them at the call site.
The owning wave-1 unit fixes the exact parameter names and return types of everything below; what
is binding here is the manager tail, because it is derived from the same fixed point.

| callee | owning file | shape called with |
| --- | --- | --- |
| `event_signal_push` | `src/event_signal.c:99` | `(signal_type, context, signal_event, process_manager, display_manager, window_manager, space_manager, signal_storage)` — the `context` payload type is the event-signal unit's to name |
| `event_signal_flush` | `src/event_signal.c:60` | `(event_loop_owned_state: &mut EventLoopOwnedState)` — `THREADS.md` §9.2; called only from `event_loop_run`, which owns the whole struct |
| `handle_message` | `src/message.c:2979` | `(response: &mut Response, message: &mut [u8], signal_event, process_manager, display_manager, window_manager, space_manager, mouse_drag_state, mission_control_mode)` |
| `window_manager_find_window` | `src/window_manager.c:1394` | `(window_manager, window_id: WindowId) -> Option<WindowId>` |
| `window_manager_find_application` | `src/window_manager.c:1409` | `(window_manager, process_id: ProcessId) -> Option<ProcessId>` |
| `window_manager_add_application` / `_remove_application` | `src/window_manager.c:1419`, `:1414` | `(window_manager, process_id: ProcessId)` |
| `window_manager_find_application_windows` | `src/window_manager.c:1424` | `(window_manager, process_id: ProcessId) -> Vec<WindowId>` — the `int *count` out-parameter collapses into the `Vec` |
| `window_manager_add_application_windows` | `src/window_manager.c:1546` | `(space_manager, window_manager, process_id: ProcessId, process_manager, display_manager, mouse_drag_state, mission_control_mode) -> Vec<WindowId>` — same collapse |
| `window_manager_add_existing_application_windows` | `src/window_manager.c:1607` | `(space_manager, window_manager, process_id: ProcessId, refresh_index: i32, process_manager, display_manager, mouse_drag_state, mission_control_mode)` |
| `window_manager_create_and_add_window` | `src/window_manager.c:1438` | `(space_manager, window_manager, process_id: ProcessId, element_ref: SendCFRetained<AXUIElement>, window_id: WindowId, one_shot_rules: bool, process_manager, display_manager, mouse_drag_state, mission_control_mode) -> Option<WindowId>` |
| `window_manager_find_managed_window` | `src/window_manager.c:283` | `(window_manager, window_id: WindowId) -> Option<SpaceId>` — §3.2 |
| `window_manager_add_managed_window` | `src/window_manager.c:293` | `(window_manager, window_id: WindowId, space_id: SpaceId)` |
| `window_manager_remove_managed_window` | `src/window_manager.c:288` | `(window_manager, window_id: WindowId)` |
| `window_manager_remove_window` / `_purify_window` | `src/window_manager.c:1399`, `:877` | `(window_manager, window_id: WindowId)` |
| `window_manager_remove_scratchpad_for_window` | `src/window_manager.c:2508` | `(window_manager, window_id: WindowId, unfloat: bool, process_manager, display_manager, space_manager) -> bool` |
| `window_manager_set_window_opacity` | `src/window_manager.c:787` | `(window_manager, window_id: WindowId, opacity: f32)` |
| `window_manager_center_mouse` | `src/window_manager.c:242` | `(window_manager, window_id: WindowId)` |
| `window_manager_{add,find,remove}_lost_focused_event` | `src/window_manager.c:1389`, `:1379`, `:1384` | `(window_manager, window_id: WindowId)` |
| `window_manager_{add,find,remove}_lost_front_switched_event` | `src/window_manager.c:1374`, `:1364`, `:1369` | `(window_manager, process_id: ProcessId)` |
| `window_manager_focused_window` | `src/window_manager.c:1352` | `(window_manager) -> Option<WindowId>` |
| `window_manager_should_manage_window` / `_is_window_eligible` / `_adjust_layer` | `src/window_manager.c:272`, `:19`, `:820` | `(window_id: WindowId, window_manager)` — no manager in C; `WindowManager` appended because the handle is resolved |
| `window_manager_find_window_at_point` | `src/window_manager.c:961` | `(window_manager, point: CGPoint) -> Option<WindowId>` |
| `window_manager_find_window_at_point_filtering_window` | `src/window_manager.c:944` | `(window_manager, point: CGPoint, filter_window_id: WindowId) -> Option<WindowId>` |
| `window_manager_focus_window_without_raise` | `src/window_manager.c:1293` | `(process_serial_number: &ProcessSerialNumber, window_id: WindowId, window_manager)` |
| `window_manager_focus_window_with_raise` | `src/window_manager.c:1324` | `(process_serial_number: &ProcessSerialNumber, window_id: WindowId, element_ref, window_manager)` |
| `window_manager_move_window` | `src/window_manager.c:415` | `(window_id: WindowId, x: f32, y: f32, window_manager)` |
| `window_manager_resize_window_relative_internal` | `src/window_manager.c:346` | `(window_id: WindowId, frame: CGRect, direction: u8, dx: i32, dy: i32, animate: bool, window_manager)` |
| `window_manager_wait_for_native_fullscreen_transition` | `src/window_manager.c:2259` | `(window_id: WindowId, window_manager)` |
| `window_manager_validate_and_check_for_windows_on_space` | `src/window_manager.c:2625` | `(space_manager, window_manager, space_id: SpaceId, display_manager)` |
| `window_manager_handle_display_add_and_remove` | `src/window_manager.c:2679` | `(space_manager, window_manager, display_id: DisplayId, display_manager)` |
| `window_manager_correct_for_mission_control_changes` | `src/window_manager.c:2650` | `(space_manager, window_manager, display_manager)` |
| `space_manager_find_view` | `src/space_manager.c:103` | `(space_manager, space_id: SpaceId, display_manager, window_manager) -> SpaceId` — the returned `struct view *` is the handle it was found by |
| `space_manager_tile_window_on_space` | `src/space_manager.c:462` | `(space_manager, window_id: WindowId, space_id: SpaceId, display_manager, window_manager) -> SpaceId` |
| `space_manager_tile_window_on_space_with_insertion_point` | `src/space_manager.c:444` | `(space_manager, window_id: WindowId, space_id: SpaceId, insertion_point: WindowId, display_manager, window_manager) -> SpaceId` |
| `space_manager_untile_window` | `src/space_manager.c:130` | `(space_manager, space_id: SpaceId, window_id: WindowId, display_manager, window_manager)` |
| `space_manager_remove_label_for_space` | `src/space_manager.c:169` | `(space_manager, space_id: SpaceId)` |
| `space_manager_mark_spaces_invalid` | `src/space_manager.c:1122` | `(space_manager, display_manager, window_manager)` |
| `space_manager_mark_spaces_invalid_for_display` | `src/space_manager.c:1106` | `(space_manager, display_id: DisplayId, display_manager, window_manager)` |
| `space_manager_handle_display_add` | `src/space_manager.c:1150` | `(space_manager, display_id: DisplayId, window_manager)` |
| `space_manager_refresh_application_windows` | `src/space_manager.c:1133` | `(space_manager, process_manager, display_manager, window_manager, mouse_drag_state, mission_control_mode) -> bool` |
| `space_manager_active_space` | `src/space_manager.c:653` | `(window_manager) -> SpaceId` |
| `space_manager_cursor_space` | `src/space_manager.c:551` | `() -> SpaceId` |
| `space_manager_is_window_on_space` | `src/space_manager.c:1091` | `(space_id: SpaceId, window_id: WindowId) -> bool` |
| `space_manager_focus_space_using_gesture` | `src/space_manager.c:927` | `(new_display_id: DisplayId, new_space_id: SpaceId, window_manager) -> bool` |
| `display_manager_remove_label_for_display` | `src/display_manager.c:47` | `(display_manager, display_id: DisplayId)` |
| `display_manager_main_display_id` / `_active_display_id` / `_point_display_id` / `_set_active_display_id` | `src/display_manager.c:86`, `:96`, `:129`, `:454` | no manager |
| `display_manager_focus_display_with_window_at_point` | `src/display_manager.c:430` | `(point: CGPoint, window_manager) -> WindowId` |
| `view_find_window_node` | `src/view.c:612` | `(space_manager, space_id: SpaceId, window_id: WindowId) -> Option<NodeId>` |
| `view_add_window_node_with_insertion_point` | `src/view.c:751` | `(space_manager, space_id: SpaceId, window_id: WindowId, insertion_point: WindowId, display_manager, window_manager)` |
| `view_remove_window_node` | `src/view.c:621` | `(space_manager, space_id: SpaceId, window_id: WindowId, display_manager, window_manager)` |
| `view_update` | `src/view.c:968` | `(space_manager, space_id: SpaceId, display_manager, window_manager)` |
| `view_destroy` | `src/view.c:1033` | `(space_manager, space_id: SpaceId, window_manager)` |
| `view_set_flag` / `view_clear_flag` | `src/view.h:219-220` | **macros, not functions** — a bit-or / bit-and-not of `View::flags` on the `View` resolved out of `space_manager`; no call, no row of its own |
| `view_is_invalid` / `view_is_dirty` | `src/view.c:840`, `:845` | `(space_manager, space_id: SpaceId) -> bool` |
| `window_node_flush` | `src/view.c:374` | `(space_id: SpaceId, node_id: NodeId, window_manager, space_manager)` |
| `insert_feedback_show` / `insert_feedback_destroy` | `src/view.c:8`, `:105` | `(space_id: SpaceId, node_id: NodeId, window_manager, space_manager)` — binding worked example (1) |
| `mouse_window_info_populate` | `src/mouse_handler.c:90` | `(mouse_drag_state, info: &mut MouseWindowInfo)` |
| `mouse_determine_drop_action` | `src/mouse_handler.c:108` | `(mouse_drag_state, source_space_id: SpaceId, source_node_id: NodeId, destination_window_id: Option<WindowId>, point: CGPoint, space_manager, window_manager) -> MouseDropAction` |
| `mouse_drop_action_stack` | `src/mouse_handler.c:133` | `(window_manager, space_manager, source_space_id, source_window_id, destination_space_id, destination_window_id, display_manager)` |
| `mouse_drop_action_swap` | `src/mouse_handler.c:153` | `(window_manager, space_manager, source_space_id, source_node_id, source_window_id, destination_space_id, destination_node_id, destination_window_id)` |
| `mouse_drop_action_warp` | `src/mouse_handler.c:181` | `(window_manager, space_manager, source_space_id, source_node_id, source_window_id, destination_space_id, destination_node_id, destination_window_id, split, child, display_manager)` |
| `mouse_drop_no_target` | `src/mouse_handler.c:218` | `(space_manager, window_manager, source_space_id, destination_space_id, window_id, node_id, display_manager)` |
| `mouse_drop_try_adjust_bsp_grid` | `src/mouse_handler.c:232` | `(window_manager, space_manager, space_id, window_id, info: &MouseWindowInfo, display_manager)` |
| `application_create` / `_destroy` / `_observe` / `_unobserve` / `_focused_window` / `_is_frontmost` | `src/application.c:125`, `:140`, `:43`, `:63`, `:91`, `:103` | no manager; `application_create(process: &Arc<Process>) -> Application` |
| `process_destroy` | `src/process_manager.c:260` | `(process: Arc<Process>)` — no manager; `WORKSPACE_CONTEXT` is a static |
| `process_manager_find_process` | `src/process_manager.c:255` | `(process_serial_number: &ProcessSerialNumber) -> Option<Arc<Process>>` — reads `PROCESS_TABLE`, a static |
| `process_manager_active_space_for_psn` | `src/process_manager.c:82` | `(connection_id: i32) -> SpaceId` |
| `window_*` readers | `src/window.c` | two shapes, and `state-access/window-application-process.md` §1 settles which is which. The `uint32_t wid` functions keep the handle: `window_space(window_id) -> SpaceId`, `window_display_id`, `window_level`, `window_sub_level`, `window_is_sticky`. The `struct window *` functions keep the **record**, because `window_create` and `window_manager_create_and_add_window` call them before the record is in any table: `window_check_flag(window: &Window, flag: WindowFlag)`, `window_set_flag` / `window_clear_flag` on `&mut Window`, `window_ax_frame(window: &Window) -> CGRect`, `window_title(window: &Window) -> Option<CFStringOwned>`, `window_title_ts(window: &Window) -> String`, `window_unobserve(window: &mut Window, window_manager: &mut WindowManager)`, `window_destroy(mut window: Window)` |
| `space_is_visible` / `_is_user` / `_is_fullscreen` / `space_display_id` | `src/space.c` | no manager |
| `space_window_list` | `src/space.h:6` | `(space_id: SpaceId, include_minimized: bool, window_manager) -> Option<Vec<WindowId>>` — the `int *count` out-parameter collapses into the `Vec`; `WindowManager` is appended because `space_window_list_for_connection` resolves window handles (`src/space.c:45`, `:58`) |
| `workspace_is_macos_*`, `workspace_application_*` | `src/workspace.m` | no manager; `MACOS_VERSION` and `WORKSPACE_CONTEXT` are statics |
| `rule_check_flag` / `rule_destroy` | `src/rule.c` | no manager; `rule_destroy` becomes `Drop` and `buf_del` becomes `Vec::swap_remove` |
| `display_bounds_constrained` | `src/display.c:123` | `(display_id: DisplayId, include_external_bar: bool, display_manager) -> CGRect` |

---

## 7. What wave 1 imports for this module

```rust
use core::ffi::{c_int, c_void};
use core::ptr::NonNull;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
```

plus, from the crate: `DisplayId`, `NodeId`, `ProcessId`, `SpaceId`, `WindowId`;
`Event`, `EventLoopOwnedState`, `MissionControlMode`, `MissionControlObserver`, `MouseDragState`,
`MouseMod`, `FfmMode`, `PendingSignal`, `Process`, `ProcessManager`, `DisplayManager`,
`WindowManager`, `SpaceManager`, `Signal`, `SIGNAL_TYPE_COUNT`, `SendCFRetained`;
and, from `objc2`: `AXObserver`, `AXUIElement`, `CFString`, `CGEvent`.

---

## 8. Judgement calls

Every place the mechanical rule was ambiguous, and what this unit chose. The other nine wave-0b
agents must make the same choice; where a phase-1 document says otherwise, the losing spelling is
named so the disagreement is visible rather than silent.

1. **The five non-manager `EventLoopOwnedState` fields are appended like managers.**
   `patterns/state-and-ownership.md` §2.2 step 3 enumerates five managers and stops, because §2.1
   defines `Managers(f)` over managers. But `signal_event`, `signal_storage`,
   `mission_control_mode`, `focus_follows_mouse_suspended_value` and `is_menu_open` are
   event-loop-owned state, and `DECISIONS.md` 13 forbids reaching event-loop-owned state through
   a global — so they must be parameters of something. `TRANSLATION_PLAN.md` §3.2 step 3 lists
   them among the globals wave 0b scans for, and its row format says the appended order is "the
   `EventLoopOwnedState` field order". That order restricted to the five managers is exactly
   §2.2 step 3's order, so the two rules agree wherever they overlap; §1 above is their union.
   Rejected: treating them as statics, which `patterns/state-and-ownership.md` §1.2 and
   `GLOSSARY.md` §8.1 both forbid.

2. **They are `&mut`, including the three `Copy` scalars.**
   `mission_control_mode`, `focus_follows_mouse_suspended_value` and `is_menu_open` are `Copy`
   and `mission_control_is_active` only reads its one. They are still `&mut`, for the reason
   §2.1 gives for managers: a read-only callee sits under a mutating caller
   (`event_handler_mission_control_exit` writes the mode at `src/event_loop.c:1542` and reads it
   through `mission_control_is_active` on the way), and a mixed `&`/`&mut` scheme makes that call
   unwritable. Uniform `&mut` reborrows.

3. **`mission_control_is_active` takes a parameter.**
   `patterns/memory-text-and-os-objects.md` §5.2 sketches it as `mission_control_is_active()`
   with no argument and sketches `event_handler_mouse_up` without a `mission_control_mode`
   parameter. That sketch cannot hold under `DECISIONS.md` 13 once the mode is a field. The
   parameter propagates to exactly the eight non-`event_loop.c` call sites the tsv already
   records — `src/space_manager.c:803`, `:853`, `:893`, `:987`, `:1013`, `:1039`, `:1064` and
   `src/display_manager.c:485` — and no further than the tsv's `g_mission_control_mode` column.

4. **Handler parameter order and names come from the `Event` variant, not from `context`/`param1`.**
   `patterns/memory-text-and-os-objects.md` §5.2 writes
   `event_handler_mouse_up(context: SendCFRetained<CGEvent>, param1: i32, …)`. `DECISIONS.md` 19
   replaced `context`/`param1` with a typed payload, and `THREADS.md` §3.1 gives `MouseUp` one
   field and no modifier, so `param1` has nothing left to carry and disappears. Payload
   parameters are named for what they hold — `process`, `process_id`, `window_id`, `space_id`,
   `display_id`, `element_ref`, `event`, `event_modifier`, `stream` — per `DECISIONS.md` 37.
   `event_modifier` rather than `modifier` is mandated by `GLOSSARY.md` §10.4 for
   `src/event_loop.c:1134`.

5. **Two payload parameters are never read by their handler and are kept anyway.**
   `event_handler_menu_opened`'s `window_id` (the C computes `ax_window_id(element)` at
   `src/application.c:20` and the handler ignores `context`) and `event_handler_mouse_moved`'s
   `event_modifier` (`THREADS.md` §3.2: "never read by the handler"). Both are fields of their
   `Event` variant, so the dispatch destructures them; wave 1 binds them as `_window_id` /
   `_event_modifier` at the binding site if the unused-parameter lint fires, but the signature
   keeps the name.

6. **`MouseMod`, not `MouseModifier`.** `THREADS.md` §3.1 spells the `MouseDown`/`MouseMoved`
   payload type `MouseModifier`; `GLOSSARY.md` §5.6 defines `MouseMod(pub u8)` from
   `src/mouse_handler.h:34-42`. The glossary is the binding spelling authority.

7. **`event_loop_begin` returns the channel pair instead of `bool`.**
   The C returns `false` on `memory_pool_init` and `sem_open` failure (`src/event_loop.c:1706`,
   `:1710`); `DECISIONS.md` 19 deletes both, so no failure remains to report and
   `DECISIONS.md` 32's "bool returns stay bool" has nothing to preserve. `THREADS.md` §4.2 step 5
   binds `event_sender` and `event_receiver` separately and step 16 hands `event_sender` to
   `message_loop_begin`, so the pair is what `main` needs. `EVENT_SENDER.set(sender.clone())`
   happens inside `event_loop_begin`, per `patterns/state-and-ownership.md` §1.4 step 2. The
   dropped `bool` and the three lines at `src/yabai.c:291-293` that test it are one
   `DEVIATIONS.md` line.

8. **`event_loop_run` takes `(receiver, state)`, in that order.**
   `THREADS.md` §2.3 writes the function that way as literal code;
   `patterns/state-and-ownership.md` §1.4 step 4 writes the call as
   `event_loop_run(state, receiver)` in prose. The code block wins. Neither order is a C order —
   the C takes one `void *context`.

9. **`event_loop_run` is `pub(crate)` although the tsv shows no cross-file caller.**
   In C it is only ever named by the `pthread_create` inside `event_loop_begin`
   (`src/event_loop.c:1718`). `DECISIONS.md` 12 moves the spawn to `main`
   (`THREADS.md` §4.2 step 15), which is a different file, so the Rust needs `pub(crate)`.

10. **`connection_handler` is `pub(crate)` although the tsv shows no cross-file caller.**
    `src/yabai.c:322-333` passes it to `SLSRegisterConnectionNotifyProc` as a function pointer;
    the tsv's extraction only follows `name(`, so it missed six references.
    `mission_control_notification_handler` really is file-local
    (`AXObserverCreate`, `src/mission_control.c:80`) and stays private.

11. **Visibility is tsv-driven, not "default `pub(crate)`".**
    `patterns/idioms-and-conventions.md` §2 sets a blanket "default `pub(crate)` for every
    function". This unit follows its own brief: `pub(crate)` only where a caller outside the file
    exists, which makes all 40 handlers and `window_did_receive_focus` private. The two rules
    differ only in how loose the skeleton is, and the tighter one is a compile error away from
    being checked.

12. **`update_window_notifications` runs on MAIN at start-up against `&mut WindowManager`.**
    `DECISIONS.md` 20 says a main-thread callback never takes a manager. Start-up is not a
    callback: `patterns/state-and-ownership.md` §1.4 step 3 runs this function, among nine
    others, on the main thread against `&mut state` before the hand-over. `THREADS.md` §4.2
    step 14 writes the call with `&` rather than `&mut`; judgement call 2 applies and it is
    `&mut`.

13. **`EVENT_HANDLER_DAEMON_MESSAGE` builds its `Response` rather than taking one.**
    The "`FILE *rsp` becomes the `Response` parameter in first position" clause applies to
    functions that *take* `FILE *rsp`. This one declares it as a local at
    `src/event_loop.c:1618` and `fdopen`s it at `:1632`, so `Response::to_client(stream)` is
    built inside the handler and the parameter is the `UnixStream`
    (`patterns/message-and-serialisation.md` §7.5).

14. **`handle_message`'s manager tail here is the tsv fixed point, not the three managers
    sketched in `patterns/message-and-serialisation.md` §6.6.** That section itself defers —
    "the exact per-function sets come from the precomputed call graph". Its
    `g_mission_control_mode`, `g_mouse_state`, `g_process_manager` and `g_signal_event` are all
    in the tsv row for `src/message.c:2979`, and `g_signal_storage` is not, which is why
    `event_handler_daemon_message` is the one handler with no `signal_storage` parameter.

15. **The `Response` type is named but not sized here.**
    `handle_message`'s first parameter is `&mut Response`
    (`patterns/message-and-serialisation.md` §6.6 writes `&mut Response`;
    `files/message.md:364` writes `&mut dyn Write`, and
    `patterns/state-and-ownership.md` §2.2 writes `&mut dyn std::io::Write`). `DECISIONS.md` 28
    requires "one `Response` type", so `&mut Response` is what this unit passes. The message unit
    owns the final spelling.
