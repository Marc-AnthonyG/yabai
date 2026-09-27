# Phase 1 map — `src/view.h`, `src/view.c`, `tests/`

Reader assignment: **view-and-tests**.
Sources read completely: `src/view.h` (253 lines), `src/view.c` (1042 lines), `tests/src/tests.m` (54 lines), `tests/src/area.c` (89 lines), `tests/makefile` (13 lines).

All C locations below are `path:line` against the working tree at the time of writing.

---

## 0. Context the phase 2 translator needs before touching these files

### 0.1 Threads that exist in the daemon

| Thread | Created at | What runs there |
|---|---|---|
| main run loop | `src/yabai.c:350` (`[NSApp run]`) | `CFRunLoop` sources: AXObserver callbacks (`src/application.c:57`, `src/mission_control.c:87`), CGEventTap (`src/mouse_handler.c:288`), `dispatch_get_main_queue()` blocks, SLS connection notify procs |
| event-loop pthread | `src/event_loop.c:1718` (`pthread_create(&event_loop->thread, NULL, &event_loop_run, event_loop)`) | every `EVENT_HANDLER_*`, and therefore **every function in `view.c`** |
| message-loop pthread | `src/message.c:3042` | `accept()` only; it never touches view state, it just posts `DAEMON_MESSAGE` (`src/message.c:3009`) |
| CVDisplayLink thread | `src/window_manager.c:702` (`CVDisplayLinkStart`) | `window_manager_animate_window_list_thread_proc` (`src/window_manager.c:537`), which mutates `struct window_animation` / `struct window_proxy` — both declared in `view.h` |
| transient proxy-build pthreads | `src/window_manager.c:666` | `window_manager_build_window_proxy_thread_proc` (`src/window_manager.c:507`), one per animated window, joined at `src/window_manager.c:680` |

**Determination method for “which thread”:** every caller of every `view.c` symbol was enumerated by grep (see §2.4 for the caller table). Every caller lives in `event_loop.c` `EVENT_HANDLER_*` bodies, in `message.c` command handlers (reached only from `EVENT_HANDLER_DAEMON_MESSAGE`, since the message-loop pthread only does `accept()` + `event_loop_post`), or in `window_manager.c` / `space_manager.c` / `mouse_handler.c` functions that are themselves only called from those two places. Conclusion: **`view.c` is single-threaded, event-loop-pthread only.** The only multi-threaded things this assignment owns are the three animation structs *declared* in `view.h` but *used* in `window_manager.c`/`sa.m`.

### 0.2 Unity-build leakage — the single biggest translation hazard in this file

`src/manifest.m:89` includes `view.c` after `event_loop.c` (:82) and before `window.c` (:90), `display_manager.c` (:93), `space_manager.c` (:94), `window_manager.c` (:95), `mouse_handler.c` (:96). Because of that:

* `view.c:45` and `view.c:111` call `update_window_notifications()`, which is **`static` in `src/event_loop.c:16`**. In Rust this must become a real cross-module call; the function has to move somewhere both modules can see (recommendation: keep it in the `event_loop` module as `pub(crate) fn update_window_notifications()`).
* Many `static` / `static inline` helpers defined in `view.c` are used from other .c files. They are **not** private and must be `pub(crate)` in Rust:

| `view.c` symbol | declared `static`? | external users |
|---|---|---|
| `area_from_cgrect` (`view.c:121`) | `static inline` | `display_manager.c:275,282` |
| `area_max_point` (`view.c:126`) | `static inline` | `display_manager.c:276,283`, `tests/src/area.c:13,19,25` |
| `area_is_in_direction` (`view.c:541`) | `static inline` | `display_manager.c:285`, `tests/src/area.c:33,36,39,42,54` |
| `area_distance_in_direction` (`view.c:563`) | `static inline` | `display_manager.c:286`, `tests/src/area.c:55` |
| `area_make_pair` (`view.c:161`) | `static` | `window_manager.c:1894` |
| `window_node_get_split` (`view.c:136`) | `static inline` | `window_manager.c:1894` |
| `window_node_get_ratio` (`view.c:151`) | `static inline` | `window_manager.c:1894` |
| `window_node_get_gap` (`view.c:156`) | `static inline` | `window_manager.c:1894`, `window_manager.c:2153` |
| `window_node_is_leaf` (`view.c:208`) | `static inline` | `window_manager.c:1185,1199,1213,1230,1247,1264` |
| `window_node_is_left_child` (`view.c:213`) | `static inline` | `window_manager.c:1184,1198,1212,1229,1246,1263,1905`, `window.c:594` |
| `window_node_is_intermediate` (`view.c:203`) | `static inline` | `space_manager.c:473` |
| `window_node_equalize` (`view.c:223`) | `static` | `space_manager.c:425` |
| `window_node_balance` (`view.c:242`) | `static` | `space_manager.c:437,477` |
| `window_node_rotate` (`view.c:469`) | non-static, **not in view.h** | `space_manager.c:401` |
| `window_node_mirror` (`view.c:494`) | non-static, **not in view.h** | `space_manager.c:413` |
| `window_node_fence` (`view.c:509`) | non-static, **not in view.h** | `window_manager.c:382,383,384,385` |
| `view_find_min_depth_leaf_node` (`view.c:525`) | non-static, **not in view.h** | none outside `view.c` — can be private |

Genuinely private to `view.c`: `window_node_get_child` (:131), `area_make_pair_for_node` (:186), `window_node_is_occupied` (:198), `window_node_is_right_child` (:218), `balance_node_add` (:237), `window_node_split` (:277), `window_node_destroy` (:335), `window_node_clear_zoom` (:348).

### 0.3 The temporary-storage arena (`ts_*`)

`src/misc/ts.h` is a bump allocator over an 8 MB `mmap` region (`src/yabai.c:279`, `ts_init(MEGABYTES(8))`) with a `PROT_NONE` guard page. `ts_reset()` is called once per event at the bottom of `event_loop_run` (`src/event_loop.c:1671`). **Every `ts_alloc*` / `ts_buf_*` pointer is only valid until the end of the current event handler.** `view.c` uses it in three places (`view.c:365`, `view.c:824`, `view.c:877`) plus indirectly through `space_window_list` (`src/space.c:34` uses `ts_alloc_list`).

---

## 1. `src/view.h`

### 1.1 Purpose

The public header for yabai's per-space layout model. It declares the BSP tree node (`struct window_node`), the per-space view that owns a tree (`struct view`), the geometry primitive (`struct area`), the enums and string tables for layout/split/child/insertion-point, and — unrelated to the tree but parked here — the three structs the window animation machinery in `window_manager.c` uses. It also defines the `view_*_flag` bit macros and the `SPACE_PROPERTY_LIST` X-macro used to serialize a space to JSON.

### 1.2 Types, constants, X-macros

#### `AX_ABS(a, b)` — `view.h:4`
`#define AX_ABS(a, b) (((a) - (b) < 0) ? (((a) - (b)) * -1) : ((a) - (b)))`
Unhygienic: evaluates `a` and `b` up to four times. Used only in `event_loop.c:709-713, 808-816`.

#### `AX_DIFF(a, b)` — `view.h:5`
`#define AX_DIFF(a, b) (AX_ABS(a, b) >= 1.5f)` — "these two coordinates differ by at least 1.5 points". Arguments at all call sites are `float` (`node->area.x`) vs `CGFloat` (`new_frame.origin.x`), so the subtraction happens in `double`.
**Rust:** `pub fn ax_diff(a: f64, b: f64) -> bool { (a - b).abs() >= 1.5 }`. Note the C promotes `float` to `double` at the call site; if phase 2 writes an `f32` version the comparison will differ in the last bits. Keep it `f64` and cast the `f32` field at the call site, exactly mirroring C's usual arithmetic conversions.

#### `SPACE_PROPERTY_LIST` X-macro — `view.h:7-19`
Twelve entries, each `SPACE_PROPERTY_ENTRY(name_string, enum_name, bit)`:

| string | enum | value |
|---|---|---|
| `"id"` | `SPACE_PROPERTY_ID` | `0x001` |
| `"uuid"` | `SPACE_PROPERTY_UUID` | `0x002` |
| `"index"` | `SPACE_PROPERTY_INDEX` | `0x004` |
| `"label"` | `SPACE_PROPERTY_LABEL` | `0x008` |
| `"type"` | `SPACE_PROPERTY_TYPE` | `0x010` |
| `"display"` | `SPACE_PROPERTY_DISPLAY` | `0x020` |
| `"windows"` | `SPACE_PROPERTY_WINDOWS` | `0x040` |
| `"first-window"` | `SPACE_PROPERTY_FIRST_WINDOW` | `0x080` |
| `"last-window"` | `SPACE_PROPERTY_LAST_WINDOW` | `0x100` |
| `"has-focus"` | `SPACE_PROPERTY_HAS_FOCUS` | `0x200` |
| `"is-visible"` | `SPACE_PROPERTY_IS_VISIBLE` | `0x400` |
| `"is-native-fullscreen"` | `SPACE_PROPERTY_IS_FULLSCREEN` | `0x800` |

Expanded three times: `enum space_property` (`view.h:21-26`), `static uint64_t space_property_val[]` (`view.h:28-33`), `static char *space_property_str[]` (`view.h:35-40`). The two arrays are consumed together by `parse_properties(...)` at `src/message.c:2481`, which relies on `array_count(space_property_str) == 12` and on index-parallelism between the two arrays.

**Rust:** one `#[derive(Clone, Copy)] pub struct SpaceProperty(pub u64)` bitflag set (or the `bitflags` crate) plus a single `pub const SPACE_PROPERTIES: [(&str, u64); 12]` table, so the two arrays cannot drift. `space_property_val` is `uint64_t` while the enum constants are `int` — `parse_properties` reads them as `uint64_t*`, so the Rust table element type must be `u64`.

#### `struct area` — `view.h:42-48`
```
float x; float y; float w; float h;
```
No pointers, `Copy`. **Rust:** `#[derive(Clone, Copy, Default)] pub struct Area { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }`.
Touched from one thread only (event loop) when it lives inside `window_node`. Note `display_manager.c:275` builds one from `CGDisplayBounds` (a `CGRect` of `CGFloat`/`f64`) — lossy `f64 -> f32` narrowing that must be preserved.

#### `struct window_capture` — `view.h:51-55`
```
struct window *window;   // borrowed, owned by g_window_manager.window table
float x, y, w, h;
```
`struct window` is forward-declared at `view.h:50`. The `window` pointer is a **borrow**: the `struct window` is owned by `g_window_manager.window` (allocated in `window_manager.c`, freed when the window is destroyed). `window_capture` values themselves live either on the stack (`window_manager.c:342` and friends build a temporary) or in a `ts_buf` arena buffer (`view.c:365`).
Single-threaded: built and consumed on the event-loop thread; `window_manager_animate_window_list_async` (`src/window_manager.c:603`) copies the fields into a `malloc`'d `window_animation` array before any other thread sees anything.
**Rust:** `#[derive(Clone, Copy)] pub struct WindowCapture { pub window: *mut Window, pub x: f32, pub y: f32, pub w: f32, pub h: f32 }` while windows are still raw-pointer managed, or `WindowId` + a lookup if phase 3 moves to id-indexed storage. Keeping the raw pointer is the faithful choice for phase 2.

#### `struct window_proxy` — `view.h:57-66`
```
uint32_t id;            // SLS window id created by SLSNewWindowWithOpaqueShapeAndContext, released by SLSReleaseWindow
CGContextRef context;   // owned: SLWindowContextCreate -> CGContextRelease (window_manager.c:496)
float tx, ty, tw, th;   // current interpolated frame
CGRect frame;           // start frame
int level; int sub_level;
CGImageRef image;       // owned: CFRetain/cgimage_restore_alpha -> CFRelease (window_manager.c:491)
```
Ownership: every one of `id`, `context`, `image` is owned by the proxy and released in `window_manager_destroy_window_proxy` (`src/window_manager.c:489-505`).
**Cross-thread fields:** `tx/ty/tw/th` are written by the CVDisplayLink thread (`window_manager.c:560-563`) with no lock, and read by the event-loop thread under `window_animations_lock` (`window_manager.c:635-642`). That is a genuine data race in the C; see §6 of `view.c` for what Rust should do.
**Rust:** raw FFI struct holding `*mut c_void`-shaped CF handles. Because the CF handles are owned, wrap them in a newtype with a `Drop` **only if** phase 2 is willing to move `window_manager_destroy_window_proxy` into that `Drop`; otherwise keep plain fields and an explicit `destroy()` to stay behaviour-identical (the C zeroes the fields after release, `window_manager.c:492,497,502`).

#### `struct window_animation` — `view.h:68-76`
```
struct window *window;  // borrowed
uint32_t wid;
float x, y, w, h;       // destination frame
int cid;                // SLS connection created by SLSNewConnection, released by SLSReleaseConnection
struct window_proxy proxy;
volatile bool skip;
```
Arrays of these are `malloc`'d in `window_manager_animate_window_list_async` (`src/window_manager.c:609`) and `free`'d on the CVDisplayLink thread (`src/window_manager.c:592`).
**Threads:** `proxy` is filled by the transient proxy-build pthread (`window_manager.c:507-533`); `skip` is written with `__atomic_store_n(..., __ATOMIC_RELEASE)` from the event-loop thread (`window_manager.c:632`) and read with `__ATOMIC_RELAXED` from the CVDisplayLink thread (`window_manager.c:558, 582`), from `window_manager_notify_jankyborders` (`window_manager.c:450`, both threads) and from `sa.m:551, 567`.
**Rust:** `skip` becomes `AtomicBool` with `Ordering::Release` on store and `Ordering::Relaxed` on load, exactly matching. Pointers into the array are handed to the CVDisplayLink callback as a `*mut WindowAnimationContext`; that stays `unsafe`.

#### `struct window_animation_context` — `view.h:78-86`
```
int animation_connection;              // owned SLS connection
int animation_easing;
float animation_duration;
uint64_t animation_clock;              // written only on the CVDisplayLink thread (window_manager.c:542)
struct window_animation *animation_list;   // owned, malloc'd at window_manager.c:609, freed at window_manager.c:592
int animation_count;
```
The context itself is `malloc`'d at `window_manager.c:605` on the event-loop thread and `free`'d at `window_manager.c:593` on the CVDisplayLink thread — an ownership transfer across threads.
**Rust:** `Box::into_raw` on the event-loop side, `Box::from_raw` in the display-link callback. This is the canonical Rust shape and is exactly equivalent.

#### `struct balance_node` — `view.h:88-92`
```
int y_count; int x_count;
```
Pure value type, used only as `window_node_balance`'s accumulator. **Rust:** `#[derive(Clone, Copy, Default)] struct BalanceNode { y_count: i32, x_count: i32 }`.

#### `enum window_insertion_point` — `view.h:94-99` + `window_insertion_point_str` — `view.h:101-106`
`INSERT_FOCUSED = 0`, `INSERT_FIRST = 1`, `INSERT_LAST = 2`; strings `"focused"`, `"first"`, `"last"`. Indexed at `src/message.c:1245`.

#### `enum window_node_child` — `view.h:108-113` + `window_node_child_str` — `view.h:115-120`
`CHILD_NONE = 0`, `CHILD_SECOND = 1`, `CHILD_FIRST = 2`; strings `"none"`, `"second_child"`, `"first_child"`. **The non-obvious ordering (SECOND before FIRST) is load-bearing**: `window.c:594` and `message.c:1234` index the string table with the enum. Indexed at `window.c:296`, `window.c:594`, `message.c:1234`.

#### `enum window_node_split` — `view.h:122-128` + `window_node_split_str` — `view.h:130-136`
`SPLIT_NONE = 0`, `SPLIT_Y = 1`, `SPLIT_X = 2`, `SPLIT_AUTO = 3`; strings `"none"`, `"vertical"`, `"horizontal"`, `"auto"`. Indexed at `window.c:289`, `window.c:587`, `message.c:1556`, `message.c:1571`.

#### `auto_balance_str` — `view.h:138-143`
`{"off", "vertical", "horizontal", "on"}` — **there is no matching enum**. It is indexed by `view->auto_balance` / `g_space_manager.auto_balance` (`message.c:1587`, `message.c:1605`), which are `uint32_t` treated as a *bitmask* of `SPLIT_Y | SPLIT_X` (see `view.c:228, 232, 255, 262`). The numeric overlap with `enum window_node_split` is deliberate: `0 = none`, `1 = SPLIT_Y only`, `2 = SPLIT_X only`, `3 = both`.
**Rust:** do **not** model `auto_balance` as `WindowNodeSplit`. Make it a `u32` bitflag (`AutoBalance: u32` with `Y = 1, X = 2`) and keep the four-string table beside it. A naive `enum` translation will make `auto_balance & SPLIT_Y` impossible to express and will silently break `space auto_balance on`.

#### `struct feedback_window` — `view.h:145-149`
```
uint32_t id;           // owned SLS window (SLSNewWindowWithOpaqueShapeAndContext -> SLSReleaseWindow)
CGContextRef context;  // owned (SLWindowContextCreate -> CGContextRelease)
```
Zeroed with `memset` at `view.c:117` after release; `id == 0` is the "not created" sentinel (`view.c:15`, `view.c:107`).

#### `NODE_MAX_WINDOW_COUNT` — `view.h:151`
`32`. The stack-per-node capacity. Checked by the caller at `mouse_handler.c:139` (`dst_node->window_count+1 < NODE_MAX_WINDOW_COUNT`) but **not** by `view_stack_window_node` itself (`view.c:730`) nor by `window_manager.c:1822`.

#### `struct window_node` — `view.h:152-167`
```
struct area area;
struct window_node *parent;                    // borrowed (back-edge, not owned)
struct window_node *left;                      // owned (malloc at view.c:279, free at view.c:345/718/719)
struct window_node *right;                     // owned
struct window_node *zoom;                       // borrowed: points at *another node in the same tree* (parent or root) or NULL
uint32_t window_list[32];                       // insertion order
uint32_t window_order[32];                      // MRU order, [0] is the visible/top one
int window_count;
float ratio;
enum window_node_split split;
enum window_node_child child;
int insert_dir;                                 // 0, or DIR_NORTH/EAST/SOUTH/WEST (360/90/180/270) or STACK (111)
struct feedback_window feedback_window;
```
`insert_dir` is an `int` holding values from `src/misc/macros.h:26-31` — **not** an enum, and `DIR_NORTH` is `360` not `0`, so `0` is a valid "no insert direction" sentinel.
Single-threaded (event loop). `window_order[0]` doubles as the hash key stored in `g_window_manager.insert_feedback` (`view.c:43`, `view.c:108`).
**Rust:** the tree is a cyclic graph (`parent` back-edges, `zoom` cross-edges). Faithful phase-2 translation keeps `*mut WindowNode` with `Box::into_raw`/`Box::from_raw`, i.e. an `unsafe` arena of raw pointers. The alternative that phase 3 should consider is an index-based arena (`Vec<WindowNode>` + `NodeId(u32)`), which removes every `unsafe` here and also fixes the dangling-`zoom`/dangling-feedback-table issues catalogued in §2.6. `Rc<RefCell<..>>` is **not** recommended: `zoom` and `parent` would need `Weak`, and the `memset(node, 0, ...)` reset idiom (`view.c:656`, `view.c:1028`) has no clean `Rc` equivalent.

#### `enum view_type` — `view.h:169-175` + `view_type_str` — `view.h:177-183`
`VIEW_DEFAULT = 0`, `VIEW_BSP = 1`, `VIEW_STACK = 2`, `VIEW_FLOAT = 3`; strings `"default"`, `"bsp"`, `"stack"`, `"float"`. Indexed at `view.c:900`, `message.c:1499`, `message.c:1531`.

#### `enum view_flag` — `view.h:185-199`
```
VIEW_LAYOUT         = 0x001
VIEW_TOP_PADDING    = 0x002
VIEW_BOTTOM_PADDING = 0x004
VIEW_LEFT_PADDING   = 0x008
VIEW_RIGHT_PADDING  = 0x010
VIEW_WINDOW_GAP     = 0x020
VIEW_AUTO_BALANCE   = 0x040
VIEW_ENABLE_PADDING = 0x080
VIEW_ENABLE_GAP     = 0x100
VIEW_IS_VALID       = 0x200
VIEW_IS_DIRTY       = 0x400
VIEW_SPLIT_TYPE     = 0x800
```
Stored in a `uint64_t` (`view.h:215`). **Rust:** `bitflags! { pub struct ViewFlags: u64 { ... } }`.

#### `struct view` — `view.h:201-216`
```
CFStringRef uuid;              // owned: SLSSpaceCopyName (view.c:995) -> CFRelease (view.c:1040); may be NULL
uint64_t sid;
struct window_node *root;      // owned: malloc at view.c:991, destroyed by window_node_destroy at view.c:1036
uint32_t insertion_point;      // a window id, or 0
enum view_type layout;
enum window_node_split split_type;
int top_padding; int bottom_padding; int left_padding; int right_padding;
int window_gap;
uint32_t auto_balance;         // bitmask, see auto_balance_str above
uint64_t flags;
```
The `struct view` itself is `malloc`'d at `view.c:988` and `free`'d by the *caller* at `src/event_loop.c:989` — `view_destroy` (`view.c:1033`) deliberately does **not** free it. Phase 2 must not turn `view_destroy` into `Drop` without also moving the `free(view)` from `event_loop.c:989`.
Views are owned by `g_space_manager.view` (a `struct table` keyed by `uint64_t sid`, `space_manager.c:112`), and `space_manager_swap_space_with_space_on_display` (`space_manager.c:765-775`) swaps both the table entries *and* the `sid`/`uuid` fields.

#### `view_check_flag` / `view_clear_flag` / `view_set_flag` — `view.h:218-220`
```
#define view_check_flag(v, x) ((v)->flags  &  (x))
#define view_clear_flag(v, x) ((v)->flags &= ~(x))
#define view_set_flag(v, x)   ((v)->flags |=  (x))
```
`view_check_flag` returns the masked `uint64_t`, not a `bool` — used in `if` (`view.c:158`, `view.c:842`, `view.c:974`, `view.c:1001-1008`) and negated (`view.c:842`, `view.c:1001`). **Rust:** `flags.contains(ViewFlags::X)`, `flags.remove(..)`, `flags.insert(..)`. No behaviour change as long as every use site is a boolean context — verified: all of them are.

### 1.3 Globals and statics in `view.h`

| Symbol | Type | Initial value | Threads | Synchronisation |
|---|---|---|---|---|
| `space_property_val` (`view.h:28`) | `static uint64_t[12]` | the twelve bit values | event loop (read only, via `message.c:2481`) | none needed, immutable |
| `space_property_str` (`view.h:35`) | `static char *[12]` | string literals | event loop (read only) | none needed; note the type is `char *` not `const char *` |
| `window_insertion_point_str` (`view.h:101`) | `static const char *[3]` | `"focused","first","last"` | event loop (read) | immutable |
| `window_node_child_str` (`view.h:115`) | `static const char *[3]` | `"none","second_child","first_child"` | event loop (read) | immutable |
| `window_node_split_str` (`view.h:130`) | `static const char *[4]` | `"none","vertical","horizontal","auto"` | event loop (read) | immutable |
| `auto_balance_str` (`view.h:138`) | `static const char *[4]` | `"off","vertical","horizontal","on"` | event loop (read) | immutable |
| `view_type_str` (`view.h:177`) | `static const char *[4]` | `"default","bsp","stack","float"` | event loop (read) | immutable |

There are no function-local statics in `view.h`. Because this is a unity build these `static` arrays exist exactly once; in a real multi-TU build they would be duplicated per TU. **Rust:** all become `pub const NAME: [&str; N]`.

### 1.4 Function declarations — `view.h:222-251`

Declaration-only; each is documented under §2.4. The header omits `window_node_rotate`, `window_node_mirror`, `window_node_fence` and `view_find_min_depth_leaf_node`, which are nonetheless non-static in `view.c` and used elsewhere (see §0.2).

### 1.5 OS/run-loop callbacks registered in `view.h`

None.

### 1.6 Comments to carry over

**None.** `grep -n "//\|/\*" src/view.h` returns nothing. Phase 2 must not invent any.

---

## 2. `src/view.c`

### 2.1 Purpose

Implements the whole BSP/stack layout algorithm for one space: splitting a leaf when a window arrives, collapsing a leaf's parent when a window leaves, re-computing every node's rectangle from the display bounds and the padding/gap settings, walking the tree for first/last/prev/next leaf, ranking neighbours in a direction, balancing and equalising split ratios, rotating and mirroring the tree, and pushing the resulting rectangles to the window manager. It also owns the "insert feedback" overlay — an SLS window drawn directly with CoreGraphics to show where the next window will go — and the JSON serialisation of a space.

### 2.2 File-scope declarations

`view.c:1-4` declares four externs; all four are defined in `src/yabai.c`:

| Symbol | Type | Threads that touch it | Synchronisation today |
|---|---|---|---|
| `g_connection` (`view.c:1`) | `int` | set once during startup on the main thread, read from event loop, CVDisplayLink and proxy threads | none — it is write-once before any thread starts |
| `g_display_manager` (`view.c:2`) | `struct display_manager` | event loop; `display_bounds_constrained` reads `.mode/.top_padding/.bottom_padding` | none |
| `g_space_manager` (`view.c:3`) | `struct space_manager` | event loop only | none |
| `g_window_manager` (`view.c:4`) | `struct window_manager` | event loop; **`.window_animations_table` and `.window_animations_lock` are also touched from the CVDisplayLink thread** (`window_manager.c:577-589`) — `.insert_feedback`, `.insert_feedback_color`, `.focused_window_id`, `.window` are event-loop only | `pthread_mutex_t window_animations_lock` covers `window_animations_table` only |

Note `view.c:4` is the only reason `view.c` can read `g_window_manager.insert_feedback_color` (`view.c:29-37`) and `g_window_manager.focused_window_id` (`view.c:787`).

**Rust:** these four globals are the crux of the whole port. They are mutable process-wide singletons touched by more than one thread in the `window_manager` case. The behaviour-preserving option for phase 2 is `static mut` behind `unsafe` accessors, or `UnsafeCell` wrappers in a `globals` module with `pub(crate) unsafe fn window_manager() -> &'static mut WindowManager`. Do not reach for `Mutex` here — it would change the locking topology and can deadlock against the existing `window_animations_lock`.

### 2.3 File-local `#define` constants

| Macro | Line | Value | Use |
|---|---|---|---|
| `INSERT_FEEDBACK_WIDTH` | `view.c:6` | `2` | stroke width and the half-offsets at `view.c:27, 55-88` |
| `INSERT_FEEDBACK_RADIUS` | `view.c:7` | `9` | rounded-rect radius at `view.c:89` |

Both are used in `float`/`CGFloat` expressions where the integer literal is promoted. **Rust:** `const INSERT_FEEDBACK_WIDTH: f64 = 2.0;` and `const INSERT_FEEDBACK_RADIUS: f32 = 9.0;` — note `cgrect_clamp_x_radius` (`src/misc/helpers.h:542`) takes and returns `float`, while the `clip_*` arithmetic at `view.c:55-88` is in `CGFloat` (`f64`). Getting these widths wrong shifts the overlay by sub-pixel amounts.

### 2.4 Functions

Every function below runs on the **event-loop pthread** unless stated otherwise. Determination: see §0.1.

---

**`void insert_feedback_show(struct window_node *node)` — `view.c:8-103`**
Creates (first call) and then redraws the translucent overlay rectangle that shows where the next window will be inserted, clipping the fill to one edge according to `node->insert_dir`.
Allocations / ownership:
* `frame_region` — `CGSNewRegionWithRect` at `view.c:12`, `CFRelease` at `view.c:102`. Balanced.
* `empty_region` — `CGRegionCreateEmptyRegion` at `view.c:17`, `CFRelease` at `view.c:19`. Balanced.
* `node->feedback_window.id` — created at `view.c:18`, released only in `insert_feedback_destroy` (`view.c:116`).
* `node->feedback_window.context` — `SLWindowContextCreate` at `view.c:26`, released only in `insert_feedback_destroy` (`view.c:115`).
* `path` — `CGPathCreateWithRoundedRect` at `view.c:89`, `CGPathRelease` at `view.c:101`. Balanced.
* `table_add` at `view.c:43` copies 4 key bytes (`sizeof(node->window_order[0])`) and stores the **node pointer** as the value.
External symbols called: `CGSNewRegionWithRect`, `CGRegionCreateEmptyRegion`, `CFRelease`, `SLSNewWindowWithOpaqueShapeAndContext`, `SLSSetWindowResolution`, `SLSSetWindowOpacity`, `SLSSetWindowLevel`, `SLSSetWindowSubLevel`, `SLWindowContextCreate`, `CGContextSetLineWidth`, `CGContextSetRGBFillColor`, `CGContextSetRGBStrokeColor`, `SLSDisableUpdate`, `CGContextClearRect`, `CGContextFlush`, `SLSReenableUpdate`, `SLSOrderWindow`, `SLSSetWindowShape`, `CGContextClipToRect`, `CGContextFillRect`, `CGContextAddPath`, `CGContextStrokePath`, `CGContextResetClip`, `CGRectGetMidX`, `CGRectGetMidY`, `CGRectInset`, `CGPathCreateWithRoundedRect`, `CGPathRelease`. yabai-internal: `sls_window_disable_shadow`, `window_level`, `window_sub_level`, `table_add`, `workspace_is_macos_sequoia`, `workspace_is_macos_tahoe`, `update_window_notifications`, `cgrect_clamp_x_radius`, `cgrect_clamp_y_radius`.
Pitfalls for translation:
* `switch (node->insert_dir)` at `view.c:53` has **no `default:`** and `clip_x/clip_y/clip_w/clip_h` are uninitialised locals (`view.c:49`). Every call site guarantees `insert_dir != 0` (`view.c:327`, `view.c:686`, `window_manager.c:1794`, `event_loop.c:1329`), so the gap is unreachable — but Rust will force a value. Use `unreachable!()` in the `_` arm rather than silently picking zeros, so a future bug is loud instead of a misdrawn overlay.
* `uint64_t tags = (1ULL << 1) | (1ULL << 9);` (`view.c:16`) and the literal `13`, `64`, `2` arguments at `view.c:18` are SLS ABI magic; they must be passed as exactly `u64`/`i32`/`f32` per `src/misc/extern.h:26`.
* `SLSSetWindowOpacity(g_connection, id, 0)` (`view.c:23`) passes `0` to a `bool` parameter (`extern.h:31`).
* `node->window_order[0]` is read at `view.c:24, 25, 42, 43` even though a node with `window_count == 0` has `window_order[0] == 0`; `window_level(0)`/`window_sub_level(0)` are then queried for a nonexistent window id.

---

**`void insert_feedback_destroy(struct window_node *node)` — `view.c:105-119`**
Removes the overlay: unregisters from the `insert_feedback` table, orders the SLS window out, releases the context and the window, and zeroes `node->feedback_window`.
External symbols: `SLSOrderWindow`, `CGContextRelease`, `SLSReleaseWindow`, `memset`. yabai-internal: `table_remove`, `workspace_is_macos_sequoia`, `workspace_is_macos_tahoe`, `update_window_notifications`.
Note the ordering: `table_remove` (`view.c:108`) uses `node->window_order[0]` as the key, so it must run **before** anything mutates `window_order`. `view_remove_window_node` violates this — see §2.6, "dangling insert-feedback entry".

---

**`static inline struct area area_from_cgrect(CGRect rect)` — `view.c:121-124`**
`CGRect` (`f64` fields) → `struct area` (`f32` fields). Narrowing conversion; keep it explicit (`rect.origin.x as f32`).

**`static inline CGPoint area_max_point(struct area area)` — `view.c:126-129`**
`{ area.x + area.w - 1, area.y + area.h - 1 }`. The additions happen in `float` (all three operands are `float`), then each result widens to `CGFloat`. A Rust version that computes in `f64` will differ for large coordinates. Translate as `CGPoint { x: (area.x + area.w - 1.0) as f64, y: (area.y + area.h - 1.0) as f64 }`.

**`static inline enum window_node_child window_node_get_child(struct window_node *node)` — `view.c:131-134`**
`node->child` unless `CHILD_NONE`, in which case the space default.

**`static inline enum window_node_split window_node_get_split(struct view *view, struct window_node *node)` — `view.c:136-149`**
Resolution order: node override → view override (unless `SPLIT_AUTO`) → space default (unless `SPLIT_AUTO`) → `w >= h ? SPLIT_Y : SPLIT_X`. Note the asymmetry at `view.c:140-146`: if `view->split_type` is set at all, the space default is skipped even when the view's value is `SPLIT_AUTO`. A Rust `match` chain must reproduce that exactly; an "if let Some" style refactor will change behaviour.

**`static inline float window_node_get_ratio(struct window_node *node)` — `view.c:151-154`**
`in_range_ii(node->ratio, 0.1f, 0.9f) ? node->ratio : g_space_manager.split_ratio`. `in_range_ii` is `src/misc/macros.h:12`, inclusive on both ends.

**`static inline int window_node_get_gap(struct view *view)` — `view.c:156-159`**
`VIEW_ENABLE_GAP ? view->window_gap : 0`.

**`static void area_make_pair(enum window_node_split split, int gap, float ratio, struct area *parent_area, struct area *left_area, struct area *right_area)` — `view.c:161-184`**
Splits a rectangle into two along `split`, leaving `gap` between them.
**This is the single most translation-sensitive function in the file.** Look at `view.c:167-172`:
```
float left_width  = (parent_area->w - gap) * ratio;
float right_width = (parent_area->w - gap) * (1 - ratio);
left_area->w   = (int)left_width;
right_area->w  = (int)right_width;
right_area->x += (int)(left_width + 0.5f) + gap;
```
The widths are **truncated** (`(int)`), but the right edge's offset is **rounded** (`(int)(left_width + 0.5f)`) — the two are deliberately different, which is what keeps a 1px seam from appearing. The `(int)` results are then stored back into `float` fields. Rust: `left_area.w = (left_width as i32) as f32;` and `right_area.x += ((left_width + 0.5) as i32 + gap) as f32;`. Do **not** use `.round()`, `.floor()` or `.trunc()` — `as i32` in Rust truncates toward zero and *saturates* on overflow, matching C's truncation for all in-range values while turning C's UB on out-of-range into a defined clamp (an improvement, not a behaviour change for any reachable input).
`gap` is an `int` promoted to `float` in `parent_area->w - gap`; in Rust write `parent_area.w - gap as f32`.
The `else` branch (`view.c:173-183`) is the same code on `h`/`y`; the local names stay `left_width`/`right_width` even though they are heights.

**`static void area_make_pair_for_node(struct view *view, struct window_node *node)` — `view.c:186-196`**
Resolves split/ratio/gap, calls `area_make_pair` on `node->left->area` and `node->right->area`, then **writes the resolved values back** into `node->split` and `node->ratio` (`view.c:194-195`). That write-back is observable: `window.c:587` prints `node->parent->split`, and `window_node_rotate` (`view.c:471-486`) branches on it.
Requires `node->left` and `node->right` to be non-NULL; the only guard is the caller's `window_node_is_leaf` check (`view.c:326`).

**`static inline bool window_node_is_occupied(struct window_node *node)` — `view.c:198-201`** — `window_count != 0`.
**`static inline bool window_node_is_intermediate(struct window_node *node)` — `view.c:203-206`** — `parent != NULL`.
**`static inline bool window_node_is_leaf(struct window_node *node)` — `view.c:208-211`** — `left == NULL && right == NULL` (both, not either).
**`static inline bool window_node_is_left_child(struct window_node *node)` — `view.c:213-216`** — `parent && parent->left == node`. Pointer identity comparison.
**`static inline bool window_node_is_right_child(struct window_node *node)` — `view.c:218-221`** — `parent && parent->right == node`.

**`static void window_node_equalize(struct window_node *node, uint32_t axis_flag)` — `view.c:223-235`**
Post-order recursion resetting `ratio` to the space default on every node whose `split` matches a bit in `axis_flag`. `axis_flag` is the `SPLIT_Y|SPLIT_X` bitmask, not an enum value.

**`static inline struct balance_node balance_node_add(struct balance_node a, struct balance_node b)` — `view.c:237-240`** — componentwise add.

**`static struct balance_node window_node_balance(struct window_node *node, uint32_t axis_flag)` — `view.c:242-275`**
Post-order recursion that sets each matching node's `ratio` to `left_leafs / total_leafs` so every leaf gets equal area.
Integer→float: `(float) left_leafs.y_count / total_leafs.y_count` (`view.c:257`) — the numerator is cast, the denominator is implicitly converted; if `total_leafs.y_count` is `0` this is `0.0f/0.0f = NaN` or `x/0 = ±inf`. Rust's `f32` division has identical IEEE semantics, so a direct translation preserves it — but note that a `ratio` of NaN then fails `in_range_ii` at `view.c:153` and silently falls back to the space default, which is the (accidental) safety net.
Bool→int at `view.c:246-247, 270-271`: `node->parent->split == SPLIT_Y` yields `0`/`1`. Rust: `(cond) as i32`.

**`static void window_node_split(struct view *view, struct window_node *node, struct window *window)` — `view.c:277-322`**
Turns a leaf into an internal node with two fresh leaves, moving the existing windows into one child and the new window into the other, according to `window_node_get_child`.
Allocates: two `struct window_node` via `malloc` + `memset` (`view.c:279-283`); freed by `window_node_destroy` (`view.c:345`) or `view_remove_window_node` (`view.c:718-719`).
The `zoom` re-parenting expression (`view.c:285-291`) is a four-way nested ternary:
```
!g_space_manager.window_zoom_persist ? NULL
: !node->zoom                        ? NULL
: node->zoom == node->parent         ? node
: view->root
```
Rust: an explicit `if/else if` chain, or a small named helper. Keep the pointer-identity comparison (`node->zoom == node->parent`).
`memcpy(left->window_list, node->window_list, sizeof(uint32_t) * node->window_count)` (`view.c:294`) — Rust: `left.window_list[..n].copy_from_slice(&node.window_list[..n])`.
**Does not check `window_node_get_child` against a null `node`** — `view_add_window_node_with_insertion_point` can pass `leaf == NULL` if `view_find_min_depth_leaf_node` returns `NULL` (`view.c:794, 797`).

**`void window_node_update(struct view *view, struct window_node *node)` — `view.c:324-333`**
Recomputes areas top-down; on a leaf, redraws the insert feedback if `insert_dir` is set. Recursive — depth equals tree depth (bounded by window count).

**`static void window_node_destroy(struct window_node *node)` — `view.c:335-346`**
Post-order free of the subtree, unmanaging every window it holds and destroying the feedback overlay. Calls `window_manager_remove_managed_window` (`window_manager.c`), `insert_feedback_destroy`, `free`.

**`static void window_node_clear_zoom(struct window_node *node)` — `view.c:348-356`** — sets `zoom = NULL` over the whole subtree.

**`void window_node_capture_windows(struct window_node *node, struct window_capture **window_list)` — `view.c:358-372`**
Walks the subtree and appends one `window_capture` per live window, using `node->zoom->area` when zoomed.
**Arena:** `ts_buf_push` (`src/misc/sbuffer.h:48`) at `view.c:365` — a stretchy buffer whose header lives in the temp-storage arena. The `**window_list` double pointer exists because `ts_buf_push` reassigns the buffer pointer. Only valid until `ts_reset()` at the end of the current event.
**Rust:** take `&mut Vec<WindowCapture>`; the arena buys nothing once the Rust side owns a `Vec`. If phase 2 wants to keep the arena for allocation-count parity, use a `bumpalo::collections::Vec` in a per-event `Bump` that is `reset()` where `ts_reset()` is called. Recommendation: plain `Vec` — the arena's only contract here is "freed at end of event", which `Vec`'s drop satisfies, and the `ts_expand`-adjacency assertions (`ts.h:78`) are a constant source of fragility.

**`void window_node_flush(struct window_node *node)` — `view.c:374-379`**
Captures the subtree and hands it to `window_manager_animate_window_list(window_list, ts_buf_len(window_list))`. `ts_buf_len(NULL) == 0` (`sbuffer.h:45`), hence the `if (window_list)` guard.

**`bool window_node_contains_window(struct window_node *node, uint32_t window_id)` — `view.c:381-388`** — linear scan of `window_list[0..window_count]`.

**`int window_node_index_of_window(struct window_node *node, uint32_t window_id)` — `view.c:390-397`**
Linear scan; **returns `0` when not found**, not `-1`. The one caller (`window.c:601`) adds 1 and prints it as `stack-index`, so "not found" and "index 0" are indistinguishable by design. Rust must return `i32`, not `Option<usize>`, or the JSON output changes.

**`void window_node_swap_window_list(struct window_node *a_node, struct window_node *b_node)` — `view.c:399-419`**
Swaps both arrays and the counts through two fixed 32-element stack buffers, then clears both `zoom` pointers.
`uint32_t tmp_window_count;` (`view.c:403`) is `uint32_t` while `window_count` is `int` — a silent signed/unsigned round trip. Harmless for 0..32; in Rust just use `i32` throughout (or `usize` consistently) and note the divergence is unobservable.
**Rust:** `std::mem::swap` on the two arrays plus the counts; no temporaries needed. This changes no observable behaviour because the arrays are `Copy` and fixed-size.

**`struct window_node *window_node_find_first_leaf(struct window_node *root)` — `view.c:421-428`** — descend `left` until leaf. Never returns NULL for a non-NULL root.
**`struct window_node *window_node_find_last_leaf(struct window_node *root)` — `view.c:430-437`** — descend `right`.
**`struct window_node *window_node_find_prev_leaf(struct window_node *node)` — `view.c:439-452`** — recursive; returns `NULL` at the root.
**`struct window_node *window_node_find_next_leaf(struct window_node *node)` — `view.c:454-467`** — mirror image.
These four are the iteration protocol used by `for (node = first_leaf(root); node; node = next_leaf(node))` at `view.c:594, 614, 826, window_manager.c:1146, 1165`. **Rust:** implement `Iterator` over leaves in phase 3, but in phase 2 keep the four functions so the loops translate one-to-one.

**`void window_node_rotate(struct window_node *node, int degrees)` — `view.c:469-492`**
Rotates the whole tree by 90/180/270 degrees: conditionally swaps children and inverts `ratio`, conditionally flips `split`, then recurses. Note it mutates `node` *before* the leaf check (`view.c:488`), so leaves also get their `split`/`ratio` touched. Not declared in `view.h`; called from `space_manager.c:401`.

**`struct window_node *window_node_mirror(struct window_node *node, enum window_node_split axis)` — `view.c:494-507`**
Post-order; swaps children where `split == axis`. Returns `node` so the recursive calls can be written inline. Not in `view.h`; called from `space_manager.c:413`.

**`struct window_node *window_node_fence(struct window_node *node, int dir)` — `view.c:509-523`**
Walks up the parent chain for the first ancestor whose split line lies on the requested side. Not in `view.h`; called from `window_manager.c:382-385`. Uses `float` comparisons on `area` fields — exact `<`/`>` on floats, which Rust reproduces exactly.

**`struct window_node *view_find_min_depth_leaf_node(struct window_node *node)` — `view.c:525-539`**
Breadth-first search for the shallowest leaf, using a fixed 256-entry stack array as the queue.
```
struct window_node *list[256] = { node };
for (int i = 0, j = 0; i < 256; ++i) {
    if (window_node_is_leaf(list[i])) return list[i];
    list[++j] = list[i]->left;
    list[++j] = list[i]->right;
}
```
**Latent out-of-bounds write.** After iteration `i`, `j == 2*i + 2`. At `i == 127` the code writes `list[255]` and then `list[256]` — one element past the array — and it keeps going for `i > 127`. This is reachable when the first 128 BFS nodes are all internal, i.e. a near-complete BSP tree with ≳129 leaves on one space. Phase 2 must not reproduce the overflow: use a `VecDeque` or a `Vec` with a real bound and return `None` when exhausted. Note the `{ node }` initialiser zero-fills `list[1..255]`, which the loop relies on never reading.
Also: `list[i]->left` / `->right` are dereferenced without a NULL check, safe only because `window_node_is_leaf` requires *both* to be NULL and the tree only ever creates children in pairs (`view.c:317-318`).

**`static inline bool area_is_in_direction(struct area *r1, CGPoint r1_max, struct area *r2, CGPoint r2_max, int direction)` — `view.c:541-561`**
"Is `r2` in `direction` from `r1`, with overlap on the perpendicular axis?" Mixed `float` (`r1->x`) and `CGFloat` (`r1_max.x`) comparisons — in C the `float` promotes to `double`. Rust must cast the `Area` fields to `f64` at each comparison, otherwise near-boundary cases flip. This function is directly exercised by the test suite (`tests/src/area.c:33-43`).

**`static inline int area_distance_in_direction(struct area *r1, CGPoint r1_max, struct area *r2, CGPoint r2_max, int direction)` — `view.c:563-581`**
Returns the absolute gap along the axis as an **`int`** — a `double` expression truncated toward zero on return (`view.c:567, 570, 573, 576`), and `INT_MAX` for an unknown direction (`view.c:580`). Rust: compute in `f64`, return `(expr) as i32`. A translation that keeps `f64` and compares floats will change the tie-breaking at `view.c:601` (`distance == best_distance`), which is what selects between two equidistant neighbours by stacking rank.

**`struct window_node *view_find_window_node_in_direction(struct view *view, struct window_node *source, int direction)` — `view.c:583-610`**
Picks the best neighbour leaf in a direction, breaking distance ties by the window's rank in the space's front-to-back order.
Allocates: `space_window_list(view->sid, &window_count, false)` (`view.c:586`) returns **arena** memory (`src/space.c:34`, `ts_alloc_list`); never freed, correct.
Returns `NULL` early if `space_window_list` returns `NULL` (`view.c:587`) — note this happens when the space has zero windows *or* when the SLS query fails, and the early return skips the whole search.
External: `space_window_list`, `window_manager_find_rank_of_window_in_list` (`window_manager.c:898`, returns `INT_MAX` when absent).

**`struct window_node *view_find_window_node(struct view *view, uint32_t window_id)` — `view.c:612-619`** — linear leaf scan; `NULL` if not found. Called ~30 times across the codebase.

**`struct window_node *view_remove_window_node(struct view *view, struct window *window)` — `view.c:621-728`**
The tree-collapse path. Three cases:
1. `window_count > 1` (`view.c:626-651`): remove the id from `window_list` and `window_order` with two `memmove`s driven by two `bool` latches, `assert` both fired, decrement the count, repoint `view->insertion_point` if it named the removed window, return `NULL`.
2. node is the root (`view.c:653-659`): clear the insertion point, destroy the overlay, `memset` the node to zero, `view_update`, return `NULL`.
3. otherwise (`view.c:661-727`): the sibling's contents are hoisted into the parent, the parent adopts the sibling's grandchildren, `zoom` pointers are re-parented with the same four-way ternary as `window_node_split`, both the node and the sibling are `free`d, and if auto-balance is on the whole tree is rebalanced and `view->root` is returned instead of `parent`.

Allocations/frees: `free(child)` at `view.c:718` and `free(node)` at `view.c:719`. **`child`'s own `left`/`right` are not freed** — they were re-parented onto `parent` at `view.c:690-708` — but only when `window_node_is_intermediate(child) && !window_node_is_leaf(child)`; when `child` is a leaf there is nothing to re-parent, which is correct.
`assert(removed_entry)` / `assert(removed_order)` at `view.c:642-643` compile out under `-DNDEBUG` (the `install` target, `makefile:27`). In Rust use `debug_assert!` to match, not `assert!`.
**Dangling insert-feedback entry** (see §2.6).

**`void view_stack_window_node(struct window_node *node, struct window *window)` — `view.c:730-749`**
Inserts `window` into a leaf's stack immediately after the currently-visible window in `window_list`, and at the front of `window_order`.
**No bounds check against `NODE_MAX_WINDOW_COUNT`.** With a full node (`window_count == 32`) the `memmove` at `view.c:742` and the writes at `view.c:745-747` run one past the arrays and `++node->window_count` reaches 33. `mouse_handler.c:139` guards its call; `window_manager.c:1822` and `view.c:779` and `view.c:807` do not. In Rust this becomes a panic — which is a behaviour change, but the C behaviour is memory corruption, so the right move is an explicit early return plus a note in the phase-2 port log.

**`struct window_node *view_add_window_node_with_insertion_point(struct view *view, struct window *window, uint32_t insertion_point)` — `view.c:751-812`**
Decides where a new window goes: empty root → root; `VIEW_BSP` → the explicit insertion point (honouring a pending `STACK` insert direction), else the space's insertion policy, else the min-depth leaf, then split; `VIEW_STACK` → stack onto the root; anything else → `NULL`.
Note `view.c:763-770`: `view->insertion_point` is temporarily overwritten with the argument and restored from `prev_insertion_point` **inside** the `if (view->insertion_point)` block (`view.c:769`). If `insertion_point` was passed but `view->insertion_point` ends up `0`, the restore never runs — reachable only if `insertion_point == 0`, in which case the overwrite did not happen either. Faithful translation: keep the same control flow, do not "clean it up".
Returns `view->root` rather than the affected leaf when auto-balance is on (`view.c:799-803`) — callers such as `mouse_handler.c:181` compare the returned pointer against another node (`src_node_rm != src_node_add`), so the identity of the returned pointer is observable.

**`struct window_node *view_add_window_node(struct view *view, struct window *window)` — `view.c:814-817`** — thin wrapper, `insertion_point = 0`.

**`uint32_t *view_find_window_list(struct view *view, int *window_count)` — `view.c:819-838`**
Collects every window id in the tree into an arena list.
**Latent arena overflow.** `capacity` starts at 13 (`view.c:823`); the growth check at `view.c:827` expands by exactly one doubling per *node*, but the node can contribute up to 32 ids. With `*window_count == 0`, `capacity == 13` and a leaf holding 32 windows, the branch expands to 26 and then writes 32 entries — 6 `uint32_t` past the allocation, into whatever the arena hands out next. Phase 2 should use a `Vec<u32>` (which cannot under-grow) and record the divergence.
`ts_expand(window_list, sizeof(uint32_t) * capacity, sizeof(uint32_t) * capacity)` (`view.c:828`) asserts that `window_list` is the most recent arena allocation (`ts.h:78`) — so **nothing else may allocate from the arena between `ts_alloc_list` at `view.c:824` and the last `ts_expand`**. That invariant is invisible at the call site and is a strong argument for `Vec` here.

**`bool view_is_invalid(struct view *view)` — `view.c:840-843`** — `!(flags & VIEW_IS_VALID)`.
**`bool view_is_dirty(struct view *view)` — `view.c:845-848`** — `flags & VIEW_IS_DIRTY`, implicitly narrowed from `uint64_t` to `bool`.

**`void view_flush(struct view *view)` — `view.c:850-858`** — if the space is visible, push the frames and clear the dirty bit; otherwise set it.

**`void view_serialize(FILE *rsp, struct view *view, uint64_t flags)` — `view.c:860-966`**
Writes one space object as JSON into the response `FILE *`.
* `TIME_FUNCTION` (`view.c:862`) expands to nothing unless `PROFILE >= 2` (`src/misc/timer.h:153`). The daemon never builds with `PROFILE`; only `tests/makefile:10` sets `PROFILE=1`, which is still below the threshold. **Rust: drop it, or gate a `#[cfg(feature = "profile2")]` scope guard.**
* `if (flags == 0x0) flags |= ~flags;` (`view.c:864`) — "no filter means all fields". In Rust: `let flags = if flags == 0 { u64::MAX } else { flags };`.
* `did_output` (`view.c:866`) drives the comma placement; note the **last** branch (`SPACE_PROPERTY_IS_FULLSCREEN`, `view.c:959-963`) deliberately does not set it.
* `fprintf(rsp, "\t\"id\":%lld", view->sid)` (`view.c:870`) prints a `uint64_t` with the **signed** `%lld`. Unobservable for real space ids, but a Rust `{}` on `u64` is not literally identical — document it and move on.
* `ts_cfstring_copy(view->uuid)` (`view.c:877`) allocates from the arena and returns `NULL` on conversion failure, handled at `view.c:878`. It does **not** handle `view->uuid == NULL`, which `SLSSpaceCopyName` can return (`view.c:995`) — `CFStringGetLength(NULL)` would crash. Rust: `view.uuid` is `Option<CFString>`; print `"<unknown>"` for `None`, which also fixes the crash.
* `space_window_list(view->sid, &window_count, true)` (`view.c:915`) — arena, not freed, and **not NULL-checked** before the `for` loop at `view.c:918`; the loop is a no-op when `window_count == 0`, so the missing check is benign only because `*count` is left at its initialiser `0` on the failure path… except `space_window_list_for_connection` sets `*count` at `src/space.c:29` before the `goto out` at `:30`. Keep the `int window_count = 0` initialiser.
* External symbols: `fprintf`, `space_manager_mission_control_index`, `space_manager_get_label_for_space`, `display_manager_display_id_arrangement`, `space_display_id`, `space_window_list`, `window_node_find_first_leaf`, `window_node_find_last_leaf`, `json_bool` (`helpers.h:233`), `space_is_visible`, `space_is_fullscreen`, `ts_cfstring_copy`.
**Rust:** keep writing into an `impl std::io::Write` (the response socket wrapped in a `BufWriter`), producing byte-identical output including the `\t` indentation and the `%f`/`%d` formats. Do **not** reach for `serde_json` — the field order, the tab indentation and the `"%d, "` separator in the `windows` array are part of the wire format that `yabai -m query` consumers parse.

**`void view_update(struct view *view)` — `view.c:968-984`**
Recomputes the root rectangle from the display bounds, applies padding, recurses, sets `VIEW_IS_VALID | VIEW_IS_DIRTY`.
`view->root->area.x += view->left_padding;` (`view.c:975`) — `int` added to `float`; Rust needs `as f32`.
External: `space_display_id`, `display_bounds_constrained` (`display.c:123`).

**`struct view *view_create(uint64_t sid)` — `view.c:986-1015`**
`malloc` + `memset` the view and its root node, set `sid`, copy the space name, enable padding and gap, and — only for user spaces — seed every setting from the space manager and lay out. Non-user spaces become `VIEW_FLOAT`.
The `if (!view_check_flag(view, VIEW_X))` guards at `view.c:1001-1008` are always true here (the struct was just zeroed), so they are dead as written but must be kept for behavioural identity if phase 2 ever reuses the function.
Owns: the returned `struct view *` (caller `space_manager_find_view` stores it in the table, `space_manager.c:112`), `view->root`, and `view->uuid` (`SLSSpaceCopyName`, a Copy-rule CF reference).
External: `malloc`, `memset`, `SLSSpaceCopyName`, `space_is_user`.

**`void view_clear(struct view *view)` — `view.c:1017-1031`**
Destroys both subtrees, unmanages the root's own windows, destroys the overlay, zeroes the root node **in place** (keeping the allocation), and re-lays out. The in-place `memset` at `view.c:1028` is why `view->root` is never reallocated and why other code may cache `view->root`.

**`void view_destroy(struct view *view)` — `view.c:1033-1042`**
Destroys the tree and releases the uuid. **Does not `free(view)`** — the caller does, at `src/event_loop.c:989`.
**Rust:** if this becomes `impl Drop for View`, the `free(view)` at the call site must go away at the same time, or the view is dropped twice. Safest phase-2 shape: keep `fn destroy(&mut self)` explicit, and let the caller drop the `Box<View>`.

### 2.5 Callbacks registered with the OS or the run loop

`view.c` registers **none**. It only *creates* SLS windows (`view.c:18`) and draws into their contexts; the notification subscription those windows depend on is refreshed by calling `update_window_notifications()` (`view.c:45`, `view.c:111`), which ultimately calls `SLSRequestNotificationsForWindows` (`event_loop.c:32`). The notifications themselves are delivered to `connection_handler`, registered in `src/yabai.c:322-334`, which is outside this assignment.

The one thing to record for phase 2: the *context pointer* stored on yabai's behalf is `node` in `table_add(&g_window_manager.insert_feedback, &node->window_order[0], node)` (`view.c:43`). Its lifetime is the node's lifetime, and the only remover is `insert_feedback_destroy` (`view.c:108`) — see the dangling-entry note below.

### 2.6 Latent defects a faithful translation must decide about

These are not style issues; each one is a place where "translate it exactly" and "make it compile in safe Rust" disagree.

1. **`view_find_min_depth_leaf_node` writes past `list[255]`** — `view.c:534-535`, analysed above.
2. **`view_find_window_list` can write past its arena allocation** — `view.c:827-834`, analysed above.
3. **`view_stack_window_node` has no `NODE_MAX_WINDOW_COUNT` bound** — `view.c:741-748`, analysed above.
4. **Dangling `insert_feedback` table entry in `view_remove_window_node`.** At `view.c:667-668` the parent copies the sibling's `window_list`/`window_order`. At `view.c:681-687`, when the sibling had an overlay, the parent inherits `child->feedback_window` (with a non-zero `id`) and `insert_feedback_show(parent)` is called — which takes the `if (!node->feedback_window.id)` branch as *false* (`view.c:15`) and therefore never runs `table_add` for the parent. Meanwhile the table still holds the entry keyed by the sibling's `window_order[0]` pointing at `child`, which is `free`d at `view.c:718`. The next `table_find(&g_window_manager.insert_feedback, &wid)` at `src/event_loop.c:948` can then dereference freed memory. An index-based node arena (§1.2, `struct window_node`) makes this a stale-index lookup instead of a use-after-free; a raw-pointer translation reproduces the bug exactly.
5. **`view_serialize` does not guard `view->uuid == NULL`** — `view.c:877`.
6. **Unsynchronised `proxy.tx/ty/tw/th`** — written on the CVDisplayLink thread (`window_manager.c:560-563`) outside `window_animations_lock`, read on the event-loop thread inside it (`window_manager.c:635-642`). Rust will not let this compile without `unsafe` or atomics; the honest fix is `AtomicU32` bit-patterns or moving the reads under the lock. Flag it for the `window_manager` reader — the struct is declared here but the race lives there.

### 2.7 Catalogue of C patterns in `view.c` and the recommended Rust translation

| # | Pattern | `path:line` examples | Recommended Rust | What a naive translation breaks |
|---|---|---|---|---|
| 1 | **Intrusive binary tree with parent back-edges and cross-edges** | `view.h:155-158`; `view.c:313-318`, `view.c:441-466`, `view.c:513` | Phase 2: `*mut WindowNode` from `Box::into_raw`, freed with `Box::from_raw`, all traversal in `unsafe`. Phase 3: `Vec<WindowNode>` arena + `NodeId(u32)` indices, which also dissolves defects 4 and 6. | `Rc<RefCell<>>` forces `Weak` for `parent`/`zoom` and makes `node->zoom == node->parent` (pointer identity, `view.c:289`) awkward and slow; `Box` alone cannot express the back-edge at all. |
| 2 | **`malloc` + `memset(0)` as "new"** | `view.c:279-283`, `view.c:988-992` | `Box::new(WindowNode::default())` with `#[derive(Default)]`. | None, provided every field's `Default` really is the zero bit pattern — check `enum` discriminants (`SPLIT_NONE = 0`, `CHILD_NONE = 0`, `VIEW_DEFAULT = 0` all are). |
| 3 | **`memset(node, 0, sizeof(...))` as in-place reset, keeping the allocation** | `view.c:117`, `view.c:656`, `view.c:1028` | `*node = WindowNode::default();` — keeps the address, which callers rely on (`view->root` is cached elsewhere). | Replacing the `Box` instead of the value changes `view->root`'s identity and breaks every cached pointer. |
| 4 | **Fixed-size arrays with a separate count** | `view.h:159-161`; `view.c:294-310`, `view.c:401-415`, `view.c:632-637` | `[u32; NODE_MAX_WINDOW_COUNT]` + `window_count: i32`, with slice ops (`copy_from_slice`, `copy_within`). | `Vec` changes the struct size and the `memset`-reset semantics; slices will *panic* where C silently overran (defect 3) — that is the desired change, but it must be logged. |
| 5 | **`memmove` on overlapping ranges** | `view.c:632`, `view.c:637`, `view.c:742`, `view.c:746` | `slice::copy_within(range, dest)` — it is the `memmove` equivalent, unlike `copy_from_slice`. | `copy_from_slice` on overlapping ranges does not compile with one slice, and splitting the borrow gives `memcpy` semantics (UB-equivalent scrambling) for the overlapping case. |
| 6 | **Bit flags in a `uint64_t` with `check/set/clear` macros** | `view.h:185-220`; `view.c:158, 842, 847, 854, 856, 974, 982-983, 997-1008` | `bitflags!` on `ViewFlags(u64)`. | Nothing, as long as `view_check_flag` stays a boolean test everywhere (verified). |
| 7 | **Bitmask masquerading as an enum (`auto_balance`)** | `view.h:138-143`, `view.h:214`; `view.c:228, 232, 255, 262, 721, 799` | Separate `AutoBalance: u32` bitflag type; keep `auto_balance_str` as a 4-entry table indexed by the raw `u32`. | Modelling it as `WindowNodeSplit` makes `axis_flag & SPLIT_Y` inexpressible and breaks `space auto_balance on` (value `3`). |
| 8 | **Int constants used as an "enum" (`insert_dir`)** | `view.h:165`; `view.c:53-84`, `view.c:327`, `view.c:681-683`, `view.c:773-776` | `i32` with the `DIR_*`/`STACK` consts from `macros.h`, or a `#[repr(i32)]` enum with an explicit `None = 0` variant. | `DIR_NORTH` is `360`, not `0` — an auto-numbered Rust enum silently changes every stored value and every wire/CLI mapping. |
| 9 | **Hash table keyed by a pointer-to-field, value is a raw node pointer** | `view.c:43`, `view.c:108`; impl at `src/misc/hashtable.h:117-137` | `HashMap<u32, NodeId>` (phase 3) or `HashMap<u32, *mut WindowNode>` (phase 2). The C copies `sizeof(uint32_t)` key bytes and stores the value pointer verbatim. | Using `*mut WindowNode` as the *key* instead of `window_order[0]` changes lookup semantics — `event_loop.c:948` looks up by window id. |
| 10 | **Arena (`ts_*`) allocation with per-event reset** | `view.c:365` (`ts_buf_push`), `view.c:824` (`ts_alloc_list`), `view.c:828` (`ts_expand`), `view.c:877` (`ts_cfstring_copy`), and `space_window_list` at `view.c:586, 915` | `Vec<T>` / `String` owned by the caller. If arena parity is wanted, `bumpalo::Bump` reset where `ts_reset()` is called. | `ts_expand` asserts the buffer is the newest arena allocation (`ts.h:78`); any interleaved arena allocation aborts under `-DDEBUG` and corrupts under `-DNDEBUG`. A `Vec` removes the hazard and fixes defect 2. |
| 11 | **Stretchy buffer via out-parameter double pointer** | `view.c:358` signature, `view.c:365-377` | `fn capture_windows(&self, out: &mut Vec<WindowCapture>)`. | The `**` exists purely because the macro reallocates; keeping a double pointer in Rust buys nothing and needs `unsafe`. |
| 12 | **`printf` into a `FILE *` response stream** | `view.c:867-965` (22 `fprintf` calls) | `write!(response, ...)` against an `impl io::Write`; keep every literal, `\t`, `\n`, comma and format specifier. | `serde_json` or `{:?}` changes field order, indentation and float formatting — `yabai -m query` consumers parse this. `%f` on a `float`/`double` prints 6 decimals; Rust's `{}` does not. None of `view.c`'s own writes use `%f`, but `message.c` ones do — keep a shared `c_float_fmt` helper. |
| 13 | **X-macro list expanded into enum + two parallel arrays** | `view.h:7-40` | One `const [(&str, u64); N]` table plus a `bitflags` type. | Two hand-written Rust arrays will drift; `message.c:2481` relies on index parallelism and on `array_count`. |
| 14 | **String tables indexed by enum discriminant** | `view.h:101, 115, 130, 138, 177` | `const [&str; N]` indexed by `variant as usize`, or an `impl Display`. | The `CHILD_SECOND` / `CHILD_FIRST` ordering (`view.h:110-112`) is *not* alphabetical or intuitive — reordering the Rust enum changes `yabai -m query --windows` output. |
| 15 | **`static inline` helpers that are actually cross-TU** | §0.2 table | `pub(crate) fn` (or `pub` for the ones the tests use). `#[inline]` is optional; LLVM will inline them anyway. | Marking them private per-module breaks `display_manager`, `window_manager`, `space_manager`, `window`, and the test binary. |
| 16 | **Deeply nested conditional expression (4-way ternary)** | `view.c:285-291`, `view.c:673-679`, `view.c:692-698`, `view.c:702-708` | Explicit `if / else if / else` chain, or a named helper like `reparented_zoom(...)`. Keep pointer-identity comparisons (`==` on `*mut`). | Rust's `match` on a reference will not do pointer identity — use `std::ptr::eq`. |
| 17 | **Float→int truncation stored back into a float** | `view.c:170-172`, `view.c:180-182`, `view.c:567-576` | `(x as i32) as f32`; `(expr) as i32` for the distance. | `.round()`/`.floor()` changes the layout by a pixel; the asymmetry between `(int)left_width` and `(int)(left_width + 0.5f)` is deliberate. |
| 18 | **Int→float implicit promotion** | `view.c:167` (`w - gap`), `view.c:975-978` (`+= left_padding`), `view.c:257` (`/ total_leafs.y_count`) | Explicit `as f32`. | Rust has no implicit numeric conversion, so this is a compile error rather than a silent bug — but choosing `f64` instead of `f32` for the intermediate changes results. |
| 19 | **`float` vs `CGFloat` (`f64`) mixed comparisons** | `view.c:543-557`, `view.c:601`, `view.h:5` | Cast `Area` fields to `f64` at the comparison, mirroring C's usual arithmetic conversions. | Doing the comparison in `f32` flips boundary cases in `area_is_in_direction`, which `tests/src/area.c` pins down. |
| 20 | **Aggregate initialisers / compound literals** | `view.c:11`, `view.c:86-88`, `view.c:123`, `view.c:128`, `view.c:239`, `view.c:245-248`, `view.c:365` | Struct literals; `CGRect`/`CGPoint` come from the `core-graphics` crate or hand-rolled `#[repr(C)]` types. | `CGRect` must be `#[repr(C)]` with `f64` fields on both x86_64 and aarch64 — it is passed by value to SLS/CG. |
| 21 | **CF retain/release pairing** | `view.c:12/102`, `view.c:17/19`, `view.c:89/101`, `view.c:26/115`, `view.c:995/1040` | `core-foundation`'s `TCFType` wrappers (`CFRetain`/`CFRelease` in `Drop`), or explicit `unsafe { CFRelease(..) }` for phase-2 fidelity. | Auto-`Drop` wrappers around `CGContextRef` will double-release if `insert_feedback_destroy` also calls `CGContextRelease`. Pick one owner per handle. |
| 22 | **SLS private-API FFI with magic integer arguments** | `view.c:12, 18, 22-26, 38-42, 91-92, 100, 114-116`, prototypes at `src/misc/extern.h:18-39, 57` | `extern "C"` block mirroring `extern.h` exactly, linked with `-F/System/Library/PrivateFrameworks -framework SkyLight` via a `build.rs` `println!("cargo:rustc-link-...")`. | Argument widths matter: `SLSNewWindowWithOpaqueShapeAndContext` takes `uint64_t *tags`, `float x`, `float y`, `int tag_size` (`extern.h:26`). `SLSSetWindowOpacity` takes a C `bool` (`extern.h:31`), not an `int`. |
| 23 | **Recursion over the tree** | `view.c:225-226, 251-252, 330-331, 337-338, 353-354, 369-370, 444, 459, 489-490, 497-498` | Plain recursion; depth is bounded by window count. | None, but Rust's default 8 MB main-thread stack and the event-loop pthread's default stack apply the same way. |
| 24 | **`assert()` that compiles out under `-DNDEBUG`** | `view.c:642-643` | `debug_assert!`. | `assert!` in Rust is always on; using it changes a release-mode silent-miscount into a panic. |
| 25 | **Out-parameter for a count** | `view.c:819` (`int *window_count`), and the calls at `view.c:585-586, 914-915` | Return `Vec<u32>` (length is the count). | Only if phase 3 reorganises; in phase 2 keeping `&mut i32` is the one-to-one translation. |
| 26 | **"Not found" encoded as a valid value** | `view.c:396` (returns `0`), `window_manager.c:908` (returns `INT_MAX`), `view.c:580` (returns `INT_MAX`) | Keep the sentinel return type (`i32`), do not switch to `Option`. | `window.c:601` prints `index + 1`, and `view.c:601` compares against `INT_MAX` — an `Option` changes both. |
| 27 | **Profiling scope macro using `__attribute__((cleanup))`** | `view.c:862` (`TIME_FUNCTION`), definition at `src/misc/timer.h:137-139` | A `struct TimeBlock` with `Drop`, behind `#[cfg(feature = "profile2")]`; or simply omit, since no shipped build defines `PROFILE >= 2`. | Emitting it unconditionally adds runtime cost the C build does not have. |
| 28 | **Pointer-identity comparison used as logic** | `view.c:215, 220, 289, 296, 677, 696, 706` | `std::ptr::eq(a, b)` on raw pointers, or `NodeId` equality in an arena world. | `==` on `&T` in Rust compares *values* if `T: PartialEq` — a silent semantic flip. |

Patterns explicitly **not** present in `view.c`: no `goto`, no `setjmp`, no `regex.h`, no `fork`/`exec`, no inline asm or SIMD, no Objective-C message sends or blocks, no function-pointer tables, no tagged unions, no CAS/atomics. (`view.c` is the only major daemon source with none of those.)

### 2.8 Comments to carry over

**None.** `grep -n "//\|/\*" src/view.c` returns nothing. `view.c` has zero comments in 1042 lines. Phase 2 must produce a Rust `view` module with zero comments too.

---

## 3. `tests/src/tests.m`

### 3.1 Purpose

The whole test harness: a single-file program that stubs the two scripting-addition blobs, `#include`s the entire daemon unity build (`src/manifest.m`), defines three macros for declaring and checking tests, pulls in `area.c` as the only test body file, and runs every registered test from its own `main`, printing per-test wall time and a pass/fail tally.

### 3.2 Types, macros, X-macro list

| Name | Line | Definition |
|---|---|---|
| `TEST_SIG(name)` | `tests.m:8` | `bool test_##name(void)` — the test function signature generator |
| `test_function` | `tests.m:9` | `typedef TEST_SIG(function);` i.e. `typedef bool test_function(void);` — a function *type*, not a pointer type |
| `TEST_FUNC(name, code)` | `tests.m:11` | Generates `static bool test_<name>(void) { char *test_name = "<name>"; bool result = true; {code} return result; }` |
| `TEST_CHECK(r, e)` | `tests.m:12` | `if ((r) != (e)) { printf(...); result = false; }` — prints `test_name`, `__LINE__`, the stringified expressions and both values with `%d` |
| `TEST_ENTRY(name)` | `tests.m:16` | `{ #name, test_##name },` |
| `TEST_LIST` | `tests.m:17-19` | Two entries: `display_area_is_in_direction`, `closest_display_in_direction` |
| anonymous `tests[]` | `tests.m:21-26` | `static struct { char *name; test_function *func; } tests[] = { TEST_LIST };` |

Notes that matter for a Rust port:
* `TEST_CHECK` prints both operands with `%d`, so it only works for values that fit an `int`. `tests.m:12` passes `r` and `e` as `bool` in `area.c:34` and as `int` in `area.c:73` — both promote to `int`.
* `test_name` is declared inside the generated function and referenced only by `TEST_CHECK`; a test body with no `TEST_CHECK` would warn.
* The ANSI escape sequences use the GNU `\e` extension (`tests.m:12, 33, 43, 48-51`), not `\033`. Rust string literals have no `\e`; write `\x1b`.

### 3.3 Globals and statics

| Symbol | Line | Type | Initial value | Threads | Synchronisation |
|---|---|---|---|---|---|
| `__src_osax_payload` | `tests.m:1` | `unsigned char[1]` | zero | main only | none |
| `__src_osax_payload_len` | `tests.m:2` | `unsigned int` | `0` | main only | none |
| `__src_osax_loader` | `tests.m:3` | `unsigned char[1]` | zero | main only | none |
| `__src_osax_loader_len` | `tests.m:4` | `unsigned int` | `0` | main only | none |
| `tests` | `tests.m:21-26` | anonymous `static struct[2]` | the two `TEST_ENTRY` rows | main only | none, immutable |

The first four are **stubs** for the symbols `xxd -i` normally generates into `src/osax/payload_bin.c` and `src/osax/loader_bin.c` (`makefile:11, 33-34`), declared `extern` at `src/sa.h:4-7` and used at `src/sa.m:222, 226`. The test binary links without the real blobs by defining them as one zero byte with length `0`. **This is the pattern the Cargo build script must preserve:** the Rust test target needs the same zero-length stand-ins, or a cfg that excludes `sa`'s installer path.

The whole daemon's globals (`g_connection`, `g_window_manager`, …, defined in `src/yabai.c`) come along through the `#include` at `tests.m:6` and are left at their zero initialisers — no test currently touches them.

### 3.4 Functions

**`int main(int argc, char **argv)` — `tests.m:28-53`**
Runs on the process's main thread; no other thread is started because `event_loop_begin`/`message_loop_begin` are never called (yabai's own `main` is compiled out by `#ifndef TESTS`, `src/yabai.c:260`).
* `array_count(tests)` (`tests.m:32`) — `src/misc/macros.h:8`.
* `read_cpu_freq()` / `read_cpu_timer()` (`tests.m:35-36, 39, 41, 47`) — `src/misc/timer.h:24-64`, available only because `tests/makefile:10` passes `-DPROFILE=1`. On arm64 they are inline `mrs cntvct_el0` / `mrs cntfrq_el0`; on x86_64 `read_cpu_freq` calibrates against `mach_absolute_time` with a 100 ms busy-wait (`timer.h:40-56`).
* Allocates nothing; frees nothing.
* External symbols: `printf`, plus the inline-asm timer reads. `argc`/`argv` are unused (the file relies on the build not passing `-Wunused-parameter` as an error; `tests/makefile:10` passes no `-W` flags at all).
* Exit code: `EXIT_SUCCESS` only if every test passed (`tests.m:53`).

### 3.5 Callbacks

None.

### 3.6 C patterns and Rust translation

| Pattern | `path:line` | Rust |
|---|---|---|
| Source-level `#include` of another test body | `tests.m:14` (`#include "area.c"`) | `mod area;` or `include!` — but the real answer is `#[cfg(test)] mod tests` inside the `view` module, or a `tests/` integration test. |
| Whole-program `#include` of the unity build | `tests.m:6` | Gone entirely: Cargo tests link the crate. |
| Macro-generated test functions + registry array | `tests.m:8-26` | `#[test] fn display_area_is_in_direction() { ... }` with `assert_eq!`. `cargo test` replaces the whole harness (registry, timing, tally, exit code). |
| Assertion macro that records rather than aborts | `tests.m:12` | `assert_eq!` aborts; to keep "report every failure in one run" use a small `check!` macro accumulating into a `Vec<String>` and asserting it is empty at the end. Recommendation: use `assert_eq!` — the C harness's soft-fail buys nothing with two tests. |
| Function *type* typedef (not pointer) | `tests.m:9` | `type TestFunction = fn() -> bool;` (Rust has no bare function types in that position). |
| Stub symbols standing in for generated binary blobs | `tests.m:1-4` | `build.rs` emits the real blobs for the binary; for tests either emit empty ones or `#[cfg(test)]` an empty `&[u8]`. |
| `\e` ANSI escapes and manual ms formatting | `tests.m:12, 33, 43, 47-51` | `\x1b`; `cargo test` output replaces it. |
| Wall-clock timing via TSC/`cntvct_el0` | `tests.m:35-47` | `std::time::Instant`, or `cargo bench` / `criterion` if per-test timing is actually wanted. |
| `EXIT_SUCCESS`/`EXIT_FAILURE` from `main` | `tests.m:53` | `cargo test`'s own exit code. |

### 3.7 Comments to carry over

**None.** `grep -n "//\|/\*" tests/src/tests.m` returns nothing.

---

## 4. `tests/src/area.c`

### 4.1 Purpose

The only test body in the repository. It builds a fixed three-display arrangement and pins down two behaviours of `view.c`'s direction geometry: `area_is_in_direction` for a west/east pair, and a local re-implementation of "which display is closest in this direction" that mirrors `display_manager_find_closest_display_in_direction` (`src/display_manager.c:265-294`).

### 4.2 Types

**`struct test_area` — `area.c:1-5`**
```
struct area area;   // the view.h type
CGPoint area_max;   // cached area_max_point(area)
```
No pointers, no threading. **Rust:** `struct TestArea { area: Area, area_max: CGPoint }`, or just compute `area_max` at each call and drop the struct.

### 4.3 Globals and statics

None at file scope. Both `display_list` arrays (`area.c:30`, `area.c:69`) are function-local, non-static, uninitialised until `init_test_display_list` fills them.

### 4.4 Functions

**`static inline void init_test_display_list(struct test_area display_list[3])` — `area.c:7-26`**
Main thread (called from the generated test functions, which `main` calls directly). Fills three displays:

| index | x | y | w | h | `area_max` |
|---|---|---|---|---|---|
| 0 | `0` | `0` | `2560` | `1440` | `(2559, 1439)` |
| 1 | `-1728` | `0` | `1728` | `1117` | `(-1, 1116)` |
| 2 | `2560` | `0` | `1920` | `1080` | `(4479, 1079)` |

So display 1 sits to the west of display 0 and display 2 to the east; all three share `y = 0`. Note the `[3]` parameter is a pointer in C — the size is not checked.
Allocates nothing. External symbols: none (`area_max_point` is yabai's, `view.c:126`).

**`TEST_FUNC(display_area_is_in_direction, {...})` — `area.c:28-44`**
Expands (via `tests.m:11`) to `static bool test_display_area_is_in_direction(void)`. Main thread. Four checks:

| local | call | expected |
|---|---|---|
| `t1` (`area.c:33`) | display 0 → display 1, `DIR_WEST` | `true` |
| `t2` (`area.c:36`) | display 0 → display 1, `DIR_EAST` | `false` |
| `t3` (`area.c:39`) | display 0 → display 2, `DIR_WEST` | `false` |
| `t4` (`area.c:42`) | display 0 → display 2, `DIR_EAST` | `true` |

`DIR_WEST`/`DIR_EAST` are `270`/`90` (`src/misc/macros.h:26-29`).

**`static inline int closest_display_in_direction(struct test_area *display_list, int display_count, int source, int direction)` — `area.c:46-64`**
Main thread. A copy of the production loop at `src/display_manager.c:277-292`, returning the *index* of the nearest display in a direction, or `-1`. Uses `INT_MAX` as the initial best distance (`area.c:49`), so `area_distance_in_direction`'s own `INT_MAX` fallback (`view.c:580`) is never selected because `distance < best_distance` is false for equal values.
Allocates nothing. External: `INT_MAX` (`limits.h`, reached transitively through `manifest.m`).

**`TEST_FUNC(closest_display_in_direction, {...})` — `area.c:66-89`**
Main thread. Six checks over the three-display arrangement:

| line | source | direction | expected index |
|---|---|---|---|
| `area.c:72-73` | 0 | `DIR_WEST` | `1` |
| `area.c:75-76` | 1 | `DIR_WEST` | `-1` |
| `area.c:78-79` | 2 | `DIR_WEST` | `0` |
| `area.c:81-82` | 0 | `DIR_EAST` | `2` |
| `area.c:84-85` | 1 | `DIR_EAST` | `0` |
| `area.c:87-88` | 2 | `DIR_EAST` | `-1` |

Note the **shadowing**: the file-local helper `closest_display_in_direction` (`area.c:46`) and the test named `closest_display_in_direction` (`area.c:66`) coexist only because `TEST_FUNC` prefixes the generated symbol with `test_`.

### 4.5 Callbacks

None.

### 4.6 C patterns and Rust translation

| Pattern | `path:line` | Rust |
|---|---|---|
| Array parameter decaying to a pointer, size in the type only | `area.c:7` (`struct test_area display_list[3]`) | `&mut [TestArea; 3]` — Rust actually enforces the length. |
| Uninitialised stack array filled by an init function | `area.c:30-31`, `area.c:69-70` | A `const` fixture: `fn test_display_list() -> [TestArea; 3]`. |
| `array_count` on a local array | `area.c:72, 75, …` (`array_count(display_list)`) | `.len()`. |
| Integer-literal directions | `area.c:33-43, 72-88` (`DIR_WEST`/`DIR_EAST`) | Keep the `i32` constants (see pattern 8 in §2.7). |
| `INT_MAX` as "no result yet" | `area.c:49` | `i32::MAX`. |
| Sentinel `-1` for "not found" | `area.c:48, 63` | Keep `i32` and `-1` to mirror `display_manager.c:271`'s `best_did = 0` idiom, or return `Option<usize>` in the test only (the test is not production code, so `Option` is fine here). |
| Float literals assigned to `float` fields from `int` literals | `area.c:9-25` (`= 0`, `= 2560`, `= -1728`) | `0.0`, `2560.0`, `-1728.0` — exact for all six values, no precision question. |

The substantive translation risk is inherited from `view.c`: `area_max_point` computes in `f32` and widens to `f64` (`view.c:128`), and `area_is_in_direction` compares `f32` fields against `f64` maxima (`view.c:543-557`). All the values in this fixture are exactly representable in `f32` (|value| ≤ 4479, integral), so this specific test cannot detect a wrong choice of float width. **Phase 2 must not use these tests as evidence that the float widths are right.**

### 4.7 Comments to carry over

**None.** `grep -n "//\|/\*" tests/src/area.c` returns nothing.

---

## 5. `tests/makefile`

### 5.1 Purpose

Three-target makefile (`clean`, `build`, `run`, plus `all` chaining all three) that compiles the single-file test harness with `clang` and runs it. It is entirely separate from the root `makefile`.

### 5.2 Contents

| Line | Content | Notes |
|---|---|---|
| `1` | `.PHONY: clean build run all` | |
| `3` | `all: clean build run` | ordered chain |
| `5-6` | `clean: rm -rf ./bin` | |
| `8-10` | `build: mkdir -p ./bin` then the `clang` line | see below |
| `12-13` | `run: ./bin/tests` | no dependency on `build` |

The compile line (`tests/makefile:10`):
```
clang ./src/tests.m -o ./bin/tests -DTESTS -DPROFILE=1 \
  -F/System/Library/PrivateFrameworks \
  -framework Carbon -framework Cocoa -framework CoreServices \
  -framework CoreVideo -framework SkyLight
```

Differences from the daemon build (`makefile:4, 66-68`) that matter:

| Aspect | daemon | tests |
|---|---|---|
| sources | `src/manifest.m` **plus** `src/osax/payload_bin.c src/osax/loader_bin.c` (`makefile:11-12`) | `src/tests.m` only — the osax blobs are stubbed (`tests.m:1-4`) |
| `-DTESTS` | no | **yes** — compiles out yabai's `main` (`src/yabai.c:260`) |
| `-DPROFILE` | unset | `=1` — enables `read_cpu_timer`/`read_cpu_freq`/`profile_begin`/`profile_end_and_print` (`timer.h:4`), but **not** `TIME_FUNCTION`, which needs `>= 2` (`timer.h:94`) |
| `-std` | `c11` | default (`gnu17` for current clang) |
| warnings | `-Wall -Wextra` | none |
| optimisation | `-g -O0` (`all`), `-O3 -DNDEBUG` (`install`) | none (`-O0` default) |
| architectures | `-arch x86_64 -arch arm64` (universal) | host only |
| `-fno-objc-arc` | yes | **no** — but `tests.m` contains no ObjC that allocates, and `workspace.m`/`sa.m` come in through the unity include, so ARC-vs-MRR only compiles because those files avoid ARC-incompatible constructs |
| `-mmacosx-version-min=11.0` | yes | no |
| `-fvisibility=hidden` | yes | no |
| Info.plist section | `-sectcreate __TEXT __info_plist` | no |
| Frameworks | identical set | identical set |

`run` does not depend on `build`, so `make run` alone uses whatever binary is already in `./bin`.

### 5.3 Translation to Cargo

* `-DTESTS` disappears: `cargo test` builds a separate test harness binary and `main` is simply not the entry point.
* `-DPROFILE=1` becomes a Cargo feature (`profile1` / `profile2`) gating the timer module; the test target can enable `profile1`.
* The framework flags belong in `build.rs`:
  `println!("cargo:rustc-link-search=framework=/System/Library/PrivateFrameworks");` and one `cargo:rustc-link-lib=framework=<name>` per framework (`Carbon`, `Cocoa`, `CoreServices`, `CoreVideo`, `SkyLight`). These are needed for `cargo test` too, since the tests link the whole crate.
* The osax stub symbols (`tests.m:1-4`) become either empty `&[u8]` constants behind `#[cfg(test)]`, or the build script emits real blobs for every profile — the latter is simpler and matches the daemon exactly, at the cost of requiring `arm64e` clang during `cargo test`. **Recommendation: build the blobs unconditionally in `build.rs` and drop the stub concept**, since the only reason the stubs exist is to avoid a slow `xxd -i` step.
* The `clean`/`build`/`run` chain becomes `cargo test`.

### 5.4 Comments to carry over

**None.** The file contains no `#` comment lines.

---

## 6. Summary of the module shape phase 2 should produce

One Rust module per source pair, as the phase plan requires:

* `src/view.rs` ← `src/view.h` + `src/view.c`, holding: `Area`, `WindowCapture`, `WindowProxy`, `WindowAnimation`, `WindowAnimationContext`, `BalanceNode`, `WindowInsertionPoint`, `WindowNodeChild`, `WindowNodeSplit`, `FeedbackWindow`, `WindowNode`, `ViewType`, `ViewFlags`, `View`, the `SPACE_PROPERTIES` table, the five string tables, and every function listed in §2.4 including the four that `view.h` forgets to declare.
* Tests move into `#[cfg(test)]` in `src/view.rs` (or `tests/area.rs`), replacing `tests/src/tests.m` + `tests/src/area.c` + `tests/makefile` entirely. The two existing test cases must be preserved exactly — they are the only automated coverage in the repository.

Items the phase 2 translator must resolve with the orchestrator before writing code, because they cannot be decided from `view.c` alone: the node-ownership model (raw `*mut` vs index arena), whether `update_window_notifications` moves to `event_loop` as `pub(crate)`, and whether the four latent defects in §2.6 are reproduced or fixed.
