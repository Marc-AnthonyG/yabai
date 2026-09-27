# Idioms and conventions

Elaborates decisions 4, 5, 30, 31, 32, 33, 37, 38, 39 and 40 of
`doc/rust-rewrite/DECISIONS.md` into rules a phase-2 translator applies without choosing.

Every table in this document is closed. If a construct is in a table, the Rust spelling in that
table is the only correct one. If a construct is not in a table, §2.4 and §7.1 say what to do,
and the answer is never "pick something reasonable".

Ground truth for everything below is the C source; line numbers were re-verified against
`src/**` at commit `dd84572`. Where an inventory in `files/` or `sweeps/` reports a different
line, the number here wins.


### Contents

| § | |
| --- | --- |
| [1](#1-the-four-rules-everything-else-is-derived-from) | The four rules everything else is derived from |
| [2](#2-naming) | Naming — C to Rust, enum variants, `misc/macros.h`, **the glossary**, unity-build visibility |
| [3](#3-module-paths) | Module paths |
| [4](#4-x-macro-lists) | X-macro lists |
| [5](#5-enums-discriminants-and-string-tables) | Enums, discriminants, positional and sparse string tables, `DIR_*`/`STACK` |
| [6](#6-flag-sets) | Flag sets: newtypes, the accessor trio, whole-set equality, the masks that are not "all" |
| [7](#7-the-numeric-conversion-rulebook) | The numeric conversion rulebook |
| [8](#8-sentinels-bool-option-and-result) | Sentinels, `bool`, `Option`, `Result` |
| [9](#9-assertions-exits-and-the-null-then-crash-sites) | `debug_assert!`, `error!`/`require!`, the three NULL-then-crash sites |
| [10](#10-the-comment-policy) | The comment policy and the complete carried-comment list |
| [11](#11-dead-code-that-is-not-translated) | Dead code that is not translated |
| [12](#12-deviationsmd) | `DEVIATIONS.md`: line format and the known deviations |

---

## 1. The four rules everything else is derived from

1. **A function, type, enum variant or constant keeps the C name** (decision 37). A reviewer
   must be able to put the C and the Rust side by side and match them line for line.
2. **A field, parameter, local or loop binding is never abbreviated** (decision 37). The C's
   `wm`, `sid`, `rsp`, `dt` are expanded, every time, to the one spelling §2.4 fixes.
3. **Only comments that exist in the C are written** (decision 38). Nothing else — no doc
   comments, no `SAFETY:`, no "this mirrors `window.c:412`". §10 is the complete list; a
   reviewer greps the Rust for `//` and checks every hit against it.
4. **Every implicit C conversion becomes an explicit `as` at the same point in the expression**
   (decision 30). §7 is the rulebook, with every distinct shape found in the source.

Rules 1 and 2 pull against each other exactly once: `struct window_node`'s field `child` and
`enum window_node_child`'s variant `CHILD_NONE`. The field is a field (rule 2 — but `child` is
not an abbreviation, so it stays `child`); the variant is a variant (rule 1 — `Child::None`).
There is no other collision.

---

## 2. Naming

### 2.1 C construct to Rust construct

| C | Rust | Example |
| --- | --- | --- |
| function `snake_case` | identical `snake_case` | `window_manager_find_window` → `window_manager_find_window` |
| `struct foo_bar` | `struct FooBar` | `struct window_node` → `struct WindowNode` |
| `enum foo_bar` (non-flag) | `enum FooBar` | `enum window_node_split` → `enum WindowNodeSplit` |
| `enum foo_bar` (OR-ed mask) | newtype `struct FooBar(pub uN)` + associated consts | §6 |
| struct field | same word, expanded per §2.4 | `->window_count` → `.window_count`; `->wid` → `.window_id` |
| function parameter | same word, expanded per §2.4 | `struct window_manager *wm` → `window_manager: &mut WindowManager` |
| local / loop binding | same word, expanded per §2.4 | `uint64_t sid` → `let space_id` |
| `#define NAME "text"` | `pub(crate) const NAME: &str = "text";` | `COMMAND_WINDOW_GRID` unchanged |
| `#define NAME <number>` | `pub(crate) const NAME: <C type>= <number>;` | `MAXLEN` → `pub(crate) const MAXLEN: usize = 512;` |
| file-scope `static const char *x[]` | `static X: [&str; N]` (screaming snake) | `view_type_str` → `VIEW_TYPE_STR` |
| file-scope `static` mutable data | see decisions 12, 18, 23 | not a naming question |
| global `g_foo` | `g_foo`, with `#[allow(non_upper_case_globals)]` on the item | `g_window_manager` |
| `EVENT_HANDLER(X)` expansion | `fn event_handler_x(...)` | §4 |
| macro `min(a, b)` etc. | `fn` or `macro_rules!` with the C name | §2.3 |

Type aliases: C `typedef`s that exist only to name a callback signature
(`observer_callback` at `src/application.h:5`, `connection_callback` at `src/misc/extern.h:2`,
`table_hash_func` / `table_compare_func` at `src/misc/hashtable.h:5,8`) become Rust `type`
aliases with the same name CamelCased: `ObserverCallback`, `ConnectionCallback`,
`TableHashFunc`, `TableCompareFunc`.

### 2.2 Enum variants

Drop the C prefix and CamelCase what is left. The prefix is already the enum name.

```rust
enum WindowNodeSplit { None, Y, X, Auto }
enum WindowNodeChild { None, Second, First }
enum WindowInsertionPoint { Focused, First, Last }
enum ViewType { Default, Bsp, Stack, Float }
enum FfmMode { Disabled, Autofocus, Autoraise }
enum PurifyMode { Disabled, Managed, Always }
enum MouseMode { None, Move, Resize, Swap, Stack }
enum DisplayArrangementOrder { Default, X, Y }
enum ExternalBarMode { Off, Main, All }
enum WindowOriginMode { Default, Focused, Cursor }
```

Where the C prefix is not the enum name, keep enough of it that the variant is unambiguous, and
**keep the C's own word**, abbreviation included — a variant is covered by rule 1, not rule 2:

| C constant | Rust |
| --- | --- |
| `SPACE_OP_ERROR_MISSING_SRC` | `SpaceOpError::MissingSrc` |
| `SPACE_OP_ERROR_MISSING_DST` | `SpaceOpError::MissingDst` |
| `WINDOW_OP_ERROR_SAME_STACK` | `WindowOpError::SameStack` |
| `WINDOW_OP_ERROR_INVALID_SRC_VIEW` | `WindowOpError::InvalidSrcView` |
| `SIGNAL_WINDOW_FOCUSED` | `SignalType::WindowFocused` |
| `SA_OPCODE_WINDOW_MOVE` | `SaOpcode::WindowMove` |
| `MISSION_CONTROL_MODE_SHOW_ALL_WINDOWS` | `MissionControlMode::ShowAllWindows` |
| `MOUSE_DROP_ACTION_WARP_BOTTOM` | `MouseDropAction::WarpBottom` |
| `EVENT_TYPE_ENTRY(APPLICATION_LAUNCHED)` | `EventType::ApplicationLaunched` |

`MissingSrc` and `InvalidSrcView` keep `Src`. Do not "fix" them to `Source`.

### 2.3 Constants, statics and the `misc/macros.h` helpers

`src/misc/macros.h` becomes `src/misc/macros.rs`. Every spelling is fixed here so ten
translators import identical names.

| C | Rust |
| --- | --- |
| `KILOBYTES(v)` / `MEGABYTES(v)` (`macros.h:4-5`) | `pub(crate) const fn kilobytes(value: u64) -> u64` and `megabytes`; called at `src/yabai.c:279`, `:283` |
| `GIGABYTES(v)` (`macros.h:6`) | **not translated** — no call site (§11) |
| `array_count(a)` (`macros.h:8`) | no item; write `array.len() as i32` at the call site |
| `min(a, b)` (`macros.h:9`) | **not translated** — the macro has no call site anywhere in `src/**`; decision 5, one `DEVIATIONS.md` row |
| `max(a, b)` (`macros.h:10`) | `pub(crate) fn max<T: PartialOrd>(first: T, second: T) -> T`; the only two surviving call sites are `src/window_manager.c:353-354` and both are `f64` (§7.12) |
| `add_and_clamp_to_zero(a, b)` (`macros.h:11`) | `pub(crate) fn add_and_clamp_to_zero(value: i32, delta: i32) -> i32` — all five call sites (`src/space_manager.c:221`, `:367-370`) are `int` |
| `in_range_ii(a, b, c)` (`macros.h:12`) | `pub(crate) fn in_range_ii<T: PartialOrd>(value: T, low: T, high: T) -> bool` — call sites are `f32` (`src/message.c:1347`) and `i32` (`src/window_manager.c:1135`) |
| `in_range_ie` (`macros.h:13`) / `in_range_ei` (`macros.h:14`) | two more functions with the same signature and the same parameter names. **Distinct functions, not one with a flag**: `in_range_ii` is used for menubar opacity (`src/message.c:1348`) while `in_range_ei`, with an *exclusive* lower bound, is used for active and normal window opacity (`src/message.c:1356`, `:1365`). Merging them breaks `window_opacity 0.0` |
| `in_range_ee` (`macros.h:15`) | **not translated** — no call site (§11) |
| `lerp(a, t, b)` (`macros.h:16`) | `pub(crate) fn lerp(start: f64, interpolant: f32, end: f32) -> f64` — the `1.0` in the macro is a `double`, see §7.4 for the body and the narrowing at the call |
| `FAILURE_MESSAGE` (`macros.h:18`) | `pub(crate) const FAILURE_MESSAGE: &[u8] = b"\x07";` |
| `MAXLEN` (`macros.h:20`) | `pub(crate) const MAXLEN: usize = 512;` |
| `REGEX_MATCH_UD/YES/NO` (`macros.h:22-24`) | not constants — `enum RegexMatch { Undefined, Yes, No }` in `src/misc/regex.rs` |
| `DIR_NORTH/EAST/SOUTH/WEST` (`macros.h:26-29`) | `pub(crate) const DIR_NORTH: i32 = 360;` and the three others, values unchanged |
| `STACK` (`macros.h:31`) | `pub(crate) const STACK: i32 = 111;` |
| `TYPE_ABS` / `TYPE_REL` (`macros.h:33-34`) | `pub(crate) const TYPE_ABS: i32 = 0x1;` / `0x2` |
| `HANDLE_TOP..HANDLE_ABS` (`macros.h:36-40`) | newtype `ResizeHandle(pub u8)` with five associated consts, §6.1 |
| `LAYER_AUTO/BELOW/NORMAL/ABOVE` (`macros.h:42-45`) | `pub(crate) const LAYER_AUTO: i32 = 0;` … §5.3 |

`min` and `max` shadow the `std` free functions by being defined in `misc::macros` and imported
explicitly; they are `fn`, not macros, because no call site double-evaluates.

`AX_ABS` / `AX_DIFF` (`src/view.h:4-5`) live in `src/view.rs`:
`pub(crate) fn ax_diff(first: f64, second: f64) -> bool`. See §7.5 for the exact body.

### 2.4 The glossary

**This is the table.** Left column is every abbreviated spelling that recurs in `src/**`
(excluding `src/osax/`). Right column is the only Rust spelling. It applies to fields,
parameters, locals, loop bindings and closure captures. It does **not** apply to function names,
type names, enum variants or `#define` string constants, which keep the C spelling by rule 1.

#### Managers and long-lived structs

| C | Rust | seen at |
| --- | --- | --- |
| `wm` | `window_manager` | `src/window_manager.h:181`, 442 occurrences |
| `sm` | `space_manager` | `src/space_manager.h`, 258 occurrences |
| `dm` | `display_manager` | `src/display_manager.h:56-58` |
| `pm` | `process_manager` | `src/process_manager.h:31-32`, `src/process_manager.c:153` |
| `ms` | `mouse_state` | `src/event_loop.c:36`, `src/mouse_handler.h:102-103` |
| `es` | `event_signal` | `src/event_signal.c:8`, 134 occurrences in that file |

#### Identifiers

| C | Rust | note |
| --- | --- | --- |
| `wid` | `window_id` | `u32`, `0` means none |
| `sid` | `space_id` | `u64`, `0` means none |
| `did` | `display_id` | `u32`, `0` means none |
| `pid` | `process_id` | `i32` (`pid_t`), `0` means none |
| `psn` | `process_serial_number` | `ProcessSerialNumber` |
| `cid` | `connection_id` | `i32`; the SkyLight connection |
| `uuid` | `uuid` | not an abbreviation; unchanged |
| `mci` | `mission_control_index` | `src/space_manager.c:824-825` |
| `a_sid` / `b_sid` / `n_sid` | `a_space_id` / `b_space_id` / `next_space_id` | `src/space_manager.c:560-575` |
| `src_sid` / `dst_sid` / `src_prev_sid` | `source_space_id` / `destination_space_id` / `source_previous_space_id` | |
| `filter_wid` | `filter_window_id` | `src/window_manager.h:125-126` |
| `psn_sid` | `psn_space_id` | `src/event_loop.c:359`; `psn` here is part of the concept, not a binding of its own |

#### Prefixes and suffixes

| C | Rust |
| --- | --- |
| `src_` (`src_view`, `src_node`, `src_window`, `src_node_add`, `src_node_rm`) | `source_` (`source_view`, `source_node`, `source_window`, `source_node_add`, `source_node_remove`) |
| `dst_` (`dst_view`, `dst_node`, `dst_window`) | `destination_` (`destination_view`, `destination_node`, `destination_window`) |
| `a_` / `b_` (`a_view`, `b_node`) | unchanged — `a_view`, `b_node` |
| bare `a` / `b` for two windows (`src/window_manager.c:1799`, `:1832`, `:1950`) | `a_window` / `b_window` |
| `dview` (`src/window_manager.c:2142`) | `display_view` |
| `ws_context` (`src/workspace.m:8-12`) | `workspace_context` |
| `srole` (`src/rule.c:11`) | `subrole`; `escaped_srole` → `escaped_subrole` |
| `app` as a field or local (`src/event_signal.c:146`, `struct rule`) | `application` |
| `insert_dir` (`src/view.h:165`) | `insert_direction` |
| `dir` as a parameter (`src/view.c:514`) | `direction` |

#### Scalars, buffers and loop bindings

| C | Rust |
| --- | --- |
| `rsp` | `response` — the `Response` of decision 28, not a `FILE` |
| `ref` (`struct window::ref`, `struct application::ref`) | `element_ref` — mandatory, `ref` is a Rust keyword |
| `window_ref` / `observer_ref` / `app_ref` | unchanged |
| `<anything>_ref` naming a CF value (`space_list_ref`, `sid_ref`, `uuid_ref`) | unchanged |
| `mod` (`src/event_loop.c:1134`, `src/mouse_handler.c:33`, `:64`) | `event_modifier` — mandatory, `mod` is a Rust keyword; it is the modifier the event carried, never the configured modifier held in `mouse_state` |
| `mod` in `ts_align` (`src/misc/ts.h:42`) | `misalignment` — the remainder `ptr & (a_ptr - 1)`; decision 17 replaces the arena, so the spelling is fixed here only to keep the table complete |
| `x_mod` / `y_mod` (`src/window_manager.c:350-351`) | `x_modifier` / `y_modifier` — the `-1` / `0` / `1` read off the `HANDLE_*` bits, not the mouse modifier above |
| `id_ptr` (`src/window.h:92`) | `id_pointer` — see decision 21; it is a liveness cell, not a pointer field |
| `len` / `cap` (`src/misc/sbuffer.h:6-7`) | `length` / `capacity` |
| `buf` | `buffer` |
| `str` (`src/event_signal.c:343`) | `string` |
| `tmp` (`src/space_manager.c:769`) | `temporary` |
| `prev` (`src/window_manager.c:1007`) | `previous` |
| `num` (`src/misc/helpers.h:321,328`) | `number` |
| `ptr` (`src/misc/ts.h:40`) | `pointer` |
| `dst` / `cursor` in `ts_string_escape` (`src/misc/helpers.h:286`) | `destination` / `cursor` |
| `dt` (`src/event_loop.c:363`, `:1266`, `:1353`, `src/event_signal.c:156`) | `delta_time` |
| `dx` / `dy` (`src/event_loop.c:1269-1270`) | `delta_x` / `delta_y` |
| `tx` / `ty` / `tw` / `th` (`src/view.h:61`) | `target_x` / `target_y` / `target_width` / `target_height` |
| `fx` / `fy` / `fw` / `fh` (`src/window_manager.c:353-356`, `:2172-2175`) | `frame_x` / `frame_y` / `frame_width` / `frame_height` — the frame being computed, never the `frame` parameter of `window_manager_resize_window_relative_internal` (`src/window_manager.c:346`) |
| `cw` / `ch` (`src/window_manager.c:2170-2171`) | `column_width` / `row_height` — `cw` divides by `c`, `ch` by `r`; §7.9 already writes `column_width` |
| `w` / `h` as a width/height field or parameter (`src/view.h:46-47`, `:53`, `:70`) | `width` / `height` |
| `x` / `y` as a coordinate | unchanged |
| `r` / `c` in `window_manager_apply_grid` (`src/window_manager.c:2124`) | `rows` / `columns` |
| `r1` / `r2` / `r1_max` / `r2_max` (`src/view.c:541-582`) | `first_area` / `second_area` / `first_area_max_point` / `second_area_max_point` |
| `t` / `mt` (`src/window_manager.c:544-554`) | `interpolant` / `eased_interpolant` |
| `f` / `wp` / `c` in `mouse_determine_drop_action` (`src/mouse_handler.c:110-112`) | `destination_window_frame` / `point_relative_to_frame_origin` / `center_rect` |
| `t` / `r` / `b` / `l` in `mouse_determine_drop_action` (`src/mouse_handler.c:113-116`) | `top_triangle` / `right_triangle` / `bottom_triangle` / `left_triangle` |
| `t` / `p` in `triangle_contains_point` (`src/misc/helpers.h:564`) | `triangle` / `point` — the spellings §7.12 already uses |
| `i` / `j` where an index is genuinely needed | `index` / `inner_index`; prefer an iterator |
| `n` as a count | `count` |
| `sr` / `sg` / `sb` / `sa` (`src/misc/helpers.h:632-635`) | `source_red` / `source_green` / `source_blue` / `source_alpha` |
| `one255` / `inv255` (`src/misc/helpers.h:601-602`) | unchanged — they are values, not abbreviations |
| `tb` (`src/misc/timer.h:104`) | not translated; the `PROFILE` block is dead code (§11) |

#### Spellings that are already correct and must not be "improved"

`connection`, `context`, `count`, `cursor`, `direction`, `flags`, `frame`, `gap`, `index`,
`info`, `label`, `layer`, `level`, `mask`, `mode`, `node`, `origin`, `parent`, `point`, `ratio`,
`root`, `size`, `split`, `type`, `value`, `view`, `window`, `zoom`.

Three C spellings collide with a Rust keyword: `ref`, `type` and `mod`. `ref` and `mod` are
renamed outright by the rows above (`element_ref`, `event_modifier`, `misalignment`), so no `r#`
escape is written for either.

`type` is a Rust keyword: a field named `type` becomes `r#type` only where it genuinely names a
type discriminator; there is no such field in the daemon (`view->layout` and `window_node::split`
carry the concept), so this never comes up.

### 2.5 Visibility: `static` in a `.c` file is not private

`src/manifest.m:80-97` includes every `.c`/`.m` into one translation unit, so a `static`
function in one file is freely called from another. **Default `pub(crate)` for every function
and every file-scope `static` that came from a `.c`/`.h` file.** Private `fn` is used only for
the functions listed as private below. Phase 3 tightens this; phase 2 must not break the build
guessing.

C `static` functions that are called from another file, verified by grepping each name across
`src/**`:

| C function | defined | called from |
| --- | --- | --- |
| `update_window_notifications` | `src/event_loop.c:16` | `src/view.c:45`, `src/view.c:111`, `src/yabai.c:341` |
| `mission_control_is_active` | `src/mission_control.c:108` | `src/display_manager.c`, `src/space_manager.c`, `src/event_loop.c:1015`, `:1067`, `:1119`, `:1156`, `:1237`, `:1348`, `:1487` |
| `scripting_addition_is_sip_friendly` | `src/sa.m:301` | `src/message.c:1304` |
| `area_from_cgrect` | `src/view.c:121` | `src/display_manager.c` |
| `area_max_point` | `src/view.c:126` | `src/display_manager.c`, `src/view.c:543-546` |
| `area_is_in_direction` | `src/view.c:541` | `src/display_manager.c` |
| `area_distance_in_direction` | `src/view.c:563` | `src/display_manager.c` |
| `area_make_pair` | `src/view.c:161` | `src/window_manager.c` |
| `window_node_get_gap` | `src/view.c:156` | `src/window_manager.c:2154` |
| `window_node_get_ratio` | `src/view.c:151` | `src/window_manager.c` |
| `window_node_get_split` | `src/view.c:136` | `src/window_manager.c` |
| `window_node_is_leaf` | `src/view.c` | `src/window_manager.c` |
| `window_node_is_left_child` | `src/view.c` | `src/window.c:594`, `src/window_manager.c` |
| `window_node_is_intermediate` | `src/view.c` | `src/space_manager.c` |
| `window_node_balance` | `src/view.c` | `src/space_manager.c`, `src/view.c:722`, `:800` |
| `window_node_equalize` | `src/view.c` | `src/space_manager.c` |
| `window_layer` | `src/window.c:113` | `src/window.c` only — private |
| `mission_control_mode_str[]` (data) | `src/mission_control.c:38` | `src/event_signal.c:338` |

Two more that are **not** `static` in C but have no caller outside their own file, and are
therefore private `fn` in Rust: `window_manager_animate_window_list_async`
(`src/window_manager.c:603`, absent from `window_manager.h`) and `message_loop_run`
(`src/message.c:3003`).

`enum mission_control_mode` is declared inside a `.c` file (`src/mission_control.c:29-36`) while
its instance is a global in `src/yabai.c:37`. In Rust the enum is defined in
`src/mission_control.rs` and `src/globals.rs` imports it. It is not redefined.

---

## 3. Module paths

Fixed by `doc/rust-rewrite/TRANSLATION_PLAN.md` §1.6-1.7 and repeated here so no translator has
to open that file. Crate root is `src/main.rs`; `src/misc.rs` and `src/ffi.rs` declare the two
directories (no `mod.rs`).

| C file | Rust module | path |
| --- | --- | --- |
| `src/yabai.c` | `crate` + `crate::globals` + `crate::state` | `src/main.rs`, `src/globals.rs`, `src/state.rs` |
| `src/view.h` + `src/view.c` | `crate::view` | `src/view.rs` |
| `src/window.h` + `src/window.c` | `crate::window` | `src/window.rs` |
| `src/application.h` + `src/application.c` | `crate::application` | `src/application.rs` |
| `src/process_manager.h` + `.c` | `crate::process_manager` | `src/process_manager.rs` |
| `src/display.h` + `src/display.c` | `crate::display` | `src/display.rs` |
| `src/display_manager.h` + `.c` | `crate::display_manager` | `src/display_manager.rs` |
| `src/space.h` + `src/space.c` | `crate::space` | `src/space.rs` |
| `src/space_manager.h` + `.c` | `crate::space_manager` | `src/space_manager.rs` |
| `src/window_manager.h` + `.c` | `crate::window_manager` | `src/window_manager.rs` |
| `src/event_loop.h` + `.c` | `crate::event_loop` | `src/event_loop.rs` |
| `src/event_signal.h` + `.c` | `crate::event_signal` | `src/event_signal.rs` |
| `src/rule.h` + `src/rule.c` | `crate::rule` | `src/rule.rs` |
| `src/message.h` + `src/message.c` | `crate::message` | `src/message.rs` |
| `src/mouse_handler.h` + `.c` | `crate::mouse_handler` | `src/mouse_handler.rs` |
| `src/workspace.h` + `src/workspace.m` | `crate::workspace` | `src/workspace.rs` |
| `src/sa.h` + `src/sa.m` | `crate::sa` | `src/sa.rs` |
| `src/mission_control.c` | `crate::mission_control` | `src/mission_control.rs` |
| `src/misc/macros.h` | `crate::misc::macros` | `src/misc/macros.rs` |
| `src/misc/log.h` | `crate::misc::log` | `src/misc/log.rs` |
| `src/misc/notify.h` | `crate::misc::notify` | `src/misc/notify.rs` |
| `src/misc/timer.h` + `src/misc/helpers.h:149-160` | `crate::misc::timer` | `src/misc/timer.rs` — only `read_os_timer` and `read_os_freq`; the `#if PROFILE >= 1` block is dead code (§11) |
| `src/misc/hashtable.h` | `crate::misc::table` | `src/misc/table.rs` |
| `src/misc/helpers.h:573-579` + `macros.h:22-24` | `crate::misc::regex` | `src/misc/regex.rs` |
| `src/misc/macros.h:18` + `src/message.c:418-427` | `crate::misc::response` | `src/misc/response.rs` |
| `src/misc/helpers.h` (the rest, minus `:149-160`) | `crate::misc::helpers` | `src/misc/helpers.rs` |
| `src/misc/service.h` | `crate::misc::service` | `src/misc/service.rs` |
| `src/misc/extern.h` | `crate::ffi::skylight`, `crate::ffi::skylight_dynamic` | `src/ffi/skylight.rs`, `src/ffi/skylight_dynamic.rs` |
| `src/misc/macho_dlsym.h` | `crate::ffi::macho` | `src/ffi/macho.rs` |
| `src/misc/autorelease.h` | none — dead code (§11) | — |
| `src/misc/memory_pool.h` | none — replaced (decision 17) | — |
| `src/misc/sbuffer.h` | none — replaced by `Vec` (decision 17) | — |
| `src/misc/ts.h` | none — replaced by `Vec`/`String` (decision 17) | — |
| `src/osax/common.h` | `OUT_DIR/osax_common.rs`, included by `crate::sa` | generated by `build.rs` |

There is never a `src/osax.rs`: creating it would make `src/osax/` that module's directory.

---

## 4. X-macro lists

There are six. Each becomes **one `macro_rules!`** (decision 31), invoked once per thing it
generates, exactly as the C `#define … / LIST / #undef` does. Nothing is hand-expanded: adding an
entry must stay a one-line change, because phase 3 relies on that.

| List | C location | entries | generates in C |
| --- | --- | --- | --- |
| `EVENT_TYPE_LIST` | `src/event_loop.h:6-46` | 40 | `enum event_type` (`:48-53`); handler names `EVENT_HANDLER_##value` via `EVENT_HANDLER(t)` (`:4`); the dispatch `switch` (`src/event_loop.c:1664-1668`) |
| `ANIMATION_EASING_TYPE_LIST` | `src/misc/helpers.h:4-25` | 21 | `enum animation_easing_type` with `value##_type` variants and the `EASING_TYPE_COUNT` sentinel (`:27-33`); `animation_easing_type_str[]` keyed `[value##_type] = #value` (`:35-40`); the easing `switch` (`src/window_manager.c:549-553`) |
| `WINDOW_PROPERTY_LIST` | `src/window.h:31-64` | 33 | `enum window_property` with explicit hex values (`:66-71`); `uint64_t window_property_val[]` (`:73-78`); `char *window_property_str[]` (`:80-85`) |
| `SPACE_PROPERTY_LIST` | `src/view.h:7-19` | 12 | `enum space_property` (`:21-26`); `space_property_val[]` (`:28-33`); `space_property_str[]` (`:35-40`) |
| `DISPLAY_PROPERTY_LIST` | `src/display.h:7-14` | 7 | `enum display_property` (`:16-21`); `display_property_val[]` (`:23-28`); `display_property_str[]` (`:30-35`) |
| `SUPPORTED_MACOS_VERSION_LIST` | `src/workspace.h:4-10` | 6 | one `static bool _workspace_is_macos_version_##name` and one `static inline bool workspace_is_macos_##name(void)` per entry (`:12-19`); the assignment block in `workspace_event_handler_begin` (`src/workspace.m:3-6`) |

### 4.1 The three property lists

They are the cleanest case: one list, three products, all in one module. `WINDOW_PROPERTY_LIST`
lives in `src/window.rs`, `SPACE_PROPERTY_LIST` in `src/view.rs`, `DISPLAY_PROPERTY_LIST` in
`src/display.rs`.

```rust
macro_rules! window_property_list {
    ($entry:ident) => {
        $entry!("id",                   WINDOW_PROPERTY_ID,                  0x000000001);
        $entry!("pid",                  WINDOW_PROPERTY_PID,                 0x000000002);
        $entry!("app",                  WINDOW_PROPERTY_APP,                 0x000000004);
        $entry!("title",                WINDOW_PROPERTY_TITLE,               0x000000008);
        $entry!("scratchpad",           WINDOW_PROPERTY_SCRATCHPAD,          0x000000010);
        $entry!("frame",                WINDOW_PROPERTY_FRAME,               0x000000020);
        $entry!("role",                 WINDOW_PROPERTY_ROLE,                0x000000040);
        $entry!("subrole",              WINDOW_PROPERTY_SUBROLE,             0x000000080);
        $entry!("root-window",          WINDOW_PROPERTY_ROOT_WINDOW,         0x000000100);
        $entry!("display",              WINDOW_PROPERTY_DISPLAY,             0x000000200);
        $entry!("space",                WINDOW_PROPERTY_SPACE,               0x000000400);
        $entry!("level",                WINDOW_PROPERTY_LEVEL,               0x000000800);
        $entry!("sub-level",            WINDOW_PROPERTY_SUB_LEVEL,           0x000001000);
        $entry!("layer",                WINDOW_PROPERTY_LAYER,               0x000002000);
        $entry!("sub-layer",            WINDOW_PROPERTY_SUB_LAYER,           0x000004000);
        $entry!("opacity",              WINDOW_PROPERTY_OPACITY,             0x000008000);
        $entry!("split-type",           WINDOW_PROPERTY_SPLIT_TYPE,          0x000010000);
        $entry!("split-child",          WINDOW_PROPERTY_SPLIT_CHILD,         0x000020000);
        $entry!("stack-index",          WINDOW_PROPERTY_STACK_INDEX,         0x000040000);
        $entry!("can-move",             WINDOW_PROPERTY_CAN_MOVE,            0x000080000);
        $entry!("can-resize",           WINDOW_PROPERTY_CAN_RESIZE,          0x000100000);
        $entry!("has-focus",            WINDOW_PROPERTY_HAS_FOCUS,           0x000200000);
        $entry!("has-shadow",           WINDOW_PROPERTY_HAS_SHADOW,          0x000400000);
        $entry!("has-parent-zoom",      WINDOW_PROPERTY_HAS_PARENT_ZOOM,     0x000800000);
        $entry!("has-fullscreen-zoom",  WINDOW_PROPERTY_HAS_FULLSCREEN_ZOOM, 0x001000000);
        $entry!("has-ax-reference",     WINDOW_PROPERTY_HAS_AX_REFERENCE,    0x002000000);
        $entry!("is-native-fullscreen", WINDOW_PROPERTY_IS_FULLSCREEN,       0x004000000);
        $entry!("is-visible",           WINDOW_PROPERTY_IS_VISIBLE,          0x008000000);
        $entry!("is-minimized",         WINDOW_PROPERTY_IS_MINIMIZED,        0x010000000);
        $entry!("is-hidden",            WINDOW_PROPERTY_IS_HIDDEN,           0x020000000);
        $entry!("is-floating",          WINDOW_PROPERTY_IS_FLOATING,         0x040000000);
        $entry!("is-sticky",            WINDOW_PROPERTY_IS_STICKY,           0x080000000);
        $entry!("is-grabbed",           WINDOW_PROPERTY_IS_GRABBED,          0x100000000);
    };
}

macro_rules! window_property_constant {
    ($name:literal, $identifier:ident, $value:literal) => {
        pub(crate) const $identifier: u64 = $value;
    };
}
window_property_list!(window_property_constant);

macro_rules! window_property_value_entry {
    ($name:literal, $identifier:ident, $value:literal) => { $value, };
}
pub(crate) static WINDOW_PROPERTY_VAL: [u64; 33] = [window_property_list!(window_property_value_entry)];

macro_rules! window_property_string_entry {
    ($name:literal, $identifier:ident, $value:literal) => { $name, };
}
pub(crate) static WINDOW_PROPERTY_STR: [&str; 33] = [window_property_list!(window_property_string_entry)];
```

The two array variants need the entries separated by `,` rather than `;`; write the list macro
with a trailing `,` per entry and let the statement-producing invocations absorb it. Whichever
separator a translator picks, **all three property lists use the same one.**

`WINDOW_PROPERTY_IS_GRABBED` is `0x100000000` — 33 bits — so the mask type is `u64` everywhere.
`SPACE_PROPERTY_*` (`0x001`..`0x800`) and `DISPLAY_PROPERTY_*` (`0x01`..`0x40`) fit in less, but
C declares both `_val[]` arrays `uint64_t`; keep `u64` so `parse_properties` has one signature.

### 4.2 `ANIMATION_EASING_TYPE_LIST`

Lives in `src/misc/helpers.rs`. Three products: the enum, the string table, and the dispatch.

```rust
macro_rules! animation_easing_type_list {
    ($entry:ident) => {
        $entry!(EaseInSine,      ease_in_sine);
        $entry!(EaseOutSine,     ease_out_sine);
        $entry!(EaseInOutSine,   ease_in_out_sine);
        $entry!(EaseInQuad,      ease_in_quad);
        $entry!(EaseOutQuad,     ease_out_quad);
        $entry!(EaseInOutQuad,   ease_in_out_quad);
        $entry!(EaseInCubic,     ease_in_cubic);
        $entry!(EaseOutCubic,    ease_out_cubic);
        $entry!(EaseInOutCubic,  ease_in_out_cubic);
        $entry!(EaseInQuart,     ease_in_quart);
        $entry!(EaseOutQuart,    ease_out_quart);
        $entry!(EaseInOutQuart,  ease_in_out_quart);
        $entry!(EaseInQuint,     ease_in_quint);
        $entry!(EaseOutQuint,    ease_out_quint);
        $entry!(EaseInOutQuint,  ease_in_out_quint);
        $entry!(EaseInExpo,      ease_in_expo);
        $entry!(EaseOutExpo,     ease_out_expo);
        $entry!(EaseInOutExpo,   ease_in_out_expo);
        $entry!(EaseInCirc,      ease_in_circ);
        $entry!(EaseOutCirc,     ease_out_circ);
        $entry!(EaseInOutCirc,   ease_in_out_circ);
    };
}
```

The second field is both the easing function's name and, stringified, the table entry: C writes
`[value##_type] = #value` (`src/misc/helpers.h:37`), so the strings are
`"ease_in_sine"` … `"ease_in_out_circ"`. Those strings are the values a user types for
`yabai -m config window_animation_easing` and are echoed back by `src/message.c:1318`, so they
are observable byte for byte.

`EASING_TYPE_COUNT` is not a variant: `pub(crate) const EASING_TYPE_COUNT: usize = 21;`, because
the C uses it only as a loop bound (`src/message.c:1321`) and an array length.

`g_window_manager.window_animation_easing` is a C `int` (`src/window_manager.h`), assigned the
loop index at `src/message.c:1322`. Keep it `AnimationEasingType` in Rust and convert the parse
loop to yield the variant; the dispatch at `src/window_manager.c:549-553` then becomes an
exhaustive `match`, which is what stops `mt` from being read uninitialised (§7.6).

### 4.3 `SUPPORTED_MACOS_VERSION_LIST`

Lives in `src/workspace.rs`. The six flags are written once in `workspace_event_handler_begin`
(`src/workspace.m:3-6`) on the main thread and read from several threads, so each becomes a
`static AtomicBool` (decision 18 covers the write-once statics; these are written once but the
read is cross-thread, so `AtomicBool` with `Ordering::Relaxed`, as the C `bool` read is).

```rust
macro_rules! supported_macos_version_list {
    ($entry:ident) => {
        $entry!(tahoe,    26);
        $entry!(sequoia,  15);
        $entry!(sonoma,   14);
        $entry!(ventura,  13);
        $entry!(monterey, 12);
        $entry!(bigsur,   11);
    };
}
```

generating, per entry, `static _workspace_is_macos_version_<name>: AtomicBool` and
`pub(crate) fn workspace_is_macos_<name>() -> bool`, plus the assignment block
`_workspace_is_macos_version_<name>.store(version.majorVersion == <major>, Relaxed)`.
`paste!`-style identifier concatenation is not available (no extra dependencies, decision 11), so
the entry macro takes the two identifiers spelled out:
`$entry!(tahoe, _workspace_is_macos_version_tahoe, workspace_is_macos_tahoe, 26);`.

### 4.4 `EVENT_TYPE_LIST`

Decision 19 replaces `enum event_type` + `void *context` + `int param1` with an `Event` enum
whose variants own their payloads, so the list macro carries three fields: the variant, the
handler function name, and the payload type.

```rust
macro_rules! event_type_list {
    ($entry:ident) => {
        $entry!(ApplicationLaunched,   event_handler_application_launched,   ProcessSerialNumber);
        $entry!(ApplicationTerminated, event_handler_application_terminated, ProcessSerialNumber);
        // ... all 40, in the C order of src/event_loop.h:7-46
    };
}
```

It generates `enum Event` and the dispatch `match` in `event_loop_run`. The payload column is
filled by the `event_loop` translator from `doc/rust-rewrite/files/entry-and-event-loop.md`; the
variant order is the C order and is not rearranged, because `Event`'s ordering is what makes the
Rust readable against `src/event_loop.c:1664-1668`.

There is no `event_type_str[]` in C and nothing serialises an event type, so no string table and
no explicit discriminants.

---

## 5. Enums, discriminants and string tables

### 5.1 Which enums carry explicit discriminants

`#[repr(...)]` plus explicit values wherever the value is observable, used as an array index, or
round-tripped through an integer field.

| Enum | C | Rust |
| --- | --- | --- |
| `enum signal_type` (`src/event_signal.h:4-45`) | positional, `SIGNAL_TYPE_UNKNOWN = 0`, terminated by `SIGNAL_TYPE_COUNT` | `#[repr(u32)]`, explicit `0..=29` on every variant; `pub(crate) const SIGNAL_TYPE_COUNT: usize = 30;` separately. Indexed at `src/event_signal.c:77`, `:338`, `:427`; iterated `0..SIGNAL_TYPE_COUNT` at `:345`; sizes `g_signal_event[SIGNAL_TYPE_COUNT]` (`src/yabai.c:27`) |
| `enum window_property` (`src/window.h:66-71`) | explicit hex, 33 bits | not an enum in Rust — `u64` consts, §4.1 |
| `enum space_property` (`src/view.h:21-26`) | explicit hex | `u64` consts |
| `enum display_property` (`src/display.h:16-21`) | explicit hex | `u64` consts |
| `enum window_flag` (`src/window.h:108-118`) | explicit hex, OR-ed | newtype, §6 |
| `enum window_rule_flag` (`src/window.h:120-126`) | explicit hex, OR-ed | newtype, §6 |
| `enum rule_flag` (`src/rule.h:8-20`) | explicit hex, OR-ed | newtype, §6 |
| `enum rule_effects_flag` (`src/rule.h:22-27`) | explicit hex, OR-ed | newtype, §6 |
| `enum view_flag` (`src/view.h:185-199`) | explicit hex, OR-ed | newtype, §6 |
| `enum mouse_mod` (`src/mouse_handler.h:34-42`) | explicit hex, OR-ed **and** used as an array index | newtype, §6; the index case is §5.3 |
| `enum window_node_split` (`src/view.h:122-128`) | positional 0..3 | `#[repr(u32)]` with explicit `0..=3` — it is stored in `view->auto_balance` (`uint32_t`, `src/view.h:214`) and indexes two tables |
| `enum window_node_child` (`src/view.h:108-113`) | positional 0..2 | `#[repr(i32)]` explicit — indexes `window_node_child_str` |
| `enum window_insertion_point` (`src/view.h:94-99`) | positional 0..2 | `#[repr(u32)]` explicit — indexes `window_insertion_point_str`, stored in `view->insertion_point` (`uint32_t`, `src/view.h:206`) |
| `enum view_type` (`src/view.h:169-175`) | positional 0..3 | `#[repr(i32)]` explicit — indexes `view_type_str` |
| `enum purify_mode` (`src/window_manager.h:26-31`) | positional 0..2 | `#[repr(i32)]` explicit — indexes `purify_mode_str` |
| `enum ffm_mode` (`src/window_manager.h:40-45`) | positional 0..2 | `#[repr(i32)]` explicit — indexes `ffm_mode_str` |
| `enum window_origin_mode` (`src/window_manager.h:54-59`) | positional 0..2 | `#[repr(i32)]` explicit — indexes `window_origin_mode_str` |
| `enum display_arrangement_order` (`src/display_manager.h:8-13`) | positional | `#[repr(usize)]` explicit — indexes `display_arrangement_order_str`, and is smuggled through a `void *` context at `src/display_manager.c:142` |
| `enum external_bar_mode` (`src/display_manager.h:22-27`) | positional | `#[repr(i32)]` explicit — indexes `external_bar_mode_str` |
| `enum mouse_mode` (`src/mouse_handler.h:44-51`) | positional 0..4 | `#[repr(u8)]` explicit — indexes `mouse_mode_str`, and lives in the atomic half of `g_mouse_state` (decision 23) |
| `enum mouse_drop_action` (`src/mouse_handler.h:23-32`) | positional | `#[repr(i32)]` explicit — stored in the atomic half of `g_mouse_state` |
| `enum mission_control_mode` (`src/mission_control.c:29-36`) | positional | `#[repr(i32)]` explicit — indexes `mission_control_mode_str` |
| `enum animation_easing_type` (`src/misc/helpers.h:27-33`) | positional 0..20 | `#[repr(usize)]` explicit — indexes `animation_easing_type_str` |
| `enum window_op_error` (`src/window_manager.h:8-24`) | positional | no explicit discriminants — never indexed, never serialised, only matched |
| `enum space_op_error` (`src/space_manager.h`) | positional | no explicit discriminants |
| `enum event_type` (`src/event_loop.h:48-53`) | positional | superseded by `Event` (§4.4); no discriminants |

Rule for anything not listed: **if the enum indexes an array, crosses a `void *`, or is stored in
an integer field, it gets `#[repr]` and explicit discriminants.** Otherwise it does not.

### 5.2 Positional string tables

`static FOO_STR: [&str; N] = [...]` with the C order, plus one accessor that keeps the indexing
visible:

```rust
pub(crate) static VIEW_TYPE_STR: [&str; 4] = ["default", "bsp", "stack", "float"];
```

and the call site stays an index, `VIEW_TYPE_STR[view.layout as usize]`, not a `Display` impl —
`animation_easing_type_str` is *also* iterated for parsing (`src/message.c:1320-1325`) and
`signal_type_str` is iterated by `signal_type_from_string` (`src/event_signal.c:343-350`), so the
tables must stay indexable *and* iterable.

| Table | C | contents |
| --- | --- | --- |
| `bool_str` | `src/misc/helpers.h:173` | `["off", "on"]`, indexed by a `bool` — write `BOOL_STR[value as usize]`, used at `src/message.c:1173,1184,1258,1269,1280` |
| `window_insertion_point_str` | `src/view.h:101-106` | `["focused", "first", "last"]` |
| `window_node_child_str` | `src/view.h:115-120` | `["none", "second_child", "first_child"]` |
| `window_node_split_str` | `src/view.h:130-136` | `["none", "vertical", "horizontal", "auto"]` |
| `auto_balance_str` | `src/view.h:138-143` | `["off", "vertical", "horizontal", "on"]` — a **second** table over the same `enum window_node_split` values, with different strings. Indexed by `view->auto_balance` at `src/message.c:1587`, `:1605` |
| `view_type_str` | `src/view.h:177-183` | `["default", "bsp", "stack", "float"]` |
| `purify_mode_str` | `src/window_manager.h:33-38` | `["on", "float", "off"]` against `PURIFY_DISABLED, PURIFY_MANAGED, PURIFY_ALWAYS` — **the string is inverted relative to the enum name.** `config window_shadow off` sets `PURIFY_ALWAYS` (`src/message.c:1335`) and reads back `"off"`. Do not correct it |
| `ffm_mode_str` | `src/window_manager.h:47-52` | |
| `window_origin_mode_str` | `src/window_manager.h:61-66` | |
| `display_arrangement_order_str` | `src/display_manager.h:15-20` | |
| `external_bar_mode_str` | `src/display_manager.h:29-34` | |
| `mouse_mode_str` | `src/mouse_handler.h:93-100` | dense over `enum mouse_mode` 0..4 despite the designated-initialiser syntax |

`auto_balance_str` and `window_node_split_str` cover the same four values. A translator who adds
a single `impl Display for WindowNodeSplit` and uses it in both places changes what
`yabai -m config auto_balance` prints. Keep two tables.

### 5.3 Sparse tables: the holes are load-bearing

Three tables are written with designated initialisers over **non-contiguous** indices. Each
becomes `[Option<&str>; N]` with the same `None` holes and the same index arithmetic. No
re-basing, no dense re-indexing, no `match` that silently covers a hole.

#### `layer_str` — `src/misc/helpers.h:175-181`

Indexed by `LAYER_AUTO = 0`, `LAYER_BELOW = kCGBackstopMenuLevelKey = 3`,
`LAYER_NORMAL = kCGNormalWindowLevelKey = 4`, `LAYER_ABOVE = kCGFloatingWindowLevelKey = 5`
(`src/misc/macros.h:42-45`; the three Core Graphics keys are consecutive members of
`_CGCommonWindowLevelKey`, verified in the SDK's `CGWindowLevel.h:23-28`). Length is
`max(key) + 1 = 6`, with `None` at 1 and 2.

```rust
pub(crate) const LAYER_AUTO:   i32 = 0;
pub(crate) const LAYER_BELOW:  i32 = 3;
pub(crate) const LAYER_NORMAL: i32 = 4;
pub(crate) const LAYER_ABOVE:  i32 = 5;

pub(crate) static LAYER_STR: [Option<&str>; 6] = [
    Some("auto"),
    None,
    None,
    Some("below"),
    Some("normal"),
    Some("above"),
];
```

Two lookup shapes, both of which must keep behaving as the C does:

* `src/window.c:115-117` indexes with a compile-time constant, so the entry is always `Some`:
  `LAYER_STR[LAYER_BELOW as usize].unwrap()`. Writing `.unwrap()` here is correct — the C would
  have read a valid entry too — and it is the only `unwrap` this document authorises.
* `src/rule.c:53` indexes with `rule->effects.layer`, an `int` field parsed from user input at
  `src/message.c` and only ever set to one of the four constants. C would read a `NULL` `char *`
  and hand it to `fprintf("%s")`, which glibc/Apple libc prints as `(null)` (decision 29 already
  fixes `(null)` as the spelling for a NULL `%s`). Rust:
  `LAYER_STR[rule.effects.layer as usize].unwrap_or("(null)")`, and an out-of-range index would
  panic where C read out of bounds — that is decision 4's bounds check and is a `DEVIATIONS.md`
  line (§12).

The three `kCG*` values are written as literals above, not resolved through the SDK, because
`src/ffi/` hand-declares Core Graphics (decision 11) and the values are ABI-stable.

#### `mouse_mod_str` — `src/mouse_handler.h:83-91`

Indexed by the **bit value**, not the ordinal: `MOUSE_MOD_NONE = 0x01` → index 1,
`ALT = 0x02` → 2, `SHIFT = 0x04` → 4, `CMD = 0x08` → 8, `CTRL = 0x10` → 16, `FN = 0x20` → 32.
The array is therefore 33 entries with `None` at 0, 3, 5, 6, 7, 9-15 and 17-31.

```rust
pub(crate) static MOUSE_MOD_STR: [Option<&str>; 33] = {
    let mut table = [None; 33];
    table[1]  = Some("none");
    table[2]  = Some("alt");
    table[4]  = Some("shift");
    table[8]  = Some("cmd");
    table[16] = Some("ctrl");
    table[32] = Some("fn");
    table
};
```

The one read is `src/message.c:1621`, `mouse_mod_str[g_mouse_state.modifier]`. `modifier` is only
ever assigned a single one of those six bits (`src/message.c:1622-1631`; the default is
`MOUSE_MOD_FN` at `src/mouse_handler.c:268`), so no hole is reachable today. Rust:
`MOUSE_MOD_STR[mouse_state.modifier() as usize].unwrap_or("(null)")`. A dense six-entry table
would print the wrong string the moment a seventh modifier is added, which is why the sparse
layout stays.

#### `mission_control_mode_str` — `src/mission_control.c:38-44`

Designated by `enum mission_control_mode`, which is contiguous, so the table is dense in
practice. Write it as `[Option<&str>; N]` anyway, with the same `Some` entries in the same
positions, so that the shape matches the other two and a future gap cannot be missed. Read from
`src/event_signal.c:338` — a cross-file read of a `.c`-file `static` (§2.5).

#### The AX notification tables

`ax_window_notification_str` / `ax_window_notification` (`src/window.h:17-29`) and
`ax_application_notification_str` / `ax_application_notification` (`src/application.h:46-66`) are
designated by `*_INDEX` constants that happen to be contiguous `0..N`. They are dense; write them
as `[&str; 3]` / `[CFStringRef; 3]` and `[&str; 7]` / `[CFStringRef; 7]`, indexed by the loop
variable of `src/window.c:9,23` and `src/application.c:46,66`. Note the **asymmetry**: there are
seven application notification entries but `AX_APPLICATION_ALL` covers only the first five (§6.4).

#### `ax_error_str` — `src/application.h:26-44`

Indexed by `-AXError`. `AXError` values are `0` and `-25200 … -25214`, so the C array is **not**
sparse: the designated indices are `0` through `15`, contiguous, because the `kAXError*`
enumerators are consecutive. Write `[&str; 16]` and index `AX_ERROR_STR[(-result) as usize]`
(`src/window.c:14`, `src/application.c:52`). The negation is part of the idiom; do not add an
`abs()` or a `match`.

#### `signal_type_str` — `src/event_signal.h:47-88`

Designated by `enum signal_type`, contiguous `0..=29`, **plus a 31st entry** `[SIGNAL_TYPE_COUNT] = "signal_type_count"` (`src/event_signal.h:88`). Dense `[&str; 31]`; the last entry is never matched because `signal_type_from_string` stops at `SIGNAL_TYPE_COUNT` (exclusive, `src/event_signal.c:345`), but it is reachable through `signal_type_str[type]` and must be present. It is both indexed
(`src/event_signal.c:77`, `:338`, `:427`) and iterated (`:345`), so it stays an array.

### 5.4 `DIR_*` and `STACK` are integers, not an enum

`DIR_NORTH = 360`, `DIR_EAST = 90`, `DIR_SOUTH = 180`, `DIR_WEST = 270`, `STACK = 111`
(`src/misc/macros.h:26-31`) are plain `i32` constants. They are stored in
`window_node::insert_dir` (`int`, `src/view.h:165`) and in local `int direction` parameters, and
they arrive from `src/message.c` parsing, so a value outside the five is representable.

They stay `pub(crate) const … : i32` and every lookup is a `match` on `i32` with a `_` arm that
does exactly what the C `switch` default or the trailing `if` chain does:

* `src/view.c:54-84` — `switch (dir)` with cases `DIR_NORTH/EAST/SOUTH/WEST/STACK` and **no**
  `default`. Rust: `match direction { DIR_NORTH => …, …, STACK => …, _ => {} }`. The empty `_`
  arm is the C's fall-through, not a new behaviour.
* `src/view.c:543-556` and `:566-578` — `if` chains and a `switch`; transcribe operator for
  operator. `area_is_in_direction` mixes strict and non-strict comparisons deliberately
  (`<=` on the four early-outs, `<`/`>` on the overlap tests); they are not symmetric.
* `src/window_manager.c:1778-1789` — an `if`/`else if` chain with no final `else`. Rust keeps the
  chain rather than becoming a `match`, so the "none of the four" path stays a no-op.
* `src/event_loop.c:1305-1320` — assigns one of the five to `insert_dir`.

A `_ => unreachable!()` arm anywhere in this family is wrong.

---

## 6. Flag sets

Decision 31: flag sets are newtypes with associated constants, and decision 11 rules out any
dependency that would provide them.
No Rust `enum`: every one of these is OR-ed, and `enum` with an OR-ed value is undefined.

### 6.1 The newtypes

Each is a newtype over **the width of the C field that stores it**, not the width of the C enum.

| C enum / macro family | C values | stored in | Rust |
| --- | --- | --- | --- |
| `enum window_flag` (`src/window.h:108-118`) | `SHADOW 0x01`, `FULLSCREEN 0x02`, `MINIMIZE 0x04`, `FLOAT 0x08`, `STICKY 0x10`, `WINDOWED 0x20`, `MOVABLE 0x40`, `RESIZABLE 0x80` | `struct window::flags`, `uint8_t` (`src/window.h:102`) | `WindowFlag(pub u8)` |
| `enum window_rule_flag` (`src/window.h:120-126`) | `MANAGED 0x01`, `FULLSCREEN 0x02`, `MFF 0x04`, `MFF_VALUE 0x08` | `struct window::rule_flags`, `uint8_t` (`:101`) | `WindowRuleFlag(pub u8)` |
| `enum rule_flag` (`src/rule.h:8-20`) | `APP_VALID 0x001` … `ONE_SHOT_REMOVE 0x200` | `struct rule::flags`, `uint16_t` (`src/rule.h:56`) | `RuleFlag(pub u16)` |
| `enum rule_effects_flag` (`src/rule.h:22-27`) | `FOLLOW_SPACE 0x01`, `OPACITY 0x02`, `LAYER 0x04` | `struct rule_effects::flags`, `uint16_t` (`src/rule.h:41`) | `RuleEffectsFlag(pub u16)` |
| `enum view_flag` (`src/view.h:185-199`) | `LAYOUT 0x001` … `SPLIT_TYPE 0x800` | `struct view::flags`, `uint64_t` (`src/view.h:215`) | `ViewFlag(pub u64)` |
| `enum mouse_mod` (`src/mouse_handler.h:34-42`) | `NONE 0x01`, `ALT 0x02`, `SHIFT 0x04`, `CMD 0x08`, `CTRL 0x10`, `FN 0x20` | `struct mouse_state::modifier`, `volatile uint8_t` (`src/mouse_handler.h:71`) | `MouseMod(pub u8)`; the field itself is an `AtomicU8` in the event-tap half of decision 23 |
| `AX_WINDOW_*` (`src/window.h:10-15`) | `DESTROYED 1<<2`, `MINIMIZED 1<<0`, `DEMINIMIZED 1<<1` | `struct window::notification`, `uint8_t` (`src/window.h:100`) | `AxWindowNotification(pub u8)` |
| `AX_APPLICATION_*` (`src/application.h:15-24`) | `WINDOW_CREATED 1<<0` … `WINDOW_TITLE_CHANGED 1<<4` | `struct application::notification`, `uint8_t` (`src/application.h:76`) | `AxApplicationNotification(pub u8)` |
| `HANDLE_*` (`src/misc/macros.h:36-40`) | `TOP 0x01`, `BOTTOM 0x02`, `LEFT 0x04`, `RIGHT 0x08`, `ABS 0x10` | `struct mouse_state::direction`, `uint8_t` (`src/mouse_handler.h:79`); and `int direction` parameters | `ResizeHandle(pub u8)` |
| `OSAX_ATTRIB_*` (`src/osax/common.h:9-15`) | `DOCK_SPACES 0x01` … `ANIM_TIME 0x40` | the handshake reply word, `uint32_t` (`src/sa.m:272`) | `OsaxAttrib(pub u32)`, generated into `OUT_DIR/osax_common.rs` by `build.rs` (decision 9) |

Shape, once, so every module writes it the same way:

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowFlag(pub u8);

impl WindowFlag {
    pub(crate) const SHADOW:     WindowFlag = WindowFlag(0x01);
    pub(crate) const FULLSCREEN: WindowFlag = WindowFlag(0x02);
    pub(crate) const MINIMIZE:   WindowFlag = WindowFlag(0x04);
    pub(crate) const FLOAT:      WindowFlag = WindowFlag(0x08);
    pub(crate) const STICKY:     WindowFlag = WindowFlag(0x10);
    pub(crate) const WINDOWED:   WindowFlag = WindowFlag(0x20);
    pub(crate) const MOVABLE:    WindowFlag = WindowFlag(0x40);
    pub(crate) const RESIZABLE:  WindowFlag = WindowFlag(0x80);
}
```

Constants drop the type prefix and keep the rest of the C name, uppercase: `WINDOW_FLOAT` →
`WindowFlag::FLOAT`, `RULE_ONE_SHOT_REMOVE` → `RuleFlag::ONE_SHOT_REMOVE`,
`VIEW_ENABLE_PADDING` → `ViewFlag::ENABLE_PADDING`, `MOUSE_MOD_FN` → `MouseMod::FN`,
`AX_WINDOW_DEMINIMIZED` → `AxWindowNotification::DEMINIMIZED`, `HANDLE_TOP` →
`ResizeHandle::TOP`.

`AX_WINDOW_*_INDEX` and `AX_APPLICATION_*_INDEX` stay separate `usize` constants (they index the
notification tables of §5.3, and the C derives the bit from the index with `1 << i`).

### 6.2 The accessor trio

C generates three `static inline`s per family (`src/window.h:128-134`, `src/rule.h:59-65`) and
three *macros* for `view` (`src/view.h:218-220`). 189 call sites across eight files.

Both the free-function and the method form exist for every family, so a borrow-checker-hostile
call site never forces a translator to invent one:

```rust
pub(crate) fn window_check_flag(window: &Window, flag: WindowFlag) -> bool { window.flags & flag.0 != 0 }
pub(crate) fn window_clear_flag(window: &mut Window, flag: WindowFlag) { window.flags &= !flag.0; }
pub(crate) fn window_set_flag(window: &mut Window, flag: WindowFlag) { window.flags |= flag.0; }
```

**Prefer the free function** for one-to-one transcription; the method is
`window.check_flag(WindowFlag::FLOAT)` with the identical body.

`view_check_flag` is the one that differs: the C macro `((v)->flags & (x))` yields the **masked
value**, not a `bool`. All 25 uses (`src/view.c` 12, `src/space_manager.c` 10,
`src/window_manager.c` 3) are in `if` or `?:` position, so `-> bool` is behaviour-preserving.
Write it `-> bool` and do not special-case any site.

### 6.3 Whole-set equality: `(a & MASK) == MASK`

Three sites, all "did every one of these bits get set". They are **not** single-bit tests and
must not become `.contains()` or `!= 0`:

| site | C | Rust |
| --- | --- | --- |
| `src/window.c:18` | `(window->notification & AX_WINDOW_ALL) == AX_WINDOW_ALL` | `window.notification & AX_WINDOW_ALL.0 == AX_WINDOW_ALL.0` |
| `src/application.c:60` | `(application->notification & AX_APPLICATION_ALL) == AX_APPLICATION_ALL` | `application.notification & AX_APPLICATION_ALL.0 == AX_APPLICATION_ALL.0` |
| `src/sa.m:282` | `(attrib & OSAX_ATTRIB_ALL) == OSAX_ATTRIB_ALL` | `attrib & OSAX_ATTRIB_ALL.0 == OSAX_ATTRIB_ALL.0` |

Two more multi-bit sites that are not equality but are still not single-bit tests:

* `parse_resize_handle` (`src/message.c:483-507`) returns composites such as
  `HANDLE_TOP | HANDLE_LEFT`. Return `ResizeHandle(ResizeHandle::TOP.0 | ResizeHandle::LEFT.0)`.
* `mouse_mod_from_cgflags` (`src/mouse_handler.c:5-17`) accumulates with `|=` and is read back
  as a whole value.

And one whole-**value** equality that is easy to mistake for a mask test:
`src/window_manager.c:374` and `:401`, `if (direction == HANDLE_ABS)`. That is `== 0x10`
exactly, not "has the ABS bit". Write `if direction == ResizeHandle::ABS.0`.

### 6.4 The masks that are not "all"

`AX_APPLICATION_ALL` (`src/application.h:20-24`) is `0x1F` — the **first five** bits. The
notification field carries **seven** (`AX_APPLICATION_WINDOW_MENU_OPENED_INDEX = 5` and
`_MENU_CLOSED_INDEX = 6`, `src/application.h:12-13`), and `application_observe`
(`src/application.c:46-54`) subscribes to all seven, setting bits 0-6. So
`application_observe` returns `true` when the five *window* notifications registered, even if the
two *menu* notifications failed. Write the constant as `0x1F` and never as
`(1 << ax_application_notification.len()) - 1`, and never derive it from the table length.

`AX_WINDOW_ALL` (`src/window.h:13-15`) is `0x07` and does cover every defined `AX_WINDOW_*` bit —
but it is still written out as the OR of the three named constants, for the same reason.

`OSAX_ATTRIB_ALL` (`src/osax/common.h:17-23`) is `0x7F` over seven defined attributes; it is
generated from `src/osax/common.h`, so it tracks that header automatically and the daemon must
not re-derive it.

### 6.5 `flags |= ~flags` — "no filter means every property"

Four sites, all the same line, verified:

| site | C |
| --- | --- |
| `src/window.c:125` (`window_nonax_serialize`) | `if (flags == 0x0) flags \|= ~flags;` |
| `src/window.c:413` (`window_serialize`) | `if (flags == 0x0) flags \|= ~flags;` |
| `src/display.c:24` (`display_serialize`) | `if (flags == 0x0) flags \|= ~flags;` |
| `src/view.c:864` (`view_serialize`) | `if (flags == 0x0) flags \|= ~flags;` |

`flags` is `uint64_t`. When it is zero, `~flags` is `u64::MAX`, so **every** bit is set — not
just the 33 / 7 / 12 defined property bits. Rust, verbatim:

```rust
if flags == 0x0 { flags |= !flags; }
```

Not `flags = WINDOW_PROPERTY_ALL`, not `flags = u64::MAX`, not a `match`. The C shape is what
keeps working when a property bit is added.

### 6.6 Property masks stay `u64`, not newtypes

`WINDOW_PROPERTY_*` / `SPACE_PROPERTY_*` / `DISPLAY_PROPERTY_*` are tested with
`if (flags & WINDOW_PROPERTY_ID)` against a plain `uint64_t flags` parameter, and the values are
looked up in `*_property_val[]` by index during parsing (`src/message.c:2425`, `:2481`, `:2538`).
They are `u64` constants and `u64` parameters, with `if flags & WINDOW_PROPERTY_ID != 0`. A
newtype here would have to be unwrapped at every `parse_properties` call and gains nothing.

---

## 7. The numeric conversion rulebook

Decision 30: **every implicit C conversion becomes an explicit `as` cast at the same point in the
expression.** Not at the end of the expression, not at the assignment — at the point where C's
usual arithmetic conversions did it. `struct area` stays `f32` (decision 30); `CGRect`,
`CGPoint`, `CGFloat` and `EventTime` are `f64`.

Decision 8 turns `overflow-checks` off in every profile, and arithmetic the C relied on wrapping
is written with `wrapping_*` anyway, so debug and release agree with C.

### 7.1 The procedure, for a shape not listed below

1. Write the C expression out with every implicit conversion made explicit, applying C's usual
   arithmetic conversions: integer promotions first, then the common real type (`float` only
   widens to `double` when a `double` operand is present — `float * float` stays `float`).
2. Transcribe that, `as` for `as`.
3. If the result is an integer and the source is a float, read §7.2 before writing `as`.

### 7.2 Float to integer: truncation toward zero, and the one divergence

C's `(int)` on a float truncates toward zero and is **undefined** out of range. Rust's `as`
truncates toward zero and **saturates** out of range. For every in-range value the two agree, so
`as i32` / `as u32` is the correct transcription, and the saturation is a `DEVIATIONS.md` line
for each site where the range is not statically obvious (§12).

Every float-to-integer site in the daemon:

| site | C | Rust |
| --- | --- | --- |
| `src/view.c:170-172` | `left_area->w = (int)left_width;` | `left_area.width = left_width as i32 as f32;` |
| `src/view.c:173` | `right_area->x += (int)(left_width + 0.5f) + gap;` | `right_area.x += ((left_width + 0.5f32) as i32 + gap) as f32;` |
| `src/view.c:180-182` | the `SPLIT_X` mirror of the above | same, on `height` / `y` |
| `src/view.c:566-578` | `return r2_max.y - r1->y;` from `CGFloat` into an `int` return | `(second_area_max_point.y - first_area.y) as i32` |
| `src/window_manager.c:1147`, `:1166` | `uint32_t area = node->area.w * node->area.h;` | `let area = (node.area.width * node.area.height) as u32;` — an `f32` product truncated to `u32`; C wraps modulo 2^32 on overflow, Rust saturates to `u32::MAX`. DEVIATIONS line |
| `src/window_manager.c:1896` | `CGPoint ca = { (int)(0.5f + a_node->area.x + a_node->area.w / 2.0f), … }` | `let center_a = CGPoint { x: ((0.5f32 + a_node.area.x + a_node.area.width / 2.0f32) as i32) as f64, y: … };` |
| `src/window_manager.c:1897-1898` | `powf((ca.x - (int)(0.5f + cf.x + cf.w / 2.0f)), 2.0f)` | the subtraction happens in `f64` (`ca.x` is `CGFloat`), then narrows at the `powf` call: `powf((center_a.x - ((0.5f32 + first.x + first.width / 2.0f32) as i32) as f64) as f32, 2.0f32)` |
| `src/event_loop.c:1269-1270` | `int dx = point.x - g_mouse_state.down_location.x;` | `let delta_x = (point.x - mouse_state.down_location.x) as i32;` — `-0.9` truncates to `0`, not `-1` |
| `src/workspace.m:132` | `return screen.safeAreaInsets.top;` from `CGFloat` into an `int` return | `inset.top as i32`; a notch inset of `36.9` is `36` |
| `src/misc/helpers.h:590-591` | `int width = CGImageGetWidth(image);` (`size_t` → `int`) | `let width = CGImageGetWidth(image) as i32;` |
| `src/misc/helpers.h:592-593` | `int pitch = width * 4;` then `calloc(height * pitch, 1)` | `let pitch = width * 4;` with the `i32` product, then `as usize` at the allocation — decision 8 means it wraps rather than panicking, matching C's optimised build |

`src/view.c:173`'s `+ 0.5f` **before** the truncation is a round-half-up, and the sibling
`left_area->w` on line 170 has **no** `+ 0.5f`. That asymmetry is what makes the two halves of a
split add up; `.round()` on both, or `f32` throughout, moves windows by a pixel.

### 7.3 `struct area` (`f32`) against `CGRect` (`f64`)

`struct area { float x, y, w, h; }` (`src/view.h:42-49`); `CGRect` is four `CGFloat` = `f64`.
Both directions are explicit:

```rust
pub(crate) fn area_from_cgrect(rect: CGRect) -> Area {
    Area {
        x: rect.origin.x as f32,
        y: rect.origin.y as f32,
        width: rect.size.width as f32,
        height: rect.size.height as f32,
    }
}
```

and the inverse, at `src/view.c:11` and everywhere a node area is handed to an API:

```rust
CGRect { origin: CGPoint { x: node.area.x as f64, y: node.area.y as f64 },
         size: CGSize { width: node.area.width as f64, height: node.area.height as f64 } }
```

`area_max_point` (`src/view.c:126-129`) computes in `f32` and returns a `CGPoint`:

```rust
pub(crate) fn area_max_point(area: Area) -> CGPoint {
    CGPoint { x: (area.x + area.width - 1.0f32) as f64, y: (area.y + area.height - 1.0f32) as f64 }
}
```

The `-1` is an inclusive-edge adjustment, not an off-by-one to fix.

A translator who makes `Area` `f64` changes window placement by up to a pixel everywhere. `Area`
is `f32`.

### 7.4 `lerp` — a `double` literal in a `float` expression

`#define lerp(a, t, b) (((1.0-t)*a) + (t*b))` (`src/misc/macros.h:16`). `1.0` is a **double**, so
`1.0 - t` is `double` and `(1.0-t)*a` is `double`; `t*b` is `float * float` = `float`; the sum is
`double`; the assignment to `proxy.tx` (`float`, `src/view.h:61`) narrows.

The only call sites are `src/window_manager.c:560-563`, where `a` is a `CGRect` component
(`f64`) and `b` is a `struct window_animation` component (`f32`).

```rust
pub(crate) fn lerp(start: f64, interpolant: f32, end: f32) -> f64 {
    ((1.0f64 - interpolant as f64) * start) + (interpolant * end) as f64
}
```

and the call site narrows:
`animation.proxy.target_x = lerp(animation.proxy.frame.origin.x, eased_interpolant, animation.x) as f32;`

Computing the whole thing in `f32`, or the whole thing in `f64`, both differ from C.

### 7.5 `AX_DIFF` — the 1.5 pixel debounce

`AX_ABS(a,b)` is `((a)-(b) < 0) ? ((a)-(b)) * -1 : (a)-(b)` and `AX_DIFF(a,b)` is
`AX_ABS(a,b) >= 1.5f` (`src/view.h:4-5`). Used 12 times in `src/event_loop.c` (four at `:709-713`, eight at `:808-816`) with one `struct area`
`float` argument and one `CGFloat` argument, so the
subtraction happens in `double` and `1.5f` promotes to `double`.

```rust
pub(crate) fn ax_diff(first: f64, second: f64) -> bool {
    let difference = first - second;
    let absolute = if difference < 0.0f64 { difference * -1.0f64 } else { difference };
    absolute >= 1.5f32 as f64
}
```

Call sites cast the `f32` operand: `ax_diff(new_origin.x, node.area.x as f64)`. The `1.5f32 as
f64` spelling is deliberate — `1.5` is exactly representable, so it equals `1.5f64`, but writing
the promotion keeps the rule mechanical for the next threshold that is not exact.

### 7.6 The animation clock and the easing call

`src/window_manager.c:544-554`:

```c
double t = (double)(current_clock - context->animation_clock) / (double)(context->animation_duration * g_cv_host_clock_frequency);
if (t <= 0.0) t = 0.0f;
if (t >= 1.0) t = 1.0f;

float mt;
switch (context->animation_easing) {
```

* `current_clock - animation_clock` is `u64 - u64`, wrapping: `.wrapping_sub(...)`, then `as f64`.
* `animation_duration` is `float`, `g_cv_host_clock_frequency` is `double`
  (`src/window_manager.c:7`), so the product is `double` before the outer `(double)` cast:
  `(animation_duration as f64 * g_cv_host_clock_frequency)`.
* `t = 0.0f` / `t = 1.0f` assign a `float` literal to a `double`; both are exact, write
  `interpolant = 0.0f64` / `1.0f64`.
* `mt = ease_in_sine(t)` narrows `double` to `float` **at the call**, because every easing
  function takes `float` (`src/misc/helpers.h:42-145`):
  `let eased_interpolant = ease_in_sine(interpolant as f32);`
* `mt` is **uninitialised** if the C `switch` matches nothing. `ANIMATION_EASING_TYPE_LIST`
  covers every value so it cannot happen, but Rust must give it a definite value: an exhaustive
  `match` on `AnimationEasingType`, with no `_` arm. That is a `DEVIATIONS.md` line (§12).

Inside the easing functions, three of them promote to `double` because `M_PI` is a `double`
(`src/misc/helpers.h:44`, `:49`, `:54`):

```rust
pub(crate) fn ease_in_sine(t: f32) -> f32 {
    1.0f32 - ((t as f64 * std::f64::consts::PI) / 2.0f64) as f32
}
```

— `cosf` takes a `float`, so the `double` argument narrows at the call; the surrounding
arithmetic is `f32`. Call `f32::cos` on the narrowed value, not `f64::cos`. The other eighteen
easing functions are `f32` throughout and need no cast.

### 7.7 The `f32` timer deltas

Three sites, all the same shape, all observable thresholds (decision 3):

| site | C | threshold |
| --- | --- | --- |
| `src/event_loop.c:363` | `float dt = ((float) read_os_timer() - last_cmd_tab_time) * (1000.0f / (float)read_os_freq());` | `dt > 1500.0f` (`:364`) |
| `src/event_loop.c:1266` | `float dt = ((float) event_time - g_mouse_state.last_moved_time) * (1000.0f / (float)read_os_freq());` | `dt < 67.67f` (`:1267`) |
| `src/event_loop.c:1353` | `float dt = ((float) read_os_timer() - last_gesture_time) * (1000.0f / (float)read_os_freq());` | `dt < 1250.0f` (`:1354`) |

Both operands of the subtraction are `uint64_t`; the explicit `(float)` on the left forces the
right one to be converted to `float` as well, so the subtraction happens in **`f32`**, losing
precision above 2^24 nanoseconds (~16.7 ms of timer range per ulp at the magnitudes involved).
That precision loss is part of the behaviour.

```rust
let delta_time = (read_os_timer() as f32 - last_cmd_tab_time as f32) * (1000.0f32 / read_os_freq() as f32);
```

Not `(read_os_timer() - last_cmd_tab_time) as f32`, which subtracts in `u64` and gives a
different answer. Not `f64` anywhere.

`read_os_freq()` returns the constant `1000000000` (`src/misc/helpers.h:157-160`); it is still
called and still cast, because constant-folding it changes nothing and inlining it by hand
diverges from the C if the function ever changes.

### 7.8 `0.05f` against a `double`

`src/event_signal.c:156-157`:

```c
EventTime dt = GetCurrentEventTime() - g_process_manager.switch_event_time;
if (dt >= 0.05f) {
```

`EventTime` is a Carbon `double`. `0.05f` is a `float` literal, promoted to `double` for the
comparison — and `0.05f` as a `double` is **not** `0.05`: it is the `double` nearest to the
`float` nearest to 0.05. Write the promotion:

```rust
let delta_time = get_current_event_time() - process_manager.switch_event_time;
if delta_time >= 0.05f32 as f64 {
```

`0.05f64` is a different number and would move the threshold that the comment at
`src/event_signal.c:148-154` describes.

### 7.9 Unsigned wrap in the grid arithmetic

`window_manager_apply_grid` (`src/window_manager.c:2124-2179`) takes six `unsigned`
parameters straight from `sscanf("%d:%d:%d:%d:%d:%d")` at `src/message.c:2220`, so
`yabai -m window --grid 0:0:0:0:0:0` reaches it with `rows == 0` and `columns == 0`:

```c
if (x >=   c) x = c - 1;
if (y >=   r) y = r - 1;
if (w <=   0) w = 1;
if (h <=   0) h = 1;
if (w >  c-x) w = c - x;
if (h >  r-y) h = r - y;
```

* `c - 1` with `c == 0` wraps to `0xFFFFFFFF`. Unsigned wrap is **defined** in C, so this is not
  undefined behaviour and is reproduced: `x = columns.wrapping_sub(1);`
* `c-x` then wraps back to `1`. `columns.wrapping_sub(x)`, twice (`:2138`) and once more at
  `:2154` in `if (c > x+w)` — write `columns > x.wrapping_add(width)`.
* `w <= 0` on an `unsigned` is `w == 0`. Keep it spelled `if width == 0 { width = 1; }`; writing
  `if width <= 0` does not compile on `u32` and writing `if (width as i32) <= 0` changes it.
* `float cw = bounds.size.width / c;` with `c == 0` is a `f64 / f64` division by zero, which is
  `inf` in both languages, then narrowed to `f32`:
  `let column_width = (bounds.size.width / columns as f64) as f32;`

The window ends up off screen. That is what C does today and decision 3 keeps it.

### 7.10 `u64` truncated to `u32`

| site | C | Rust |
| --- | --- | --- |
| `src/event_loop.c:946` | `uint32_t wid = (uint64_t)(intptr_t) context;` | `let window_id = (context as usize as u64) as u32;` |
| `src/event_loop.c:954` | same | same |
| `src/window_manager.c:2300` | `uint32_t sid = window_space(window->id);` — a 64-bit space id into a `u32` | `let space_id = window_space(window.id) as u32;`, and the comparison at `:2309` becomes `space_id != space_manager_active_space() as u32` |

The first two are no-ops today (producers only ever post a `u32`), but the cast is written so
the Rust says what the C says. The third is a real truncation: `src/window_manager.c:2309` spins
`while (sid != space_manager_active_space())` comparing a truncated `u32` against a full `u64`,
which C also truncates for the comparison. Reproduce the truncation on **both** sides, as C does.

`src/event_loop.c:1134`, `uint8_t mod = (uint8_t) param1;` — an `int` event parameter truncated
to a byte: `let event_modifier = param1 as u8;`.

### 7.11 Integer accumulation that wraps

| site | C | Rust |
| --- | --- | --- |
| `src/message.c:352` | `*value = *value * 10 + token_char_int_table[(int)c];` on `int *value` | `*value = value.wrapping_mul(10).wrapping_add(TOKEN_CHAR_INT_TABLE[c as usize]);` |
| `src/message.c:377` | `*value = *value * 16 + (uint32_t)token_char_int_table[(int)c];` on `uint32_t *value` | `*value = value.wrapping_mul(16).wrapping_add(TOKEN_CHAR_INT_TABLE[c as usize] as u32);` |

Signed overflow is undefined in C but wraps in the optimised build, and `yabai -m window --focus
99999999999` reaches it. `wrapping_*` makes debug and release agree with the C binary users have.

`token_char_int_table` is `static const int[]` with designated character initialisers
(`src/message.c:283-…`); its implicit length is `'f' + 1`, so it stays an `[i32; 103]` indexed by the byte, holes left at `0`.
`c` is a C `char` — **signed** on both targets — and the C indexes with `(int)c`, so a byte above
`0x7f` indexes negatively and reads out of bounds. Both loops reject such a byte before the
index (`src/message.c:349`, `:371-375`), so the site is unreachable; index with `c as usize`
after the range check and record the difference (§12).

### 7.12 `int` to `float`, and `float` inside `double` expressions

| site | C | Rust |
| --- | --- | --- |
| `src/view.c:167-168` | `(parent_area->w - gap) * ratio` — `gap` is `int`, promoted to `float` | `(parent_area.width - gap as f32) * ratio` |
| `src/view.c:168` | `(1 - ratio)` — `1` is `int`, promoted to `float` | `(1.0f32 - ratio)` |
| `src/window_manager.c:353-354` | `float fw = max(1, frame.size.width + dx * x_mod);` — `dx * x_mod` is `float`, the sum is `double`, `1` promotes to `double`, the result narrows to `float` | `let frame_width = max(1.0f64, frame.size.width + (delta_x * x_modifier as f32) as f64) as f32;` |
| `src/misc/hashtable.h:132` | `float load = (1.0f * table->count) / table->capacity;` | `let load = (1.0f32 * count as f32) / capacity as f32;` |
| `src/misc/helpers.h:542-548` | `radius * 2 > CGRectGetWidth(frame)` — `float` against `CGFloat` | `(radius * 2.0f32) as f64 > frame.size.width`, then `radius = (frame.size.width / 2.0f64) as f32` |
| `src/misc/helpers.h:566-568` | `float l1 = (p.x - t[0].x) * (t[2].y - t[0].y) - …` — all `CGFloat`, narrowed on assignment | `let l1 = ((point.x - triangle[0].x) * (triangle[2].y - triangle[0].y) - (triangle[2].x - triangle[0].x) * (point.y - triangle[0].y)) as f32;` |
| `src/rule.c:60` | `(uint32_t)(rule->effects.flags << 16) \| (uint32_t)rule->flags` — `uint16_t` promotes to `int` before the shift, so `flags << 16` can set the sign bit | `((effects.flags as u32) << 16) \| (rule.flags as u32)` — same bits, no signed shift |
| `src/display_manager.c:150-153` | `float a_coord = a_center.y;` from a `CGPoint` | `let a_coord = a_center.y as f32;` — the comparator's two-key compare happens in `f32`, and narrowing is what makes near-equal display centres compare equal |
| `src/window.c:14`, `src/application.c:52` | `ax_error_str[-result]` on a negative `AXError` | `AX_ERROR_STR[(-result) as usize]` |
| `src/misc/hashtable.h:78` | `table->hash(key) % table->capacity` — `unsigned long % int` converts the `int` | `(hash % capacity as u64) as usize` |

`clampf_range` (`src/misc/helpers.h:581-586`) and `cgrect_contains_point`
(`src/misc/helpers.h:558-562`) are single-type and need no cast: `f32` and `f64` respectively.

### 7.13 The SIMD kernel

`cgimage_restore_alpha` (`src/misc/helpers.h:588-672`) is transcribed instruction for
instruction with `std::arch` SSE2 and NEON intrinsics (decision 36). The conversions inside are
lane conversions, not C conversions, and `_mm_cvtps_epi32` (round-to-nearest-even) versus
`vcvtnq_s32_f32` (round-to-nearest) is an **existing** per-architecture difference in the C that
is reproduced, not unified.

The loop bound is `for (int i = 0; i < height*width; i += 4)` with `pixel += 4` per iteration, so
a buffer whose pixel count is not a multiple of four reads and writes up to three pixels past the
end. Decision 4 forbids reproducing that: the 1-to-3 pixel tail goes through the same routine on
a padded scratch copy, and the difference is a `DEVIATIONS.md` line (§12).

---

## 8. Sentinels, `bool`, `Option` and `Result`

Decision 32: C `bool` returns stay `bool`; nullable pointers become `Option`; observable
sentinels stay; `Result` appears only at `io::Write`.

### 8.1 Sentinels that stay, exactly as they are

| C | meaning | Rust | sentinel |
| --- | --- | --- | --- |
| `uint32_t wid` | window id | `u32` | `0` means none (`src/event_loop.c:646`, `src/misc/helpers.h:499-504`) |
| `uint32_t did` | display id | `u32` | `0` means none (`src/display_manager.c:165-197`) |
| `uint64_t sid` | space id | `u64` | `0` means none (`src/space.c:105`, `src/window.c:83`) |
| `pid_t pid` | process id | `i32` | `0` means none (`src/process_manager.c:26-29`) |
| `int` mission-control index | 1-based space index | `i32` | `0` means not found (`src/view.c:885`) |
| `int` display arrangement | 1-based | `i32` | `0` means not found (`src/display_manager.c:165-197`, the `int result = 0` that survives every `goto`) |
| `int area_distance_in_direction` | pixels | `i32` | `i32::MAX` means "no candidate" (`src/view.c:580`), seeded into `best_distance`/`best_rank` at `src/view.c:589-590` and `src/display_manager.c:273` |
| `int window_manager_find_rank_of_window_in_list` | 0-based rank | `i32` | `i32::MAX` means not in the list (`src/window_manager.c:908`) |
| `uint32_t best_area` | `window_manager_find_smallest_managed_window` seed | `u32` | `u32::MAX` (`src/window_manager.c:1163`) |

**Do not turn a `0`-means-none id into `Option<NonZeroU32>`.** The ids are passed to SkyLight,
compared against each other, stored in struct fields and printed into the query JSON; the
sentinel reaches the outside world. Converting at every boundary would add a conversion at 400
call sites and would still have to un-convert to print.

`INT_MAX` is `i32::MAX`, not `Option::None`. `src/view.c:589-590` seeds two accumulators with it
and compares `<`, so any `Option`-based rewrite changes which node wins a tie.

### 8.2 Tri-states stay three-valued

Three families, all of which serialise their middle value:

| C | values | Rust |
| --- | --- | --- |
| `REGEX_MATCH_UD/YES/NO` (`src/misc/macros.h:22-24`) | `0/1/2` | `enum RegexMatch { Undefined, Yes, No }` in `src/misc/regex.rs`; `regex_match` (`src/misc/helpers.h:573-579`) returns it |
| `RULE_PROP_UD/ON/OFF` (`src/rule.h:4-6`) | `0/1/2` | stays `i32` in `struct rule_effects::{manage, sticky, mff, fullscreen}`, because `json_optional_bool` prints it |
| `SIGNAL_PROP_UD/YES/NO` (`src/event_signal.h:90-92`) | `0/1/2` | stays `i32` in `struct signal::active`, same reason |

`json_optional_bool(int value)` (`src/misc/helpers.h:225-231`) prints `"null"` / `"true"` /
`"false"`; it is used at `src/rule.c:50-54` and `src/event_signal.c:426`. Collapsing
`REGEX_MATCH_UD` into "no match" inverts every rule that omits a filter — a rule with no `title`
pattern matches every title today.

`regex_match` returns `int` in C and `RegexMatch` in Rust; every caller compares against all
three, so no `bool` conversion is introduced.

### 8.3 `bool` stays `bool`

`bool foo(...)` becomes `fn foo(...) -> bool`. No `Result<(), Error>`, no `?`.

C frequently **continues after a failure** and accumulates a flag: `parse_rule`
(`src/message.c:2596-2804`) keeps parsing every key after one fails so it can report them all,
and `did_parse` collects the verdict. A `?` chain returns on the first failure and changes what
the user sees. `?` does not appear in the daemon's control flow at all.

`enum window_op_error` and `enum space_op_error` are **results, not errors**: they are matched on
by the caller to pick a message, and `WINDOW_OP_ERROR_SUCCESS` is a normal outcome. They are
plain enums returned by value, never `Result`.

### 8.4 Nullable pointers become `Option`

A C function returning `struct window *` / `struct view *` / `struct window_node *` with `NULL`
for "none" returns `Option<…>` (decision 14 fixes what the `…` is: a handle plus a lookup, or a
borrow of an owned collection). Every `if (!view) return …;` stays an early return on `None`.

Two lookups are **not** nullable and must not gain an `Option`:

* `space_manager_find_view` (`src/space_manager.c`) creates the view on demand and returns it, so
  it never yields `NULL`. The three sites the inventories flag as "unchecked NULL"
  (`src/window_manager.c:1754`, `:2628`, `:2691`) are therefore correct as written; do not add a
  `None` branch that C does not have.
* `view_create` (`src/view.c`) always returns a view.

### 8.5 Where `Result` is allowed

Exactly one place: `std::io::Write` on the response socket. The C ignores every `fprintf` return
(`src/message.c` never checks one), so the `Response` of decision 28 **swallows** write errors.
No `.unwrap()` on a socket write: a client that hangs up mid-response would take the daemon down,
which C does not do.

`regcomp` (`src/message.c:2641`) returns non-zero for a bad pattern and the C tests `== 0`; the
Rust wrapper returns `bool` (decision 26).

### 8.6 The one authorised `unwrap`

`LAYER_STR[LAYER_BELOW as usize].unwrap()` and its two siblings at `src/window.c:115-117`, where
the index is a compile-time constant naming a `Some` entry (§5.3). Everything else uses
`unwrap_or`, a `match`, or an early return.

---

## 9. Assertions, exits and the NULL-then-crash sites

### 9.1 `assert` becomes `debug_assert!`

Decision 33. C's `assert` vanishes under `-DNDEBUG`, which `makefile:26` passes for the `install`
target while the default `all` target (`makefile:4`) keeps it; `debug_assert!` has the same
on-in-debug / off-in-release shape under cargo. Twelve `assert`s exist, three of which belong to
the `ts` arena that decision 17 replaces and therefore do not survive:

| site | C | Rust |
| --- | --- | --- |
| `src/space_manager.c:451` | `assert(node);` | `debug_assert!(node.is_some());` |
| `src/window_manager.c:2334` | `assert(node);` | `debug_assert!(node.is_some());` |
| `src/window_manager.c:2363` | `assert(node);` | `debug_assert!(node.is_some());` |
| `src/display_manager.c:99` | `assert(uuid);` | `debug_assert!(uuid.is_some());` |
| `src/display_manager.c:362` | `assert(uuid);` | `debug_assert!(uuid.is_some());` |
| `src/sa.m:150` | `assert(uid == 0);` | `debug_assert!(user_id == 0);` |
| `src/sa.m:255` | `assert(*zero == '\0');` | `debug_assert!(buffer[zero_index] == 0);` |
| `src/view.c:642` | `assert(removed_entry);` | `debug_assert!(removed_entry);` |
| `src/view.c:643` | `assert(removed_order);` | `debug_assert!(removed_order);` |
| `src/misc/ts.h:38`, `:78`, `:91` | arena invariants | not translated — decision 17 removes the arena; one `DEVIATIONS.md` line |

`assert` never becomes `assert!`, `expect`, `unreachable!` or `panic!`. A `debug_assert!` whose
condition is an `Option` is written `.is_some()`, not `.unwrap()`.

### 9.2 `error!`, `require!`, `warn!`, `debug!`

Decision 33: macros, not functions, so the call sites keep the C format text verbatim and the
compiler sees divergence where C sees `exit`.

```rust
macro_rules! debug {
    ($($argument:tt)*) => { if g_verbose.load(Ordering::Relaxed) { print!($($argument)*); } };
}
macro_rules! warn {
    ($($argument:tt)*) => { eprint!($($argument)*); };
}
macro_rules! error {
    ($($argument:tt)*) => {{ eprint!($($argument)*); std::process::exit(libc::EXIT_FAILURE); }};
}
macro_rules! require {
    ($($argument:tt)*) => {{ eprint!($($argument)*); std::process::exit(libc::EXIT_SUCCESS); }};
}
```

`error!` writes to **stderr** and exits `EXIT_FAILURE` (`src/misc/log.h:26-35`). `require!`
writes to **stderr** and exits `EXIT_SUCCESS` (`src/misc/log.h:37-46`) — the success code is
load-bearing: `src/yabai.c:268`, `:272`, `:276` use it so a launchd job with
`KeepAlive.SuccessfulExit = false` (`src/misc/service.h:26-32`) does not respawn yabai forever
when it refuses to start for a configuration reason. `debug!` writes to **stdout** behind
`g_verbose`; `warn!` writes to stderr unconditionally.

`require!` does **not** post a user notification. `notify(...)` (`src/misc/notify.h:29-48`) is a
separate macro with fifteen call sites (`src/misc/helpers.h:467,473,485`, `src/sa.m:277`-`:404`),
none of which is a `require!` site. Keep the two apart.

`__FUNCTION__` inside a format string becomes the function's name written as a string literal at
that call site (decision 33): `debug("%s: %d\n", __FUNCTION__, wid)` becomes
`debug!("{}: {}\n", "event_handler_sls_window_ordered", window_id)`. There are 87 `debug` calls
and most of them carry `__FUNCTION__`; the literal must be the **Rust** function's name, which is
the C handler name lowercased for `EVENT_HANDLER` expansions.

`debug_message` (`src/misc/log.h:48-61`) stays a function, not a macro: it takes two `&str` and
walks a NUL-separated message. Its loop `for (;*message;) message += fprintf(stdout, " %s", message);`
advances by the number of bytes printed, which is `strlen + 1` — a split on `\0` in Rust
reproduces it.

The exit points are inventoried in `sweeps/c-idioms-and-semantics.md` §5.26; nothing in this
document changes any exit code.

### 9.3 The three NULL-then-crash sites

Decision 4: a NULL dereference is never reproduced; it becomes an early return, and the removal
is a `DEVIATIONS.md` line. Three sites in the daemon dereference a `view_find_window_node` /
`view_add_window_node_with_insertion_point` result that can be `NULL`, with no check:

| site | C | what the C does | Rust |
| --- | --- | --- | --- |
| `src/window_manager.c:1819-1820` | `struct window_node *a_node = view_find_window_node(a_view, a->id);` then `if (a_node->window_count+1 >= NODE_MAX_WINDOW_COUNT)` | crashes when `a` is managed by `a_view` but has no node in it | `let Some(a_node) = view_find_window_node(a_view, a_window.id) else { return WindowOpError::InvalidSrcNode; };` — the same variant the function already returns for a missing `a_view` at `:1805` and `:1814` |
| `src/mouse_handler.c:138-139` | `struct window_node *dst_node = view_find_window_node(dst_view, dst_window->id);` then `if (dst_node->window_count+1 < NODE_MAX_WINDOW_COUNT)` | crashes when the drop target is not in the destination view | `let Some(destination_node) = view_find_window_node(destination_view, destination_window.id) else { return; };` — the function returns `void`, and the `if` body is already the "do the stack" branch, so `None` falls through to the same place a full stack does |
| `src/mouse_handler.c:211` | `if (src_node_rm != src_node_add && src_node_rm != src_node_add->parent)` | crashes when `view_add_window_node_with_insertion_point` returned `NULL` | test the `Option` first: the whole condition is `false` when `source_node_add` is `None`, so the second `window_node_capture_windows` is skipped and the flush at `:215` still runs |

The first one returns after `b` has already been untiled at `src/window_manager.c:1810-1812`,
which is exactly what the neighbouring `return WINDOW_OP_ERROR_MAX_STACK` at `:1820` does. The
early return is therefore in the same state the C reaches on its own failure path.

`mouse_drop_try_adjust_bsp_grid` (`src/mouse_handler.c:232`) **does** check its
`view_find_window_node` result before flushing it (`:261-262`), so the asymmetry inside
`src/mouse_handler.c` is real: only the two sites above change.

### 9.4 The other unchecked dereference, which is not a NULL deref

`src/view.c:877`, `ts_cfstring_copy(view->uuid)`, passes `view->uuid` to `CFStringGetLength`
without checking it; `SLSSpaceCopyName` can return `NULL` (`src/view.c:995`). This is a NULL
passed to a CF function, not a Rust `Option` dereference: `view.uuid` is `Option<CFRetained<CFString>>`
and the `None` case prints `"<unknown>"`, which is what `src/view.c:878` already prints when the
conversion fails. One `DEVIATIONS.md` line.

---

## 10. The comment policy

Decision 38: **only comments present in the C source are carried over, verbatim, at the matching
place.** No doc comments (`///`, `//!`), no `SAFETY:` comments, no comments describing the
translation, no `TODO` a translator invents, no section banners a translator invents. Soundness
arguments live in `THREADS.md`, not in the code.

"Verbatim" includes the typos: `**iff*` (`src/misc/service.h:232`), `boostrapped`
(`src/misc/service.h:302`), `hecking` (`src/process_manager.c:165`), `friggin`
(`src/workspace.m:44`), `NOTE(asmvik)` with that exact spelling. Do not fix any of them.

"At the matching place" means the same position relative to the code, not the same line number.
A comment that sat above a `goto err;` chain that RAII removed sits above the statement that
replaced it.

The C comment markers survive as Rust `//` and the block comments as `//` too: Rust has `/* */`
but the daemon's block comments are all single-line, and mixing the two forms across eighteen
modules invites divergence. `/* silence compiler warning.. */` becomes
`// silence compiler warning..`; the inline `/* kCGSEventDockControl */ 30` form has no `//`
equivalent, so those keep `/* */` (they are the only ones).

### 10.1 What a reviewer checks

`grep -rn '//\|/\*' src/**/*.rs` must produce exactly the set below, once per listed site, plus
nothing else. 67 distinct texts across 92 sites; 66 texts across 91 sites are carried (the 67th,
`src/manifest.m:49`, is commented-out dead code and is listed in §11 instead).

| C file | comment sites |
| --- | --- |
| `src/window_manager.c` | 26 |
| `src/event_loop.c` | 19 |
| `src/message.c` | 17 |
| `src/misc/service.h` | 10 |
| `src/display_manager.c` | 4 |
| `src/space_manager.c` | 4 |
| `src/workspace.m` | 3 |
| `src/mouse_handler.c` | 2 |
| `src/mouse_handler.h` | 2 |
| `src/process_manager.c` | 2 |
| `src/application.c` | 1 |
| `src/event_signal.c` | 1 |
| `src/manifest.m` | 1 (not carried — §11) |

Every other file under `src/` (excluding `src/osax/`) contains **no comments at all**:
`src/display.c`, `src/display.h`, `src/event_loop.h`, `src/event_signal.h`, `src/message.h`,
`src/mission_control.c`, `src/process_manager.h`, `src/rule.c`, `src/rule.h`, `src/sa.h`, `src/application.h`,
`src/sa.m`, `src/space.c`, `src/space.h`, `src/space_manager.h`, `src/view.c`, `src/view.h`,
`src/window.c`, `src/window.h`, `src/window_manager.h`, `src/workspace.h`, `src/yabai.c`, and
every header under `src/misc/` except `service.h`. A `//` in one of the corresponding Rust
modules is a policy violation, with one deliberate exception: `#[allow(...)]` attributes and
`#[cfg(...)]` are attributes, not comments.

Three matches that look like comments and are **not** — they are inside string literals and must
be reproduced as literal text, not as comments: `src/misc/service.h:10` and `src/sa.m:23`, `:52`
(`-//Apple//DTD PLIST 1.0//EN` in the plist templates) and `src/yabai.c:200` (the documentation
URL printed by `--help`).

### 10.2 The complete list

The count prefix is how many sites share that exact text; the locations follow. The text is the
comment block with its common indentation stripped.

`src/application.c:30-34`
```c
//
// NOTE(asmvik): Flag events that are already queued, but not yet processed,
// so that they will be ignored; the memory we allocated is still valid and will
// be freed when this event is handled.
//
```

`src/display_manager.c:312-316`
```c
//
// NOTE(asmvik): SLSGetRevealedMenuBarBounds is broken on Apple Silicon,
// but we expected it to return the full display bounds along with the menubar
// height. Combine this information ourselves using two separate functions..
//
```

`src/display_manager.c:325-328`
```c
//
// NOTE(asmvik): Height needs to be offset by 1 because that is the actual
// position on the screen that windows can be positioned at..
//
```

**2x** — `src/display_manager.c:370`; `src/display_manager.c:388`
```c
return false; // This does not return a correct result on modern macOS versions.
```

`src/event_loop.c:22`
```c
// NOTE(asmvik): Subscribe to all windows because of window_destroyed (and ordered) notifications
```

`src/event_loop.c:27`
```c
// NOTE(asmvik): Subscribe to windows that have a feedback_border because of window_ordered notifications
```

**2x** — `src/event_loop.c:106-109`; `src/event_loop.c:125-128`
```c
//
// NOTE(asmvik): Do this again in case of race-conditions between the previous check and key-value observation subscription.
// Not actually sure if this can happen in practice..
//
```

`src/event_loop.c:140-143`
```c
//
// NOTE(asmvik): If we somehow receive a duplicate launched event due to the subscription-timing-mess above,
// simply ignore the event..
//
```

**2x** — `src/event_loop.c:185`; `src/event_loop.c:586`
```c
} else /* if (g_window_manager.window_origin_mode == WINDOW_ORIGIN_CURSOR) */ {
```

**7x** — `src/event_loop.c:201-209`; `src/event_loop.c:288-296`; `src/event_loop.c:448-456`; `src/event_loop.c:510-518`; `src/window_manager.c:2559-2567`; `src/window_manager.c:2588-2596`; `src/window_manager.c:2607-2615`
```c
//
// @cleanup
//
// :AXBatching
//
// NOTE(asmvik): Batch all operations and mark the view as dirty so that we can perform a single flush,
// making sure that each window is only moved and resized a single time, when the final layout has been computed.
// This is necessary to make sure that we do not call the AX API for each modification to the tree.
//
```

**5x** — `src/event_loop.c:227-234`; `src/event_loop.c:321-328`; `src/event_loop.c:469-476`; `src/event_loop.c:530-537`; `src/window_manager.c:2635-2642`
```c
//
// @cleanup
//
// :AXBatching
//
// NOTE(asmvik): Flush previously batched operations if the view is marked as dirty.
// This is necessary to make sure that we do not call the AX API for each modification to the tree.
//
```

**2x** — `src/event_loop.c:1217`; `src/event_loop.c:1323`
```c
/* silence compiler warning.. */
```

`src/event_loop.c:1365-1369`
```c
//
// NOTE(asmvik): Look for a window with role AXSheet or AXDrawer
// and forward focus to it because we are not allowed to focus the main
// window in these cases.
//
```

`src/event_loop.c:1400-1405`
```c
//
// NOTE(asmvik): If any **floating** window would be fully occluded by
// autoraising the window below the cursor we do not actually perform the
// focus change, as it is likely that the user is trying to reach for the
// smaller window that sits on top of the window we would otherwise raise.
//
```

`src/event_signal.c:148-154`
```c
//
// NOTE(asmvik): We always receive an application_front_switched event *before* an application_terminated
// event. We need to know the difference between a front_switched + application_terminated sequence and a regular
// application switch followed by a user-initiated termination of the previously focused application. The system
// events are triggered within an interval that appear to be impossible to match even with a user automated sequence.
// The dt threshold below is triple the average interval computed, to allow for some leeway.
//
```

`src/message.c:22`
```c
/* --------------------------------DOMAIN CONFIG-------------------------------- */
```

**8x** — `src/message.c:90`; `src/message.c:96`; `src/message.c:128`; `src/message.c:180`; `src/message.c:190`; `src/message.c:217`; `src/message.c:233`; `src/message.c:252`
```c
/* ----------------------------------------------------------------------------- */
```

`src/message.c:92`
```c
/* --------------------------------DOMAIN DISPLAY------------------------------- */
```

`src/message.c:98`
```c
/* --------------------------------DOMAIN SPACE--------------------------------- */
```

`src/message.c:130`
```c
/* --------------------------------DOMAIN WINDOW-------------------------------- */
```

`src/message.c:182`
```c
/* --------------------------------DOMAIN QUERY--------------------------------- */
```

`src/message.c:192`
```c
/* --------------------------------DOMAIN RULE---------------------------------- */
```

`src/message.c:219`
```c
/* --------------------------------DOMAIN SIGNAL-------------------------------- */
```

`src/message.c:235`
```c
/* --------------------------------COMMON ARGUMENTS----------------------------- */
```

`src/message.c:311`
```c
// NOTE(asmvik): don't go past the null-terminator
```

`src/misc/service.h:44-51`
```c
//
// NOTE(asmvik): A launchd service has the following states:
//
//          1. Installed / Uninstalled
//          2. Active (Enable / Disable)
//          3. Bootstrapped (Load / Unload)
//          4. Running (Start / Stop)
//
```

`src/misc/service.h:134-139`
```c
//
// NOTE(asmvik): Temporarily remove filename.
// We know the filepath will contain a slash, as
// it is controlled by us, so don't bother checking
// the result..
//
```

`src/misc/service.h:148-150`
```c
//
// NOTE(asmvik): Restore original filename.
//
```

**2x** — `src/misc/service.h:211-213`; `src/misc/service.h:277-279`
```c
//
// NOTE(asmvik): Check if service is bootstrapped
//
```

`src/misc/service.h:220-225`
```c
//
// NOTE(asmvik): Service is not bootstrapped and could be disabled.
// There is no way to query if the service is disabled, and we cannot
// bootstrap a disabled service. Try to enable the service. This will be
// a no-op if the service is already enabled.
//
```

`src/misc/service.h:230-233`
```c
//
// NOTE(asmvik): Bootstrap service into the target domain.
// This will also start the program **iff* RunAtLoad is set to true.
//
```

`src/misc/service.h:239-243`
```c
//
// NOTE(asmvik): The service has already been bootstrapped.
// Tell the bootstrapped service to launch immediately; it is an
// error to bootstrap a service that has already been bootstrapped.
//
```

`src/misc/service.h:286-290`
```c
//
// NOTE(asmvik): Service is not bootstrapped, but the program
// could still be running an instance that was started **while the service
// was bootstrapped**, so we tell it to stop said service.
//
```

`src/misc/service.h:296-304`
```c
//
// NOTE(asmvik): Service is bootstrapped; we stop a potentially
// running instance of the program and unload the service, making it
// not trigger automatically in the future.
//
// This is NOT the same as disabling the service, which will prevent
// it from being boostrapped in the future (without explicitly re-enabling
// it first).
//
```

`src/mouse_handler.c:69-75`
```c
case /* kCGSEventDockControl */ 30: {
    int type = CGEventGetIntegerValueField(event, /* kCGEventGestureHIDType */ 110);
    if (type == /* kIOHIDEventTypeDockSwipe */ 23) {
        int motion = CGEventGetIntegerValueField(event, /* kCGEventGestureSwipeMotion */ 123);
        if (motion == /* kCGGestureMotionHorizontal */ 1) {
            int phase = CGEventGetIntegerValueField(event, /* kCGEventGesturePhase */ 132);
            if (phase == /* kCGSGesturePhaseBegan */ 1) {
```

`src/mouse_handler.c:77`
```c
} else if (phase == /* kCGSGesturePhaseEnded */ 4 || phase == /* kCGSGesturePhaseCancelled */ 8) {
```

**2x** — `src/mouse_handler.h:11`; `src/mouse_handler.h:19`
```c
(1 << /* kCGSEventDockControl */ 30)
```

**2x** — `src/process_manager.c:98-102`; `src/window_manager.c:1594-1598`
```c
//
// NOTE(asmvik): display_space_list(..) uses a linear allocator,
// and so we only need to track the beginning of the first list along
// with the total number of windows that have been allocated.
//
```

`src/process_manager.c:164-168`
```c
//
// NOTE(asmvik): Some garbage applications (e.g Steam) are reported twice with the same PID and PSN for some hecking reason.
// It is by definition NOT possible for two processes to exist at the same time with the same PID and PSN.
// If we detect such a scenario we simply discard the dupe notification..
//
```

`src/space_manager.c:944-953`
```c
//
// NOTE(asmvik): MacOS does not have an API that allows for space activation.
// However, we can synthesize a sequence of high velocity gestures to skip the
// animation instead.
//
// :Attribution
// https://github.com/jurplel/InstantSpaceSwitcher
// https://github.com/thenickdude/wacom-driver-fix/blob/bdfda9a788934c88d09d31ea6a42664b9ba1471e/Readme.md
// Technique first observed in practice, and reverse-engineered from, BetterTouchTool.
//
```

`src/space_manager.c:959-963`
```c
CGEventSetIntegerValueField(event_dock_control, /* kCGSEventTypeField            */  55, /* kCGSEventDockControl       */ 30);
CGEventSetIntegerValueField(event_dock_control, /* kCGEventGestureHIDType        */ 110, /* kIOHIDEventTypeDockSwipe   */ 23);
CGEventSetIntegerValueField(event_dock_control, /* kCGEventGestureSwipeMotion    */ 123, /* kCGGestureMotionHorizontal */  1);
CGEventSetDoubleValueField(event_dock_control,  /* kCGEventGestureSwipeProgress  */ 124, sign);
CGEventSetDoubleValueField(event_dock_control,  /* kCGEventGestureSwipeVelocityX */ 129, sign * 9999.0);
```

`src/space_manager.c:966`
```c
CGEventSetIntegerValueField(event_dock_control, /* kCGEventGesturePhase */ 132, /* kCGSGesturePhaseBegan */ 1);
```

`src/space_manager.c:968`
```c
CGEventSetIntegerValueField(event_dock_control, /* kCGEventGesturePhase */ 132, /* kCGSGesturePhaseEnded */ 4);
```

`src/window_manager.c:78-82`
```c
//
// NOTE(asmvik): display_space_list(..) uses a linear allocator,
// and so we only need to track the beginning of the first list along
// with the total number of spaces that have been allocated.
//
```

`src/window_manager.c:731-739`
```c
//
// NOTE(asmvik): Attempting to check the window frame cache to prevent unnecessary movement and resize calls to the AX API
// is not reliable because it is possible to perform operations that should be applied, at a higher rate than the AX API events
// are received, causing our cache to become out of date and incorrectly guard against some changes that **should** be applied.
// This causes the window layout to **not** be modified the way we expect.
//
// A possible solution is to use the faster CG window notifications, as they are **a lot** more responsive, and can be used to
// track changes to the window frame in real-time without delay.
//
```

`src/window_manager.c:748`
```c
// NOTE(asmvik): Due to macOS constraints (visible screen-area), we might need to resize the window *before* moving it.
```

`src/window_manager.c:756`
```c
// NOTE(asmvik): Due to macOS constraints (visible screen-area), we might need to resize the window *after* moving it.
```

`src/window_manager.c:885`
```c
} else /*if (wm->purify_mode == PURIFY_ALWAYS) */ {
```

`src/window_manager.c:1271-1278`
```c
//
// :SynthesizedEvent
//
// NOTE(asmvik): These events will be picked up by an event-tap
// registered at the "Annotated Session" location; specifying that an
// event-tap is placed at the point where session events have been
// annotated to flow to an application.
//
```

`src/window_manager.c:1306-1311`
```c
//
// @hack
// Artificially delay the activation by 40ms. This is necessary
// because some applications appear to be confused if both of
// the events appear instantaneously.
//
```

`src/window_manager.c:1454-1456`
```c
//
// NOTE(asmvik): Attempt to track **all** windows.
//
```

`src/window_manager.c:1473-1475`
```c
//
// NOTE(asmvik): However, only **root windows** are eligible for management.
//
```

`src/window_manager.c:1479-1485`
```c
//
// NOTE(asmvik): A lot of windows misreport their accessibility role, so we allow the user
// to specify rules to make sure that we do in fact manage these windows properly.
//
// This part of the rule must be applied at this stage (prior to other rule properties), and if
// no such rule matches this window, it will be ignored if it does not have a role of kAXWindowRole.
//
```

`src/window_manager.c:1516-1519`
```c
//
// NOTE(asmvik): Print window information when debug_output is enabled.
// Useful for identifying and creating rules if this window should in fact be managed.
//
```

`src/window_manager.c:1531-1533`
```c
//
// NOTE(asmvik): Print window information when debug_output is enabled.
//
```

`src/window_manager.c:1623-1631`
```c
//
// @cleanup
//
// :Workaround
//
// NOTE(asmvik): The AX API appears to always include a single element for Finder that returns an empty window id.
// This is likely the desktop window. Other similar cases should be handled the same way; simply ignore the window when
// we attempt to do an equality check to see if we have correctly discovered the number of windows to track.
//
```

`src/window_manager.c:1659-1666`
```c
//
// NOTE(asmvik): MacOS API does not return AXUIElementRef of windows on inactive spaces.
// However, we can just brute-force the element_id and create the AXUIElementRef ourselves.
//
// :Attribution
// https://github.com/decodism
// https://github.com/lwouis/alt-tab-macos/issues/1324#issuecomment-2631035482
//
```

`src/window_manager.c:1884-1891`
```c
//
// :NaturalWarp
//
// NOTE(asmvik): Precalculate both target areas and select the one that has the closest distance to the source area.
// This allows the warp to feel more natural in terms of where the window is placed on screen, however, this is only utilized
// for warp operations where both operands belong to the same space. There may be a better system to handle this if/when multiple
// monitors should be supported.
//
```

`src/window_manager.c:1931-1937`
```c
//
// :NaturalWarp
//
// TODO(asmvik): Warp operations with operands that belong to different monitors does not yet implement a heuristic to select
// the target area that feels the most natural in terms of where the window is placed on screen. Is it possible to do better when
// warping between spaces that belong to the same monitor as well??
//
```

`src/window_manager.c:2270-2276`
```c
//
// NOTE(asmvik): Window has exited native-fullscreen mode.
// We need to spin lock until the display is finished animating
// because we are not actually able to interact with the window.
//
// The display_manager API does not work on macOS Monterey.
//
```

`src/window_manager.c:2285-2289`
```c
//
// NOTE(asmvik): Window has exited native-fullscreen mode.
// We need to spin lock until the display is finished animating
// because we are not actually able to interact with the window.
//
```

`src/window_manager.c:2302-2306`
```c
//
// NOTE(asmvik): The window must become the focused window
// before we can change its fullscreen attribute. We focus the
// window and spin lock until a potential space animation has finished.
//
```

`src/window_manager.c:2318-2321`
```c
//
// NOTE(asmvik): We toggled the fullscreen attribute and must
// now spin lock until the post-exit space animation has finished.
//
```

`src/window_manager.c:2455`
```c
// TODO(asmvik): Both functions use the same underlying API and could be combined in a single function to reduce redundant work.
```

`src/workspace.m:40-54`
```c
//
// :WorstApiEverMade
//
// NOTE(asmvik): Because the developers of this API did such an amazing job
// there is no way for us to actually just friggin loop through the currently
// registered observations and then call removeObservation on them..
//
// Instead we just try to force remove the observations that **could** be present
// at this point in time, because it will complain if we try to actually release
// the object when it has observers present.
//
// We can't actually correctly track whether it did actually get unobserved previously,
// because even when our notification callback is triggered it will claim that we try
// to remove a non-existing observation when it just called us back.
//
```

**2x** — `src/workspace.m:217-224`; `src/workspace.m:242-249`
```c
//
// :WorstApiEverMade
//
// NOTE(asmvik): For some stupid reason it is possible to get notified by the system
// about a change, and NOT being able to remove ourselves from observation because
// it claims that we are not observing the key-path, but we clearly are, as we would
// otherwise not be here in the first place..
//
```

---

## 11. Dead code that is not translated

Decision 5. Each item below produces **no Rust**, and each produces one `DEVIATIONS.md` line
(§12). A translator who finds something not on this list translates it.

| C | what it is | why |
| --- | --- | --- |
| `src/misc/autorelease.h` (whole file) | `hook_nsobject_autorelease`, `hook_autoreleasepool_drain`, `hook_autoreleasepool_release` and their `method_setImplementation` machinery | commented out of the unity build at `src/manifest.m:49`; never compiled |
| `src/yabai.c:158-162` | `#if 0` block calling the three `hook_*` functions | never compiled |
| `src/misc/timer.h:4-155`, everything inside `#if PROFILE >= 1` | `struct profile_anchor`, `g_profiler`, `read_cpu_timer`, `read_cpu_freq`, `profile_begin`, `profile_end_and_print`, `struct time_block`, `BEGIN_TIME_BLOCK`, `END_TIME_BLOCK` | `PROFILE` is never defined — not by `makefile`, not by `src/manifest.m`, not by any `#define` in `src/**` — so `#if PROFILE >= 1` is always false and none of it is compiled |
| `TIME_FUNCTION` (58 call sites) | `src/display.c:22`, `src/display_manager.c` (1), `src/event_loop.c:1616`, `src/event_signal.c` (2), `src/message.c` (11), `src/rule.c:6`, `src/space_manager.c` (4), `src/view.c` (1), `src/window.c` (2), `src/window_manager.c` (34) | expands to nothing; the 58 statements disappear |
| `TIME_BLOCK`, `TIME_BODY` | no call sites at all | — |
| `PROFILER_END_TRANSLATION_UNIT` (`src/yabai.c:356`) | the `_Static_assert` anchor-count guard | expands to nothing |
| `profile_begin()` / `profile_end_and_print()` (`src/event_loop.c:1656`, `:1673`) | the two live-looking profiler calls in the event loop | both expand to nothing; note the C `#define profile_begin();` carries its own trailing semicolon |
| `void window_unknown_serialize(FILE *, uint32_t, uint64_t);` (`src/window.h:140`) | declared, **never defined** anywhere | the real function is `window_nonax_serialize` (`src/window.c:121`), called from `src/window_manager.c:48`. Use that name; do not create a stub |
| `void window_manager_tile_window(struct window_manager *, struct window *);` (`src/window_manager.h:117`) | declared, no definition, no caller | — |
| `daemon_deprecated` (`src/message.c:429-437`) | `__unused static inline`, no caller | kept deliberately in C with `__unused`; not translated |
| `CFSTRINGNUM32` (`src/misc/helpers.h:321-326`) | no callers anywhere in `src/**` | `CFNUM32` (`:328-331`) **is** called from `sls_window_disable_shadow` (`:335`) and stays |
| `min(a, b)` (`src/misc/macros.h:9`) | the macro has no call site in `src/**`; `max` has two (`src/window_manager.c:353-354`) | — |
| `GIGABYTES(value)` (`src/misc/macros.h:6`) | no call site; `KILOBYTES` and `MEGABYTES` are called from `src/yabai.c:279`, `:283` | — |
| `in_range_ee(a, b, c)` (`src/misc/macros.h:15`) | no call site; the other three are all used | — |
| `buf_free` (`src/misc/sbuffer.h:20`) | never called | `buf_*` is replaced by `Vec` anyway (decision 17) |
| `table_free` (`src/misc/hashtable.h:57-73`) | never called outside its own definition | the `Table` of decision 16 frees through `Drop`; there is no free function |
| `SLSClearWindowTags` (`src/misc/extern.h:29`), `SLSMoveWindow` (`:65`), `SLSNewWindow` (`:25`), `SLSSetWindowBackgroundBlurRadiusStyle` (`:32`), `SLSSetWindowTags` (`:28`), `SLSSetWindowTransform` (`:87`), `SLSTransactionOrderWindow` (`:91`) | seven SkyLight declarations with no caller in the daemon | decision 5 covers "declarations with no caller"; they are not declared in `src/ffi/skylight.rs`. `sweeps/ffi-surface.md` recommends declaring them behind `#[allow(dead_code)]` — decision 5 wins |

`src/misc/timer.rs` therefore holds **only** the two clock readers that live code calls,
`read_os_timer` and `read_os_freq` (`src/misc/helpers.h:149-160`), moved there from
`helpers.h` per the module map in §3. The `__rdtsc` and `mrs cntvct_el0` / `mrs cntfrq_el0`
paths belong to `read_cpu_timer` / `read_cpu_freq`, which are inside `#if PROFILE >= 1` and are
**not** translated, even though `TRANSLATION_PLAN.md` §1.6 lists them — decision 5 wins.

Not dead, despite looking it:

* `- (void)dealloc` on `workspace_context` (`src/workspace.m:201-207`) is never reached because
  the object is never released, but it is part of the class's method table and the Objective-C
  runtime can reach it. Translate it.
* `src/misc/memory_pool.h`, `src/misc/sbuffer.h` and `src/misc/ts.h` are live; decision 17
  **replaces** them with owned `Vec`/`String`/queues rather than deleting them. That is not a
  dead-code line, and their behaviour changes are `DEVIATIONS.md` lines of their own.
* `mouse_mod_str`'s 27 `NULL` holes (§5.3) are unreachable today and stay.
* `if (w <= 0) w = 1;` / `if (h <= 0) h = 1;` (`src/window_manager.c:2136-2137`) on `unsigned`
  is `== 0`, not dead: `--grid 1:1:0:0:0:0` reaches it.

---

## 12. `DEVIATIONS.md`

### 12.1 The line format

`doc/rust-rewrite/DEVIATIONS.md` is one markdown table, appended to in file order. Four columns,
one row per deviation, no prose between rows:

```
| C location | kind | what C does | what Rust does |
```

* **C location** — `path:line` or `path:line-line`, exactly as this document cites them.
* **kind** — one word from this closed set:
  `overrun`, `use-after-free`, `race`, `uninitialised`, `null-deref`, `leak`, `narrowing`,
  `dead-code`.
* **what C does** — the observable consequence, not the mechanism: "writes past the end of a
  1024-entry stack array", not "no bounds check".
* **what Rust does** — the replacement behaviour, in the same tense.

One row per site, not per function and not per category. A row is added by whoever translates
that file, in the same commit.

Example rows, in the exact shape:

```
| src/event_loop.c:19-31 | overrun | writes past the end of a 1024-entry stack array once more than 1024 windows are tracked | stops filling at 1024 and subscribes to the first 1024, as the SkyLight call already caps |
| src/window_manager.c:548-553 | uninitialised | reads `mt` uninitialised if the easing switch matches nothing | the match is exhaustive over `AnimationEasingType`, so `eased_interpolant` is always assigned |
| src/misc/timer.h:4-155 | dead-code | the `PROFILE >= 1` block is never compiled because `PROFILE` is never defined | not translated |
```

### 12.2 The deviations already known

Every latent defect the inventories report, each with its decided Rust behaviour under decision
4, plus the decision-5 removals. This is the starting content of `DEVIATIONS.md`; a translator
adds rows, never removes one.

#### Memory safety

| C location | kind | what C does | what Rust does |
| --- | --- | --- | --- |
| `src/event_loop.c:19-31` | overrun | `update_window_notifications` fills a `uint32_t window_list[1024]` from a `table_for` with no bound; more than 1024 tracked windows writes past the end | stops filling at 1024; the same 1024 ids go to `SLSRequestNotificationsForWindows`, which the C already caps |
| `src/window_manager.c:848-867` | overrun | `check_list[window_count]` is a VLA written through `check_list[check_count++]` inside a loop whose bound `check_count` it also grows, and `window_count == 0` makes it zero-length while `check_list[0]` is still written | a `Vec<u32>` that grows; the `window_count == 0` case returns before the write |
| `src/window_manager.c:848-856` | overrun | `relation_count` can exceed `window_count` if the iterator yields more relations than the associated-window list had entries, overrunning `parent_list`/`child_list` | two `Vec<u32>` that grow |
| `src/misc/helpers.h:612-670` | overrun | `cgimage_restore_alpha` steps the pixel pointer four at a time to `height*width`, reading and writing up to three pixels past the buffer when the pixel count is not a multiple of four | a 1-to-3 pixel tail goes through the same routine on a padded scratch copy (decision 36) |
| `src/view.c:667-687`, read at `src/event_loop.c:948` | use-after-free | when a removed sibling had an overlay, the parent inherits `feedback_window` and `insert_feedback_show(parent)` takes the early-out, so the `insert_feedback` table keeps an entry pointing at the child that `src/view.c:718` frees; the next `table_find` dereferences it | the node arena of decision 15 makes this a stale `NodeId` and the lookup misses |
| `src/window.h:92`, `src/application.c:36`, `src/event_loop.c:280` and the nine probes | use-after-free | `id_ptr` is a self-referential `volatile` pointer CAS-ed to `NULL` to mark a window dead while its event is still queued | the liveness cell of decision 21: claim-for-destruction is a compare-exchange, the probes are loads, and an event for a dead window is dropped |
| `src/window_manager.c:621`, read at `:694` | use-after-free | `animation->window` is a borrowed `struct window *` that nothing clears if the window dies mid-animation | the animation carries the window id and re-looks it up (decision 14) |
| `src/event_signal.c:107` | overrun | `g_signal_storage` is bumped with a bare `__sync_fetch_and_add` and an overrun runs off the end of the pool into the guard page | an owned queue (decision 17); no ceiling and no guard-page fault |
| `src/misc/memory_pool.h:29-45` | use-after-free | the event pool wraps to the start when full and hands out memory that a queued event may still reference | an owned `mpsc` queue (decision 19); no wrap |
| `src/space_manager.c:1157-1190` | use-after-free | a VLA snapshot of `struct view *` taken from the table is dereferenced while the same loop mutates the table | a `Vec<(u64, CFRetained<CFString>)>` snapshot keyed by space id, re-resolved per iteration |
| `src/event_loop.c:191-243`, `:275-337`, `:439-485`, `:502-546` | use-after-free | `ts_alloc_list(struct view *, n)` snapshots view pointers, the graph is then mutated, and the snapshot is dereferenced to flush | a `Vec<u64>` of space ids, re-resolved in the flush loop |
| `src/event_loop.c:1186-1204` | use-after-free | the mouse drop target is read after the node it points at may have been freed | handles plus lookups (decision 14) |

#### NULL dereferences

| C location | kind | what C does | what Rust does |
| --- | --- | --- | --- |
| `src/window_manager.c:1819-1820` | null-deref | dereferences a `NULL` `view_find_window_node` result | early return `WindowOpError::InvalidSrcNode`, the variant the same function already returns at `:1805` and `:1814` |
| `src/mouse_handler.c:138-139` | null-deref | dereferences a `NULL` `view_find_window_node` result | early `return`; the caller sees the same state as a full stack |
| `src/mouse_handler.c:211` | null-deref | dereferences a `NULL` `view_add_window_node_with_insertion_point` result | the condition is `false` for `None`, the second capture is skipped, the flush still runs |
| `src/view.c:877` | null-deref | passes a `NULL` `view->uuid` to `CFStringGetLength` when `SLSSpaceCopyName` returned `NULL` at `:995` | `view.uuid` is an `Option`; `None` prints `"<unknown>"`, which `:878` already prints for a failed conversion |
| `src/space.c:31-32` | null-deref | never checks `SLSWindowQueryWindows` / `SLSWindowQueryResultCopyWindows`; a `NULL` crashes in `SLSWindowIteratorAdvance` | early return of an empty list |
| `src/display_manager.c:456-458` | null-deref | dereferences a display that has already been removed | lookup miss, early return |
| `src/event_loop.c:281` | null-deref | `WINDOW_DESTROYED` can reach `window->application` after the application was cleared to `NULL` at `:281` | the window's application is an `Option`; `None` skips the branch |

#### Races and unsynchronised access

| C location | kind | what C does | what Rust does |
| --- | --- | --- | --- |
| `src/window_manager.c:560-563` vs `:635-642` | race | the CVDisplayLink callback writes `proxy.tx/ty/tw/th` while the event-loop thread reads them, unsynchronised | `AtomicU32` holding the `f32` bits (decision 24) |
| `src/view.h:70` `volatile bool skip` | race | `volatile` is not an atomic | `AtomicBool` (decision 24) |
| `src/yabai.c:27-52`, every `extern` re-declaration | race | 21 mutable globals touched by the event-loop thread and the main thread with no locking | `EventLoopOwnedState` moved into the event-loop thread (decision 12), `OnceLock` for write-once values and `AtomicBool` for `g_verbose` (decision 18) |
| `src/mouse_handler.h:62-80` | race | `g_mouse_state` is read by the event tap on the main thread and written by the event loop, with only `volatile` on two fields | split in two: a static of atomics for the tap's half, event-loop-owned for the drag half (decision 23) |
| `src/process_manager.h` process table | race | the Carbon handler inserts on the main thread while the event loop reads | `Arc<Process>` with atomic fields behind a `Mutex` never held across an ObjC or AX call (decision 22) |

#### Uninitialised reads

| C location | kind | what C does | what Rust does |
| --- | --- | --- | --- |
| `src/window_manager.c:548-553` | uninitialised | `float mt;` is read after a `switch` with no `default`; unreachable today because the X-macro covers every value | an exhaustive `match` over `AnimationEasingType` with no `_` arm |

#### Numeric behaviour

| C location | kind | what C does | what Rust does |
| --- | --- | --- | --- |
| `src/window_manager.c:1147`, `:1166` | narrowing | `(uint32_t)(area.w * area.h)` wraps modulo 2^32 when the `f32` product exceeds `u32::MAX`, and is undefined when it is negative | `as u32` saturates to `u32::MAX` and clamps negatives to `0` |
| `src/view.c:170-182`, `:566-578`, `src/event_loop.c:1269-1270`, `src/workspace.m:132`, `src/window_manager.c:1896-1898` | narrowing | float-to-int conversion is undefined out of range | `as` saturates; every in-range value is identical |
| `src/window_manager.c:2300`, `:2309` | narrowing | a 64-bit space id is stored in a `uint32_t` and then compared against a full 64-bit active space id, truncating the comparison | the truncation is reproduced on both sides |
| `src/message.c:352` | narrowing | signed `int` overflow in the decimal accumulator is undefined and wraps in the optimised build | `wrapping_mul` / `wrapping_add`, so debug and release both wrap |
| `src/message.c:349,352` | overrun | `token_char_int_table[(int)c]` indexes negatively for a byte above `0x7f` because `char` is signed; unreachable because the loop rejects such a byte first | indexed with `c as usize` after the same check |
| `src/misc/helpers.h:273`, `:308` | narrowing | `*cursor >= 0x00 && *cursor <= 0x1f` on a signed `char` lets UTF-8 continuation bytes through unescaped | over `u8` the condition is written `byte <= 0x1f` alone, which is the same set |

#### Leaks the C makes on purpose

| C location | kind | what C does | what Rust does |
| --- | --- | --- | --- |
| `src/window_manager.c:1680-1708` | leak | leaks an `AXUIElementRef` per iteration when the role is `NULL` or is not `kAXWindowRole` — up to ~32767 per call | `CFRetained` releases on drop; memory use only, no semantic change |
| `src/window_manager.c:2709-2740` | leak | `AXUIElementCreateSystemWide` is never released | kept alive for the process lifetime in a `OnceLock`; no release, so no change |
| `src/misc/service.h:53-78` | leak | `posix_spawn_file_actions_destroy` is never called because the process exits immediately | kept as-is (decision 34) |
| `src/yabai.c:164-177` | leak | the lock file's descriptor is deliberately never closed, because closing it releases the `fcntl` lock | the descriptor is leaked on purpose; a Rust `File` must **not** be dropped (decision 34) |
| `src/misc/sbuffer.h:20`, `src/misc/hashtable.h:57-73` | leak | `buf_free` and `table_free` are never called; every buffer and table leaks at exit by design | `Vec` and `Table` free through `Drop`; memory use only |

#### Container semantics that must not drift

| C location | kind | what C does | what Rust does |
| --- | --- | --- | --- |
| `src/misc/hashtable.h:120-123` | narrowing | `table_add` does **not** overwrite an existing key | the `Table` of decision 16 keeps the "add does not overwrite" rule; `HashMap::insert` is not used |
| `src/misc/sbuffer.h:19` | narrowing | `buf_del` is a swap-remove that evaluates to the **old** length, so a loop that deletes must re-test the same index; `src/event_loop.c:572-575` and `src/window_manager.c:1569-1572` compensate with `--i` and the other thirteen call sites `break` immediately | `Vec::swap_remove`, with the same `--i` compensation at those two sites and the same `break` at the rest |

#### Dead code removed under decision 5

One row per entry of §11: `src/misc/autorelease.h`, `src/yabai.c:158-162`,
`src/misc/timer.h:4-155` (including the 58 `TIME_FUNCTION` sites, `TIME_BLOCK`, `TIME_BODY`,
`PROFILER_END_TRANSLATION_UNIT`, `profile_begin`, `profile_end_and_print`),
`src/window.h:140`, `src/window_manager.h:117`, `src/message.c:429-437`,
`src/misc/helpers.h:321-326`, `src/misc/sbuffer.h:20`, `src/misc/hashtable.h:57-73`,
`src/misc/macros.h:6`, `:9`, `:15`, and the seven `src/misc/extern.h` declarations.

### 12.3 What is *not* a deviation

Do not add a row for any of these; they are reproduced exactly and a row would suggest they were
changed:

* `flags |= ~flags` setting all 64 bits (§6.5).
* The unsigned wrap in `window_manager_apply_grid` (§7.9) — unsigned wrap is defined in C.
* `purify_mode_str` being inverted relative to the enum names (§5.2).
* `require!` exiting `EXIT_SUCCESS` (§9.2).
* `AX_APPLICATION_ALL` covering only five of seven notification bits (§6.4).
* The `f32` timer deltas losing precision (§7.7).
* `snprintf` truncation at `MAXLEN`, wherever the C truncates rather than failing.
* The asymmetric `[`/`]` quirks in the query JSON (decision 3).
* `usleep` on the event-loop thread at `src/window_manager.c:1313`, `:2278`, `:2291`, `:2309` —
  blocking every pending event is the behaviour, not a defect to fix.
* `window_manager_move_window_relative`'s inverted guard at `src/window_manager.c:335`.
