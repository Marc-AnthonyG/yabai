# `src/message.h` + `src/message.c` — final Rust signatures

Wave 0b, work unit W0b-1. One row per C function, in `src/message.c` source order. Wave 1 pastes
the **Rust signature** column verbatim into `src/message.rs` with `todo!()` for every body; after
wave 1 no signature in this file changes without a `DEVIATIONS.md` line.

## How every row was derived

`DECISIONS.md` 13 and `patterns/state-and-ownership.md` §2, applied in that order, with
`state-access.tsv` used as the starting point and each row re-checked against the C:

1. **C parameter list, in C order**, with every pointer rewritten to its handle
   (`patterns/state-and-ownership.md` §3): `struct window *` → `WindowId`, and a *nullable*
   `struct window *` → `Option<WindowId>` (`DECISIONS.md` 32). `FILE *rsp` becomes
   `response: &mut Response` and keeps its C position, which is first in every function that
   takes one. `char *message` / `char **message` becomes `&mut [u8]` at `handle_message` and
   `&mut MessageCursor` everywhere below it (`patterns/message-and-serialisation.md` §2.1-§2.3).
2. **No manager is declared by any C signature in this file**, so step 2 of §2.2 is vacuous for
   every row: the whole manager block is appended.
3. **Every manager in `Managers(f)` is appended after the whole C parameter list**, in the
   `src/yabai.c:27-37` declaration order — `signal_event` (`:27`), `process_manager` (`:28`),
   `display_manager` (`:29`), `window_manager` (`:30`), `space_manager` (`:31`),
   `mouse_drag_state` (`:33`), `mission_control_mode` (`:37`). Note `window_manager` **before**
   `space_manager`. Every manager parameter is `&mut`, with no read-only variant.
4. **Statics are never parameters.** In this file that removes `g_verbose`
   (`static VERBOSE: AtomicBool`, read/written at `src/message.c:1173-1177`), the event-tap half
   of `g_mouse_state` (`static MOUSE_TAP_STATE`, `:1621-1664` — `modifier`, `action1`, `action2`,
   `drop_action`), `g_event_loop` (`static EVENT_SENDER`, `:3010`), `g_message_loop`
   (`static MESSAGE_LOOP`, `:3003-3044`), `g_connection`, the three window levels, `g_bs_port`
   and the process table.
5. **Out-parameters.** A count out-parameter paired with a returned buffer collapses into the
   returned `Vec`; no function in this file has that shape. `int property_count` collapses into
   the lengths of the two slices it describes. The non-count out-parameters (`char **label`,
   `char **key`/`char **value`, `int *value`) follow
   `patterns/message-and-serialisation.md` §2.6, §3 and §5 as recorded per row.
6. **Return types.** `bool` stays `bool` (`DECISIONS.md` 32); `uint8_t` returns whose `0` is a
   meaningful "unrecognised" value stay `u8`; `struct selector` becomes `Selector<Target>`;
   `struct token`, `struct token_value` and `struct properties` become `Token`, `TokenValue`
   and `Properties`.

**Thread context.** Every function from `get_token` to `handle_message` runs only on the
event-loop thread: the single entry point is `handle_message`, called from
`EVENT_HANDLER(DAEMON_MESSAGE)` (`src/event_loop.c:1634`). `message_loop_begin` runs on the main
thread during start-up only (`src/yabai.c:344`), and `message_loop_run` is the body of the
message-loop thread. Neither of the last two takes a manager, which is `DECISIONS.md` 20 and is
also what the mechanical rule gives, since both reach only statics.

**Types referenced below and where they come from.** `Response` (`crate::misc::response`,
`DECISIONS.md` 28); `Token`, `TokenValue`, `TokenType`, `MessageCursor<'message>`, `Selector<T>`,
`SelectorOutcome<T>`, `Properties`, `KeyValuePair`, `LabelType` (`crate::message`,
`GLOSSARY.md` §2.1, §2.5, §4); `WindowId` / `SpaceId` / `DisplayId` (`crate::handles`,
`GLOSSARY.md` §1); `ProcessManager`, `MouseDragState` (`crate::state`); `DisplayManager`,
`WindowManager`, `SpaceManager`, `MissionControlMode`, `Rule`, `Signal` and
`SIGNAL_TYPE_COUNT` (their own modules, `GLOSSARY.md` §2.1, §2.5); `MAXLEN`
(`crate::misc::macros`, `GLOSSARY.md` §7, `= 512`).

---

## The functions

| # | C name | file:line | thread context | event-loop-owned managers touched (transitively) | Rust signature | visibility |
|---|---|---|---|---|---|---|
| 1 | `get_token` | `src/message.c:298` | event loop | none | `fn get_token(&mut self) -> Token` | private, `impl MessageCursor<'_>` |
| 2 | `token_prefix` | `src/message.c:317` | event loop | none | `fn token_prefix(token: Token, message_bytes: &[u8], candidate: &str) -> bool` | private |
| 3 | `token_equals` | `src/message.c:327` | event loop | none | `fn token_equals(token: Token, message_bytes: &[u8], candidate: &str) -> bool` | private |
| 4 | `token_is_valid` | `src/message.c:338` | event loop | none | `fn is_valid(self) -> bool` | private, `impl Token` |
| 5 | `token_is_positive_integer` | `src/message.c:343` | event loop | none | `fn token_is_positive_integer(token: Token, message_bytes: &[u8]) -> Option<i32>` | private |
| 6 | `token_is_hexadecimal` | `src/message.c:358` | event loop | none | `fn token_is_hexadecimal(token: Token, message_bytes: &[u8]) -> Option<u32>` | private |
| 7 | `token_is_float` | `src/message.c:383` | event loop | none | `fn token_is_float(token: Token, message_bytes: &[u8]) -> Option<f32>` | private |
| 8 | `token_to_value` | `src/message.c:397` | event loop | none | `fn token_to_value(token: Token, message_bytes: &[u8]) -> TokenValue` | private |
| 9 | `daemon_fail` | `src/message.c:418` | event loop | none | `macro_rules! daemon_fail { ($response:expr, $($argument:tt)*) => { $response.fail(format_args!($($argument)*)) }; }` | private macro |
| 10 | `daemon_deprecated` | `src/message.c:429` | — | — | **not translated** — see the dead table | — |
| 11 | `parse_key_value_pair` | `src/message.c:440` | event loop | none | `fn parse_key_value_pair(message_bytes: &mut [u8], token_start: usize) -> Option<KeyValuePair>` | private |
| 12 | `parse_value_type` | `src/message.c:472` | event loop | none | `fn parse_value_type(type_of_change: &[libc::c_char; MAXLEN]) -> u8` | private |
| 13 | `parse_resize_handle` | `src/message.c:483` | event loop | none | `fn parse_resize_handle(handle: &[libc::c_char; MAXLEN]) -> u8` | private |
| 14 | `parse_label` | `src/message.c:554` | event loop | none | `fn parse_label(response: &mut Response, message_bytes: &[u8], token: Token, label_type: LabelType, label: &mut Option<String>) -> bool` | private |
| 15 | `parse_property` | `src/message.c:613` | event loop | none | `fn parse_property(properties: &mut Properties, property: &[u8], property_val: &[u64], property_str: &[&str]) -> bool` | private |
| 16 | `parse_properties` | `src/message.c:625` | event loop | none | `fn parse_properties(response: &mut Response, message_bytes: &mut [u8], token: Token, property_val: &[u64], property_str: &[&str]) -> Properties` | private |
| 17 | `parse_display_selector` | `src/message.c:665` | event loop | `DisplayManager` | `fn parse_display_selector(response: &mut Response, message_cursor: &mut MessageCursor, acting_display_id: DisplayId, optional: bool, display_manager: &mut DisplayManager) -> Selector<DisplayId>` | private |
| 18 | `parse_space_selector` | `src/message.c:790` | event loop | `SpaceManager` | `fn parse_space_selector(response: &mut Response, message_cursor: &mut MessageCursor, acting_space_id: SpaceId, optional: bool, space_manager: &mut SpaceManager) -> Selector<SpaceId>` | private |
| 19 | `parse_window_selector` | `src/message.c:871` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `fn parse_window_selector(response: &mut Response, message_cursor: &mut MessageCursor, acting_window_id: Option<WindowId>, optional: bool, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager) -> Selector<WindowId>` | private |
| 20 | `parse_insert_selector` | `src/message.c:1130` | event loop | none | `fn parse_insert_selector(response: &mut Response, message_cursor: &mut MessageCursor) -> Selector<i32>` | private |
| 21 | `handle_domain_config` | `src/message.c:1152` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager`, `MouseDragState` | `fn handle_domain_config(response: &mut Response, domain: Token, message_cursor: &mut MessageCursor, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState)` | private |
| 22 | `handle_domain_display` | `src/message.c:1700` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager`, `MissionControlMode` | `fn handle_domain_display(response: &mut Response, domain: Token, message_cursor: &mut MessageCursor, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mission_control_mode: &mut MissionControlMode)` | private |
| 23 | `handle_domain_space` | `src/message.c:1759` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager`, `MouseDragState`, `MissionControlMode` | `fn handle_domain_space(response: &mut Response, domain: Token, message_cursor: &mut MessageCursor, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | private |
| 24 | `handle_domain_window` | `src/message.c:2047` | event loop | `ProcessManager`, `DisplayManager`, `WindowManager`, `SpaceManager`, `MouseDragState`, `MissionControlMode` | `fn handle_domain_window(response: &mut Response, domain: Token, message_cursor: &mut MessageCursor, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | private |
| 25 | `handle_domain_query` | `src/message.c:2419` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager`, `MouseDragState` | `fn handle_domain_query(response: &mut Response, domain: Token, message_cursor: &mut MessageCursor, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState)` | private |
| 26 | `parse_rule` | `src/message.c:2596` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `fn parse_rule(response: &mut Response, message_cursor: &mut MessageCursor, rule: &mut Rule, token: Token, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager) -> bool` | private |
| 27 | `handle_domain_rule` | `src/message.c:2806` | event loop | `ProcessManager`, `DisplayManager`, `WindowManager`, `SpaceManager`, `MouseDragState`, `MissionControlMode` | `fn handle_domain_rule(response: &mut Response, domain: Token, message_cursor: &mut MessageCursor, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | private |
| 28 | `handle_domain_signal` | `src/message.c:2864` | event loop | `signal_event` | `fn handle_domain_signal(response: &mut Response, domain: Token, message_cursor: &mut MessageCursor, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT])` | private |
| 29 | `handle_message` | `src/message.c:2979` | event loop | `signal_event`, `ProcessManager`, `DisplayManager`, `WindowManager`, `SpaceManager`, `MouseDragState`, `MissionControlMode` | `pub(crate) fn handle_message(response: &mut Response, message: &mut [u8], signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | `pub(crate)` — `src/event_loop.c:1634` |
| 30 | `message_loop_run` | `src/message.c:3003` | message loop | none | `fn message_loop_run()` | private |
| 31 | `message_loop_begin` | `src/message.c:3016` | start-up only (main thread) | none | `pub(crate) fn message_loop_begin(socket_path: &Path) -> bool` | `pub(crate)` — `src/yabai.c:344` |

`src/message.h` declares exactly two of these, rows 29 and 31; both are defined and both are
reached from another file, so both are `pub(crate)` and nothing else in the module is.

---

## Not translated

| C name | file:line | why |
|---|---|---|
| `daemon_deprecated` | `src/message.c:429` | `__unused`, no caller anywhere in `src/`. `DECISIONS.md` 5 and `patterns/message-and-serialisation.md` §7.7: removed outright, one `DEVIATIONS.md` line, **no** `#[allow(dead_code)]` stub. |

Two sub-function removals that belong in `DEVIATIONS.md` but are not whole functions, recorded
here so no one looks for them in the table above:

* `token_char_int_table` (`src/message.c:283-296`) — the sparse designated-initializer array is
  replaced by the arithmetic it performs on the domain it is reached with
  (`patterns/message-and-serialisation.md` §3.1).
* the `void *context` parameter of `message_loop_run` (`src/message.c:3003`) — the pthread
  trampoline argument, always `NULL`, ignored, and suppressed with a `#pragma` at `:3001`. A
  Rust thread closure has no trampoline argument, so the parameter disappears.
* `TIME_FUNCTION` at its eleven sites in this file (`:667`, `:792`, `:873`, `:1154`, `:1702`,
  `:1761`, `:2049`, `:2421`, `:2598`, `:2808`, `:2866`) — `DECISIONS.md` 5.

---

## Per-row notes where the C needs reading against the Rust

**1 `get_token`.** A method on `MessageCursor`, not a free function: the C's `char **message`
is the cursor itself. The reborrow `MessageCursor::cursor_at` is what the C's
`parse_display_selector(rsp, &value, …)` (`:2685`) and `parse_space_selector(rsp, &value, …)`
(`:2699`) do inside `parse_rule`, and it is a method too, so it needs no row of its own — it has
no C original.

**4 `token_is_valid`.** `text && length > 0` loses its first half: a `Token` is an index pair and
its `text` is never NULL in the daemon (`patterns/message-and-serialisation.md` §2.2).

**5-7 the classifiers.** `bool` + out-parameter becomes `Option`, exactly per
`patterns/message-and-serialisation.md` §3: every caller reads the out-parameter only after
`true`. This is not the `bool`-stays-`bool` case of `DECISIONS.md` 32, because the `bool` is the
tag of an out-parameter rather than an answer of its own.

**9 `daemon_fail`.** It is variadic in C and becomes a macro (`DECISIONS.md` 33 for the sibling
macros, `patterns/message-and-serialisation.md` §7.1 for this one). The `if (!rsp) return;` guard
at `:420` is *not* in the macro: it is `Response::silent()`, which the three probing parses at
`:1706`, `:1765` and `:2053` construct. `Response::fail` and `Response::fail_pieces` live in
`crate::misc::response`; the roughly sixty `%.*s` call sites go through `fail_pieces` with
`FailurePiece::Bytes`, never through the macro, because the interpolated bytes must not be
UTF-8-validated.

**11 `parse_key_value_pair`.** The three C out-parameters collapse into `Option<KeyValuePair>`:
`None` covers both C failure shapes, and because `None` is returned before the in-place NUL is
written, the caller's `token.text` still prints the whole unmodified `k!=` or `k`
(`:2611`, `:2884`). `KeyValuePair::exclusion` carries the `bool *exclusion` the C leaves
untouched on failure; both callers pre-initialise it to `false`.

**12-13 `parse_value_type` / `parse_resize_handle`.** The argument is the `sscanf` scanset
buffer, a NUL-terminated C string inside a `char[MAXLEN]` (`:1971`, `:1982`, `:2231`, `:2243`,
`:2259`), so it is `&[libc::c_char; MAXLEN]` and **not** shrunk to 256
(`patterns/message-and-serialisation.md` §4.2-§4.3). `0` stays a meaningful return value, so no
`Option` (`DECISIONS.md` 32).

**14 `parse_label`.** Three C states — `false` (reported error), `true` with `*label == NULL`
(the "remove the label" path taken for an invalid token), `true` with an owned string — so the
`bool` stays and the out-parameter stays, now as `&mut Option<String>`. The owned `malloc`
(`:596-601`) becomes the `String`, moved on to
`display_manager_set_label_for_display` / `space_manager_set_label_for_space` /
`window_manager_set_scratchpad_for_window`, all of which take ownership in the C.
`message_bytes` is needed for the three reserved-identifier scans and the three `%.*s` failure
messages.

**15-16 `parse_property` / `parse_properties`.** `int property_count` disappears into the slice
lengths; `property_val` and `property_str` are the paired `*_property_val[]` / `*_property_str[]`
tables of `display.h`, `view.h` and `window.h`. `parse_properties` needs `&mut [u8]` because it
writes a NUL over each comma in place (`:2434-2440` reads the result back through the same
buffer).

**17-20 the four selector producers.** The C `union` plus `bool did_parse` is the tri-state
`Selector<Target>` / `SelectorOutcome<Target>` of
`patterns/message-and-serialisation.md` §5.1; `optional` stays a plain `bool` and suppresses the
message for the `TOKEN_TYPE_INVALID` arm only (`:778`, `:864`, `:1120`).
`parse_window_selector` resolves to a `WindowId`, never a reference, because the selector
outlives calls that mutate the window manager (`:2078`, `:2094`, `:2112`); its nullable
`struct window *acting_window` becomes `Option<WindowId>`.

**19 `parse_window_selector` reaches three managers**, which is easy to under-count: the
`largest` / `smallest` / `prev` / `next` / `first` / `last` and the six `stack.` arms all go
through `window_manager_find_*_managed_window(&g_space_manager, &g_window_manager, …)`, and
those resolve a view through `space_manager_find_view`, which creates a missing view and runs
`view_update` → `display_bounds_constrained` → `DisplayManager` (`src/display.c:123`). That is
the same chain as worked example (4) of `patterns/state-and-ownership.md` §2.3.

**21 `handle_domain_config` takes no `MouseDragState`.** Its only `g_mouse_state` references
(`:1621-1664`) are `modifier`, `action1`, `action2` and `drop_action`, which are the
`MOUSE_TAP_STATE` static of `DECISIONS.md` 23 and are read and written with `Ordering::Relaxed`.
The one callee that also names `g_mouse_state`, `window_manager_set_focus_follows_mouse`
(`src/window_manager.c:219-229`), touches only `handle` and `runloop_source`, also in that
static. `g_verbose` at `:1173-1177` is `VERBOSE: AtomicBool`, likewise not a parameter.

**22 `handle_domain_display` has no loop** (`:1700-1757`): at most one of `--focus` / `--space` /
`--label` is parsed and the rest of the message is ignored. It still takes `WindowManager`,
because `display_manager_focus_display` reaches it (worked example (5) of §2.3), and
`MissionControlMode`, because `display_manager_focus_space` → `mission_control_is_active`
(`src/mission_control.c:110`) reads `g_mission_control_mode`.

**24 `handle_domain_window` is the widest row.** `ProcessManager` comes from
`g_process_manager.finder_psn` inside `window_manager_warp_window`
(`src/window_manager.c:1927`), `window_manager_send_window_to_space` (`:2104`) and
`window_manager_toggle_scratchpad_window` (`:2475`) — the `ProcessManager` field of
`EventLoopOwnedState`, not the `PROCESS_TABLE` static. `MouseDragState` and
`MissionControlMode` both arrive through one chain: `window --scratchpad recover` at `:2401`
calls `window_manager_scratchpad_recover_windows` → `space_manager_refresh_application_windows`
→ `window_manager_add_existing_application_windows` → `window_manager_create_and_add_window`,
whose two debug branches call `window_serialize(stdout, window, 0)`
(`src/window_manager.c:1523`, `:1537`), and `window_serialize` reads
`g_mouse_state.window` at `src/window.c:706`.

**25 `handle_domain_query` takes `MouseDragState` and `SpaceManager` for the same reason**:
`window_serialize` (`:2581`) and `window_manager_query_windows_for_*` (`:2554`, `:2567`,
`:2589`) reach `g_mouse_state.window`, and `window_serialize` resolves a view through
`view_find_window_node` (`src/window.c:445`) — the `SpaceManager` that worked example (2) of
`patterns/state-and-ownership.md` §2.3 adds and that `state-access.tsv` misses, because the
lookup goes through `window_manager_find_managed_window` rather than a textual
`g_space_manager`. It does **not** take `MissionControlMode`: no query path reaches it.

**26 `parse_rule`.** `struct rule *rule` is the caller's owned `Rule` (`:2811`, `:2834`), so
`&mut Rule`; `rule_destroy` becomes that value's `Drop` (`DECISIONS.md` 26). The nested
`parse_display_selector(rsp, &value, …)` / `parse_space_selector(rsp, &value, …)` at `:2685`
and `:2699` are `MessageCursor::cursor_at` reborrows, which is why `parse_rule` carries all three
managers even though its own body names none.

**28 `handle_domain_signal` takes `signal_event` and nothing else.** `event_signal_add`,
`event_signal_remove_by_index`, `event_signal_remove` and `event_signal_list` are the only
callees that touch an event-loop-owned global, and all four touch `g_signal_event`, which is the
`EventLoopOwnedState::signal_event` field (`patterns/state-and-ownership.md` §1.2). It is not one
of the five "managers" §2.2 step 3 enumerates, but it is an event-loop-owned global the C reaches
through a global, so `DECISIONS.md` 13 makes it a parameter; `src/yabai.c:27` puts it first in
the append order.

**29 `handle_message`.** The union of rows 21-28, in the `src/yabai.c:27-37` order.
`message: &mut [u8]` is the block `EVENT_HANDLER(DAEMON_MESSAGE)` allocated and read into
(`src/event_loop.c:1613-1644`); `handle_message` builds the `MessageCursor` over it.

**30-31 the message loop.** `g_message_loop` is `static MESSAGE_LOOP: OnceLock<MessageLoop>`
(`GLOSSARY.md` §8.2, `patterns/state-and-ownership.md` §1.3) holding the listening
`UnixListener` and the accept `JoinHandle`, and the sender is `static EVENT_SENDER`
(`GLOSSARY.md` §8.1). Statics are never parameters, so `message_loop_run` takes nothing and
`message_loop_begin` keeps only its C parameter. `is_running` disappears: it is only ever set
`true` and read as the accept loop's condition (`:3005`, `:3041`). The return stays `bool`
(`DECISIONS.md` 32); `src/yabai.c:344-346` treats `false` as fatal.

---

## What this module expects of other modules

Every call below is made from `src/message.c` today. The shapes are what these signatures
require; the owning module's own `state-access/*.md` row must match, parameter for parameter.

### `crate::display_manager`

* `display_manager_active_display_id() -> DisplayId` — no manager.
* `display_manager_arrangement_display_id(arrangement: i32, display_manager: &mut DisplayManager) -> DisplayId`
* `display_manager_cursor_display_id() -> DisplayId` — no manager.
* `display_manager_find_closest_display_in_direction(source_display_id: DisplayId, direction: i32) -> DisplayId` — no manager.
* `display_manager_first_display_id(display_manager: &mut DisplayManager) -> DisplayId`
* `display_manager_last_display_id(display_manager: &mut DisplayManager) -> DisplayId`
* `display_manager_prev_display_id(display_id: DisplayId, display_manager: &mut DisplayManager) -> DisplayId`
* `display_manager_next_display_id(display_id: DisplayId, display_manager: &mut DisplayManager) -> DisplayId`
* `display_manager_focus_display(display_id: DisplayId, space_id: SpaceId, window_manager: &mut WindowManager)` — worked example (5) of §2.3; `DisplayManager` is **not** in its set.
* `display_manager_focus_space(display_id: DisplayId, space_id: SpaceId, mission_control_mode: &mut MissionControlMode) -> SpaceOpError`
* `display_manager_get_display_for_label(display_manager: &mut DisplayManager, label: &[u8]) -> Option<&mut DisplayLabel>` — the C returns `struct display_label *` and the caller reads only `->did`.
* `display_manager_set_label_for_display(display_manager: &mut DisplayManager, display_id: DisplayId, label: String)` — takes ownership.
* `display_manager_remove_label_for_display(display_manager: &mut DisplayManager, display_id: DisplayId) -> bool`
* `display_manager_query_displays(response: &mut Response, flags: u64, display_manager: &mut DisplayManager) -> bool`

### `crate::display`

* `display_serialize(response: &mut Response, display_id: DisplayId, flags: u64, display_manager: &mut DisplayManager)`
* `display_space_id(display_id: DisplayId) -> SpaceId` — no manager.

### `crate::space`

* `space_display_id(space_id: SpaceId) -> DisplayId`, `space_is_user(space_id: SpaceId) -> bool`,
  `space_is_fullscreen(space_id: SpaceId) -> bool` — none takes a manager.

### `crate::space_manager`

* `space_manager_active_space(window_manager: &mut WindowManager) -> SpaceId`
* `space_manager_prev_space(space_id: SpaceId) -> SpaceId`, `space_manager_next_space(space_id: SpaceId) -> SpaceId`, `space_manager_first_space() -> SpaceId`, `space_manager_last_space() -> SpaceId`, `space_manager_cursor_space() -> SpaceId`, `space_manager_mission_control_space(desktop_id: i32) -> SpaceId` — none takes a manager.
* `space_manager_get_space_for_label(space_manager: &mut SpaceManager, label: &[u8]) -> Option<SpaceId>`
* `space_manager_find_view(space_manager: &mut SpaceManager, space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> SpaceId` — the C never returns NULL (`src/space_manager.c:103-111` creates the view when the lookup misses), so it hands back the handle and the caller re-resolves the `View` through `SpaceManager::view` at each use (`patterns/state-and-ownership.md` §3.2, §4.1). `handle_domain_config` then reads and writes the padding / gap / layout / split-type / auto-balance fields of that view at `:1394-1631`.
* `space_manager_focus_space(space_id: SpaceId, window_manager: &mut WindowManager, mission_control_mode: &mut MissionControlMode) -> SpaceOpError`
* `space_manager_switch_space(space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mission_control_mode: &mut MissionControlMode, mouse_drag_state: &mut MouseDragState) -> SpaceOpError`
* `space_manager_move_space_to_space(acting_space_id: SpaceId, selector_space_id: SpaceId, window_manager: &mut WindowManager, mission_control_mode: &mut MissionControlMode) -> SpaceOpError`
* `space_manager_swap_space_with_space(acting_space_id: SpaceId, selector_space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mission_control_mode: &mut MissionControlMode, mouse_drag_state: &mut MouseDragState) -> SpaceOpError`
* `space_manager_move_space_to_display(space_manager: &mut SpaceManager, space_id: SpaceId, display_id: DisplayId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, mission_control_mode: &mut MissionControlMode) -> SpaceOpError`
* `space_manager_add_space(space_id: SpaceId, mission_control_mode: &mut MissionControlMode) -> SpaceOpError`
* `space_manager_destroy_space(space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode) -> SpaceOpError`
* `space_manager_equalize_space` / `_balance_space(space_manager: &mut SpaceManager, space_id: SpaceId, axis_flag: u32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> bool`
* `space_manager_mirror_space(space_manager: &mut SpaceManager, space_id: SpaceId, axis: WindowNodeSplit, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> bool`
* `space_manager_rotate_space(space_manager: &mut SpaceManager, space_id: SpaceId, degrees: i32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> bool`
* `space_manager_set_padding_for_space(space_manager: &mut SpaceManager, space_id: SpaceId, type_of_change: i32, top: i32, bottom: i32, left: i32, right: i32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> bool`
* `space_manager_set_gap_for_space(space_manager: &mut SpaceManager, space_id: SpaceId, type_of_change: i32, gap: i32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> bool`
* `space_manager_toggle_padding_for_space` / `_toggle_gap_for_space(space_manager: &mut SpaceManager, space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> bool`
* `space_manager_toggle_mission_control(space_id: SpaceId, window_manager: &mut WindowManager, mission_control_mode: &mut MissionControlMode)`
* `space_manager_toggle_show_desktop(space_id: SpaceId, window_manager: &mut WindowManager, mission_control_mode: &mut MissionControlMode)`
* `space_manager_toggle_window_split(space_manager: &mut SpaceManager, window_id: WindowId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)`
* `space_manager_set_layout_for_space(space_manager: &mut SpaceManager, space_id: SpaceId, view_type: ViewType, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, mouse_drag_state: &mut MouseDragState)`
* `space_manager_set_layout_for_all_spaces(space_manager: &mut SpaceManager, layout: ViewType, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, mouse_drag_state: &mut MouseDragState)`
* `space_manager_set_top_padding_for_all_spaces` / `_bottom_` / `_left_` / `_right_` / `_window_gap_for_all_spaces(space_manager: &mut SpaceManager, value: i32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)`
* `space_manager_set_split_type_for_all_spaces(space_manager: &mut SpaceManager, split_type: WindowNodeSplit)` — no other manager.
* `space_manager_set_auto_balance_for_all_spaces(space_manager: &mut SpaceManager, auto_balance: u32)` — no other manager.
* `space_manager_mark_spaces_invalid(space_manager: &mut SpaceManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)`
* `space_manager_set_label_for_space(space_manager: &mut SpaceManager, space_id: SpaceId, label: String)` — takes ownership.
* `space_manager_remove_label_for_space(space_manager: &mut SpaceManager, space_id: SpaceId) -> bool`
* `space_manager_query_space(response: &mut Response, space_id: SpaceId, flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager) -> bool`
* `space_manager_query_spaces_for_display(response: &mut Response, display_id: DisplayId, flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager) -> bool`
* `space_manager_query_spaces_for_displays(response: &mut Response, flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager) -> bool`
* `space_manager_query_spaces_for_window(response: &mut Response, window_id: WindowId, flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager) -> bool`

### `crate::view`

* `view_update(space_manager: &mut SpaceManager, space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)`
* `view_flush(space_manager: &mut SpaceManager, space_id: SpaceId, window_manager: &mut WindowManager)`
* `view_clear(space_manager: &mut SpaceManager, space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, mouse_drag_state: &mut MouseDragState)`
* `view_set_flag` and the `View` padding / gap / layout / split-type / auto-balance fields are
  reached through the resolved `View` at `:1394-1631`; they are field writes, not calls.

### `crate::window`

* `window_serialize(response: &mut Response, window_id: WindowId, flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState)` — worked example (2) of `patterns/state-and-ownership.md` §2.3, verbatim.
* `window_display_id(window_id: WindowId) -> DisplayId` — no manager.

### `crate::window_manager`

* `window_manager_focused_window(window_manager: &mut WindowManager) -> Option<WindowId>`
* `window_manager_find_window(window_manager: &mut WindowManager, window_id: WindowId) -> Option<WindowId>`
* `window_manager_find_window_below_cursor(window_manager: &mut WindowManager) -> Option<WindowId>`
* `window_manager_find_recent_managed_window(window_manager: &mut WindowManager) -> Option<WindowId>`
* `window_manager_find_closest_managed_window_in_direction(window_manager: &mut WindowManager, window_id: WindowId, direction: i32, space_manager: &mut SpaceManager) -> Option<WindowId>`
* `window_manager_find_sibling_for_managed_window` / `_first_nephew_` / `_second_nephew_` / `_uncle_` / `_first_cousin_` / `_second_cousin_for_managed_window(window_manager: &mut WindowManager, window_id: WindowId) -> Option<WindowId>`
* `window_manager_find_prev_managed_window` / `_next_managed_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, display_manager: &mut DisplayManager) -> Option<WindowId>` — the two C-declared managers keep their C positions and `DisplayManager` is appended.
* `window_manager_find_first_managed_window` / `_last_` / `_largest_` / `_smallest_managed_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, display_manager: &mut DisplayManager) -> Option<WindowId>`
* `window_manager_find_prev_window_in_stack` / `_next_` / `_first_` / `_last_` / `_recent_window_in_stack(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, display_manager: &mut DisplayManager) -> Option<WindowId>`
* `window_manager_find_window_in_stack(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, index: i32, display_manager: &mut DisplayManager) -> Option<WindowId>`
* `window_manager_focus_window_with_raise(window_process_serial_number: &ProcessSerialNumber, window_id: WindowId, window_ref: AXUIElementRef)` — no manager.
* `window_manager_close_window(window_id: WindowId, window_manager: &mut WindowManager) -> bool`, `window_manager_minimize_window(window_id: WindowId, window_manager: &mut WindowManager) -> WindowOpError`, `window_manager_deminimize_window(window_id: WindowId, window_manager: &mut WindowManager) -> WindowOpError`, `window_manager_toggle_window_shadow(window_id: WindowId, window_manager: &mut WindowManager)` — none takes a manager.
* `window_manager_send_window_to_space(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, destination_space_id: SpaceId, moved_by_rule: bool, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)`
* `window_manager_swap_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, a_window: WindowId, b_window: WindowId, display_manager: &mut DisplayManager) -> WindowOpError`
* `window_manager_warp_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, a_window: WindowId, b_window: WindowId, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState) -> WindowOpError`
* `window_manager_stack_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, a_window: WindowId, b_window: WindowId, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState) -> WindowOpError`
* `window_manager_set_window_insertion(space_manager: &mut SpaceManager, window_id: WindowId, direction: i32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> WindowOpError`
* `window_manager_apply_grid(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, rows: u32, columns: u32, x: u32, y: u32, width: u32, height: u32, display_manager: &mut DisplayManager) -> WindowOpError`
* `window_manager_move_window_relative(window_manager: &mut WindowManager, window_id: WindowId, type_of_change: i32, delta_x: f32, delta_y: f32) -> WindowOpError`
* `window_manager_resize_window_relative(window_manager: &mut WindowManager, window_id: WindowId, direction: i32, delta_x: f32, delta_y: f32, animate: bool, display_manager: &mut DisplayManager, space_manager: &mut SpaceManager) -> WindowOpError`
* `window_manager_adjust_window_ratio(window_manager: &mut WindowManager, window_id: WindowId, type_of_change: i32, ratio: f32, space_manager: &mut SpaceManager) -> WindowOpError`
* `window_manager_make_window_floating(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, should_float: bool, force: bool, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)`
* `window_manager_make_window_sticky(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, should_sticky: bool, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)`
* `window_manager_toggle_window_zoom_parent` / `_zoom_fullscreen(window_manager: &mut WindowManager, window_id: WindowId)`
* `window_manager_toggle_window_windowed_fullscreen(window_id: WindowId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)`
* `window_manager_toggle_window_native_fullscreen(window_id: WindowId, window_manager: &mut WindowManager)`
* `window_manager_toggle_window_expose(window_id: WindowId, window_manager: &mut WindowManager)` — no manager.
* `window_manager_toggle_window_pip(space_manager: &mut SpaceManager, window_id: WindowId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)`
* `window_manager_set_window_layer(window_id: WindowId, layer: i32, window_manager: &mut WindowManager) -> bool`
* `window_manager_set_opacity(window_manager: &mut WindowManager, window_id: WindowId, opacity: f32) -> bool`
* `window_manager_set_window_opacity_enabled(window_manager: &mut WindowManager, enabled: bool)`
* `window_manager_set_active_window_opacity` / `_normal_window_opacity` / `_menubar_opacity(window_manager: &mut WindowManager, opacity: f32)`
* `window_manager_set_purify_mode(window_manager: &mut WindowManager, mode: PurifyMode)`
* `window_manager_set_focus_follows_mouse(window_manager: &mut WindowManager, mode: FfmMode)` — **no** `MouseDragState`; the tap handle is the `MOUSE_TAP_STATE` static.
* `window_manager_set_scratchpad_for_window(window_manager: &mut WindowManager, window_id: WindowId, label: String, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState) -> bool` — takes ownership of the label.
* `window_manager_remove_scratchpad_for_window(window_manager: &mut WindowManager, window_id: WindowId, unfloat: bool, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState) -> bool`
* `window_manager_toggle_scratchpad_window_by_label(window_manager: &mut WindowManager, label: &[u8], process_manager: &mut ProcessManager) -> bool`
* `window_manager_scratchpad_recover_windows(process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)`
* `window_manager_validate_and_check_for_windows_on_space(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, space_id: SpaceId, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)`
* `window_manager_query_window_rules(response: &mut Response, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)`
* `window_manager_query_windows_for_spaces(response: &mut Response, space_list: &[SpaceId], flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState)` — the `(pointer, count)` pair collapses into one slice; the call at `:2567` passes `&[acting_space_id]`.
* `window_manager_query_windows_for_display(response: &mut Response, display_id: DisplayId, flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState)`
* `window_manager_query_windows_for_displays(response: &mut Response, flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState)`

### `crate::rule`

* `rule_add(rule: Rule, window_manager: &mut WindowManager)` — the C hands the `struct rule` to the manager's `rules` buffer by value.
* `rule_apply(rule: &Rule, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)`
* `rule_reapply_all(process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)`
* `rule_reapply_by_index(index: i32, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode) -> bool`
* `rule_reapply_by_label(label: &[u8], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode) -> bool`
* `rule_remove_by_index(index: i32, window_manager: &mut WindowManager) -> bool`
* `rule_remove_by_label(label: &[u8], window_manager: &mut WindowManager) -> bool`
* `rule_destroy` — **gone**, it is `Drop for Rule` (`DECISIONS.md` 26); the two C call sites at `:2820` and `:2837` become the end of the value's scope.

### `crate::event_signal`

* `event_signal_add(signal_type: SignalType, signal: Signal, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT])` — takes the `Signal` by value.
* `event_signal_remove_by_index(index: i32, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT]) -> bool`
* `event_signal_remove(label: &[u8], signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT]) -> bool`
* `event_signal_list(response: &mut Response, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT])`
* `signal_type_from_string(string: &[u8]) -> SignalType` — no manager.
* `event_signal_destroy` — **gone**, it is `Drop for Signal`; `:2957` becomes the end of scope.

### `crate::sa`, `crate::misc`

* `scripting_addition_is_sip_friendly() -> bool`, `scripting_addition_order_window(a_window_id: WindowId, order: i32, b_window_id: WindowId) -> bool` — neither takes a manager. `b_window_id` keeps `WindowId(0)` as the "no reference window" sentinel (`DECISIONS.md` 32).
* `rgba_color_from_hex(color: u32) -> RgbaColor` — no manager.
* `string_equals` becomes byte-slice equality (`c_string_at(message_bytes, index) == constant.as_bytes()`); `string_copy` becomes `String::from_utf8_lossy(..).into_owned()` at the point a value is stored (`DECISIONS.md` 28). Neither survives as a call.
* `event_loop_post(&g_event_loop, DAEMON_MESSAGE, NULL, sockfd)` at `:3010` becomes
  `EVENT_SENDER.get().unwrap().send(Event::DaemonMessage(stream))` — the `struct event_loop *`
  parameter disappears and the whole `Event` replaces the `type`/`context`/`param1` triple
  (`GLOSSARY.md` §11).

---

## Judgement calls, and what each one settles

Every place where the mechanical rule was ambiguous or two contract documents disagreed.

1. **`FILE *rsp` is `&mut Response`, not `&mut dyn std::io::Write`.**
   `patterns/state-and-ownership.md` §2.2 step 3 and its worked example (2) write
   `response: &mut dyn std::io::Write`. `GLOSSARY.md` §10.4 ("`rsp` as a `FILE *` parameter →
   `response: &mut Response`"), `GLOSSARY.md` §2.5 and `patterns/message-and-serialisation.md`
   §7.2 ("`Response` is the only type any parse function takes, so no call site can accidentally
   lose the suppression") all say `Response`. `DECISIONS.md` 28 requires one type that owns the
   failure prefix byte **and** the "no response wanted" case, which `dyn Write` cannot express —
   the three probing parses at `:1706`, `:1765`, `:2053` need `Response::silent()`. `Response`
   wins everywhere in this file, including in the cross-module expectations above.

2. **The appended manager order is the `src/yabai.c:27-37` order, not the order
   `patterns/message-and-serialisation.md` §6.6 sketches.** That sketch writes
   `(display_manager, space_manager, window_manager)` for `handle_message` and
   `handle_domain_config`. `DECISIONS.md` 13, `patterns/state-and-ownership.md` §2.2 step 3 and
   `TRANSLATION_PLAN.md` §3.2 all fix `window_manager` (`:30`) **before** `space_manager`
   (`:31`), and §2.2 calls that "the only tie-break there is". The rule wins; §6.6 is
   illustrative and its own text defers to the precomputed call graph.

3. **`signal_event` and `mission_control_mode` are parameters, appended in the same
   `src/yabai.c:27-37` order.** `patterns/state-and-ownership.md` §2.2 step 3 enumerates five
   managers and neither is among them, but both are `EventLoopOwnedState` fields
   (§1.1, §1.2), not statics, so nothing can reach them any other way, and `TRANSLATION_PLAN.md`
   §3.2 step 3 lists both among the eight event-loop-owned globals a row must record and fixes
   the append order as the `EventLoopOwnedState` field order — which is the `src/yabai.c:27-37`
   order with the five managers as a subsequence. `signal_event` therefore sorts before
   `process_manager` (`:27` before `:28`) and `mission_control_mode` last (`:37`).

4. **`MouseDragState` is a parameter of rows 21, 23, 24, 25 and 27.** The original reading of
   this file gave it to rows 24 and 25 only, and that part still holds on its own terms:
   `DECISIONS.md` 23 splits the struct, every `g_mouse_state` reference in `handle_domain_config`
   — its own at `:1621-1664` and the one inside `window_manager_set_focus_follows_mouse` — is in
   the atomic `MOUSE_TAP_STATE` half, and of the 87 `g_mouse_state` sites in `src/` the drag half
   is reachable *through a `g_mouse_state` name* only via `window_serialize` (`src/window.c:706`).
   The cross-module pass added `mouse_drag_state` to rows 21 (`handle_domain_config`), 23
   (`handle_domain_space`) and 27 (`handle_domain_rule`) for a reason no `g_mouse_state` grep can
   find: `patterns/state-and-ownership.md` §5.4 gives the Rust-only `view_free_node` a
   `&mut MouseDragState`, because it clears `MouseDragState::feedback_node` when a node slot is
   freed and the §5.1 arena recycles that `NodeId`. Row 21 reaches it through
   `space_manager_set_layout_for_all_spaces` → `view_clear`, row 23 through
   `space_manager_destroy_space` and `space_manager_set_layout_for_space`, and row 27 through
   `rule_apply` and the three `rule_reapply_*`. See `state-access/signal-rule-mouse.md` §8.7 and
   `state-access/view.md` N12.

5. **`SpaceManager` is in `window_serialize`'s set although `state-access.tsv` does not show
   it.** The tsv extraction is textual and `window.c:445` resolves the view through
   `window_manager_find_managed_window` rather than naming `g_space_manager`.
   `patterns/state-and-ownership.md` §2.3 worked example (2) computes the set by hand and
   includes `SpaceManager`; that is the binding answer, and row 25 carries it.

6. **`parse_label` keeps its `bool` return and its out-parameter.**
   `files/message.md:488` proposes `Result<Option<String>, ()>`; `DECISIONS.md` 32 says `Result`
   appears only at `io::Write`, and "bool stays bool". The C has three states — error, "remove
   the label", "set this label" — and only the count-out-parameter rule collapses an
   out-parameter, so `(&mut Option<String>) -> bool` is the mechanical transposition.
   `Option<Option<String>>` was rejected as unreadable and not required by any rule.

7. **`message_loop_run()` takes nothing, and `message_loop_begin` takes no `Sender<Event>`.**
   `THREADS.md` §2.4 sketches `message_loop_begin(socket_path, event_sender) -> bool` and
   `message_loop_run(listener, event_sender)`, removing `g_message_loop` entirely.
   `GLOSSARY.md` §2.5 and §8.2 and `patterns/state-and-ownership.md` §1.3 all keep
   `static MESSAGE_LOOP: OnceLock<MessageLoop>` holding the `UnixListener` and the accept
   `JoinHandle`, and `patterns/state-and-ownership.md` §1.2 makes the sender
   `static EVENT_SENDER`. §2.1's "statics are never parameters" then leaves both signatures
   bare. The `THREADS.md` body — the C's `socket`/`bind`/`chmod`/`listen`/`fcntl` order,
   `UnixListener::from_raw_fd`, `SOMAXCONN` written out — is unaffected and still applies.

8. **`socket_path: &Path`.** The C parameter is `char *`. `THREADS.md` and `files/message.md`
   both write `&Path`, and `std::fs::remove_file` / `set_permissions` take it; `GLOSSARY.md`
   does not rule on it. `&Path` adopted; `SOCKET_FILE` is a `String` and the caller passes
   `Path::new(..)`.

9. **`parse_value_type`'s parameter is named `type_of_change`.** `type` is a Rust keyword and
   `GLOSSARY.md` §11's table of every C `type` does not list `src/message.c:472`. The value it
   returns is `TYPE_ABS` / `TYPE_REL`, which is exactly what §11 names `type_of_change` at
   `space_manager_set_gap_for_space`, `space_manager_set_padding_for_space`,
   `window_manager_move_window_relative` and `window_manager_adjust_window_ratio`. Same word
   here. (`parse_resize_handle`'s `handle` needs no rename.)

10. **Where the added `message_bytes` parameter goes.** `Token` is an index pair, so several
    functions gain a buffer parameter that has no C original. The pattern doc places it
    inconsistently, so one rule is applied: in a function whose first C parameter is the token,
    the buffer follows the token (`token_prefix`, `token_equals`, `token_is_*`, `token_to_value`
    — matching `patterns/message-and-serialisation.md` §2.5 and §3 verbatim); in a function
    whose first C parameter is `FILE *rsp`, the buffer follows `response` (`parse_properties`
    — matching §2.6 verbatim, and `parse_label` by the same rule). Functions that take a
    `&mut MessageCursor` need no separate buffer parameter.

11. **`MessageCursor` is written without an explicit lifetime in parameter position.**
    `patterns/message-and-serialisation.md` §5.3, §5.4, §5.5 and §6.6 all write
    `&mut MessageCursor`; `&mut MessageCursor<'_>` compiles identically. The doc's spelling is
    kept so the pasted signatures match the pattern document character for character.

12. **`get_token` and `token_is_valid` are methods, not free functions.** §2.3 and §2.2 of
    `patterns/message-and-serialisation.md` define them as `MessageCursor::get_token` and
    `Token::is_valid`. `DECISIONS.md` 37 keeps C *names*, and both keep theirs within their
    `impl`; the C name `token_is_valid` survives as `Token::is_valid` because the `token_`
    prefix is the receiver. `token_prefix` / `token_equals` stay free functions, as the pattern
    doc writes them, because they take a candidate string as well.

13. **`daemon_fail` has no `fn` form.** It is variadic in C. Row 9 gives the `macro_rules!`
    definition of `patterns/message-and-serialisation.md` §7.1; the function it expands to,
    `Response::fail`, belongs to `crate::misc::response` and is that module's row, not this
    one's.

14. **Selector targets are handle newtypes, not raw integers.**
    `patterns/message-and-serialisation.md` §5.1 tabulates `Target` as `u32` / `u64` / `u32` /
    `i32`. `GLOSSARY.md` §1 makes `acting_did` a `DisplayId`, `sid` a `SpaceId` and `wid` a
    `WindowId` and forbids implicit conversion in either direction, and
    `patterns/state-and-ownership.md` §3 turns `struct window *` into `WindowId`. Handles win:
    `Selector<DisplayId>`, `Selector<SpaceId>`, `Selector<WindowId>`. `parse_insert_selector`
    keeps `Selector<i32>` because `DIR_NORTH`/`STACK` are `i32` constants (`GLOSSARY.md` §7) and
    no handle type covers a direction.
