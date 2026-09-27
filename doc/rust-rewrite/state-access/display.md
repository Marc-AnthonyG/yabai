# State access — `src/display_manager.h`, `src/display_manager.c`, `src/display.h`, `src/display.c`

Final Rust signature of every function defined in the four files, in C source order. Wave 1
pastes the **Rust signature** column verbatim, prefixed by the **visibility** column.

`src/display.h` and `src/display_manager.h` define no functions — they hold declarations, the
`DISPLAY_PROPERTY_LIST` X-macro, the `display_callback` typedef and four `static` lookup tables.
Every declaration in both headers has a definition in the matching `.c`; none is a dangling
declaration.

47 functions are defined across the two `.c` files. 44 are translated; 3 are dead
(`DECISIONS.md` 5) and are listed in the last table.

Derivation: `patterns/state-and-ownership.md` §2.2, applied literally.

1. C parameters in C order, pointers rewritten to handles (§3.1).
2. A manager the C declares keeps its C position — in this module that is only
   `struct display_manager *dm`, which is parameter one in the four label functions and in
   `display_manager_begin`.
3. Every remaining manager of `Managers(f)` appended after the whole C parameter list, in the
   `src/yabai.c:27-35` order: `process_manager`, `display_manager`, `window_manager`,
   `space_manager`, `mouse_drag_state`. No function in this module reaches more than one.

`FILE *rsp` is `response: &mut Response` in first position (`GLOSSARY.md` §10.4,
`patterns/message-and-serialisation.md` §7). `g_connection` is the `CONNECTION` static and never
a parameter (§2.1); it is the reason so many rows here touch no manager at all despite a
non-empty `transitive_globals` column in `state-access.tsv`.

---

## 1. `src/display.c`

| C name | file:line | thread | managers touched transitively | Rust signature | visibility |
|---|---|---|---|---|---|
| `display_handler` | `src/display.c:6` | main run loop | none | `unsafe extern "C-unwind" fn display_handler(display_id: CGDirectDisplayID, flags: CGDisplayChangeSummaryFlags, context: *mut c_void)` | `pub(crate)` |
| `display_serialize` | `src/display.c:20` | event loop | `DisplayManager` | `fn display_serialize(response: &mut Response, display_id: DisplayId, flags: u64, display_manager: &mut DisplayManager)` | `pub(crate)` |
| `display_uuid` | `src/display.c:101` | any | none | `fn display_uuid(display_id: DisplayId) -> Option<CFStringOwned>` | `pub(crate)` |
| `display_id` | `src/display.c:112` | any | none | `fn display_id(uuid: &CFString) -> DisplayId` | `pub(crate)` |
| `display_bounds_constrained` | `src/display.c:123` | event loop | `DisplayManager` | `fn display_bounds_constrained(display_id: DisplayId, ignore_external_bar: bool, display_manager: &mut DisplayManager) -> CGRect` | `pub(crate)` |
| `display_center` | `src/display.c:173` | event loop | none | `fn display_center(display_id: DisplayId) -> CGPoint` | `pub(crate)` |
| `display_space_id` | `src/display.c:179` | event loop | none | `fn display_space_id(display_id: DisplayId) -> SpaceId` | `pub(crate)` |
| `display_space_count` | `src/display.c:190` | — | — | **not translated** — see §3 | — |
| `display_space_list` | `src/display.c:218` | any | none | `fn display_space_list(display_id: DisplayId) -> Option<Vec<SpaceId>>` | `pub(crate)` |

### Row notes

**`display_handler`.** The C is `static DISPLAY_EVENT_HANDLER(display_handler)`, which expands to
`void display_handler(uint32_t did, CGDisplayChangeSummaryFlags flags, void *context)`. The shape
is fixed by `objc2_core_graphics::CGDisplayReconfigurationCallBack`
(`patterns/ffi-objc-and-os.md` §19.5), so `CGDirectDisplayID` stays rather than becoming a
`DisplayId`; the body wraps it as `DisplayId(display_id)` when it builds the event. It posts
`Event::DisplayAdded` / `DisplayRemoved` / `DisplayMoved` / `DisplayResized`, each carrying a
`DisplayId` (`THREADS.md` §3.2), through the `EVENT_SENDER` static. `g_event_loop` in the tsv row
is that static, not a manager, so `DECISIONS.md` 20 is satisfied without argument: the callback
touches no event-loop-owned memory. `context` is always `NULL` (`display_manager.c:505`,
`THREADS.md:1129`) and is kept only because the callback type demands it.

**`display_serialize`.** `Direct` is `DisplayManager` twice: `&g_display_manager` at `:58` and
`g_display_manager.current_display_id` at `:95`. Callees add nothing —
`display_manager_display_id_arrangement` and `display_manager_get_label_for_display` are both
`DisplayManager`-only, and `space_manager_mission_control_index` (`space_manager.c:491`, called at
`:79`) reaches `g_connection` and nothing else, verified in the C: it is a straight
`SLSCopyManagedDisplaySpaces` walk with no manager access. So `Managers = { DisplayManager }`,
appended after `flags`. `flags` is `u64`, mutated in the body by the
`if flags == 0x0 { flags |= !flags; }` idiom (`patterns/idioms-and-conventions.md` §6.5) — the
`window_serialize` worked example in `patterns/state-and-ownership.md` §2.3 writes the identical
parameter without `mut`, so this row does too and the body opens with a rebinding `let`.

**`display_uuid`** is `CGDisplayCreateUUIDFromDisplayID` + `CFUUIDCreateString`, a create/copy pair
that can fail, so `Option<CFStringOwned>` (`patterns/memory-text-and-os-objects.md` §1.2).
Both `CFRelease` calls in the C body disappear into `Drop`. Reached from
`display_manager_active_display_id` during `display_manager_begin`, which runs on main, and from
the event loop everywhere else — hence `any`.

**`display_id`** returns `DisplayId`, not `Option<DisplayId>`: `display.c:115` returns `0` when
`CFUUIDCreateFromString` fails, and that `0` is the C's "none" sentinel, propagated unchanged by
`display_manager_dock_display_id`, `_point_display_id` and `_arrangement_display_id` and tested
by their callers (`DECISIONS.md` 32, `patterns/state-and-ownership.md` §3.1). `CFStringRef uuid`
is a borrowed argument, so `&CFString` (`patterns/memory-text-and-os-objects.md` §1.1).

**`display_bounds_constrained`.** `Direct` is `DisplayManager::mode`, `::top_padding`,
`::bottom_padding` (`:129-136`). Every callee is manager-free: `display_manager_main_display_id`,
`display_manager_menu_bar_hidden`, `display_manager_menu_bar_rect`, `display_manager_dock_hidden`,
`display_manager_dock_display_id`, `display_manager_dock_rect`,
`display_manager_dock_orientation`, and `workspace_display_notch_height`.
`Managers = { DisplayManager }`.

**`display_space_list`.** `int *count` is the length of the returned buffer, so it collapses into
the `Vec` (§2.2, third mechanical consequence). C's NULL return and C's zero-length return are
both the empty `Vec`; the `if (space_list)` guards at `display.c:78`, `space_manager.c:53` and the
rest become `is_empty()` checks, and the out-of-bounds `space_list[0]` read at `display.c:79`
(reachable in C when `CFArrayGetCount` is 0) becomes a `first()` that yields nothing. One
`DEVIATIONS.md` line. Called from `space_manager_begin` (`space_manager.c:1221`), which
`patterns/state-and-ownership.md` §1.4 step 3 runs on the **main** thread, as well as from the
event loop — hence `any`.

---

## 2. `src/display_manager.c`

| C name | file:line | thread | managers touched transitively | Rust signature | visibility |
|---|---|---|---|---|---|
| `display_manager_query_displays` | `src/display_manager.c:5` | event loop | `DisplayManager` | `fn display_manager_query_displays(response: &mut Response, flags: u64, display_manager: &mut DisplayManager) -> bool` | `pub(crate)` |
| `display_manager_get_label_for_display` | `src/display_manager.c:23` | event loop | `DisplayManager` (declared) | `fn display_manager_get_label_for_display(display_manager: &mut DisplayManager, display_id: DisplayId) -> Option<&mut DisplayLabel>` | `pub(crate)` |
| `display_manager_get_display_for_label` | `src/display_manager.c:35` | event loop | `DisplayManager` (declared) | `fn display_manager_get_display_for_label(display_manager: &mut DisplayManager, label: &[u8]) -> Option<&mut DisplayLabel>` | `pub(crate)` |
| `display_manager_remove_label_for_display` | `src/display_manager.c:47` | event loop | `DisplayManager` (declared) | `fn display_manager_remove_label_for_display(display_manager: &mut DisplayManager, display_id: DisplayId) -> bool` | `pub(crate)` |
| `display_manager_set_label_for_display` | `src/display_manager.c:61` | event loop | `DisplayManager` (declared) | `fn display_manager_set_label_for_display(display_manager: &mut DisplayManager, display_id: DisplayId, label: String)` | `pub(crate)` |
| `display_manager_main_display_uuid` | `src/display_manager.c:80` | — | — | **not translated** — see §3 | — |
| `display_manager_main_display_id` | `src/display_manager.c:86` | event loop | none | `fn display_manager_main_display_id() -> DisplayId` | `pub(crate)` |
| `display_manager_active_display_uuid` | `src/display_manager.c:91` | any | none | `fn display_manager_active_display_uuid() -> Option<CFStringOwned>` | private |
| `display_manager_active_display_id` | `src/display_manager.c:96` | any | none | `fn display_manager_active_display_id() -> DisplayId` | `pub(crate)` |
| `display_manager_dock_display_uuid` | `src/display_manager.c:107` | event loop | none | `fn display_manager_dock_display_uuid() -> Option<CFStringOwned>` | private |
| `display_manager_dock_display_id` | `src/display_manager.c:113` | event loop | none | `fn display_manager_dock_display_id() -> DisplayId` | `pub(crate)` |
| `display_manager_point_display_uuid` | `src/display_manager.c:124` | event loop | none | `fn display_manager_point_display_uuid(point: CGPoint) -> Option<CFStringOwned>` | private |
| `display_manager_point_display_id` | `src/display_manager.c:129` | event loop | none | `fn display_manager_point_display_id(point: CGPoint) -> DisplayId` | `pub(crate)` |
| `display_manager_coordinate_comparator` | `src/display_manager.c:140` | event loop | none | `unsafe extern "C-unwind" fn display_manager_coordinate_comparator(a_display: *const c_void, b_display: *const c_void, context: *mut c_void) -> CFComparisonResult` | private |
| `display_manager_display_id_arrangement` | `src/display_manager.c:165` | event loop | `DisplayManager` | `fn display_manager_display_id_arrangement(display_id: DisplayId, display_manager: &mut DisplayManager) -> i32` | `pub(crate)` |
| `display_manager_arrangement_display_uuid` | `src/display_manager.c:199` | event loop | `DisplayManager` | `fn display_manager_arrangement_display_uuid(arrangement: i32, display_manager: &mut DisplayManager) -> Option<CFStringOwned>` | private |
| `display_manager_arrangement_display_id` | `src/display_manager.c:221` | event loop | `DisplayManager` | `fn display_manager_arrangement_display_id(arrangement: i32, display_manager: &mut DisplayManager) -> DisplayId` | `pub(crate)` |
| `display_manager_cursor_display_id` | `src/display_manager.c:232` | event loop | none | `fn display_manager_cursor_display_id() -> DisplayId` | `pub(crate)` |
| `display_manager_prev_display_id` | `src/display_manager.c:239` | event loop | `DisplayManager` | `fn display_manager_prev_display_id(display_id: DisplayId, display_manager: &mut DisplayManager) -> DisplayId` | `pub(crate)` |
| `display_manager_next_display_id` | `src/display_manager.c:247` | event loop | `DisplayManager` | `fn display_manager_next_display_id(display_id: DisplayId, display_manager: &mut DisplayManager) -> DisplayId` | `pub(crate)` |
| `display_manager_first_display_id` | `src/display_manager.c:255` | event loop | `DisplayManager` | `fn display_manager_first_display_id(display_manager: &mut DisplayManager) -> DisplayId` | `pub(crate)` |
| `display_manager_last_display_id` | `src/display_manager.c:260` | event loop | `DisplayManager` | `fn display_manager_last_display_id(display_manager: &mut DisplayManager) -> DisplayId` | `pub(crate)` |
| `display_manager_find_closest_display_in_direction` | `src/display_manager.c:266` | event loop | none | `fn display_manager_find_closest_display_in_direction(source_display_id: DisplayId, direction: i32) -> DisplayId` | `pub(crate)` |
| `display_manager_menu_bar_hidden` | `src/display_manager.c:297` | event loop | none | `fn display_manager_menu_bar_hidden() -> bool` | `pub(crate)` |
| `display_manager_menu_bar_rect` | `src/display_manager.c:304` | event loop | none | `fn display_manager_menu_bar_rect(display_id: DisplayId) -> CGRect` | `pub(crate)` |
| `display_manager_dock_hidden` | `src/display_manager.c:334` | event loop | none | `fn display_manager_dock_hidden() -> bool` | `pub(crate)` |
| `display_manager_dock_orientation` | `src/display_manager.c:339` | event loop | none | `fn display_manager_dock_orientation() -> i32` | `pub(crate)` |
| `display_manager_dock_rect` | `src/display_manager.c:347` | event loop | none | `fn display_manager_dock_rect() -> CGRect` | `pub(crate)` |
| `display_manager_active_display_is_animating` | `src/display_manager.c:355` | — | — | **not translated** — see §3 | — |
| `display_manager_display_is_animating` | `src/display_manager.c:373` | event loop | none | `fn display_manager_display_is_animating(display_id: DisplayId) -> bool` | `pub(crate)` |
| `display_manager_active_display_count` | `src/display_manager.c:391` | any | none | `fn display_manager_active_display_count() -> i32` | private |
| `display_manager_active_display_list` | `src/display_manager.c:398` | any | none | `fn display_manager_active_display_list() -> Vec<DisplayId>` | `pub(crate)` |
| `display_manager_find_element_at_point` | `src/display_manager.c:406` | event loop | `WindowManager` | `fn display_manager_find_element_at_point(point: CGPoint, window_manager: &mut WindowManager) -> Option<CFRetained<AXUIElement>>` | private |
| `display_manager_focus_display_with_window_at_point` | `src/display_manager.c:430` | event loop | `WindowManager` | `fn display_manager_focus_display_with_window_at_point(point: CGPoint, window_manager: &mut WindowManager) -> WindowId` | `pub(crate)` |
| `display_manager_set_active_display_id` | `src/display_manager.c:454` | event loop | none | `fn display_manager_set_active_display_id(display_id: DisplayId)` | `pub(crate)` |
| `display_manager_focus_display` | `src/display_manager.c:463` | event loop | `WindowManager` | `fn display_manager_focus_display(display_id: DisplayId, space_id: SpaceId, window_manager: &mut WindowManager)` | `pub(crate)` |
| `display_manager_focus_space` | `src/display_manager.c:483` | event loop | `MissionControlMode` | `fn display_manager_focus_space(display_id: DisplayId, space_id: SpaceId, mission_control_mode: &mut MissionControlMode) -> SpaceOpError` | `pub(crate)` |
| `display_manager_begin` | `src/display_manager.c:497` | start-up only (main) | `DisplayManager` (declared) | `fn display_manager_begin(display_manager: &mut DisplayManager) -> bool` | `pub(crate)` |

### Row notes

**`display_manager_query_displays`.** The C takes no manager; `Managers = { DisplayManager }` comes
entirely from `display_serialize`, so `display_manager` is appended after `flags`. The `bool`
return stays `bool` (`DECISIONS.md` 32); the `if (!display_list) return false;` at `:11` becomes
the empty-`Vec` check, which in Rust can only be reached when `CGGetActiveDisplayList` reports no
displays.

**The four label functions.** `struct display_manager *dm` is parameter one in the C, and step 2
keeps it there; step 3 appends nothing. `struct display_label *` is not one of the record types
`DECISIONS.md` 14 turns into a handle — labels are a plain `Vec<DisplayLabel>` field of the
manager, never pointed at across a call — so the returned pointer becomes
`Option<&mut DisplayLabel>`, matching `space_manager_get_label_for_space`, whose returned value is
mutated in place at `patterns/state-and-ownership.md:941-943`. `char *label` is an owned
allocation from `message.c:596` in `set_`, so it is `String` moved in (`§3.2`); it is a borrowed
lookup key in `get_display_for_label`, where `patterns/message-and-serialisation.md:586-589`
fixes the call site as `c_string_at(message_bytes, value.token.start)`, a `&[u8]`, so the
parameter is `&[u8]` and `string_equals` at `:39` becomes `display_label.label.as_bytes() == label`.

**`display_manager_active_display_id`** runs on main inside `display_manager_begin`
(`display_manager.c:499`) and on the event loop from `event_loop.c`, `message.c` and
`space_manager.c` — `any`. `assert(uuid)` at `:99` becomes `debug_assert!`
(`DECISIONS.md` 33), and the `None` arm therefore still has to do something defined in a release
build; that is a body decision, one `DEVIATIONS.md` line.

**`display_manager_coordinate_comparator`** is passed to `CFArraySortValues` at `:180` and `:210`
and matches `objc2_core_foundation::CFComparatorFunction`
(`patterns/ffi-objc-and-os.md` §19.7). It takes no manager, which is what makes the two sort calls
legal while `display_manager_display_id_arrangement` and
`display_manager_arrangement_display_uuid` hold `&mut DisplayManager`: `g_display_manager.order`
travels into the callback as the `context` integer the C already packs, read back as
`context as usize as u32`. The parameter names are `a_display` / `b_display` per
`GLOSSARY.md` §10.3.

**`display_manager_find_closest_display_in_direction`.** The header spells parameter one
`acting_did` (`display_manager.h:77`); the definition spells it `source_did`
(`display_manager.c:266`). The C source is ground truth, so it is `source_display_id`
(`GLOSSARY.md` §10.2, `s_did` row). `direction` stays `i32`: `DIR_NORTH` and friends are
`pub(crate) const … : i32` (`GLOSSARY.md:1129`), not an enum. No manager — the only callees are
`display_manager_active_display_list` and the four `area_*` helpers of `view.c`.

**`display_manager_active_display_count` / `_list`.** Both are reached from `space_manager_begin`
(`space_manager.c:1216`), which runs on main in `patterns/state-and-ownership.md` §1.4 step 3, and
from the event loop afterwards — `any`. `_list`'s `int *count` is the returned buffer's length and
collapses into the `Vec`. `_count` keeps its own return: it is a separate `CGGetActiveDisplayList`
probe with a NULL buffer, not a length paired with a returned array, so the "a count that is not
derived from the same buffer stays" clause of §2.2 applies.

**`display_manager_find_element_at_point`** reads `g_window_manager.system_element`
(`:409`), which `GLOSSARY.md` §3.1 keeps as a `WindowManager` field, so
`Managers = { WindowManager }` and the manager is appended after `point`. Both
`AXUIElementCopyElementAtPosition` and `AXUIElementCopyAttributeValue` are copy-rule calls, so the
return is `Option<CFRetained<AXUIElement>>`; the `element_ref` leak on the `!role` path
(`:414` returns without releasing `element_ref`) disappears into `Drop`. One `DEVIATIONS.md` line.

**`display_manager_focus_display_with_window_at_point`** returns the **window** id the C calls
`element_id`, not a display id: `event_loop.c:1443-1446` stores it straight into
`g_mouse_state.ffm_window_id`, which is `MouseDragState::ffm_window_id: WindowId`
(`patterns/state-and-ownership.md` §1.1). `WindowId(0)` is the `goto out` / `goto err_ref` result
and is what `if (!wid)` at `event_loop.c:1444` tests. `Managers = { WindowManager }` comes only
from `display_manager_find_element_at_point`: `window_manager_focus_window_with_raise`
(`window_manager.c:1324`) reaches no manager — its `#if 1` arm is
`_SLPSSetFrontProcessWithOptions`, `window_manager_make_key_window` and
`AXUIElementPerformAction`, and `window_manager_make_key_window` (`window_manager.c:1280-1290`)
touches only the `g_event_bytes` buffer, which `§1.2` turns into a local.

**`display_manager_focus_display`** is worked example (5) of
`patterns/state-and-ownership.md` §2.3 and this row reproduces it exactly, including the fact that
`DisplayManager` is **not** in the set despite the function's name and file.
`Managers = { WindowManager }` comes from `g_window_manager` at `:465` and `:468`, from
`window_manager_find_window_on_space_by_rank_filtering_window` and `window_manager_center_mouse`
(both declare `struct window_manager *`), and from `space_manager_active_space`
(`space_manager.c:653`), which reaches `g_window_manager` and nothing else. Note that example
(5)'s supporting claim that `window_manager_focus_window_with_raise` reads
`WindowManager::focused_window_psn` through `window_manager_focus_window_without_raise` does not
hold against the C — `:1324-1334` never calls `_without_raise`. The conclusion is unchanged; the
route is the two declared-manager callees.

**`display_manager_focus_space`** takes no manager. `mission_control_is_active`
(`mission_control.c:108`) reads `g_mission_control_mode`, which
`patterns/state-and-ownership.md` §1.2 makes the `EventLoopOwnedState::mission_control_mode`
**field** — not a manager, and §2.2 step 3's append list is exactly the five managers of
`src/yabai.c:27-35`. `display_manager_display_is_animating`, `space_display_id` and
`scripting_addition_focus_space` reach only `g_connection` and the `OSAX_PATHS` static. See the
judgement note at the end of this file: wave 1 needs `mission_control_is_active` to be settled,
and `patterns/memory-text-and-os-objects.md:1321` writes it as a zero-argument call.

**`display_manager_begin`** runs on main, at `yabai.c:303`, and still takes `&mut DisplayManager`.
`DECISIONS.md` 20 restricts **main-thread callbacks**, not start-up; `patterns/state-and-ownership.md`
§1.4 step 3 names `display_manager_begin` among the calls that run "on the main thread against
`&mut state`", before the state is moved into the event-loop thread. Its `DisplayManager` is
declared in C at position one and stays there. `dm->labels` is never initialised
(`display_manager.c:498-505`); `Vec::new()` from `Default` is the zeroed global the C relies on.

---

## 3. Dead functions — not translated

`DECISIONS.md` 5: a definition with no caller anywhere in the tree is removed, one line each in
`DEVIATIONS.md`. Verified by grepping the whole of `src/` except `src/osax/`; each appears exactly
twice, as its header declaration and its definition. All three are also on the dead list of
`files/space-and-display.md:1615-1617`.

| C name | file:line | why |
|---|---|---|
| `display_space_count` | `src/display.c:190` | declared `display.h:43`, defined, never called. Its body is `display_space_list` without the list |
| `display_manager_main_display_uuid` | `src/display_manager.c:80` | declared `display_manager.h:61`, defined, never called. `display_manager_main_display_id` is the one that is used |
| `display_manager_active_display_is_animating` | `src/display_manager.c:355` | declared `display_manager.h:83`, defined, never called. `display_manager_display_is_animating` (`:373`) is the live twin, and the C comment at `:370` — "This does not return a correct result on modern macOS versions." — belongs to the live twin as well and is carried there |

Their declarations in `display.h` and `display_manager.h` go with them.

---

## 4. Cross-module calls this module makes

Argument shapes this module expects to pass. Each is owned by another wave-0b document; a
disagreement means the fixed point was computed wrong somewhere and is resolved there, not here.

| callee | module | shape this module passes |
|---|---|---|
| `space_manager_mission_control_index(space_id: SpaceId) -> i32` | `space_manager` | `display.c:79`, no manager |
| `space_manager_active_space(window_manager: &mut WindowManager) -> SpaceId` | `space_manager` | `display_manager.c:475`, reborrows this module's `&mut WindowManager` |
| `space_display_id(space_id: SpaceId) -> DisplayId` | `space` | `display_manager.c:491`, no manager |
| `window_manager_find_window_on_space_by_rank_filtering_window(window_manager: &mut WindowManager, space_id: SpaceId, rank: i32, filter_window_id: WindowId) -> Option<WindowId>` | `window_manager` | `display_manager.c:465`, manager declared first in C |
| `window_manager_focus_window_with_raise(window_process_serial_number: &ProcessSerialNumber, window_id: WindowId, window_ref: AXUIElementRef)` | `window_manager` | `display_manager.c:443`, `:467`, no manager |
| `window_manager_center_mouse(window_manager: &mut WindowManager, window_id: WindowId)` | `window_manager` | `display_manager.c:468`, manager declared first in C |
| `mission_control_is_active(mission_control_mode: &mut MissionControlMode) -> bool` | `mission_control` | `display_manager.c:485`; see judgement 5 |
| `scripting_addition_focus_space(space_id: SpaceId) -> bool` | `sa` | `display_manager.c:494`, no manager |
| `workspace_display_notch_height(display_id: DisplayId) -> i32` | `workspace` | `display.c:141`, no manager |
| `workspace_is_macos_bigsur() -> bool`, `_monterey()`, `_ventura()` | `workspace` | `display_manager.c:375-377`, read the `MACOS_VERSION` static |
| `ax_window_id(element_ref: &AXUIElement) -> u32` | `misc::helpers` | `display_manager.c:438`; stays `u32` with its `0` sentinel per `patterns/memory-text-and-os-objects.md` §1.12, wrapped as `WindowId` by this module |
| `ts_cfstring_copy(string: &CFString) -> Option<String>` | `misc::helpers` | `display.c:40` |
| `json_bool(value: bool) -> &'static str` | `misc::helpers` | `display.c:95` |
| `area_from_cgrect(rect: CGRect) -> Area`, `area_max_point(area: Area) -> CGPoint` | `view` | `display_manager.c:275-276`, `:282-283`; `static inline` in the C, so `view` must expose them `pub(crate)` |
| `area_is_in_direction(first_area: &Area, first_area_max_point: CGPoint, second_area: &Area, second_area_max_point: CGPoint, direction: i32) -> bool` | `view` | `display_manager.c:285`; same `static inline` note |
| `area_distance_in_direction(first_area: &Area, first_area_max_point: CGPoint, second_area: &Area, second_area_max_point: CGPoint, direction: i32) -> i32` | `view` | `display_manager.c:286`; same `static inline` note |

---

## 5. Judgement calls

Every place the mechanical rule did not decide on its own, and what this file chose.

1. **`Response` versus `&mut dyn std::io::Write`.** `patterns/state-and-ownership.md` §2.2 says
   `FILE *rsp` becomes `response: &mut dyn std::io::Write`;
   `patterns/message-and-serialisation.md` §7 and `GLOSSARY.md` §10.1 and §10.4 say
   `response: &mut Response`, and `Response` is a concrete type carrying the failure prefix and the
   silent case that a `dyn Write` cannot. Chosen: `&mut Response`. Two rows,
   `display_serialize` and `display_manager_query_displays`.

2. **Visibility is its own column, so no signature line carries `pub`.** Wave 1 pastes
   `<visibility> <signature>`. `patterns/ffi-objc-and-os.md` §19.5 and §19.7 write the two callback
   signatures with a bare `pub`; in a binary crate that is the same reachability as `pub(crate)`,
   and `pub(crate)` is what this table uses throughout for uniformity with the other nine modules.

3. **`display_handler` is `pub(crate)` although the tsv shows no caller outside its file.** The
   tsv's extraction counts calls; `display_manager.c:505` takes the function's **address**, which
   crosses the file boundary just as hard. Same reasoning would apply to
   `display_manager_coordinate_comparator` except that both of its address-takings
   (`display_manager.c:180`, `:210`) are inside its own file, so it stays private.

4. **`struct display_label *` returns become `Option<&mut DisplayLabel>`.** The mechanical rule
   says "a returned record pointer becomes Option of its handle", but a label has no handle —
   `patterns/state-and-ownership.md` §3.1 defines five handle types and none is a label, and
   `DECISIONS.md` 14 lists windows, applications, views and tree nodes, not labels. The precedent
   is `space_manager_get_label_for_space`, written as returning a mutable borrow at
   `patterns/state-and-ownership.md:941-943`. `&mut` rather than `&` even for
   `display_manager_get_display_for_label`, whose one caller only reads `->did`, because a
   read-only variant reintroduces exactly the mixed-borrow problem §2.1 rejects for managers.

5. **`display_manager_focus_space` takes `mission_control_mode: &mut MissionControlMode`.**
   Settled across all nine documents by the cross-module pass, against this file's first reading.
   `§2.2` step 3 enumerates five managers because `§2.1` defines `Managers(f)` over managers, but
   `TRANSLATION_PLAN.md` §3.2 — the spec for this wave — names `g_mission_control_mode` among the
   eight event-loop-owned globals step 3 scans for and fixes the append order as the
   `EventLoopOwnedState` field order, which puts `mission_control_mode` (`yabai.c:37`) last.
   `DECISIONS.md` 13 then forbids `mission_control_is_active` from reaching the field through a
   global, so it takes the parameter and every one of the 45 functions whose `transitive_globals`
   contain `g_mission_control_mode` takes it too. `patterns/memory-text-and-os-objects.md:1321`
   writes `mission_control_is_active()` with no arguments; that sketch predates the field's move
   into `EventLoopOwnedState` and does not survive it.

6. **`display_manager_begin` keeps its manager although it runs on main.** `DECISIONS.md` 20 is
   about main-thread **callbacks** dereferencing event-loop-owned memory; `display_manager_begin`
   runs before the hand-over, and `patterns/state-and-ownership.md` §1.4 step 3 names it
   explicitly as running "on the main thread against `&mut state`". The only rows where
   `DECISIONS.md` 20 bites in this module are `display_handler`, which touches nothing but the
   `EVENT_SENDER` static, and `display_manager_coordinate_comparator`, which is synchronous inside
   the caller's own thread.

7. **`CFStringOwned` rather than `CFRetained<CFString>`.** Settled by the cross-module pass,
   against this file's first reading. `GLOSSARY.md` §2.5 names the owned `CFStringRef`
   `CFStringOwned` in `crate::ffi` and the glossary is the spelling authority;
   `state-access/window-application-process.md` judgement 5 and `state-access/view.md`
   (`View::uuid`) already spell it that way, so the four `CFStringRef` returns here are
   `Option<CFStringOwned>`. It is not a third wrapper type — it is the crate alias for the
   `CFRetained<CFString>` of `patterns/memory-text-and-os-objects.md` §1.1, which that section's
   "exactly two wrapper types" rule is unaffected by. The one owned `CFArray` return in these nine
   files, `application_window_list`, has no alias and stays `CFRetained<CFArray>`.

8. **`display_handler` keeps `CGDirectDisplayID`, not `DisplayId`.** Its shape is dictated by
   `CGDisplayReconfigurationCallBack`. The parameter is still spelled `display_id` rather than the
   C's `did`, per `GLOSSARY.md` §10.2, which costs nothing because no body in either file both
   takes a `display_id` parameter and calls the `display_id` function.

9. **`flags: u64` without `mut`.** `display.c:24` mutates the parameter; so does `window.c:413`,
   whose signature `patterns/state-and-ownership.md` §2.3 example (2) writes as plain
   `flags: u64`. Followed, so that the nine modules' serialize functions read the same.

10. **Thread contexts wider than the phase-1 inventory says.** `files/space-and-display.md` marks
    `display_space_list`, `display_manager_active_display_list` and
    `display_manager_active_display_count` as event loop. They are reached from
    `space_manager_begin` (`space_manager.c:1216`, `:1221`), which
    `patterns/state-and-ownership.md` §1.4 step 3 moves onto the main thread, so they are `any`.
    Same for `display_uuid`, `display_id`, `display_manager_active_display_uuid` and
    `display_manager_active_display_id` by way of `display_manager_begin`. None of these takes a
    manager, so nothing about the hand-over is at risk.

11. **`display_space_list` returns `Vec<SpaceId>`, collapsing C's NULL and C's empty array into
    one value.** The assignment's rule ("a returned list becomes Vec") is followed rather than
    `Option<Vec<SpaceId>>`; every one of the eleven call sites guards with `if (space_list)` and
    then loops `count` times, so an empty `Vec` reproduces both C outcomes. Same for
    `display_manager_active_display_list`.
