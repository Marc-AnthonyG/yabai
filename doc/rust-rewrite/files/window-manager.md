# Phase 1 map — `src/window_manager.h` + `src/window_manager.c`

Reader: **window-manager**. Sources read in full: `src/window_manager.h` (216 lines),
`src/window_manager.c` (2765 lines). Every C location below is `path:line` against the
tree at commit `dd84572`.

Supporting headers read to resolve types used here (not owned by this reader, cited only as
context): `src/view.h`, `src/window.h`, `src/rule.h`, `src/application.h`, `src/space.h`,
`src/display.h`, `src/display_manager.h`, `src/space_manager.h`, `src/sa.h`,
`src/misc/hashtable.h`, `src/misc/sbuffer.h`, `src/misc/ts.h`, `src/misc/helpers.h`,
`src/misc/macros.h`, `src/misc/timer.h`, `src/misc/log.h`, `src/misc/extern.h`,
`src/event_loop.c`, `src/message.c`, `src/mouse_handler.c`, `src/yabai.c`, `src/manifest.m`.

---

## 0. Build context that constrains the translation

`src/manifest.m` is a unity build: it `#include`s every header (`manifest.m:45-78`) and then every
`.c`/`.m` in a fixed order (`manifest.m:80-97`). `window_manager.c` is included at `manifest.m:95`,
i.e. **after** `view.c`, `window.c`, `space_manager.c`, and **before** `mouse_handler.c` and
`yabai.c`.

Consequences the Rust port must handle explicitly:

- `window_manager.c:1-7` declares only 7 externs of its own. It also freely uses
  `g_window_manager`, `g_space_manager`, `g_connection` and `g_verbose`, which it never declares —
  it inherits those `extern` declarations from translation units included earlier
  (`view.c:1-4`, `window.c:1`). In Rust every module must name its dependencies; a `globals`
  module (or passing `&mut WindowManager` everywhere) has to be introduced.
- `window_manager.c:48` calls `window_nonax_serialize`, but `window.h:140` declares
  `window_unknown_serialize`. The definition is `window.c:121` `window_nonax_serialize`. The header
  declaration is dead/stale; only the unity build makes the call compile. **Phase 2 must use the
  `window.c` name**, and the stale declaration in `window.h` should not be carried into Rust.
- `window_manager.h:117` declares `window_manager_tile_window(struct window_manager *, struct window *)`.
  There is **no definition anywhere in the tree and no caller** (verified by grep over `src/`).
  Do not create a Rust function for it.
- `window_manager_animate_window_list_async` (`window_manager.c:603`) is non-`static` but is
  **not** declared in `window_manager.h`, and has no caller outside this file. In Rust it is a
  private `fn` of the module.

---

## 1. `src/window_manager.h`

### 1.1 Purpose

Declares the daemon-wide window-manager state (`struct window_manager`), the error enum returned by
every window operation, three small mode enums with their string tables, the scratchpad record, and
the ~110 public entry points implemented in `window_manager.c`. It is pure declaration: the only
executable content is three `static const char *[]` string tables, which become per-TU copies in C.

### 1.2 `#define` constants

| Name | Value | Meaning | Used at |
|---|---|---|---|
| `kCPSAllWindows` | `0x100` | `_SLPSSetFrontProcessWithOptions` mode bit | not used in this file |
| `kCPSUserGenerated` | `0x200` | mode bit for a user-initiated front-process switch | `window_manager.c:1320`, `:1329` |
| `kCPSNoWindows` | `0x400` | mode bit: front process with no window | `window_manager.c:1927`, `:2104`, `:2475` |

Rust: `const K_CPS_ALL_WINDOWS: u32 = 0x100;` etc. They are passed as the `mode` argument of
`_SLPSSetFrontProcessWithOptions(psn: *mut ProcessSerialNumber, wid: u32, mode: u32)`
(`misc/extern.h:81`), so the type is `u32`. `kCPSAllWindows` is unused — keep it only if another
reader's module needs it; otherwise drop it.

### 1.3 Enums and their string tables

#### `enum window_op_error` (`window_manager.h:8-24`)

14 variants, implicit values 0..13:
`WINDOW_OP_ERROR_SUCCESS`, `_INVALID_SRC_VIEW`, `_INVALID_SRC_NODE`, `_INVALID_DST_VIEW`,
`_INVALID_DST_NODE`, `_INVALID_OPERATION`, `_SAME_WINDOW`, `_CANT_MINIMIZE`, `_ALREADY_MINIMIZED`,
`_MINIMIZE_FAILED`, `_NOT_MINIMIZED`, `_DEMINIMIZE_FAILED`, `_MAX_STACK`, `_SAME_STACK`.

Consumers are in `message.c`, which switches on the value to pick an error string. The numeric
values themselves are **not** part of the socket protocol (the daemon writes text), so a Rust
`enum WindowOpError` without explicit discriminants is safe. Prefer
`Result<(), WindowOpError>` with `WINDOW_OP_ERROR_SUCCESS` modelled as `Ok(())`, but note that
`window_manager_resize_window_relative` returns `WINDOW_OP_ERROR_INVALID_DST_NODE` and
`mouse_handler.c:245,254` compares against that specific variant — the variant must stay
distinguishable.

#### `enum purify_mode` (`window_manager.h:26-31`) + `purify_mode_str[]` (`:33-38`)

`PURIFY_DISABLED=0, PURIFY_MANAGED=1, PURIFY_ALWAYS=2`.
**Trap:** the string table is *not* in enum order — it reads `{"on", "float", "off"}`, i.e.
`purify_mode_str[PURIFY_DISABLED] == "on"` (shadows on), `[PURIFY_MANAGED] == "float"`,
`[PURIFY_ALWAYS] == "off"`. This is deliberate (the user-facing setting is `window_shadow`).
A naive Rust `impl Display` that names the variants would change the daemon's query output.
Keep the table as a `const [&str; 3]` indexed by the discriminant.

#### `enum ffm_mode` (`window_manager.h:40-45`) + `ffm_mode_str[]` (`:47-52`)

`FFM_DISABLED=0, FFM_AUTOFOCUS=1, FFM_AUTORAISE=2`; strings `{"disabled","autofocus","autoraise"}`
— in order.

#### `enum window_origin_mode` (`window_manager.h:54-59`) + `window_origin_mode_str[]` (`:61-66`)

`WINDOW_ORIGIN_DEFAULT=0, WINDOW_ORIGIN_FOCUSED=1, WINDOW_ORIGIN_CURSOR=2`; strings
`{"default","focused","cursor"}` — in order.

All three tables are `static const char *[]` in a header: in the unity build there is exactly one
copy. In Rust: `const PURIFY_MODE_STR: [&str; 3] = [...];` in this module, `pub` so `message.c`'s
translation can read them.

### 1.4 `struct scratchpad` (`window_manager.h:68-72`)

| Field | C type | Owner of the pointee |
|---|---|---|
| `label` | `char *` | **owned by this struct.** Allocated by `string_copy` at `window_manager.c:160` (or by the caller in `message.c`), freed at `window_manager.c:2514`. The *same* pointer is aliased into `window->scratchpad` at `window_manager.c:2502` and cleared at `:2512`. |
| `window` | `struct window *` | **borrowed**; owned by `window.c`'s `window_create`/`window_destroy`. |

Rust: `struct Scratchpad { label: String, window: WindowId }` — see §3.9 for why the raw
`*mut window` alias should become an id.

### 1.5 `struct window_manager` (`window_manager.h:74-103`)

Exactly one instance exists: `g_window_manager`, defined at `yabai.c:30`.

| Field | C type | Contents / ownership | Threads that touch it |
|---|---|---|---|
| `system_element` | `AXUIElementRef` | Owned CF ref created at `window_manager.c:2711`, never released (process-lifetime). | main (create), never read in this file |
| `application` | `struct table` | key `pid_t` (4 bytes, copied into malloc'd key by `_table_add`), value `struct application *` (**borrowed**, owned by `application.c`). | event-loop; created on main at `window_manager.c:2727` |
| `window` | `struct table` | key `uint32_t` wid, value `struct window *` (**borrowed**, owned by `window.c`). | event-loop; also read by `event_loop.c:23` (`update_window_notifications`) on the event-loop thread |
| `managed_window` | `struct table` | key `uint32_t` wid, value `struct view *` (**borrowed**, owned by `space_manager`). | event-loop |
| `window_lost_focused_event` | `struct table` | key `uint32_t` wid, value is the sentinel `(void *)(intptr_t)1` (`window_manager.c:1391`) — a **set**, not a map. | event-loop |
| `application_lost_front_switched_event` | `struct table` | key `pid_t`, value sentinel `1` (`window_manager.c:1376`) — a **set**. | event-loop |
| `window_animations_table` | `struct table` | key `uint32_t` wid, value `struct window_animation *` pointing **into the malloc'd `context->animation_list` array** (`window_manager.c:673`). | **event-loop thread writes (`:662`, `:673`) and the CVDisplayLink thread removes (`:584`)** — guarded by `window_animations_lock` on both sides |
| `insert_feedback` | `struct table` | key `uint32_t` wid, value `struct window_node *`. Initialised here (`window_manager.c:2733`) but only used by `view.c` and `event_loop.c:28`. | event-loop |
| `window_animations_lock` | `pthread_mutex_t` | init at `window_manager.c:2734`, never destroyed. | event-loop + CVDisplayLink |
| `rules` | `struct rule *` | **stretchy buffer** (`misc/sbuffer.h`), owns its elements; each `struct rule` owns 5 heap strings + 4 `regex_t`, freed by `rule_destroy`. | event-loop |
| `applications_to_refresh` | `struct application **` | **stretchy buffer** of borrowed `struct application *`. Pushed at `window_manager.c:1716`, swap-removed at `:1733`; iterated by `space_manager_refresh_application_windows`. | event-loop |
| `focused_window_id` | `uint32_t` | 0 = none. Written by `event_loop.c:53`, read here at `:778`, `:813`, `:1303`, `:1922`, `:1995`, `:1997`, `:2031`, `:2033`, `:2099`. | event-loop (+ main at `window_manager.c:2761`) |
| `focused_window_psn` | `ProcessSerialNumber` | 8-byte Carbon struct (`{ long highLongOfPSN; long lowLongOfPSN; }`). Read at `:1297`, `:1304`; written at `:2762` and `event_loop.c:54`. | event-loop (+ main at init) |
| `last_window_id` | `uint32_t` | read at `:1051`; written at `:2760` and `event_loop.c:50`. | event-loop (+ main at init) |
| `enable_mff` | `bool` | mouse-follows-focus master switch; read `:249`. | event-loop |
| `ffm_mode` | `enum ffm_mode` | written `:229`. | event-loop |
| `purify_mode` | `enum purify_mode` | written `:766`, read `:881-885`. | event-loop |
| `window_origin_mode` | `enum window_origin_mode` | set at `:2716`; **never read in this file** (consumed by `space_manager.c`/`message.c`). | event-loop |
| `enable_window_opacity` | `bool` | `:234`, `:777`, `:789`. | event-loop |
| `menubar_opacity` | `float` | `:798`. | event-loop |
| `active_window_opacity` | `float` | `:778`, `:804`, `:2763`. | event-loop |
| `normal_window_opacity` | `float` | `:778`, `:811`, `:1493`. | event-loop |
| `window_opacity_duration` | `float` | `:784`. | event-loop |
| `window_animation_duration` | `float` | read `:610`, `:709`, `:722`; **saved/zeroed/restored** around a batch at `:2656-2676`. | event-loop |
| `window_animation_easing` | `int` (holds `enum animation_easing_type`) | `:611`, `:2724`. | event-loop |
| `insert_feedback_color` | `struct rgba_color` (`{u32 p; f32 r,g,b,a;}`, `misc/helpers.h:162`) | `:2725`; consumed by `view.c`. | event-loop |
| `scratchpad_window` | `struct scratchpad *` | **stretchy buffer**, owns each `label`. `:2433`, `:2498`, `:2510-2515`. | event-loop |

**Field-name trap:** `scratchpad_window` is singular but is a *list*. Do not translate it as an
`Option`.

### 1.6 Declaration-only items with no definition

- `window_manager_tile_window` (`window_manager.h:117`) — no definition, no caller. Drop.

---

## 2. `src/window_manager.c`

### 2.1 Purpose

The daemon's window bookkeeping and window-operation layer. It owns five hash tables that index
every tracked application/window/managed-window plus two "lost event" sets, applies user rules to
newly discovered windows, implements every `yabai -m window …` operation (move, resize, warp, swap,
stack, grid, float, sticky, minimize, fullscreen, scratchpad, layer, opacity, shadow), and hosts the
proxy-window animation engine that drives moves/resizes through a private SkyLight connection and a
`CVDisplayLink`.

### 2.2 Externs and globals reached from this file

Declared at the top of the file (`window_manager.c:1-7`):

| Symbol | Type | Defined at | Use here |
|---|---|---|---|
| `g_bs_port` | `mach_port_t` | `yabai.c:49` | `:440` `bootstrap_look_up` for jankyborders |
| `g_event_bytes` | `uint8_t *` | `yabai.c:42` (malloc 0x100, `yabai.c:141`) | `:1280-1290`, `:1298-1317` — **shared scratch buffer, mutated in place** |
| `g_event_loop` | `struct event_loop` | `yabai.c:34` | `:1467` `event_loop_post(WINDOW_FOCUSED, …)` |
| `g_workspace_context` | `void *` (ObjC object) | `yabai.c:35` | `:2753` |
| `g_process_manager` | `struct process_manager` | `yabai.c:28` | `:1927`, `:2104`, `:2475` (`finder_psn`), `:2740` (`process` table) |
| `g_mouse_state` | `struct mouse_state` | `yabai.c:33` | `:221`, `:224`, `:226` |
| `g_cv_host_clock_frequency` | `double` | `yabai.c:38`, set from `CVGetHostClockFrequency()` at `yabai.c:144` | `:545` |

Reached **without a local declaration** (inherited from earlier TUs in the unity build):

| Symbol | Defined at | Use here |
|---|---|---|
| `g_window_manager` | `yabai.c:30` | `:30-33`, `:577`, `:584`, `:610-611`, `:619`, `:631`, `:662`, `:673`, `:675`, `:709`, `:722`, `:833`, `:1297`, `:1303-1304` |
| `g_space_manager` | `yabai.c:31` | `:2503`, `:2519`, `:2536` |
| `g_connection` | `yabai.c:50` | `:255`, `:799`, `:841`, `:845`, `:950-955`, `:967-972`, `:981`, `:2116`, `:2459` |
| `g_verbose` | `yabai.c:51` (via `misc/log.h:4`) | `:1521`, `:1535`; also implicit inside `debug()` |

### 2.3 File-scope and function-local statics

There are **no file-scope variables** in `window_manager.c` other than the two static function
definitions. The only static *data* is:

| Name | Type / size | Initial value | Scope | Threads |
|---|---|---|---|---|
| `process_name` | `char[PROC_PIDPATHINFO_MAXSIZE]` = `char[4096]` | zero (BSS) | **function-local static** inside `window_manager_window_connection_is_jankyborders` (`window_manager.c:935`) | event-loop thread only (called from `:953`, `:970`) |

`process_name` is a function-local static purely to keep a 4 KiB buffer off the stack. It is
**not** re-initialised per call and `proc_name()` does not clear it on failure, so a failed
`proc_name` leaves the previous process' name in place and the `strcmp` at `:941` compares stale
data. Translate as a stack `[u8; 4096]` (or a `Vec`) — 4 KiB on the event-loop stack is fine and
removes the staleness bug; note that removing it is a (desirable) behaviour change.

Also static, but functions rather than data:
- `hash_wm` (`:9-12`) — `TABLE_HASH_FUNC`, returns `*(uint32_t *)key`. Identity hash.
- `compare_wm` (`:14-17`) — `TABLE_COMPARE_FUNC`, `*(uint32_t*)a == *(uint32_t*)b`.

Both are passed as function pointers to `table_init` (`:2727-2733`) and stored in every
`struct table`. Note both read the key as `uint32_t` even for the two `pid_t`-keyed tables; on
this target `pid_t` is `int32_t`, so the reinterpret is width-correct and sign only affects bucket
placement (`hash(key) % capacity` on an `unsigned long`), never correctness.

### 2.4 Threads in play, and how each was determined

| Thread | Entry point | How determined |
|---|---|---|
| **main run loop** | `main` (`yabai.c:261`) then `[NSApp run]` (`yabai.c:350`) | Direct read of `main`. Hosts the CGEventTap source (`mouse_handler.c:288` `CFRunLoopAddSource(CFRunLoopGetMain(), …)`), the `SLSRegisterConnectionNotifyProc` procs (`yabai.c:322-334`), Carbon `InstallEventHandler` (`process_manager.c:251`), NSNotification/KVO observers (`workspace.m`), and `dispatch_get_main_queue()` blocks. |
| **event-loop pthread** | `event_loop_run` (`event_loop.c:1647`), spawned at `event_loop.c:1718` from `event_loop_begin`, called at `yabai.c:291` | Direct read. It dequeues `struct event` and dispatches `EVENT_HANDLER_*` via the X-macro switch at `event_loop.c:1664-1668`. |
| **message-loop pthread** | `message_loop_run` (`message.c:3003`), spawned at `message.c:3042` | Direct read. It only does `accept()` then `event_loop_post(&g_event_loop, DAEMON_MESSAGE, NULL, sockfd)`. **It never calls into `window_manager`.** |
| **proxy-builder pthreads** | `window_manager_build_window_proxy_thread_proc` (`window_manager.c:507`), spawned at `:666`, joined at `:680` | Direct read. |
| **CVDisplayLink thread** | `window_manager_animate_window_list_thread_proc` (`window_manager.c:537`), registered `:701`, started `:702` | Direct read; CoreVideo runs output callbacks on its own display-link thread. |
| **forked child** | none in this file | grep: no `fork`/`posix_spawn`/`exec` in `window_manager.c`. |

**The key conclusion for phase 2:** because `DAEMON_MESSAGE` is handled inside the event loop
(`event_loop.c:1614-1644`, calling `handle_message` at `:1634`), **every socket command runs on the
event-loop pthread**, not on the message-loop pthread. Combined with the fact that the event tap
and the SLS notify procs only ever `event_loop_post`, this means:

> Every public `window_manager_*` function except `window_manager_init` and
> `window_manager_begin` runs **only** on the event-loop pthread.

`window_manager_init` (`yabai.c:336`) and `window_manager_begin` (`yabai.c:338`) run on the **main
thread**, before `[NSApp run]`. The event-loop pthread is already alive at that point
(`yabai.c:291`), but its queue can only be fed by main-run-loop callbacks, and the run loop has not
started yet — so in practice there is no concurrent access during init. Phase 2 can keep a
single-threaded `&mut WindowManager` for everything except the animation path, but the reasoning
above is the justification and should be preserved (see §6 open questions).

### 2.5 Function catalogue

Legend: **T** = thread. `EL` = event-loop pthread. `MAIN` = main thread (pre-`NSApp run`).
`PROXY` = proxy-builder pthread. `CVDL` = CVDisplayLink thread. "Allocates" lists heap/temp-storage
allocations and who frees them. "External symbols" lists non-yabai calls (libc / CoreFoundation /
ApplicationServices / SkyLight / CoreVideo / pthread).

---

**`static TABLE_HASH_FUNC(hash_wm)` → `unsigned long hash_wm(void *key)`** — `:9`
Identity hash of a `uint32_t` key. T: EL+MAIN+CVDL (invoked from inside `table_*`). Allocates:
nothing. External: none.

**`static TABLE_COMPARE_FUNC(compare_wm)` → `int compare_wm(void *a, void *b)`** — `:14`
`uint32_t` equality. T: same as above. Allocates: nothing. External: none.

**`bool window_manager_is_window_eligible(struct window *window)`** — `:19`
`window->is_root && (window_is_real(window) || rule_flag(MANAGED))`. T: EL. Allocates: nothing.
External: none.

**`void window_manager_query_window_rules(FILE *rsp)`** — `:25`
Serialises `g_window_manager.rules` as a JSON array into `rsp`. T: EL (query command). Allocates:
nothing (writes to the caller's `FILE*`, which `event_loop.c:1632` `fdopen`ed on the client socket
and `fclose`s at `:1637`). External: `fprintf`.

**`void window_manager_query_windows_for_spaces(FILE *rsp, uint64_t *space_list, int space_count, uint64_t flags)`** — `:38`
Serialises every window on the given spaces; falls back to `window_nonax_serialize` for windows not
in the table. T: EL. Allocates: `space_window_list_for_connection` returns **temp-storage (ts)**
memory reset by `event_loop.c:1671` at the end of the event. External: `fprintf`.

**`void window_manager_query_windows_for_display(FILE *rsp, uint32_t did, uint64_t flags)`** — `:54`
T: EL. Allocates: `display_space_list` → ts. External: `fprintf` (indirect).

**`void window_manager_query_windows_for_displays(FILE *rsp, uint64_t flags)`** — `:63`
Concatenates every display's space list **by relying on the ts bump allocator laying them out
contiguously** (see the carried-over comment at `:78-82`). T: EL. Allocates: ts. External: none
directly.

**`bool window_manager_rule_matches_window(struct rule *rule, struct window *window, char *window_title, char *window_role, char *window_subrole)`** — `:91`
Four `regex_match` tests with per-field include/exclude polarity. T: EL. Allocates: nothing.
External: `regexec` (via `regex_match`, `misc/helpers.h:573`).

**`void window_manager_apply_manage_rule_effects_to_window(…, struct rule_effects *effects)`** — `:108`
Applies only the `manage` property. T: EL. Allocates: nothing. External: none.

**`void window_manager_apply_rule_effects_to_window(…, struct rule_effects *effects)`** — `:119`
Applies space/display move, sticky, mff, layer, opacity, native fullscreen, scratchpad, grid.
T: EL. Allocates: `string_copy(effects->scratchpad)` at `:160` — **ownership moves into
`wm->scratchpad_window` on success, and is `free`d at `:162` on failure**. External:
`AXUIElementSetAttributeValue`, `free`.

**`void window_manager_apply_manage_rules_to_window(…, bool one_shot_rules)`** — `:171`
Combines matching `manage` rules; marks one-shots for removal. T: EL. Allocates: nothing.
External: none.

**`void window_manager_apply_rules_to_window(…, bool one_shot_rules)`** — `:195`
Same shape for the full effect set. T: EL. Allocates: nothing (the `struct rule_effects effects = {0}`
at `:198` is a stack value; its `scratchpad` field points into a rule and is only copied at `:160`).
External: none.

**`void window_manager_set_focus_follows_mouse(struct window_manager *wm, enum ffm_mode mode)`** — `:219`
Tears down and rebuilds the CGEventTap with a different mask. T: EL — **but it calls
`mouse_handler_end`/`mouse_handler_begin`, which do `CFRunLoopRemoveSource`/`CFRunLoopAddSource` on
`CFRunLoopGetMain()` (`mouse_handler.c:288`, `:299`) from the event-loop thread.** Allocates:
CF objects owned by `g_mouse_state`. External: `CGEventTapCreate`, `CFMachPortCreateRunLoopSource`,
`CFRunLoopAddSource`, `CFRunLoopRemoveSource`, `CFRelease`, `CFMachPortInvalidate`.

**`void window_manager_set_window_opacity_enabled(struct window_manager *wm, bool enabled)`** — `:232`
Iterates `wm->window` with `table_for`. T: EL. Allocates: nothing. External: none.

**`void window_manager_center_mouse(struct window_manager *wm, struct window *window)`** — `:242`
Warps the cursor to the window centre when mouse-follows-focus applies. T: EL (also reached from
`display_manager.c:468` and `event_loop.c:47`, both EL). Allocates: nothing. External:
`SLSGetCurrentCursorLocation`, `CGRectContainsPoint`, `CGDisplayBounds`, `CGWarpMouseCursorPosition`.

**`bool window_manager_should_manage_window(struct window *window)`** — `:272`
Five early-out predicates then the standard/level/movable test. T: EL. External: `window_is_sticky`
→ SkyLight (via `window.c`).

**`struct view *window_manager_find_managed_window(struct window_manager *wm, struct window *window)`** — `:283`
`table_find(&wm->managed_window, &window->id)`. T: EL (also `window.c:444`, EL). Allocates: nothing.

**`void window_manager_remove_managed_window(struct window_manager *wm, uint32_t wid)`** — `:288`
T: EL. Frees the table's malloc'd key + bucket (`hashtable.h:143-145`).

**`void window_manager_add_managed_window(struct window_manager *wm, struct window *window, struct view *view)`** — `:293`
No-op for `VIEW_FLOAT`; otherwise inserts and purifies. T: EL. Allocates: table bucket + 4-byte key
(malloc'd by `_table_add`, `hashtable.h:125-126`).

**`enum window_op_error window_manager_adjust_window_ratio(struct window_manager *wm, struct window *window, int type, float ratio)`** — `:300`
`TYPE_REL`/`TYPE_ABS` on `node->parent->ratio`, clamped to `[0.1, 0.9]`. T: EL. External: none.
**Note:** the `switch` at `:310` has no `default`; an out-of-range `type` silently skips the
assignment and still runs `window_node_update`/flush.

**`enum window_op_error window_manager_move_window_relative(struct window_manager *wm, struct window *window, int type, float dx, float dy)`** — `:330`
**Inverted guard:** returns `INVALID_SRC_VIEW` when the window *is* managed (`:335` `if (view) return`).
T: EL. Allocates: see `window_manager_animate_window`. External: none.

**`void window_manager_resize_window_relative_internal(struct window *window, CGRect frame, int direction, float dx, float dy, bool animate)`** — `:346`
Computes the new frame from a handle bitmask and applies it, animated or through the
`AX_ENHANCED_UI_WORKAROUND` macro. T: EL (also `event_loop.c:1272`, EL). External:
`AXUIElementCopyAttributeValue`/`SetAttributeValue` (inside the macro), `AXValueCreate`, `CFRelease`.

**`enum window_op_error window_manager_resize_window_relative(struct window_manager *wm, struct window *window, int direction, float dx, float dy, bool animate)`** — `:368`
Managed path adjusts BSP fences; floating path resizes directly. T: EL (also `mouse_handler.c:245,254`,
EL). Allocates: nothing here. External: as above.

**`void window_manager_move_window(struct window *window, float x, float y)`** — `:415`
T: EL (also `event_loop.c:1262`). Allocates: `AXValueCreate` CF ref, released at `:422`. External:
`AXValueCreate`, `AXUIElementSetAttributeValue`, `CFRelease`.

**`void window_manager_resize_window(struct window *window, float width, float height)`** — `:425`
Symmetric to the above. Same T/allocations.

**`static inline void window_manager_notify_jankyborders(struct window_animation *animation_list, int animation_count, uint32_t event, bool skip, bool wait)`** — `:437`
Builds a fixed 4104-byte message and sends it out-of-line over Mach to the `git.felix.jbevent`
bootstrap service. T: **EL** (`:653-654`, `:689`) **and CVDL** (`:579`). Allocates: the message is a
stack struct; `mach_send` uses `MACH_MSG_VIRTUAL_COPY` and `deallocate = false`
(`misc/helpers.h:216-219`). External: `bootstrap_look_up`, `mach_msg`, `usleep`.

**`static void window_manager_create_window_proxy(int animation_connection, float alpha, struct window_proxy *proxy)`** — `:463`
Creates a borderless SLS window, draws the captured image into its context. T: **PROXY** (via `:531`)
**and EL** (`:652`). Allocates: `CGSNewRegionWithRect` + `CGRegionCreateEmptyRegion` CF refs
released at `:485-486`; `proxy->id` (an SLS window) and `proxy->context` (a `CGContextRef`) are
released by `window_manager_destroy_window_proxy`. External: `CGSNewRegionWithRect`,
`CGRegionCreateEmptyRegion`, `SLSNewWindowWithOpaqueShapeAndContext`, `SLSWindowSetShadowProperties`
(via `sls_window_disable_shadow`), `SLSSetWindowOpacity`, `SLSSetWindowResolution`,
`SLSSetWindowAlpha`, `SLSSetWindowLevel`, `SLSSetWindowSubLevel`, `SLWindowContextCreate`,
`CGContextClearRect`, `CGContextDrawImage`, `CGContextFlush`, `CFRelease`.

**`static void window_manager_destroy_window_proxy(int animation_connection, struct window_proxy *proxy)`** — `:489`
Releases image, context and SLS window; nulls each field. T: **EL** (`:663`) **and CVDL** (`:585`).
External: `CFRelease`, `CGContextRelease`, `SLSReleaseWindow`.

**`static void *window_manager_build_window_proxy_thread_proc(void *data)`** — `:507`
Reads the live window's alpha/level/sub-level/bounds, hardware-captures its image, and builds the
proxy. T: **PROXY** — or **EL** synchronously at `:669` when `pthread_create` fails. Allocates:
`animation->proxy.image` — either `CFRetain` of an element of the captured array, or a fresh image
from `cgimage_restore_alpha` (`misc/helpers.h:588`, which itself `calloc`s a pixel buffer);
freed by `window_manager_destroy_window_proxy`. External: `SLSGetWindowAlpha`, `SLSGetWindowBounds`,
`SLSHWCaptureWindowList`, `CFRetain`, `CFArrayGetValueAtIndex`, `CFRelease`.

**`static CVReturn window_manager_animate_window_list_thread_proc(CVDisplayLinkRef link, const CVTimeStamp *now, const CVTimeStamp *output_time, CVOptionFlags flags, CVOptionFlags *flags_out, void *data)`** — `:537`
The per-frame animation step; on the final frame it swaps the real windows back in, tears the
proxies down, frees the context and stops/releases the display link **from inside its own
callback**. T: **CVDL**. Allocates: one `SLSTransactionCreate` per frame, released at `:574`;
`free(context->animation_list)` and `free(context)` at `:592-593`. External: `SLSTransactionCreate`,
`SLSTransactionSetWindowTransform`, `SLSGetWindowAlpha`, `SLSTransactionSetWindowAlpha`,
`SLSTransactionCommit`, `CFRelease`, `CGAffineTransformMakeTranslation/MakeScale/Concat`,
`pthread_mutex_lock/unlock`, `SLSDisableUpdate`, `SLSReenableUpdate`, `SLSReleaseConnection`,
`free`, `CVDisplayLinkStop`, `CVDisplayLinkRelease`.

**`void window_manager_animate_window_list_async(struct window_capture *window_list, int window_count)`** — `:603`
Sets up the whole animation: opens a private SLS connection, fills `animation_list`, either adopts
an in-flight animation's proxy state or spawns a builder thread per window, registers every entry in
`window_animations_table`, joins the builders, swaps proxies in via the scripting addition, applies
the final AX frames, then starts the display link. T: **EL**. Allocates: `malloc` for `context`
(`:605`) and `context->animation_list` (`:609`) — **freed on the CVDL thread at `:592-593`**;
`ts_alloc_list(pthread_t, window_count)` at `:615` (temp storage); the SLS connection at `:607`
released on CVDL at `:591`; the `CVDisplayLinkRef` at `:700` released on CVDL at `:596`.
External: `SLSNewConnection`, `SLSDisableUpdate`, `SLSReenableUpdate`, `malloc`, `memset`,
`pthread_mutex_lock/unlock`, `pthread_create`, `pthread_join`, `CFRetain`, `SLSGetWindowAlpha`,
`SLSTransactionCreate/OrderWindowGroup/SetWindowSystemAlpha/Commit`, `CFRelease`,
`CVDisplayLinkCreateWithActiveCGDisplays`, `CVDisplayLinkSetOutputCallback`, `CVDisplayLinkStart`,
`__asm__ __volatile__ ("" ::: "memory")` (`:648`).

**`void window_manager_animate_window_list(struct window_capture *window_list, int window_count)`** — `:705`
Dispatch: animate if `window_animation_duration != 0`, else set frames synchronously. T: EL (also
`view.c:378`, `mouse_handler.c:178,215`). External: none.

**`void window_manager_animate_window(struct window_capture capture)`** — `:718`
Single-window form; **takes the capture by value**. T: EL. External: none.

**`void window_manager_set_window_frame(struct window *window, float x, float y, float width, float height)`** — `:729`
Resize → move → resize, wrapped in `AX_ENHANCED_UI_WORKAROUND`. T: **EL** and, indirectly through
`:694`, still EL (the set-frame loop at `:692-696` runs before the display link starts). Allocates:
two `AXValueCreate` refs; **`size_ref` is created at `:747` and released only at `:759` inside
`if (size_ref)`; `position_ref` at `:744`, released at `:753`** — no leak, but the release sites are
not adjacent to the creations. External: `AXValueCreate`, `AXUIElementCopyAttributeValue`,
`AXUIElementSetAttributeValue`, `CFBooleanGetValue`, `CFRelease`.

**`void window_manager_set_purify_mode(struct window_manager *wm, enum purify_mode mode)`** — `:764`
T: EL. External: none.

**`bool window_manager_set_opacity(struct window_manager *wm, struct window *window, float opacity)`** — `:774`
`opacity == 0.0f` means "recompute from focus state". T: EL. External: none directly
(`scripting_addition_set_opacity` is yabai's own `sa.m`).

**`void window_manager_set_window_opacity(struct window_manager *wm, struct window *window, float opacity)`** — `:787`
Three guards then delegate. T: EL (also `event_loop.c:40,43,1607`). External: none.

**`void window_manager_set_menubar_opacity(struct window_manager *wm, float opacity)`** — `:796`
T: EL. External: `SLSSetMenuBarInsetAndAlpha`.

**`void window_manager_set_active_window_opacity(struct window_manager *wm, float opacity)`** — `:802`
T: EL. External: `_SLPSGetFrontProcess`, `GetProcessPID` (via `window_manager_focused_window`).

**`void window_manager_set_normal_window_opacity(struct window_manager *wm, float opacity)`** — `:809`
T: EL. Note the `continue` at `:813` continues the **inner bucket chain loop** of `table_for`
(`hashtable.h:36`), which is the intended semantics.

**`void window_manager_adjust_layer(struct window *window, int layer)`** — `:820`
No-op unless `window->layer == LAYER_AUTO`. T: EL (also `space_manager.c:134`, `mouse_handler.c:142`).

**`bool window_manager_set_window_layer(struct window *window, int layer)`** — `:827`
Sets the window's layer, then walks the SLS parent/child relation graph to re-layer every
descendant. T: EL. Allocates: **three C99 VLAs** `parent_list`, `child_list`, `check_list`, each
`uint32_t[window_count]` (`:849-850`, `:859`). External: `SLSCopyAssociatedWindows`,
`SLSWindowQueryWindows`, `SLSWindowQueryResultCopyWindows`, `SLSWindowIteratorAdvance`,
`SLSWindowIteratorGetParentID`, `SLSWindowIteratorGetWindowID`, `CFArrayGetCount`, `CFRelease`.
**Hazards:** (a) `window_count == 0` gives zero-length VLAs; (b) `check_list[check_count++]` at
`:866` is bounded by `window_count` only if the relation graph is a forest — a cycle or a repeated
child overflows the VLA; (c) `relation_count` can exceed `window_count` if the iterator yields more
rows than the array had entries.

**`void window_manager_purify_window(struct window_manager *wm, struct window *window)`** — `:877`
Shadow on/off per purify mode; mirrors the result into `WINDOW_SHADOW`. T: EL.

**`int window_manager_find_rank_of_window_in_list(uint32_t wid, uint32_t *window_list, int window_count)`** — `:898`
Returns the 0-based index, or `INT_MAX` when absent. The `rank` variable is redundant — it always
equals `i`. T: EL (only caller is `view.c:600`).

**`struct window *window_manager_find_window_on_space_by_rank_filtering_window(struct window_manager *wm, uint64_t sid, int rank, uint32_t filter_wid)`** — `:911`
**1-based** rank over the windows on `sid` that are in the table, skipping `filter_wid`. T: EL
(also `display_manager.c:465`). Allocates: `space_window_list` → ts.

**`static inline bool window_manager_window_connection_is_jankyborders(int window_cid)`** — `:933`
T: EL. Uses the function-local static described in §2.3. External: `SLSConnectionGetPID`,
`proc_name` (libproc), `strcmp`.

**`struct window *window_manager_find_window_at_point_filtering_window(struct window_manager *wm, CGPoint point, uint32_t filter_wid)`** — `:944`
Hit-tests below `filter_wid`, skipping yabai's own connection and jankyborders' overlay. T: EL.
External: `SLSFindWindowAndOwner`.

**`struct window *window_manager_find_window_at_point(struct window_manager *wm, CGPoint point)`** — `:961`
Same, starting from the top (`SLSFindWindowAndOwner(…, 0, 1, 0, …)`). T: EL.

**`struct window *window_manager_find_window_below_cursor(struct window_manager *wm)`** — `:978`
T: EL. External: `SLSGetCurrentCursorLocation`.

**`struct window *window_manager_find_closest_managed_window_in_direction(struct window_manager *wm, struct window *window, int direction)`** — `:985`
T: EL.

The next block is 17 near-identical tree/stack navigation helpers. All are T: EL, allocate nothing,
and call no external symbols; they only walk `struct view`/`struct window_node` and finish with
`window_manager_find_window`:

| Function | Line | One-liner |
|---|---|---|
| `window_manager_find_prev_managed_window` | `:999` | previous BSP leaf of the active space |
| `window_manager_find_next_managed_window` | `:1013` | next BSP leaf |
| `window_manager_find_first_managed_window` | `:1027` | first leaf from `view->root` |
| `window_manager_find_last_managed_window` | `:1038` | last leaf |
| `window_manager_find_recent_managed_window` | `:1049` | `wm->last_window_id`, only if still managed |
| `window_manager_find_prev_window_in_stack` | `:1060` | previous entry in `node->window_list` (loop starts at 1) |
| `window_manager_find_next_window_in_stack` | `:1077` | next entry (loop stops at `window_count-1`) |
| `window_manager_find_first_window_in_stack` | `:1094` | `window_list[0]`, only when `window_count > 1` |
| `window_manager_find_last_window_in_stack` | `:1105` | `window_list[count-1]`, only when `count > 1` |
| `window_manager_find_recent_window_in_stack` | `:1116` | `window_order[1]` |
| `window_manager_find_window_in_stack` | `:1127` | 1-based `index` into `window_list` |
| `window_manager_find_largest_managed_window` | `:1138` | max `area.w * area.h` over leaves |
| `window_manager_find_smallest_managed_window` | `:1157` | min area, `<=` so the **last** tie wins |
| `window_manager_find_sibling_for_managed_window` | `:1176` | the other child of `node->parent` |
| `window_manager_find_first_nephew_for_managed_window` | `:1190` | `sibling->left` |
| `window_manager_find_second_nephew_for_managed_window` | `:1204` | `sibling->right` |
| `window_manager_find_uncle_for_managed_window` | `:1218` | the other child of the grandparent |
| `window_manager_find_first_cousin_for_managed_window` | `:1235` | `uncle->left` |
| `window_manager_find_second_cousin_for_managed_window` | `:1252` | `uncle->right` |

**Integer trap in the two area functions:** `uint32_t area = node->area.w * node->area.h;` at `:1147`
and `:1166` multiplies two `float`s and truncates to `u32`. A negative or >4G product wraps. In Rust
`(node.area.w * node.area.h) as u32` reproduces the C truncation for in-range values but **saturates
instead of wrapping** for out-of-range ones. Keep `as u32` and note the difference; do not "fix" it
to `f32` comparison.

**`static void window_manager_make_key_window(ProcessSerialNumber *window_psn, uint32_t window_id)`** — `:1269`
Hand-builds two synthetic CGEvent records in `g_event_bytes` and posts them. T: EL. External:
`memset`, `memcpy`, `SLPSPostEventRecordTo`. Byte layout (must be preserved exactly):
`memset(bytes, 0, 0xf8)`; `[0x04] = 0xf8`; `[0x3a] = 0x10`; `memcpy(bytes+0x3c, &wid, 4)`;
`memset(bytes+0x20, 0xff, 0x10)`; then `[0x08] = 0x01` → post, `[0x08] = 0x02` → post.

**`void window_manager_focus_window_without_raise(ProcessSerialNumber *window_psn, uint32_t window_id)`** — `:1293`
When the target PSN is already frontmost, hand-posts a deactivate/activate pair with a 40 ms sleep
between them, then sets the front process and makes the window key. T: EL. External: `memset`,
`memcpy`, `SLPSPostEventRecordTo`, `usleep`, `_SLPSSetFrontProcessWithOptions`, `SameProcess`
(via `psn_equals`). Byte layout: `memset(bytes, 0, 0xf8)`; `[0x04]=0xf8`; `[0x08]=0x0d`;
`[0x8a]=0x02` + `memcpy(bytes+0x3c, &focused_window_id, 4)` → post to the focused PSN;
`usleep(40000)`; `[0x8a]=0x01` + `memcpy(bytes+0x3c, &window_id, 4)` → post to the target PSN.
**`usleep(40000)` blocks the whole event loop for 40 ms.**

**`void window_manager_focus_window_with_raise(ProcessSerialNumber *window_psn, uint32_t window_id, AXUIElementRef window_ref)`** — `:1324`
`#if 1` branch: set front process, make key, `AXUIElementPerformAction(kAXRaiseAction)`. The `#else`
branch (`:1333`, `scripting_addition_focus_window`) is dead code. T: EL (also `display_manager.c:443,467`).
External: `_SLPSSetFrontProcessWithOptions`, `AXUIElementPerformAction`.

**`struct application *window_manager_focused_application(struct window_manager *wm)`** — `:1339`
Wrapped in `#pragma clang diagnostic ignored "-Wdeprecated-declarations"` (`:1337-1338`, popped at
`:1362`) because `GetProcessPID` is deprecated. T: EL (+ MAIN via `window_manager_begin`). External:
`_SLPSGetFrontProcess`, `GetProcessPID`.

**`struct window *window_manager_focused_window(struct window_manager *wm)`** — `:1352`
T: EL (+ MAIN at `:2758`, and `space_manager.c:656`). External: as above plus
`AXUIElementCopyAttributeValue` inside `application_focused_window`.

Six trivial table wrappers, all T: EL, all allocating only the table's 4-byte key + bucket on `add`
and freeing them on `remove`:

| Function | Line | Table | Value |
|---|---|---|---|
| `window_manager_find_lost_front_switched_event` | `:1364` | `application_lost_front_switched_event` | `!= NULL` test |
| `window_manager_remove_lost_front_switched_event` | `:1369` | ″ | — |
| `window_manager_add_lost_front_switched_event` | `:1374` | ″ | sentinel `(void*)(intptr_t)1` |
| `window_manager_find_lost_focused_event` | `:1379` | `window_lost_focused_event` | `!= NULL` test |
| `window_manager_remove_lost_focused_event` | `:1384` | ″ | — |
| `window_manager_add_lost_focused_event` | `:1389` | ″ | sentinel `1` |

Six more for the `window` and `application` tables: `window_manager_find_window` (`:1394`),
`window_manager_remove_window` (`:1399`), `window_manager_add_window` (`:1404`),
`window_manager_find_application` (`:1409`), `window_manager_remove_application` (`:1414`),
`window_manager_add_application` (`:1419`). All T: EL (+ MAIN for `add_application` at `:2745`).

**`struct window **window_manager_find_application_windows(struct window_manager *wm, struct application *application, int *window_count)`** — `:1424`
Filters `wm->window` by owning application into a ts list sized `wm->window.count`. T: EL.
Allocates: `ts_alloc_list(struct window *, wm->window.count)` — temp storage, reset per event.

**`struct window *window_manager_create_and_add_window(struct space_manager *sm, struct window_manager *wm, struct application *application, AXUIElementRef window_ref, uint32_t window_id, bool one_shot_rules)`** — `:1438`
The window-discovery pipeline: create → title/role/subrole → reject AXUnknown → observe → replay a
lost focus event → insert → apply manage rules → apply rules → purify → opacity → decide float.
T: EL (+ MAIN via `window_manager_begin` → `add_existing_application_windows`).
**Ownership:** it **takes ownership of `window_ref`** (callers `CFRetain` at `:1561` and `:1639`, or
hand it a freshly created `element_ref` at `:1701`); on the two failure paths (`:1450`, `:1462`) it
calls `window_destroy`, which releases the ref. Allocates: `window_create` (heap `struct window`);
`window_title_ts`/`window_role_ts`/`window_subrole_ts` → temp storage. External: `debug`/`fprintf`,
`CFRelease` (indirect). Uses the `goto out` cleanup idiom (`:1495-1502` → `:1542`).

**`struct window **window_manager_add_application_windows(struct space_manager *sm, struct window_manager *wm, struct application *application, int *count)`** — `:1546`
Walks the AX window list, creates every unseen window, then garbage-collects one-shot rules.
T: EL. Allocates: `ts_alloc_list(struct window *, window_count)`; `CFRetain(window_ref)` at `:1561`
(handed off). Frees: `CFRelease(window_list)` at `:1576`; `rule_destroy` + `buf_del` at `:1568-1572`.
External: `CFArrayGetCount`, `CFArrayGetValueAtIndex`, `CFRetain`, `CFRelease`.
**Iteration trap:** `buf_del` is a **swap-remove** (`sbuffer.h:19`), and the loop compensates with
`--i; --rule_len;`. Also `buf_del(b,x)` evaluates to the *old* length (post-decrement), so
`if (buf_del(...))` is "the buffer was non-empty", not "an element was removed".

**`static uint32_t *window_manager_existing_application_window_list(struct application *application, int *window_count)`** — `:1580`
Concatenates every display's space list (same ts-contiguity trick, comment at `:1594-1598`) and asks
SkyLight for that connection's windows. `application == NULL` means "all connections". T: EL (+ MAIN).
Allocates: ts.

**`bool window_manager_add_existing_application_windows(struct space_manager *sm, struct window_manager *wm, struct application *application, int refresh_index)`** — `:1607`
Reconciles the AX window list against SkyLight's; when they disagree and this is the first attempt
(`refresh_index == -1`), brute-forces `_AXUIElementCreateWithRemoteToken` over element ids
`0 .. 0x7ffe` to materialise refs for windows on inactive spaces; otherwise it either clears the
application from `applications_to_refresh` or leaves it queued. T: EL (+ MAIN at `:2746`).
Allocates: `CFDataCreateMutable(NULL, 0x14)` at `:1668` released at `:1711`; `ts_buf_push` into
`app_window_list` (temp storage); `buf_push(wm->applications_to_refresh, …)` at `:1716`;
`CFRetain(window_ref)` at `:1639`. External: `CFArrayGetCount`, `CFArrayGetValueAtIndex`,
`CFDataCreateMutable`, `CFDataIncreaseLength`, `CFDataGetMutableBytePtr`, `memcpy`,
`_AXUIElementCreateWithRemoteToken`, `AXUIElementCopyAttributeValue`, `CFEqual`, `CFRelease`.
**Remote-token layout** (`:1671-1679`): a 0x14-byte buffer with `u32 pid` at `+0x0`,
`u32 0x636f636f` ("coco") at `+0x8`, and `u64 element_id` at `+0xc`. Bytes `+0x4` and `+0x10..0x14`
stay zero (`CFDataIncreaseLength` zero-fills). This must be reproduced byte-exactly.
**Leak:** `element_ref` is released only when `role` is non-NULL **and** `CFEqual(role, kAXWindowRole)`
**and** the wid did not match (`:1703`). When `role` is NULL, or the role is not `kAXWindowRole`, the
element ref leaks — up to ~32767 leaks per call. A Rust RAII port removes the leak; that is a
behaviour change worth recording (memory use only, not semantics).

**`enum window_op_error window_manager_set_window_insertion(struct space_manager *sm, struct window *window, int direction)`** — `:1748`
Sets/clears the BSP insertion point and its visual feedback. T: EL. **Unchecked NULL:** `view` from
`space_manager_find_view` is dereferenced at `:1754` without a NULL test (unlike `:1840`/`:1844`
which check `layout` on a possibly-NULL pointer too). External: none.

**`enum window_op_error window_manager_stack_window(struct space_manager *sm, struct window_manager *wm, struct window *a, struct window *b)`** — `:1799`
Un-tiles/un-floats `b` and stacks it onto `a`'s node. T: EL. **`a_node` from `view_find_window_node`
is dereferenced at `:1820` without a NULL check.** External: none directly.

**`enum window_op_error window_manager_warp_window(struct space_manager *sm, struct window_manager *wm, struct window *a, struct window *b)`** — `:1832`
Four cases: same parent + singleton (either re-insert at the insertion point or swap lists), same
space (the `:NaturalWarp` distance heuristic), different space (untile, move, retile). T: EL.
Allocates: `window_node_capture_windows` builds a `ts_buf` of `struct window_capture`.
External: `powf`. **Float/int mix at `:1896-1898`:** `CGPoint ca` components are `CGFloat` (f64) built
from `(int)(0.5f + …)` truncations of f32 expressions; then `powf` takes `float`. Translating all of
this to `f32` or all to `f64` changes which of `dcf`/`dcs` wins on ties. Reproduce the exact
widths: `(0.5f32 + x) as i32` then widen to `f64` for `ca`, and `powf` on `f32`.

**`enum window_op_error window_manager_swap_window(struct space_manager *sm, struct window_manager *wm, struct window *a, struct window *b)`** — `:1950`
Same-node case swaps positions inside one stack; cross-node case swaps the whole window lists and,
across spaces, re-homes every window. T: EL. Allocates: ts capture buffer. External: none.
**Note `:1968-2001`:** when both windows are in the same node, `a_view`/`b_view` may be NULL-derived
but are not dereferenced before the early `return`.

**`enum window_op_error window_manager_minimize_window(struct window *window)`** — `:2057` — T: EL.
External: `AXUIElementSetAttributeValue`.

**`enum window_op_error window_manager_deminimize_window(struct window *window)`** — `:2068` — T: EL.
External: `AXUIElementSetAttributeValue`.

**`bool window_manager_close_window(struct window *window)`** — `:2078`
Presses the AX close button. T: EL. Allocates: `button` CF ref released at `:2087`. External:
`AXUIElementCopyAttributeValue`, `AXUIElementPerformAction`, `CFRelease`.

**`void window_manager_send_window_to_space(struct space_manager *sm, struct window_manager *wm, struct window *window, uint64_t dst_sid, bool moved_by_rule)`** — `:2092`
Refocuses a neighbour if the window was focused/visible, untiles, moves, retiles. T: EL. External:
`SLSSpaceSetFrontPSN`, `_SLPSSetFrontProcessWithOptions`.

**`enum window_op_error window_manager_apply_grid(struct space_manager *sm, struct window_manager *wm, struct window *window, unsigned r, unsigned c, unsigned x, unsigned y, unsigned w, unsigned h)`** — `:2124`
Clamps the grid cell, subtracts padding/gaps, computes the frame, animates. T: EL. External: none
directly. **Unsigned traps at `:2134-2139`:** `if (w <= 0) w = 1;` and `if (h <= 0) h = 1;` are
**always false** for `unsigned` (clang warns; the code relies on it). `if (x >= c) x = c - 1;` with
`c == 0` yields `x = UINT_MAX`, and `cw = bounds.size.width / c` then divides by zero (float → inf).
In Rust with `u32` these become a compile warning and a **panic** (`0u32 - 1` in debug). Phase 2 must
either keep the exact wrapping semantics (`c.wrapping_sub(1)`) or reject `r == 0 || c == 0` earlier
and document the divergence.

**`void window_manager_make_window_floating(struct space_manager *sm, struct window_manager *wm, struct window *window, bool should_float, bool force)`** — `:2181` — T: EL.

**`void window_manager_make_window_sticky(struct space_manager *sm, struct window_manager *wm, struct window *window, bool should_sticky)`** — `:2215` — T: EL.

**`void window_manager_toggle_window_shadow(struct window *window)`** — `:2245` — T: EL.

**`void window_manager_wait_for_native_fullscreen_transition(struct window *window)`** — `:2259`
**Spin-waits with `usleep(100000)` on the event-loop thread** until the space/display settles. Two
strategies depending on the macOS version. T: EL. External: `usleep`.

**`void window_manager_toggle_window_native_fullscreen(struct window *window)`** — `:2296`
Focuses the window, spin-waits for the space to match (`:2309`), toggles `kAXFullscreenAttribute`,
spin-waits again. T: EL. External: `AXUIElementSetAttributeValue`, `usleep`.
**`uint32_t sid = window_space(window->id);` at `:2300` truncates a `uint64_t` space id to 32 bits**,
then compares it against the full 64-bit `space_manager_active_space()` at `:2309`. If the high half
is ever non-zero this loop never terminates. Reproduce the truncation faithfully (`as u32`) or flag
it; do not silently widen.

**`void window_manager_toggle_window_zoom_parent(struct window_manager *wm, struct window *window)`** — `:2326`
T: EL. Contains `assert(node)` at `:2334`.

**`void window_manager_toggle_window_zoom_fullscreen(struct window_manager *wm, struct window *window)`** — `:2355`
T: EL. `assert(node)` at `:2363`.

**`void window_manager_toggle_window_windowed_fullscreen(struct window *window)`** — `:2384`
Saves/restores `window->windowed_frame`. T: EL.

**`void window_manager_toggle_window_expose(struct window *window)`** — `:2402`
T: EL. External: `CoreDockSendNotification(CFSTR("com.apple.expose.front.awake"), 0)`.

**`void window_manager_toggle_window_pip(struct space_manager *sm, struct window *window)`** — `:2410`
T: EL.

**`static inline struct window *window_manager_find_scratchpad_window(struct window_manager *wm, char *label)`** — `:2431`
Linear scan of `wm->scratchpad_window` by label. T: EL. External: `strcmp` via `string_equals`.

**`bool window_manager_toggle_scratchpad_window_by_label(struct window_manager *wm, char *label)`** — `:2442` — T: EL.

**`bool window_manager_toggle_scratchpad_window(struct window_manager *wm, struct window *window, int forced_mode)`** — `:2448`
**The hardest control flow in the file.** A `switch (forced_mode)` at `:2461` `goto`s **into the
middle of an if/else-if/else chain**: `mode_0:` (`:2468`) is in front of the chain, `mode_1:`
(`:2470`) is the first branch's body, `mode_2:` (`:2479`) the second's, `mode_3:` (`:2483`) the
third's. `forced_mode == 0` evaluates the conditions normally; 1/2/3 force a branch and skip the
conditions. T: EL. External: `SLSWindowIsOrderedIn`, `_SLPSSetFrontProcessWithOptions`.

**`bool window_manager_set_scratchpad_for_window(struct window_manager *wm, struct window *window, char *label)`** — `:2492`
**Takes ownership of `label` on success** (stores it in the buffer at `:2498-2501` and aliases it
into `window->scratchpad` at `:2502`); returns `false` without taking ownership when the label is
already used. T: EL.

**`bool window_manager_remove_scratchpad_for_window(struct window_manager *wm, struct window *window, bool unfloat)`** — `:2508`
Clears the alias, `free`s the label, swap-removes the entry, optionally re-floats. T: EL.
External: `free`.

**`void window_manager_scratchpad_recover_windows(void)`** — `:2529`
T: EL. Allocates: ts list.

**`static void window_manager_validate_windows_on_space(struct window_manager *wm, struct view *view, uint32_t *window_list, int window_count)`** — `:2540`
O(n·m) removal of windows that are in the view but no longer on the space; batches via `VIEW_IS_DIRTY`.
T: EL. Allocates: `view_find_window_list` → ts.

**`static void window_manager_check_for_windows_on_space(struct window_manager *wm, struct view *view, uint32_t *window_list, int window_count)`** — `:2579`
The mirror: adds windows that are on the space but not in the view. T: EL.

**`void window_manager_validate_and_check_for_windows_on_space(struct space_manager *sm, struct window_manager *wm, uint64_t sid)`** — `:2625`
Runs both, then flushes once if dirty and visible. T: EL (also `space_manager.c:209`). **`view` is
dereferenced at `:2628` without a NULL check.** Allocates: ts.

**`void window_manager_correct_for_mission_control_changes(struct space_manager *sm, struct window_manager *wm)`** — `:2650`
Temporarily zeroes `wm->window_animation_duration` (`:2656-2657`, restored `:2676`) so the
reconciliation applies frames synchronously, then walks every display/space. T: EL.
**The save/restore is not exception- or early-return-safe**; there is no early return today, but a
Rust translation should use a guard type (`RestoreAnimationDurationOnDrop`) to keep it that way.

**`void window_manager_handle_display_add_and_remove(struct space_manager *sm, struct window_manager *wm, uint32_t did)`** — `:2679`
Adopts windows on the display's first user space, then refreshes/invalidates views. T: EL.
`view` dereferenced at `:2691` without a NULL check.

**`void window_manager_init(struct window_manager *wm)`** — `:2709`
Creates the system-wide AX element (1.0 s messaging timeout), sets every default, initialises the
seven tables (capacity 150 each) and the mutex. **T: MAIN** (`yabai.c:336`). Allocates:
`AXUIElementCreateSystemWide` (never released), 7 × `malloc(sizeof(struct bucket*) * 150)` inside
`table_init`. External: `AXUIElementCreateSystemWide`, `AXUIElementSetMessagingTimeout`,
`pthread_mutex_init`, `malloc`, `memset`.

**`void window_manager_begin(struct space_manager *sm, struct window_manager *wm)`** — `:2737`
Walks `g_process_manager.process`, creates+observes an `struct application` per observable process,
imports its windows, then seeds the focus state. **T: MAIN** (`yabai.c:338`). Allocates:
`NSAutoreleasePool` (`:2739`, drained `:2756`), one `struct application` per process (owned by
`wm->application` on success, destroyed at `:2749` on failure). External: ObjC message sends
`[[NSAutoreleasePool alloc] init]` and `[pool drain]`.

### 2.6 Callbacks registered with the OS or a run loop

This file registers exactly **one** OS callback, plus one pthread entry point.

| # | Kind | Registration site | Callback | Thread it fires on | Context pointer | Context lifetime |
|---|---|---|---|---|---|---|
| 1 | `CVDisplayLink` output callback | `window_manager.c:701` `CVDisplayLinkSetOutputCallback(link, window_manager_animate_window_list_thread_proc, context)`, started at `:702` | `window_manager_animate_window_list_thread_proc` (`:537`) | **CVDisplayLink thread** (CoreVideo-owned) | `struct window_animation_context *` malloc'd at `:605` | From `:605` until the callback itself `free`s it at `:593` on the final frame. The `CVDisplayLinkRef` is also released by the callback at `:596`, from inside the callback. **No other code holds a reference to either**, so a late/duplicate final frame would be a use-after-free. |
| 2 | pthread entry | `window_manager.c:666` `pthread_create(&thread, NULL, &window_manager_build_window_proxy_thread_proc, &context->animation_list[i])` | `window_manager_build_window_proxy_thread_proc` (`:507`) | a fresh pthread per window | `struct window_animation *` — an **interior pointer into the malloc'd `animation_list` array** | Valid for the whole animation; the spawning thread `pthread_join`s every one of them at `:680` before touching the same elements again, so the interior pointers are not dangling. But see §3.11: the same element is simultaneously reachable through `window_animations_table`. |

Callbacks this file *does not* register but is *reached from*: the CGEventTap (`mouse_handler.c:278`,
main run loop → posts to the event loop), the SLS connection notify procs (`yabai.c:322-334`, main
run loop → post), Carbon app events (`process_manager.c:251`, main run loop → post), and the
NSWorkspace/KVO observers in `workspace.m` (main run loop → post). None of them call
`window_manager_*` directly; they all funnel through `event_loop_post`. That is the single most
important fact for the Rust threading model.

There are **no** `AXObserver` registrations, signal handlers, `setjmp`, `fork`/`exec`, or dispatch
blocks in this file.

### 2.7 Shared mutable state and today's synchronisation

| State | Written by | Read by | Synchronisation today |
|---|---|---|---|
| `g_window_manager.window_animations_table` | EL (`:662` remove, `:673` add) | CVDL (`:584` remove) | `window_animations_lock` held on both sides (`:619`/`:675`, `:577`/`:589`) |
| `struct window_animation::skip` (`view.h:75`, `volatile bool`) | EL `__atomic_store_n(…, __ATOMIC_RELEASE)` (`:633`) | EL (`:449`, `:558`) and CVDL (`:558`, `:582`) via `__atomic_load_n(…, __ATOMIC_RELAXED)` | atomics only; no lock |
| `struct window_proxy::tx/ty/tw/th` of an **in-flight** animation | CVDL, every frame, **without the lock** (`:560-563`) | EL, under the lock (`:635-642`) | **none — this is a genuine data race in the C code.** The lock does not cover the CVDL writer. |
| `g_temp_storage.used` (`misc/ts.h:7`) | EL, PROXY (none here), CVDL (none) | — | `__sync_bool_compare_and_swap` / `__sync_fetch_and_add`; reset only by EL at `event_loop.c:1671` |
| `g_event_bytes` (256-byte global) | EL (`:1280-1290`, `:1298-1317`) | EL | none needed — single thread |
| everything else in `struct window_manager` | EL (+ MAIN during init) | EL | none — single thread by construction |

---

## 3. C pattern catalogue → recommended Rust translation

### 3.1 Hash tables keyed by an integer id (`struct table`)

**Sites:** `window_manager.c:9-17` (hash/compare), `:2727-2733` (7 × `table_init`), and every
`table_add`/`table_remove`/`table_find`/`table_for` call. Implementation in `misc/hashtable.h`:
separate chaining, `max_load = 0.75`, doubling rehash, **the table `malloc`s a copy of the key**
(`hashtable.h:126-128`) and frees it on remove (`:143`); the **value is an opaque borrowed pointer**.

**Rust:** `HashMap<u32, V>` / `HashMap<pid_t, V>`, with `V` chosen per table:

| C table | Rust |
|---|---|
| `application` | `HashMap<i32, Box<Application>>` or `HashMap<i32, ApplicationId>` into a slab |
| `window` | `HashMap<u32, Box<Window>>` / slab |
| `managed_window` | `HashMap<u32, ViewId>` (**not** `&View` — see §3.9) |
| `window_lost_focused_event` | `HashSet<u32>` |
| `application_lost_front_switched_event` | `HashSet<i32>` |
| `window_animations_table` | see §3.11 — `HashMap<u32, usize>` (index into the animation list) behind the mutex |
| `insert_feedback` | `HashMap<u32, NodeId>` |

**Behaviour a naive translation changes:**
- **Iteration order.** `table_for` (`hashtable.h:34-41`) iterates bucket index ascending, then chain
  order (append-at-tail, so insertion order within a bucket). With the identity hash this is
  "id mod capacity" order — deterministic across runs. `std::collections::HashMap` iterates in a
  **randomised, per-process** order. Where order is observable — `event_loop.c:23-25`
  (`update_window_notifications` fills a fixed `uint32_t[1024]`, so which windows survive a >1024
  overflow depends on order) and `window_manager_find_application_windows` (`:1429`) — use
  `indexmap::IndexMap` or a `BTreeMap<u32, _>` to keep a deterministic order. Recommend
  `HashMap<_, _, BuildHasherDefault<IdHasher>>` where `IdHasher` is the identity hash, **plus**
  documenting that order is not the same as C's.
- **The sentinel value `(void *)(intptr_t) 1`** (`:1376`, `:1391`) is not a pointer; `HashSet` is
  the right shape and removes the cast entirely.
- `_table_add` **does not overwrite an existing non-NULL value** (`hashtable.h:120-123`);
  `HashMap::insert` does. Where re-adding an existing key can happen (`:296`, `:673`), use
  `entry().or_insert(...)` to preserve "first write wins".

`unsafe`: none, once values are ids/`Box`es rather than borrowed raw pointers.

### 3.2 Stretchy buffers (`buf_push` / `buf_del` / `buf_len`)

**Sites:** `wm->rules` (`:30-33`, `:176`, `:200`, `:1565-1573`), `wm->applications_to_refresh`
(`:1716`, `:1733`), `wm->scratchpad_window` (`:2433`, `:2498`, `:2510-2515`). Implementation
`misc/sbuffer.h:11-32`: a `{int len; int cap; char buf[0];}` header immediately before the elements,
`realloc`-grown, `buf_free` frees the header.

**Rust:** `Vec<T>`.

**Behaviour a naive translation changes:**
- **`buf_del(b, x)` is `Vec::swap_remove`, not `Vec::remove`.** It writes the last element over
  index `x` and decrements the length (`sbuffer.h:19`). Every call site depends on that:
  `:1569` compensates with `--i; --rule_len;` so the swapped-in rule gets re-examined; `:1733`
  invalidates the caller's `refresh_index`; `:2515` reorders the scratchpad list. Using
  `Vec::remove` would silently change which rules get evaluated.
- **`buf_del` evaluates to the *old* length** (post-decrement inside a comma expression). The
  `if (buf_del(wm->rules, i))` at `:1569` therefore means "the buffer was non-empty", which is
  always true there. In Rust just call `swap_remove` and take the branch unconditionally.
- `buf_len` is `int`, not `usize`. Negative never happens, but the `for (int i = 0; i < buf_len(...))`
  loops re-evaluate `buf_len` each iteration (`:30`, `:176`, `:200`, `:2433`, `:2510`) — relevant
  only if the body mutated the buffer, which it does not.

`unsafe`: none.

### 3.3 Arena / temp-storage allocation (`ts_alloc_list`, `ts_buf_push`, `*_ts` helpers)

**Sites:** `ts_alloc_list(pthread_t, window_count)` (`:615`),
`ts_alloc_list(struct window *, …)` (`:1427`, `:1553`), `ts_buf_push(app_window_list, …)` (`:1652`),
`ts_buf_del` (`:1694`), `window_title_ts`/`window_role_ts`/`window_subrole_ts` (`:1442-1444`), and
every `space_window_list*`/`display_space_list`/`view_find_window_list` return value.

`misc/ts.h` is a lock-free bump allocator over an 8 MiB `mmap` region with a `PROT_NONE` guard page
(`ts.h:20-23`), bumped with `__sync_bool_compare_and_swap` (`ts.h:59`), **reset to zero once per
event by `event_loop.c:1671`**, and `exit(EXIT_FAILURE)` on overflow (`ts.h:31-32`).

**Rust:** the safe, faithful translation is **plain `Vec`/`String` owned by the caller**, returned by
value, because the "reset at the end of each event" lifetime maps exactly onto "dropped at the end of
the handler". The cross-function contiguity trick at `:73-88` and `:1589-1602` (concatenating several
`display_space_list` results by keeping only the first pointer and summing the counts — see the
comments at `:78-82` and `:1594-1598`) **does not survive** that translation: those two loops must be
rewritten to `let mut space_list = Vec::new(); for did in … { space_list.extend(display_space_list(did)); }`.
That is a real structural change, not a mechanical one, and it is the single place where the
ts-allocator's semantics are load-bearing.

If a bump arena is wanted anyway (for allocation-rate parity), `bumpalo::Bump` with a
`&'event Bump` threaded through the handler signature is the closest match; but the `exit()` on
overflow and the 8 MiB cap should be dropped.

`unsafe`: none with `Vec`.

### 3.4 `goto out` cleanup

**Sites:** `window_manager.c:1495-1502` → `out:` at `:1542` (the only true one in this file);
`:575` `if (t != 1.0f) goto out;` → `out:` at `:598` (an early *return*, not cleanup);
`:2462-2465` (the scratchpad `goto`s, §3.13).

**Rust:** the `:1495-1502` cluster is a chain of "if condition, stop configuring but still return
the window" — translate to an early `return Some(window)` from an inner helper, or to a labelled
block `'configure: { … break 'configure; … }`. The `:575` one is `return CVReturn::Success` directly.

### 3.5 Bit flags

**Sites:** `window_check_flag`/`set`/`clear` (`window.h:128-134`) used at `:275`, `:277`, `:891-893`,
`:1496-1497`, `:1510`, `:1514`, `:1529`, `:1813-1816`, `:2202-2206`, `:2229-2235`, `:2249-2254`,
`:2391-2395`; `window_check_rule_flag` family (`window.h:132-134`) at `:21`, `:111-114`, `:138-142`,
`:203`, `:244-245`, `:280`, `:1498-1501`, `:2189`; `rule_check_flag`/`rule_set_flag`
(`rule.h:59-61`) at `:93-103`, `:177-187`, `:201-211`, `:1567`; `rule_effects_check_flag`
(`rule.h:63-65`) at `:125`, `:145`, `:149`; `view_check_flag`/`set_flag`/`clear_flag`
(`view.h:218-220`) at `:324`, `:2044-2050`, `:2145`, `:2152`, `:2421`, `:2574`, `:2602`, `:2620`,
`:2646`; and the raw handle mask `direction & HANDLE_LEFT` etc. (`:350-351`, `:355-356`, `:382-385`).

**Rust:** `bitflags::bitflags!` for `WindowFlag` (`u8`), `WindowRuleFlag` (`u8`), `RuleFlag` (`u16`),
`RuleEffectsFlag` (`u16`), `ViewFlag` (`u64`), `HandleDirection` (`u8`). The underlying integer
widths must be kept: `window->flags` and `rule_flags` are `uint8_t` (`window.h:101-102`),
`rule->flags` and `rule_effects->flags` are `uint16_t` (`rule.h:41`, `:56`), `view->flags` is
`uint64_t` (`view.h:215`).

**Behaviour a naive translation changes:** `window_check_flag` returns `bool` from `w->flags & x`
(an implicit `!= 0`); `view_check_flag` is a **macro returning the masked integer**
(`view.h:218`), used only in boolean position here. Both map to `.contains()`.

### 3.6 X-macro generated code

**Sites in this file:** `:551-554`

```c
#define ANIMATION_EASING_TYPE_ENTRY(value) case value##_type: mt = value(t); break;
    ANIMATION_EASING_TYPE_LIST
#undef ANIMATION_EASING_TYPE_ENTRY
```

expanding `misc/helpers.h:4-25` into 21 `case` arms, one per easing function.

**Rust:** a `#[repr(i32)] enum AnimationEasing` with 21 variants plus a `match` that calls the
corresponding `fn(f32) -> f32`, or a `const EASING_FN: [fn(f32) -> f32; 21]` indexed by the
discriminant. A macro is unnecessary — the list is fixed.

**Behaviour a naive translation changes:** the C `switch` has **no `default`**, so an out-of-range
`context->animation_easing` leaves `mt` **uninitialised** and the frame uses garbage. Rust forces
exhaustiveness. Recommend `AnimationEasing::try_from(i32)` at the boundary where `message.c` sets
the value, and an `unreachable!()`/`ease_out_circ` fallback here; either way, document that the
uninitialised-`mt` path is gone.

### 3.7 `printf` into a `FILE *` response stream

**Sites:** `:29`, `:33`, `:35` (`window_manager_query_window_rules`); `:45`, `:49`, `:51`
(`query_windows_for_spaces`); `:1522-1524`, `:1536-1538` (`fprintf(stdout, …)` behind `g_verbose`);
plus every `debug(...)` call (`:1445`, `:1448`, `:1459`, `:1513`, `:1528`, `:1657`, `:1715`, `:1718`,
`:1732`, `:1738`, `:2752`), which is `vfprintf(stdout, …)` gated on `g_verbose` (`misc/log.h:6-15`).

**Rust:** the response stream is a `std::fs::File`/`UnixStream` obtained from the accepted socket fd
(`event_loop.c:1632` `fdopen(param1, "w")`). Translate `FILE *rsp` to a
`&mut dyn std::io::Write` (or a concrete `BufWriter<UnixStream>`) and `fprintf` to `write!`.

**Behaviour a naive translation changes:**
- C's `fprintf` is buffered and flushed at `event_loop.c:1636`; a Rust `UnixStream` is unbuffered by
  default, producing many more `write(2)` calls. Wrap in `BufWriter` and flush explicitly to keep
  the same syscall shape and, more importantly, the same "one response, then close" framing.
- `fprintf` failures are ignored in C. `write!` returns `Result`; swallowing it with `let _ =` keeps
  behaviour identical. **Do not propagate the error** — a client that hangs up mid-response must not
  abort the command, or behaviour changes.
- The JSON is assembled by hand with `[`, `,`, `]` and a trailing `\n` (`:29-35`, `:45-51`). Keep the
  manual assembly; switching to `serde_json` changes key order and whitespace and therefore the
  wire format.

### 3.8 Fixed char arrays and NUL handling

**Sites:** `static char process_name[PROC_PIDPATHINFO_MAXSIZE]` (`:935`) filled by `proc_name` and
compared with `strcmp` (`:941`).

**Rust:** `let mut process_name = [0u8; PROC_PIDPATHINFO_MAXSIZE];` then
`libc::proc_name(pid, ptr as *mut c_void, len as u32)`; on success take
`CStr::from_bytes_until_nul(&process_name)` and compare to `"borders"`. `unsafe` is unavoidable for
the `proc_name` FFI call itself.

**Behaviour a naive translation changes:** as noted in §2.3, the C code compares stale contents when
`proc_name` fails (returns `<= 0`). A Rust version that checks the return value and returns `false`
on failure is more correct but different. Record the choice.

### 3.9 Raw pointers used as cross-structure links

**Sites:** `wm->managed_window` stores `struct view *` (`:296`); `wm->window` stores `struct window *`;
`wm->applications_to_refresh` stores `struct application *`; `struct scratchpad::window` stores
`struct window *` (`window_manager.h:71`); `window->scratchpad` aliases the `char *` owned by
`wm->scratchpad_window[i].label` (`:2502` / `:2512-2514`); `struct window_animation::window` stores
`struct window *` (`view.h:70`, set at `:621`) and is dereferenced on the event-loop thread at `:694`.

**Rust:** replace every borrowed raw pointer with an **id** (`WindowId(u32)`, `SpaceId(u64)`,
`Pid(i32)`) and keep the owning collection separate. `struct scratchpad` becomes
`{ label: String, window: WindowId }`; `window.scratchpad` becomes `Option<String>` (a clone — the
labels are short and set rarely) or `Option<Rc<str>>` shared with the list entry. `Rc<str>` is the
closer match to the C aliasing and keeps the "free once" behaviour of `:2514`.

**Behaviour a naive translation changes:** the C code can hold a `struct view *` in
`managed_window` that a later `view_destroy` invalidates; the id-based version turns that into a
lookup miss instead of a dangling read. That is strictly better but changes what happens on the
(rare) stale-view path — worth a note.

`unsafe`: none.

### 3.10 CF retain/release pairing

**Sites (create → release):** `AXValueCreate` `:418`→`:422`, `:428`→`:432`, `:743`→`:753`,
`:747`→`:759`; `CGSNewRegionWithRect`+`CGRegionCreateEmptyRegion` `:468-469`→`:485-486`;
`SLSHWCaptureWindowList` array `:521`→`:526` with `CFRetain` of an element at `:524`;
`cgimage_restore_alpha` result `:525` (owned) → released at `:492`; `SLSTransactionCreate`
`:556`→`:574`, `:656`→`:660`; `SLSWindowQueryWindows`/`…CopyWindows`/`SLSCopyAssociatedWindows`
`:841-846`→`:870-872`; `AXUIElementCopyAttributeValue` button `:2083`→`:2087`;
`application_window_list` `:1549`→`:1576` and `:1615`→`:1743`; `CFRetain(window_ref)` `:1561`,
`:1639` (handed to `window_manager_create_and_add_window`); `CFDataCreateMutable` `:1668`→`:1711`;
`_AXUIElementCreateWithRemoteToken` `:1680` → released only at `:1703` (see the leak in §2.5);
`AXUIElementCopyAttributeValue` role `:1683`→`:1707`; `AXUIElementCreateSystemWide` `:2711` → never.
Also `CFRetain(existing_animation->proxy.image)` at `:646`.

**Rust:** `core-foundation` / `core-graphics` crates' `TCFType` wrappers
(`CFRetain` on `wrap_under_get_rule`, `CFRelease` on `Drop`) for the typed CF objects, and a
hand-rolled `struct OwnedCfRef<T>(*const c_void)` with a `Drop` impl for the SkyLight `CFTypeRef`
returns that have no crate binding (`SLSTransactionCreate`, `SLSWindowQueryWindows`,
`CGRegionCreateEmptyRegion`). `AXUIElementRef` needs its own wrapper —
`accessibility-sys`/`core-foundation` do not cover the private `_AXUIElementCreateWithRemoteToken`.

**Behaviour a naive translation changes:** RAII fixes the `:1680` leak and makes the release order at
`:870-872` deterministic (`query` before `iterator`, whereas Drop order would release in reverse
declaration order). The SkyLight iterator is derived from the query; releasing the query first (as C
does) is apparently safe, but a Rust `Drop` ordering flip is an unforced difference — bind them in
an order that reproduces `:870-872` if you keep them as separate locals.

`unsafe`: unavoidable at every `extern "C"` call.

### 3.11 Atomics, the compiler barrier, and the animation data race

**Sites:** `__atomic_load_n(&animation_list[i].skip, __ATOMIC_RELAXED)` at `:449`, `:558`, `:582`;
`__atomic_store_n(&existing_animation->skip, true, __ATOMIC_RELEASE)` at `:633`;
`__asm__ __volatile__ ("" ::: "memory")` at `:648`; `pthread_mutex_lock/unlock` at `:577`/`:589`
and `:619`/`:675`.

**Rust:**
- `skip: AtomicBool` with `load(Relaxed)` / `store(true, Release)` — a direct mapping.
- `__asm__ __volatile__ ("" ::: "memory")` → `std::sync::atomic::compiler_fence(Ordering::SeqCst)`.
  Its purpose at `:648` is to make sure the `proxy` fields copied at `:635-647` are committed before
  `SLSGetWindowAlpha`/`window_manager_create_window_proxy` read them. A `compiler_fence` is the
  literal equivalent; if the intent was a *hardware* fence, `fence(Ordering::SeqCst)` is stronger
  than the C code and changes timing.
- `pthread_mutex_t` → `std::sync::Mutex<AnimationRegistry>`, where `AnimationRegistry` owns the
  `HashMap<u32, usize>` *and* the animation list, so that the lock actually protects the data.

**The hard part.** `context->animation_list` is `malloc`'d on the event-loop thread, handed out as
interior pointers to N builder threads (joined before reuse), registered in a table read by the
event-loop thread, mutated every frame by the CVDisplayLink thread, and `free`d by the CVDisplayLink
thread. Rust cannot express that with `&mut`. Recommended shape:

```
struct AnimationContext {            // Arc'd, one per animation batch
    connection: SlsConnection,        // released on Drop
    easing: AnimationEasing,
    duration: f32,
    clock: Cell<u64>,                 // CVDL-thread only
    entries: Box<[AnimationEntry]>,   // fixed length, never reallocated
}
struct AnimationEntry {
    window: WindowId, wid: u32, x: f32, y: f32, w: f32, h: f32,
    cid: i32,
    skip: AtomicBool,
    proxy: UnsafeCell<WindowProxy>,   // the raced field group
}
```

with `window_animations_table: Mutex<HashMap<u32, Arc<AnimationContext>>>` plus the index, so the
table holds a strong reference instead of an interior raw pointer and the `Arc` keeps the context
alive past the callback. The builder threads become `std::thread::scope(|s| …)` over
`entries.iter()` with `&AnimationEntry` (shared, because the proxy is in an `UnsafeCell`), joined by
the scope.

**The pre-existing data race at `:560-563` (CVDL writes `proxy.tx/ty/tw/th`) versus `:635-642`
(EL reads them under the lock) must be decided explicitly.** Options, in order of preference:
1. Move `tx/ty/tw/th` into four `AtomicU32` (bit-cast `f32`) and use `Relaxed` — no lock needed, no
   UB, identical observable values modulo tearing that cannot happen on 32-bit loads anyway.
2. Take `window_animations_lock` in the CVDL frame callback too — correct, but adds a lock
   acquisition per frame per animation and changes timing.
3. Keep the race with `UnsafeCell` + raw reads — faithful, but UB under Rust's model.

Option 1 is the recommendation: it preserves behaviour exactly and removes the UB.

`unsafe`: option 1 needs none; the `CVDisplayLinkSetOutputCallback` trampoline needs an
`extern "C" fn(..., *mut c_void) -> CVReturn` that does `Arc::from_raw`/`Arc::into_raw`.

### 3.12 Struct layouts handed to the OS or another process

Three places where the byte layout is a contract:

1. **`window_manager_notify_jankyborders` message** (`:441-446`):
   `struct { uint32_t event; uint32_t count; uint32_t proxy_wid[512]; uint32_t real_wid[512]; }`
   = 4 + 4 + 2048 + 2048 = **4104 bytes**, sent out-of-line over Mach to the `git.felix.jbevent`
   service. Rust: `#[repr(C)] struct JankyBordersEvent { event: u32, count: u32, proxy_wid: [u32; 512], real_wid: [u32; 512] }`.
   **Overflow hazard:** `data.proxy_wid[data.count]` at `:451` is bounded by `animation_count`, not
   by 512. Animating >512 windows at once overruns the stack struct. Rust would panic on the index —
   a crash instead of memory corruption, which is a behaviour change; clamp to 512 and document it.
   The `#pragma clang diagnostic ignored "-Wmissing-field-initializers"` at `:435-436`/`:461` exists
   only because of the `= { event, 0 }` partial initialiser — Rust's `..Default::default()` or
   `[0u32; 512]` removes the need.
2. **The AX remote token** (`:1668-1679`): a 0x14-byte `CFMutableData` with `u32 pid` at `+0x0`,
   `u32 0x636f636f` at `+0x8`, `u64 element_id` at `+0xc`. Rust: build a `[u8; 0x14]` and write with
   `copy_from_slice` on `to_ne_bytes()` — **native endianness**, matching the C `*(uint32_t *)` store.
3. **`g_event_bytes`** (`:1280-1290`, `:1298-1317`): a 0x100-byte buffer with hand-picked offsets
   passed to `SLPSPostEventRecordTo`. Rust: a `[u8; 0x100]` global (`static mut` behind the
   single-threaded invariant, or a field of the event-loop state) with the exact same writes.
   Every offset is listed in §2.5.

`SLSNewWindowWithOpaqueShapeAndContext` is also called with a **pointer to a single `uint64_t` tag**
and `tag_size = 64` (`:471-472`) — the API takes the size in *bits*, so this is correct as written;
in Rust pass `&mut tags as *mut u64` and the literal `64`.

### 3.13 `goto` into the middle of a control structure

**Site:** `window_manager_toggle_scratchpad_window` (`:2448-2490`). `switch (forced_mode)` at `:2461`
jumps to labels placed *inside* the branches of the if/else-if/else chain at `:2469-2487`.

**Rust (recommended):** enumerate the modes and split the three bodies into named helpers:

```
enum ScratchpadMode { Auto, Hide, ShowHere, MoveAndShow }
// Auto  -> compute (visible_space, ordered_in) and pick one of the three
// Hide      == label mode_1  (:2470)
// ShowHere  == label mode_2  (:2479)
// MoveAndShow == label mode_3 (:2483)
```

`forced_mode` is an `int` today with callers passing `0` (`:2445`), `3` (`:2518`) and values from
`message.c`. Keep an `i32 -> ScratchpadMode` conversion at the boundary and make an out-of-range
value fall through to `Auto`, matching the C `switch` with no `default` (which falls into `mode_0`).

### 3.14 Function pointers

**Sites:** `hash_wm` / `compare_wm` stored in every `struct table` (`:2727-2733`, typedefs at
`hashtable.h:5,8`); `window_manager_build_window_proxy_thread_proc` as a `pthread` entry (`:666`);
`window_manager_animate_window_list_thread_proc` as a `CVDisplayLinkOutputCallback` (`:701`).

**Rust:** the table hashers disappear into `BuildHasher`. The two thread entry points become
`extern "C" fn` trampolines (`CVDisplayLink`) and a plain closure (`thread::scope`).

### 3.15 `regex.h`

**Sites:** `regex_match(valid, &rule->app_regex, …)` ×4 at `:94`, `:97`, `:100`, `:103`, calling
`regexec` (`misc/helpers.h:573-579`) on POSIX extended regexes compiled by `rule.c`.

**Rust:** `regex::Regex` is **not** a drop-in: POSIX ERE and Rust's `regex` differ on backreferences
(unsupported in both), leftmost-longest vs leftmost-first alternation semantics, and some character
class spellings. Since `regex_match` only asks "does it match at all", the alternation difference is
invisible; POSIX bracket expressions like `[[:alpha:]]` are supported by the `regex` crate. The
remaining risk is `REG_ICASE`/`REG_EXTENDED` flag parity — check what `rule.c` passes to `regcomp`
before choosing. Alternative: bind `regcomp`/`regexec` through `libc` and keep byte-for-byte parity.
This decision belongs to the `rule` reader; flag it as a cross-module dependency.

**Also note the tri-state return:** `REGEX_MATCH_UD = 0`, `YES = 1`, `NO = 2`
(`misc/macros.h:22-24`). `window_manager_rule_matches_window` compares against
`REGEX_MATCH_YES`/`NO` chosen by the exclude flag, so an *undefined* (invalid/absent) pattern falls
through as "does not disqualify". In Rust: `Option<bool>` where `None` = undefined.

### 3.16 `AX_ENHANCED_UI_WORKAROUND` — a macro that wraps a code block

**Sites:** `:361-364`, `:405`, `:741-761`. Definition `misc/helpers.h:524-530`: read
`AXEnhancedUserInterface`, set it false, run the block, set it back true if it had been true.

**Rust:** a guard type is the right shape —

```
struct RestoreEnhancedUserInterfaceOnDrop { app_ref: AXUIElementRef, was_enabled: bool }
```

which restores in `Drop`. **Behaviour note:** the C macro restores only on the normal path; there is
no early return inside any of the three blocks today, so a `Drop` guard is equivalent *and* safer.
Do not use a closure-taking `with_enhanced_ui_disabled(|| …)` if the block needs `&mut` access to
surrounding state — the guard avoids the borrow problem.

### 3.17 Profiling macros (`TIME_FUNCTION`, `TIME_BODY`)

**Sites:** `TIME_FUNCTION` at `:27`, `:40`, `:56`, `:65`, `:302`, `:332`, `:348`, `:370`, `:707`,
`:720`, `:1295`, `:1326`, `:1341`, `:1354`, `:1750`, `:1801`, `:1834`, `:1952`, `:2059`, `:2070`,
`:2080`, `:2094`, `:2126`, `:2183`, `:2217`, `:2247`, `:2261`, `:2298`, `:2328`, `:2357`, `:2386`,
`:2404`, `:2412`, `:2450`; `TIME_BODY(label, { … })` at `:617`, `:678`, `:684`, `:688`, `:692`.

With the default `PROFILE` setting these expand to **nothing** (`misc/timer.h:151-162`), and
`TIME_BODY(label, c)` expands to just `c`. At `PROFILE >= 2` they use
`__attribute((cleanup(END_TIME_BLOCK)))`, i.e. a scope guard.

**Rust:** drop them, or introduce `#[cfg(feature = "profile")]` and a `TimeBlock` guard struct.
Since only pre-existing comments carry over and these are not comments, the simplest faithful
translation is to **delete them and keep the blocks**: `TIME_BODY(label, { … })` becomes a bare
block `{ … }` (note the C form is `do { TIME_BLOCK(label); c } while (0)` at `PROFILE>=2`, so the
braces in the call sites are already the block).

### 3.18 `#pragma clang diagnostic` regions

`:435-436`/`:461` (`-Wmissing-field-initializers`), `:535-536`/`:601` (`-Wunused-parameter`),
`:1337-1338`/`:1362` (`-Wdeprecated-declarations`). In Rust these become `..Default::default()`,
`_` parameter names, and nothing (deprecation is a C-API concept that the `extern` block does not
carry). Do not translate the pragmas as comments.

### 3.19 VLAs (C99 variable-length arrays)

**Site:** `:849-850`, `:859` in `window_manager_set_window_layer`. **Rust:** `Vec<u32>` with
`with_capacity(window_count)`; the `check_list` growth at `:866` becomes `push`, which removes the
overflow hazard (§2.5) at the cost of allocating.

### 3.20 Float ↔ int conversions that must not be "cleaned up"

| Site | C expression | What happens | Rust |
|---|---|---|---|
| `:635-643` | `(int)(existing_animation->proxy.tx)` × 8 | f32 → i32 **truncation toward zero**, then back to f32/CGFloat | `x as i32 as f32` — keep both casts |
| `:1147`, `:1166` | `uint32_t area = node->area.w * node->area.h;` | f32 product truncated to u32, wrapping on overflow | `as u32` (saturating in Rust — note the divergence) |
| `:1896` | `(int)(0.5f + a_node->area.x + a_node->area.w / 2.0f)` | round-half-up via truncation | `(0.5f32 + …) as i32` |
| `:1897-1898` | `powf((ca.x - (int)(…)), 2.0f)` | `CGFloat`(f64) minus `int`, narrowed to `float` for `powf` | `((ca.x - (…) as f64) as f32).powf(2.0)` |
| `:545-547` | `double t`, then `t = 0.0f` / `t = 1.0f` | f32 literals widened to f64 | `let mut t: f64`; `t = 0.0; t = 1.0;` |
| `:575` | `if (t != 1.0f)` | f64 compared to a widened f32 literal | `if t != 1.0` |
| `:560-563` | `lerp(a, mt, b)` = `((1.0-mt)*a) + (mt*b)` with `float mt` | **computed in `double`** (the `1.0` literal), result narrowed to the f32 `proxy.tx` | compute in `f64`, assign `as f32` — a pure-`f32` lerp gives different last-bit results |
| `:2170-2175` | `bounds.size.width / c` with `unsigned c` | f64 / u32 → f64, then narrowed into `float cw` | `bounds.size.width / c as f64` then `as f32` |
| `:389`, `:394` | `(float) dx / (float) y_fence->area.w` | both already f32; the casts are no-ops | plain `dx / area.w` |

### 3.21 ObjC message sends

**Site:** `window_manager_begin` (`:2739` `[[NSAutoreleasePool alloc] init]`, `:2756` `[pool drain]`).
This is the file's only Objective-C. **Rust:** `objc2`/`objc2-foundation`'s
`NSAutoreleasePool`/`autoreleasepool(|_| { … })`. Note that `event_loop.c:1653`/`:1677` already wraps
each event in a pool, so this one only matters for the main-thread startup path.

### 3.22 `assert`

`:2334`, `:2363`. Rust: `assert!(node.is_some())` — or better, `let Some(node) = … else { return };`
since both call sites immediately guard on `node` anyway. `assert` is compiled out with `NDEBUG` in
release C builds; `assert!` is **not** compiled out in Rust release builds. Use `debug_assert!` to
match, or convert to a real guard.

### 3.23 Intrusive linked lists / tagged unions / setjmp / inline asm / SIMD

- **Intrusive linked lists:** none in this file. (`struct bucket::next` is inside `hashtable.h`;
  `struct window_node` is an explicit binary tree owned by `view.c`.)
- **Tagged unions:** none.
- **`setjmp`/`longjmp`:** none.
- **Inline asm:** one occurrence, the compiler barrier at `:648` (§3.11).
- **SIMD:** none directly; reached indirectly through `cgimage_restore_alpha`
  (`misc/helpers.h:588-660`, SSE2 on x86_64 / NEON on arm64) called at `:525`. That helper belongs
  to the `helpers` reader; from here it is just "an owned `CGImageRef` comes back".
- **CAS on `id_ptr` as a liveness check:** the `window->id_ptr` idiom (`window.h:92`) is **not** used
  in this file; it lives in `window.c`/`event_loop.c`.
- **fork/exec:** none in this file.

---

## 4. Comments that must be carried over verbatim

Only these survive into the Rust source. Every one is a `//` line comment. Line numbers are the C
locations; the "attach to" column says where the comment belongs in the translated code.

| C lines | Attach to | Text (first line shown; carry the whole block) |
|---|---|---|
| `78-82` | inside `window_manager_query_windows_for_displays`, before `if (!space_list) space_list = list;` | `NOTE(asmvik): display_space_list(..) uses a linear allocator, / and so we only need to track the beginning of the first list along / with the total number of spaces that have been allocated.` |
| `731-739` | top of `window_manager_set_window_frame` | `NOTE(asmvik): Attempting to check the window frame cache to prevent unnecessary movement and resize calls to the AX API …` (9 lines, including the blank `//` separators and the second paragraph about CG window notifications) |
| `748` | before the first `AXUIElementSetAttributeValue(kAXSizeAttribute)` | `NOTE(asmvik): Due to macOS constraints (visible screen-area), we might need to resize the window *before* moving it.` |
| `756` | before the second `kAXSizeAttribute` set | `NOTE(asmvik): Due to macOS constraints (visible screen-area), we might need to resize the window *after* moving it.` |
| `885` | the `else` arm of `window_manager_purify_window` | `/*if (wm->purify_mode == PURIFY_ALWAYS) */` — an inline block comment on the `else`; carry it as `// if (wm.purify_mode == PurifyMode::Always)` or keep the arm explicit and drop it |
| `1271-1278` | top of `window_manager_make_key_window` | `:SynthesizedEvent` tag + `NOTE(asmvik): These events will be picked up by an event-tap registered at the "Annotated Session" location; …` |
| `1306-1311` | before `usleep(40000)` in `window_manager_focus_window_without_raise` | `@hack / Artificially delay the activation by 40ms. This is necessary because some applications appear to be confused if both of the events appear instantaneously.` |
| `1454-1456` | before `window_observe(window)` | `NOTE(asmvik): Attempt to track **all** windows.` |
| `1473-1475` | before `if (window->is_root)` | `NOTE(asmvik): However, only **root windows** are eligible for management.` |
| `1479-1485` | before `window_manager_apply_manage_rules_to_window` | `NOTE(asmvik): A lot of windows misreport their accessibility role, … / This part of the rule must be applied at this stage (prior to other rule properties), …` |
| `1516-1519` | before the `if (g_verbose)` block in the not-eligible branch | `NOTE(asmvik): Print window information when debug_output is enabled. / Useful for identifying and creating rules if this window should in fact be managed.` |
| `1531-1533` | before the `if (g_verbose)` block in the child-window branch | `NOTE(asmvik): Print window information when debug_output is enabled.` |
| `1594-1598` | inside `window_manager_existing_application_window_list` | same linear-allocator note as `78-82`, but ending `… total number of **windows** that have been allocated.` — **the two texts differ in the last word; do not deduplicate them** |
| `1623-1631` | before `if (!window_id) { ++empty_count; continue; }` | `@cleanup / :Workaround / NOTE(asmvik): The AX API appears to always include a single element for Finder that returns an empty window id. …` |
| `1659-1666` | before the `CFDataCreateMutable` brute-force block | `NOTE(asmvik): MacOS API does not return AXUIElementRef of windows on inactive spaces. … / :Attribution / https://github.com/decodism / https://github.com/lwouis/alt-tab-macos/issues/1324#issuecomment-2631035482` |
| `1884-1891` | before the distance heuristic in `window_manager_warp_window` | `:NaturalWarp / NOTE(asmvik): Precalculate both target areas and select the one that has the closest distance to the source area. …` |
| `1931-1937` | before the cross-space warp branch | `:NaturalWarp / TODO(asmvik): Warp operations with operands that belong to different monitors does not yet implement a heuristic …` |
| `2270-2276` | inside the first spin loop of `window_manager_wait_for_native_fullscreen_transition` | `NOTE(asmvik): Window has exited native-fullscreen mode. … / The display_manager API does not work on macOS Monterey.` |
| `2285-2289` | inside the second spin loop | same first paragraph, **without** the Monterey sentence — again, two distinct texts |
| `2302-2306` | before `window_manager_focus_window_with_raise` in `toggle_window_native_fullscreen` | `NOTE(asmvik): The window must become the focused window before we can change its fullscreen attribute. …` |
| `2318-2321` | before the final `window_manager_wait_for_native_fullscreen_transition` call | `NOTE(asmvik): We toggled the fullscreen attribute and must now spin lock until the post-exit space animation has finished.` |
| `2455` | above `bool visible_space = …` in `window_manager_toggle_scratchpad_window` | `TODO(asmvik): Both functions use the same underlying API and could be combined in a single function to reduce redundant work.` |
| `2559-2567` | inside `window_manager_validate_windows_on_space`, before `view_remove_window_node` | `@cleanup / :AXBatching / NOTE(asmvik): Batch all operations and mark the view as dirty so that we can perform a single flush, …` |
| `2588-2596` | inside `window_manager_check_for_windows_on_space`, first branch | identical `:AXBatching` block |
| `2607-2615` | inside `window_manager_check_for_windows_on_space`, second branch | identical `:AXBatching` block |
| `2635-2642` | in `window_manager_validate_and_check_for_windows_on_space`, before the flush | `@cleanup / :AXBatching / NOTE(asmvik): Flush previously batched operations if the view is marked as dirty. …` |

That is **26 comment blocks**. There are no other comments in either file. `window_manager.h` has
**zero** comments.

---

## 5. Hazards a phase-2 translator must decide on, not discover

1. `window_manager_set_window_layer` VLA overflow (`:866`) — §2.5.
2. `window_manager_notify_jankyborders` fixed 512-slot arrays vs unbounded `animation_count`
   (`:451-452`) — §3.12.
3. `switch (context->animation_easing)` with no `default` leaving `mt` uninitialised (`:550-554`) — §3.6.
4. The CVDisplayLink/event-loop data race on `proxy.tx/ty/tw/th` (`:560-563` vs `:635-642`) — §3.11.
5. `window_manager_apply_grid` unsigned underflow on `c == 0`/`r == 0` and the dead
   `if (w <= 0)` checks (`:2134-2139`) — §2.5.
6. `uint32_t sid = window_space(window->id)` truncating a 64-bit space id (`:2300`) — §2.5.
7. `_AXUIElementCreateWithRemoteToken` element-ref leak (`:1680-1708`) — §2.5.
8. `buf_del` being swap-remove, and evaluating to the *old* length (`:1569`, `:1733`, `:2515`) — §3.2.
9. `purify_mode_str[]` not matching the enum order (`window_manager.h:33-38`) — §1.3.
10. Unchecked `space_manager_find_view` results dereferenced at `:1754`, `:2628`, `:2691`, and
    unchecked `view_find_window_node` at `:1820`.
11. `usleep` on the event-loop thread: 40 ms at `:1313`, 100 ms spin loops at `:2278`, `:2291`,
    `:2309`. These block every pending event, including socket commands.
12. `window_manager_move_window_relative`'s inverted guard (`:335`) — it is not a typo; a managed
    window genuinely cannot be moved this way.
13. The ts-allocator contiguity assumption at `:73-88` and `:1589-1602` does not survive a
    `Vec`-based translation — §3.3.

---

## 6. Open questions for the phase-2 translator

- **Where does `struct window_manager` live in Rust?** The C code passes `struct window_manager *wm`
  to most functions but reaches `g_window_manager` directly in 12 places (`:30-33`, `:577`, `:584`,
  `:610-611`, `:619`, `:631`, `:662`, `:673`, `:675`, `:709`, `:722`, `:833`, `:1297`, `:1303-1304`)
  and `g_space_manager` in 3 (`:2503`, `:2519`, `:2536`). A single `&mut WindowManager` threaded
  through every call is the cleanest, but the animation path stores references into it from another
  thread. Decide before writing the module: one `Daemon` struct owning all managers and passed as
  `&mut`, versus a `thread_local`/`static` with interior mutability.
- **Does the Rust port keep the `t <= 0.0 / t >= 1.0` clamping and the `!=` float comparison at
  `:575` exactly, or does it switch to a frame counter?** The C version depends on
  `CVTimeStamp::hostTime` arithmetic and `g_cv_host_clock_frequency`; changing it changes animation
  length.
- **Which crate binds `CVDisplayLink`?** `core-video-sys` is stale; a hand-rolled `extern "C"` block
  against `CoreVideo.framework` is probably needed for `CVDisplayLinkCreateWithActiveCGDisplays`,
  `CVDisplayLinkSetOutputCallback`, `CVDisplayLinkStart/Stop/Release` and `CVTimeStamp`. Confirm
  `CVTimeStamp`'s `#[repr(C)]` layout (only `hostTime` is read, at `:542`/`:543`).
- **`regex.h` vs the `regex` crate** — §3.15; the decision belongs to the `rule` module but this file
  is the only consumer of `regex_match`'s tri-state result.
- **Is the startup ordering (`window_manager_init` on main at `yabai.c:336` while the event-loop
  pthread from `yabai.c:291` is already alive) actually race-free?** The argument in §2.4 is that
  nothing can enqueue an event before `[NSApp run]` at `yabai.c:350`, because every producer is a
  main-run-loop callback. This should be confirmed against the `workspace`/`process_manager`/
  `mission_control` readers' findings before the Rust port relies on it to justify `&mut` access.
- **Should `window_origin_mode` (`window_manager.h:93`, written at `:2716`, never read here) stay in
  this struct?** It is consumed elsewhere; confirm with the `space-manager`/`message` readers.
