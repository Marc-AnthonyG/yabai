# Phase 1 mapping — `window` / `application` / `process_manager`

Reader scope: `src/window.h`, `src/window.c`, `src/application.h`, `src/application.c`,
`src/process_manager.h`, `src/process_manager.c`.

All C locations are `path:line` against the working tree at commit `dd84572`.

---

## 0. Context shared by all three files

### 0.1 Unity build consequences that bite this translation unit

`src/manifest.m` includes every header (lines 63–78) and then every implementation file
(lines 80–97). There is exactly one translation unit, so:

* **`window.c` uses `g_mouse_state` with no declaration of its own.** `src/window.c:706` reads
  `g_mouse_state.window`, but `window.c` declares only five externs (`src/window.c:1-5`). It
  compiles because `src/event_loop.c:6` (`extern struct mouse_state g_mouse_state;`) is included
  at `src/manifest.m:82`, before `window.c` at `:90`. Phase 2 must add the dependency explicitly:
  the Rust `window` module needs access to the mouse-state global.
* `window.c` also calls, without any local declaration, `window_manager_find_managed_window`,
  `view_find_window_node`, `window_node_is_left_child`, `window_node_index_of_window`,
  `space_display_id`, `space_manager_mission_control_index`, `space_is_fullscreen`,
  `space_is_visible`, `display_manager_display_id_arrangement`. These resolve from headers
  included earlier in the manifest. In Rust each becomes an explicit `use`.
* **`window_unknown_serialize` is declared at `src/window.h:140` but never defined.** The real
  function is `window_nonax_serialize` (`src/window.c:121`), which is *not* declared in
  `window.h` at all; the only caller (`src/window_manager.c:48`) sees the definition because
  `window.c` precedes `window_manager.c` in the manifest. Phase 2: drop the phantom declaration,
  export `window_nonax_serialize`.
* Header-level `static` arrays (`ax_window_notification`, `ax_window_notification_str`,
  `window_property_val`, `window_property_str`, `ax_error_str`, `ax_application_notification`,
  `ax_application_notification_str`) exist in exactly one copy because of the unity build. In
  Rust they are ordinary module-level statics; no behaviour depends on the `static` keyword.
* `src/window.h:4` `const CFStringRef kAXFullscreenAttribute = CFSTR("AXFullScreen");` is a
  *definition* in a header (external linkage, not `static`). Legal only because of the unity
  build.

### 0.2 Thread inventory relevant to these files

Determined by reading `src/yabai.c:261-353`, `src/event_loop.c:1647-1725`,
`src/message.c:3003-3046`, `src/mouse_handler.c:274-291`, `src/window_manager.c:698-703`.

| Thread | Created at | What from these files runs on it |
| --- | --- | --- |
| **main run loop** (`[NSApp run]`, `src/yabai.c:350`) | process start | `application_notification_handler` (`src/application.c:6`) — the AXObserver source is added to `CFRunLoopGetMain()` at `src/application.c:57`; `process_handler` (`src/process_manager.c:151`) — Carbon handler installed on `GetApplicationEventTarget()` at `src/process_manager.c:251`; `process_manager_begin`, `process_manager_add_running_processes`, `process_create`, `process_pid_for_psn`, `process_is_being_debugged`, `process_manager_find_process`; plus `application_create` / `application_observe` / `application_unobserve` / `application_destroy` during startup via `window_manager_begin` (`src/window_manager.c:2737-2757`, called from `src/yabai.c:338`); and `window_create`/`window_observe`/`window_destroy` for existing windows discovered during that same startup pass. |
| **event-loop pthread** (`src/event_loop.c:1718`) | `event_loop_begin` | every `EVENT_HANDLER_*`, therefore: `window_create`, `window_observe`, `window_unobserve`, `window_destroy` (`src/window_manager.c:1440-1462`, `src/event_loop.c:314-315`, `:628-629`), `application_create`/`observe`/`unobserve`/`destroy` (`src/event_loop.c:146-152`, `:318-319`), `process_destroy` (`src/event_loop.c:344`), `process_manager_active_space_for_psn` (`src/event_loop.c:360`), and **all serialization** — `window_serialize` / `window_nonax_serialize` reached from `EVENT_HANDLER(DAEMON_MESSAGE)` (`src/event_loop.c:1614`) → `handle_message` → `window_manager_query_windows_for_spaces` (`src/window_manager.c:48`). Also every `window_*` query helper called from message handlers. |
| **message-loop pthread** (`src/message.c:3042`) | `message_loop_begin` | nothing from these files; it only `accept()`s and calls `event_loop_post`. |
| **CVDisplayLink thread** (`src/window_manager.c:700`) | per animation | reads `struct window` fields for frames (`window_manager_animate_window_list_thread_proc`); none of the functions in these three files. |
| **transient window-proxy pthreads** (`src/window_manager.c:666`) | per animated window | same: reads `struct window` fields only. |
| **main queue dispatch blocks** (`src/event_loop.c:93`, `:157`, `:1478`, `:1516`, `:1520`) | ad hoc | `process_manager_find_process` (`src/event_loop.c:94`, `:158`). These run on the main thread. |
| **forked children** | `fork`/`posix_spawn` in `src/misc/service.h` and config execution | none of these files. |

**Conclusion for phase 2**: `pm->process` (the hash table) is touched *only* from the main
thread. `struct process *` values, however, escape to the event-loop thread through
`event_loop_post`; the only fields that cross the boundary under synchronisation are
`terminated` and `ns_application`. `struct window` and `struct application` are created and
destroyed on *both* the main thread (startup) and the event-loop thread (steady state), but
never concurrently, because startup finishes before `[NSApp run]` starts dispatching — except
that the event-loop thread is already running from `src/yabai.c:291`. This is a genuine,
pre-existing race in the C code; a faithful Rust translation reproduces it and should not
"fix" it silently.

### 0.3 The temp-storage allocator (`ts_*`)

`src/misc/ts.h`. A process-global, lock-free bump allocator over an 8 MiB `mmap` region
(`src/yabai.c:279`) with a `PROT_NONE` guard page. `ts_alloc_aligned` / `ts_alloc_unaligned`
bump `g_temp_storage.used` with `__sync_*` CAS, so allocation is thread-safe; **deallocation is
not** — `ts_reset()` (`src/misc/ts.h:100`) sets `used = 0` non-atomically and is called once per
event from the event-loop thread (`src/event_loop.c:1671`). Overflow calls `exit(EXIT_FAILURE)`
(`src/misc/ts.h:31-33`).

Functions in this reader that hand out `ts` memory: `window_space_list` (`src/window.c:99`),
`window_property_title_ts` (`:719`, via `ts_cfstring_copy`/`ts_string_copy`), `window_title_ts`
(`:726`), `window_role_ts` (`:1006`), `window_subrole_ts` (`:1027`), and every `ts_string_escape`
call inside the two serializers. Nothing is ever freed.

**Rust recommendation**: a `TempArena` owned by the event-loop thread, reset at exactly the same
point (after each event handler returns). Because the C allocator is global and the only two
*other* producers reachable from these files (`display_space_list`, `display_manager_active_display_list`
inside `process_manager_active_space_for_psn`) also run on the event-loop thread, a
single-threaded `bumpalo::Bump` behind a thread-local is behaviour-equivalent *for these files*.
Return `&'arena str` / `&'arena [u64]` rather than raw pointers. Do **not** switch these to
`String`/`Vec`: `window_space_list`'s caller (`src/space_manager.c:31`, `:1094`) assumes the
pointer stays valid for the rest of the event, and `process_manager_active_space_for_psn` relies
on the *contiguity* of successive `display_space_list` results (see the carried-over comment at
`src/process_manager.c:98-102`).

### 0.4 CoreFoundation ownership convention in these files

`Copy`/`Create` returns +1, everything else +0. Every `CFRelease` in these files pairs with a
`Copy`/`Create` in the same function, except the three CF handles stored on `struct window`
(`role`, `subrole`, `title`) which are created in `window_create` (`src/window.c:1103-1105`) and
released in `window_destroy` (`:1139-1141`), and `window->ref`, which is created by the *caller*
(`AXUIElementCopyAttributeValue` on the application's window list, `src/window_manager.c:1440`)
and released in `window_destroy` (`:1142`).

**Rust recommendation**: a single `struct CfOwned<T>(NonNull<T>)` newtype with
`impl Drop { CFRelease }` and an explicit `into_raw()` for the cases that must not drop.
Use it for every local `CFTypeRef` in the goto-ladders (section 7.1). Keep the *struct fields*
as plain `Option<CFStringRef>` + manual release inside `window_destroy`, because the C code frees
explicitly at a point chosen relative to a CAS and to an in-flight event queue; making `Drop`
do it would move the release to a different program point.

---

## 1. `src/window.h`

### 1.1 Purpose

Declares `struct window` — yabai's per-window record, holding the AX element, the cached
CoreFoundation role/subrole/title, the last known frame, and the two flag bytes that drive
tiling decisions. It also defines the AX-notification table that `window_observe` subscribes to,
and the `WINDOW_PROPERTY_LIST` X-macro that generates the query-property enum, value table and
name table used by `message.c` and by the two serializers in `window.c`.

### 1.2 Types, constants, X-macro lists

#### `kAXFullscreenAttribute` — `src/window.h:4`
`const CFStringRef`, value `CFSTR("AXFullScreen")`. External linkage, defined in a header
(only legal in the unity build). Read on the event-loop thread (`src/window.c:835`).
**Rust**: `static AX_FULLSCREEN_ATTRIBUTE: OnceLock<CFString>` or a `cfstr!("AXFullScreen")`
helper that creates the CFString once. A fresh CFString per call is behaviourally identical
here (only used as an AX attribute key, matched by value).

#### AX window-notification index/bit constants — `src/window.h:6-15`

| Name | Value |
| --- | --- |
| `AX_WINDOW_MINIMIZED_INDEX` | `0` |
| `AX_WINDOW_DEMINIMIZED_INDEX` | `1` |
| `AX_WINDOW_DESTROYED_INDEX` | `2` |
| `AX_WINDOW_DESTROYED` | `1 << 2` = `0x04` |
| `AX_WINDOW_MINIMIZED` | `1 << 0` = `0x01` |
| `AX_WINDOW_DEMINIMIZED` | `1 << 1` = `0x02` |
| `AX_WINDOW_ALL` | `0x07` |

**Gotcha**: the *index* order (minimized=0, deminimized=1, destroyed=2) is the order used to index
`ax_window_notification[]`, but the `_INDEX` defines are declared in a different order from the
array initialiser. The array uses designated initialisers (`src/window.h:19-21`, `:26-28`) so the
mapping is index→value, not declaration order. A Rust `[&str; 3]` / `[CFStringRef; 3]` literal
must be written in *index* order: `[kAXWindowMiniaturizedNotification, kAXWindowDeminiaturizedNotification, kAXUIElementDestroyedNotification]`.

#### `ax_window_notification_str[]` — `src/window.h:17-22`
`static const char *[3]`. Debug strings only (`src/window.c:14`).
**Rust**: `const AX_WINDOW_NOTIFICATION_STR: [&str; 3]`.

#### `ax_window_notification[]` — `src/window.h:24-29`
`static CFStringRef[3]`, statically initialised from the `CFSTR(...)` macros
`kAXWindowMiniaturizedNotification` / `kAXWindowDeminiaturizedNotification` /
`kAXUIElementDestroyedNotification` (verified: all three are `#define … CFSTR("…")` in the SDK's
`AXNotificationConstants.h`). Read on the event-loop thread and on the main thread.
**Rust**: cannot be a `const` (CFString creation is a runtime call). Use
`static AX_WINDOW_NOTIFICATION: OnceLock<[CFString; 3]>` initialised on first use, or build the
three `CFString`s in a lazily-initialised module struct. Note that `AXObserverAddNotification`
takes the `CFStringRef` at +0, so the statics must outlive every registration — a `OnceLock`
does.

#### `WINDOW_PROPERTY_LIST` — `src/window.h:31-64`
An X-macro with 33 entries, each `(name_string, enum_name, uint64 bit)`. The bits run
`0x000000001` … `0x100000000`, i.e. bit 0 through bit 32 — **the last entry,
`WINDOW_PROPERTY_IS_GRABBED = 0x100000000`, does not fit in 32 bits**.

It generates three things:

* `enum window_property` (`src/window.h:66-71`) — an enum whose largest enumerator is
  `0x100000000`. In C the enum's underlying type is therefore at least 64-bit. Any Rust
  translation must use `u64`.
* `static uint64_t window_property_val[]` (`src/window.h:73-78`) — the 33 bits in list order.
* `static char *window_property_str[]` (`src/window.h:80-85`) — the 33 names in list order.

Full list in order (index → name → bit):

```
 0 id                    0x000000001    17 stack-index           0x000040000
 1 pid                   0x000000002    18 can-move              0x000080000
 2 app                   0x000000004    19 can-resize            0x000100000
 3 title                 0x000000008    20 has-focus             0x000200000
 4 scratchpad            0x000000010    21 has-shadow            0x000400000
 5 frame                 0x000000020    22 has-parent-zoom       0x000800000
 6 role                  0x000000040    23 has-fullscreen-zoom   0x001000000
 7 subrole               0x000000080    24 has-ax-reference      0x002000000
 8 root-window           0x000000100    25 is-native-fullscreen  0x004000000
 9 display               0x000000200    26 is-visible            0x008000000
10 space                 0x000000400    27 is-minimized          0x010000000
11 level                 0x000000800    28 is-hidden             0x020000000
12 sub-level             0x000001000    29 is-floating           0x040000000
13 layer                 0x000002000    30 is-sticky             0x080000000
14 sub-layer             0x000004000    31 is-grabbed            0x100000000
15 opacity               0x000008000
16 split-type            0x000010000
```

(33 entries; `WINDOW_PROPERTY_SPLIT_CHILD = 0x000020000` sits between `split-type` and
`stack-index`.)

**Rust**: `bitflags!` over `u64` gives the enum and the bit values; keep a parallel
`const WINDOW_PROPERTY_STR: [&str; 33]` and `const WINDOW_PROPERTY_VAL: [u64; 33]` in *exactly*
this order, because `message.c` looks up a user-supplied property name by linear scan over
`window_property_str` and uses the same index into `window_property_val`. Changing the order
changes nothing observable, but keeping it makes phase-2 diffing against the C trivial.

#### `struct window` — `src/window.h:87-106`

| Field | C type | Size/offset notes | Owner of the pointee | Cross-thread |
| --- | --- | --- | --- | --- |
| `application` | `struct application *` | | **Not owned.** Owned by `g_window_manager.application` table. **Can be set to `NULL`** at `src/event_loop.c:281` when the app terminates while a destroy is racing. | written: event-loop thread only. Read on CVDisplayLink/proxy threads via `window->application` in animation code. |
| `ref` | `AXUIElementRef` | CF +1 | **Owned by `struct window`**, created by the caller of `window_create` (`src/window_manager.c:1440`, from `application_window_list`/`AXUIElementCopyAttributeValue`), released at `src/window.c:1142`. | created/released on whichever thread runs `window_create`/`window_destroy`; AX calls also made from the main thread inside `application_notification_handler`? No — that handler only reads `element`, never `window->ref`. |
| `id` | `uint32_t` | | | **written by `window_destroy` (`src/window.c:1138`) on the event-loop thread, read by `EVENT_HANDLER(WINDOW_DESTROYED)` (`src/event_loop.c:607`) and by the main-thread AX callback path**. Unsynchronised by design; see `id_ptr`. |
| `id_ptr` | `uint32_t *volatile` | points at `&window->id` | self-referential | **The liveness token.** CAS'd from the main thread at `src/application.c:36` and from the event-loop thread at `src/event_loop.c:280, 647, 681, 731, 833, 877, 930, 960, 1159, 1240`. |
| `role` | `CFStringRef` | CF +1, may be `NULL` | **Owned**, released `src/window.c:1139` | event-loop thread |
| `subrole` | `CFStringRef` | CF +1, may be `NULL` | **Owned**, released `src/window.c:1140` | event-loop thread |
| `title` | `CFStringRef` | CF +1, may be `NULL` | **Owned**, released `src/window.c:1141` | event-loop thread; replaced in `EVENT_HANDLER(WINDOW_TITLE_CHANGED)` |
| `frame` | `CGRect` (4 × `CGFloat`/`f64`) | 32 bytes | | written on the event-loop thread, **read on the CVDisplayLink and proxy threads** during animations (`src/window_manager.c:2393`) |
| `windowed_frame` | `CGRect` | 32 bytes | | event-loop thread (`src/window_manager.c:2396`, `src/event_loop.c:699`, `:779`) |
| `is_root` | `bool` | | | set once in `window_create` (`src/window.c:1106`) |
| `is_eligible` | `bool` | | | written from `src/rule.c:124,167` and `src/window_manager.c:1490`, read at `src/event_loop.c:308,622` — all event-loop thread |
| `notification` | `uint8_t` | bitmask over `AX_WINDOW_*` | | only `window_observe`/`window_unobserve` |
| `rule_flags` | `uint8_t` | `enum window_rule_flag` | | event-loop thread |
| `flags` | `uint8_t` | `enum window_flag` | | event-loop thread, plus read from the mouse handler path |
| `opacity` | `float` | | | event-loop thread (`src/window_manager.c:150`, `:791`, `src/message.c:2360`) |
| `layer` | `int` | holds a `LAYER_*` value (`kCGWindowLevelKey`) | | event-loop thread (`src/window_manager.c:822`, `:837`) |
| `scratchpad` | `char *` | | **Not owned — alias.** The owner is `wm->scratchpad_window[i].label`; freed at `src/window_manager.c:2514`. `window_destroy` deliberately does not free it. | event-loop thread |

**Layout note**: `struct window` is never passed to the OS, so Rust may reorder it freely. It
*is* passed as the AXObserver `refcon` (`src/window.c:10`) and as an event `context`
(`src/application.c:24,26,38`), i.e. only as an opaque pointer. Use `#[repr(Rust)]` and
`Box::into_raw`.

**Rust shape**:

```text
pub struct Window {
    pub application: *mut Application,     // nullable, not owned
    pub reference: AXUIElementRef,         // owned, released in window_destroy
    pub id: u32,
    pub id_ptr: AtomicPtr<u32>,            // set to &raw mut (*boxed).id after Box::into_raw
    pub role: Option<CFStringRef>,         // owned
    pub subrole: Option<CFStringRef>,      // owned
    pub title: Option<CFStringRef>,        // owned
    pub frame: CGRect,
    pub windowed_frame: CGRect,
    pub is_root: bool,
    pub is_eligible: bool,
    pub notification: u8,
    pub rule_flags: u8,
    pub flags: u8,
    pub opacity: f32,
    pub layer: i32,
    pub scratchpad: *mut c_char,           // alias, not owned
}
```

`id_ptr` must be filled **after** `Box::into_raw`, exactly as C fills it after `malloc`
(`src/window.c:1101`), so the address is stable.

#### `enum window_flag` — `src/window.h:108-118`
`WINDOW_SHADOW 0x01`, `WINDOW_FULLSCREEN 0x02`, `WINDOW_MINIMIZE 0x04`, `WINDOW_FLOAT 0x08`,
`WINDOW_STICKY 0x10`, `WINDOW_WINDOWED 0x20`, `WINDOW_MOVABLE 0x40`, `WINDOW_RESIZABLE 0x80`.
Stored in a `uint8_t`; all eight bits used.
**Rust**: `bitflags!(struct WindowFlag: u8)`.

#### `enum window_rule_flag` — `src/window.h:120-126`
`WINDOW_RULE_MANAGED 0x01`, `WINDOW_RULE_FULLSCREEN 0x02`, `WINDOW_RULE_MFF 0x04`,
`WINDOW_RULE_MFF_VALUE 0x08`. Stored in `uint8_t`.
**Rust**: `bitflags!(struct WindowRuleFlag: u8)`.

#### Inline flag accessors — `src/window.h:128-134`
`window_check_flag` / `window_clear_flag` / `window_set_flag` and the three `rule_flag`
equivalents. Note `window_check_flag` returns `bool` from `w->flags & x`, i.e. an implicit
non-zero test; `bitflags`' `contains()` is `(bits & x) == x`, which differs when `x` has more
than one bit set. No call site passes a multi-bit value today, but phase 2 should use
`intersects()` to stay exactly faithful.

### 1.3 Globals / statics

`ax_window_notification_str`, `ax_window_notification`, `window_property_val`,
`window_property_str` (all file-scope `static`, one instance thanks to the unity build) and
`kAXFullscreenAttribute` (external linkage). All are written once at static-init time and
read-only afterwards, from both the main and event-loop threads. No synchronisation is needed
and none exists.

### 1.4 Functions
Only the six inline flag helpers (§1.2). All are pure field reads/writes, thread-agnostic,
allocate nothing, call nothing external.

### 1.5 OS/run-loop callbacks
None.

### 1.6 C patterns
* **X-macro generating three parallel tables** (`src/window.h:31-85`). Rust: `bitflags!` + two
  `const` arrays, or a single `const PROPERTIES: [(&str, u64); 33]` and derive the two views from
  it. Keeping the order is what matters.
* **Designated-initialiser sparse arrays** (`src/window.h:17-29`). Rust literal arrays, written
  in index order — see the gotcha in §1.2.
* **Bit flags in a `uint8_t`** (`src/window.h:108-134`). Rust: `bitflags`.
* **`const` object defined in a header** (`src/window.h:4`). Rust: `OnceLock`.

### 1.7 Comments to carry over
None. `src/window.h` contains no comments.

---

## 2. `src/window.c`

### 2.1 Purpose

Implements everything yabai knows how to ask about a single window: the private-SkyLight
queries (space, display, level, sub-level, tags, parent, alpha), the Accessibility queries
(frame, role, subrole, title, settability, minimized/fullscreen state), the two JSON
serializers used by `yabai -m query`, and the construction/teardown of `struct window` together
with its AX notification subscriptions.

### 2.2 Types
None declared here except the anonymous packed message struct inside
`SLSGetWindowSubLevel__Internal` (`src/window.c:932-941`):

```c
#pragma pack(push,4)
struct {
    mach_msg_header_t header;   // 24 bytes
    NDR_record_t      NDR_record; // 8 bytes
    uint32_t          window_id;
    int32_t           sub_level;
    int32_t           padding1;
    int32_t           padding2;
} msg = {0};
#pragma pack(pop)
```

Total 48 bytes (`0x30`); send size is `0x24` = 36 (header + NDR + `window_id`), receive size
`0x30`. **This layout is handed to the kernel**, so Rust must use
`#[repr(C, packed(4))]` and must keep `mach_msg_header_t` byte-identical
(`msgh_bits: u32, msgh_size: u32, msgh_remote_port: u32, msgh_local_port: u32,
msgh_voucher_port: u32, msgh_id: i32`) and `NDR_record_t` as six `u8`s plus two reserved bytes.
Zero-initialise the whole struct (`= {0}`) before filling.

### 2.3 Globals / statics

| Symbol | Type | Where | Threads | Synchronisation |
| --- | --- | --- | --- | --- |
| `g_window_manager` | `struct window_manager` (extern, defined `src/yabai.c:30`) | `src/window.c:1` | read on the event-loop thread only from this file (`:444`, `:623`) | none |
| `g_layer_normal_window_level` | `int` (extern, `src/yabai.c:39`) | `src/window.c:2` | written once at startup on the main thread (`src/yabai.c:145`), read on the event-loop thread (`:116`, `:1081`) | none; safe because startup precedes use in practice |
| `g_layer_below_window_level` | `int` | `src/window.c:3` | same | none |
| `g_layer_above_window_level` | `int` | `src/window.c:4` | same | none |
| `g_connection` | `int` (extern, `src/yabai.c:50`) | `src/window.c:5` | written once on the main thread (`src/yabai.c:143`), read from **every** thread | none |
| `g_mouse_state` | `struct mouse_state` | **not declared in this file** — leaks in from `src/event_loop.c:6` (see §0.1) | read at `src/window.c:706` on the event-loop thread | none |
| `process_name` | **function-local** `static char[PROC_PIDPATHINFO_MAXSIZE]` (4096 bytes) | `src/window.c:173` inside `window_nonax_serialize` | event-loop thread only (serialization only happens under `DAEMON_MESSAGE`) | none, and none needed |
| `CGSGetConnectionPortById` | file-scope `static mach_port_t (*)(int)` declared in `src/misc/extern.h:4` | used `src/window.c:946`, `:956` | resolved once on the main thread (`src/yabai.c:148`), read on the event-loop thread | none |

**Rust**: `g_connection` → `static CONNECTION: AtomicI32`. The three layer levels → `AtomicI32`
or a `OnceLock<LayerLevels>`. `CGSGetConnectionPortById` → `static CGS_GET_CONNECTION_PORT_BY_ID:
AtomicPtr<c_void>` set from `macho_find_symbol`, transmuted at the call site. `process_name` →
a `[u8; PROC_PIDPATHINFO_MAXSIZE]` local (it does not need to be static; nothing escapes it past
the `fprintf`), or a thread-local if phase 2 wants to preserve the "no stack blow-up" intent.

### 2.4 Functions

All of these run on the **event-loop thread** in steady state; the ones marked † also run on the
**main thread** during startup, because `window_manager_begin` (`src/yabai.c:338`) →
`window_manager_add_existing_application_windows` → `window_manager_create_and_add_window`
(`src/window_manager.c:1438`) runs before `[NSApp run]`.

| Line | Signature | What it does | Allocates / frees | External symbols |
| --- | --- | --- | --- | --- |
| 7 | `bool window_observe(struct window *window)` † | Registers all three AX window notifications on the *application's* observer, with `window` as refcon; sets one bit of `window->notification` per success. Returns true only if all three are set. | nothing | `AXObserverAddNotification` |
| 21 | `void window_unobserve(struct window *window)` † | Removes the registered notifications and clears the bits. | nothing | `AXObserverRemoveNotification` |
| 31 | `CFStringRef window_display_uuid(uint32_t wid)` | Managed-display UUID for a window, falling back to best-display-for-bounds. **Returns +1; caller must `CFRelease`.** | CF +1 returned | `SLSCopyManagedDisplayForWindow`, `SLSGetWindowBounds`, `SLSCopyBestManagedDisplayForRect` |
| 42 | `uint32_t window_display_id(uint32_t wid)` | UUID → `CGDirectDisplayID`. | releases both CF objects it makes | `CFUUIDCreateFromString`, `CGDisplayGetDisplayIDFromUUID`, `CFRelease` |
| 56 | `static uint64_t window_display_space(uint32_t wid)` | Current space of the window's display. | releases the uuid | `SLSManagedDisplayGetCurrentSpace`, `CFRelease` |
| 67 | `uint64_t window_space(uint32_t wid)` | First space id the window belongs to, else the display's current space. goto-ladder. | releases both CFArrays | `SLSCopySpacesForWindows`, `CFArrayGetCount`, `CFArrayGetValueAtIndex`, `CFNumberGetType`, `CFNumberGetValue`, `CFRelease` |
| 89 | `uint64_t *window_space_list(uint32_t wid, int *count)` | All spaces the window is on. **Returns `ts` memory** (lifetime = current event). | `ts_alloc_list(uint64_t, *count)`; never freed | same as above |
| 113 | `static inline const char *window_layer(int level)` | Maps a CG window level to `"below"`/`"normal"`/`"above"`/`"unknown"` via `layer_str` (`src/misc/helpers.h:175`). | nothing | none |
| 121 | `void window_nonax_serialize(FILE *rsp, uint32_t wid, uint64_t flags)` | JSON for a window yabai has no AX handle for. Computes only what the flags ask for, then emits fields in fixed order with `,\n` separators. | `ts_string_escape` (ts memory), `window_property_title_ts` (ts) | `proc_name`, `fprintf`, plus the SLS calls listed below |
| 409 | `void window_serialize(FILE *rsp, struct window *window, uint64_t flags)` | Same JSON for a tracked window; adds role/subrole, split/stack info from the view tree, focus/zoom/visibility. | same | `SLSWindowIsOrderedIn`, `fprintf`, `ts_*` |
| 713 | `char *window_property_title_ts(uint32_t wid)` | Title via `SLSCopyWindowProperty(kCGSWindowTitle)`; **returns ts memory**, `""` when absent. | ts; releases the CF value | `SLSCopyWindowProperty`, `CFRelease` |
| 724 | `char *window_title_ts(struct window *window)` | Cached title as UTF-8 in ts memory, `""` if `title` is NULL. | ts | `CFStringGetMaximumSizeForEncoding`, `CFStringGetCString` (via `ts_cfstring_copy`) |
| 729 | `CFStringRef window_title(struct window *window)` | **Returns +1** AX title (or NULL). Caller owns. | CF +1 returned | `AXUIElementCopyAttributeValue` |
| 736 | `CGPoint window_ax_origin(struct window *window)` | AX position, `{0,0}` on failure. | releases the AXValue | `AXUIElementCopyAttributeValue`, `AXValueGetValue`, `CFRelease` |
| 751 | `CGRect window_ax_frame(struct window *window)` | AX position + size, `{0,0,0,0}` on failure. | releases both AXValues | same + `kAXSizeAttribute` |
| 773 | `bool window_ax_can_move(struct window *window)` | `AXUIElementIsAttributeSettable(kAXPosition)`; `result` is declared uninitialised (`:775`) and explicitly zeroed only on the non-success path. Rust must pass a zero-initialised `Boolean` out-param. | nothing | `AXUIElementIsAttributeSettable` |
| 782 | `bool window_can_move(struct window *window)` | Cached `WINDOW_MOVABLE` flag. | nothing | none |
| 787 | `bool window_ax_can_resize(struct window *window)` | `kAXSize` settability. | nothing | `AXUIElementIsAttributeSettable` |
| 796 | `bool window_can_resize(struct window *window)` | Cached `WINDOW_RESIZABLE` flag. | nothing | none |
| 801 | `bool window_can_minimize(struct window *window)` | `kAXMinimized` settability. Not cached. | nothing | `AXUIElementIsAttributeSettable` |
| 810 | `bool window_is_undersized(struct window *window)` | `frame.size.width <= 500.0f \|\| frame.size.height <= 500.0f`. **`CGFloat` is `double`; the literal is a `float` promoted to `double`, so the comparison is exact at 500.0.** | nothing | none |
| 817 | `static bool window_is_minimized(struct window *window)` | AX `kAXMinimized`. | releases the CFBoolean | `AXUIElementCopyAttributeValue`, `CFBooleanGetValue`, `CFRelease` |
| 830 | `bool window_is_fullscreen(struct window *window)` | AX `AXFullScreen`. | releases the CFBoolean | same |
| 843 | `bool window_is_sticky(uint32_t wid)` | True when the window is on more than one space. goto-ladder. | releases both CFArrays | `SLSCopySpacesForWindows`, `CFArrayGetCount`, `CFRelease` |
| 859 | `bool window_shadow(uint32_t wid)` | `!(tags & 0x8)`. | nothing | none (calls `window_tags`) |
| 865 | `float window_opacity(uint32_t wid)` | `SLSGetWindowAlpha`; **ignores the CGError**, so a failed call returns the pre-initialised `0.0f`. | nothing | `SLSGetWindowAlpha` |
| 872 | `uint32_t window_parent(uint32_t wid)` | Parent window id via the SLS window-query iterator. Three-level goto-ladder. | releases iterator, query, array | `SLSWindowQueryWindows`, `SLSWindowQueryResultCopyWindows`, `SLSWindowIteratorGetCount`, `SLSWindowIteratorAdvance`, `SLSWindowIteratorGetParentID`, `CFRelease` |
| 899 | `int window_level(uint32_t wid)` | Ventura+ uses the query iterator; older uses `SLSGetWindowLevel`. **The goto labels `err1`/`err2` live inside the `if` block** — a `goto` from inside the block to a label inside the same block. | same as `window_parent` | `workspace_is_macos_*`, `SLSGetWindowLevel`, iterator calls |
| 930 | `static int SLSGetWindowSubLevel__Internal(int cid, uint32_t wid)` | Hand-rolled MIG call (see §2.2). Ignores the `mach_msg` return code and reads `msg.sub_level` unconditionally. | nothing heap; uses the thread's special reply port | `CGSGetConnectionPortById` (fn-ptr), `mig_get_special_reply_port`, `mach_msg`, `NDR_record`, `workspace_is_macos_tahoe` |
| 954 | `int window_sub_level(uint32_t wid)` | Uses the internal MIG path when `CGSGetConnectionPortById` resolved, else `SLSGetWindowSubLevel`. | nothing | both |
| 963 | `uint64_t window_tags(uint32_t wid)` | Window tags via the query iterator. Same three-level ladder. | releases all three | iterator calls |
| 989 | `CFStringRef window_ax_role(struct window *window)` | **Returns +1** AX role (or NULL). | CF +1 returned | `AXUIElementCopyAttributeValue` |
| 996 | `CFStringRef window_role(struct window *window)` | Returns the cached `window->role` at **+0**. | nothing | none |
| 1001 | `char *window_role_ts(struct window *window)` | Cached role as UTF-8 ts string, `""` if NULL. | ts | `ts_cfstring_copy` |
| 1010 | `CFStringRef window_ax_subrole(struct window *window)` | **Returns +1**. | CF +1 returned | `AXUIElementCopyAttributeValue` |
| 1017 | `CFStringRef window_subrole(struct window *window)` | Cached, +0. | nothing | none |
| 1022 | `char *window_subrole_ts(struct window *window)` | ts string. | ts | `ts_cfstring_copy` |
| 1031 | `static bool window_is_root(struct window *window)` | True when AX parent is absent or equals the application element. | releases the parent | `AXUIElementCopyAttributeValue`, `CFEqual`, `CFRelease` |
| 1044 | `bool window_is_real(struct window *window)` | role == `AXWindow` and subrole ∈ {Standard, Floating, Dialog}. goto-out. | nothing | `CFEqual` |
| 1062 | `bool window_is_standard(struct window *window)` | role == `AXWindow` and subrole == `AXStandardWindow`. goto-out. | nothing | `CFEqual` |
| 1078 | `bool window_level_is_standard(struct window *window)` | `window_level(id) == g_layer_normal_window_level`. | nothing | none |
| 1084 | `bool window_is_unknown(struct window *window)` | subrole == `AXUnknown`; false when subrole is NULL. | nothing | `CFEqual` |
| 1093 | `struct window *window_create(struct application*, AXUIElementRef, uint32_t)` † | `malloc` + `memset` + populate; queries AX frame/role/subrole/title, root-ness, shadow, minimized, movable, resizable, fullscreen, sticky. | `malloc(sizeof(struct window))`; takes ownership of `window_ref`; acquires three CF +1 strings | `malloc`, `memset`, everything above |
| 1136 | `void window_destroy(struct window *window)` † | Sets `id = 0` **first** (the liveness signal read at `src/event_loop.c:607`), releases the three optional CF strings and `ref`, then `free`s. | releases 4 CF objects, frees the struct. **Does not** free `scratchpad` (aliased, §1.2) and **does not** clear `id_ptr`. | `CFRelease`, `free` |

### 2.5 OS / run-loop callbacks registered here

`window_observe` (`src/window.c:10`) registers three AX notifications **on the application's
observer** (`window->application->observer_ref`, created in `application_observe`), with
`window` as the context/refcon pointer.

* Registration site: `src/window.c:10`.
* Callback: `application_notification_handler` (`src/application.c:6`) — the observer's single
  callback, so all three window notifications land there.
* Thread: the **main run loop**, because the observer's run-loop source was added to
  `CFRunLoopGetMain()` at `src/application.c:57`.
* Context lifetime: the `struct window *` must outlive the registration. It is removed by
  `window_unobserve` (`src/window.c:26`), which every `window_destroy` call site invokes first
  (`src/window_manager.c:1461`, `src/event_loop.c:314`, `:628`) — **except**
  `src/window_manager.c:1450`, where a window rejected as `AXUnknown` is destroyed **without**
  ever having been observed (correct: `window_observe` had not run yet).
* **Race**: `AXObserverRemoveNotification` does not guarantee that an already-delivered
  notification is not still in flight on the main thread. This is precisely why
  `application.c:36` CASes `id_ptr` to `NULL` before posting `WINDOW_DESTROYED`, and why every
  event handler re-CASes it (§7.6).

No other callback is registered in `window.c`.

### 2.6 C patterns in `window.c`, and the Rust translation

#### (a) `goto`-ladder resource cleanup
Sites: `src/window.c:73-86` (`window_space`), `:94-110` (`window_space_list`), `:849-856`
(`window_is_sticky`), `:879-896` (`window_parent`), `:907-922` (`window_level`, labels nested
inside an `if`), `:969-985` (`window_tags`), `:1050-1059` / `:1068-1075` (`window_is_real`,
`window_is_standard` — pure early-exit, no resources).

**Rust**: for the resource ladders, wrap each CF handle in `CfOwned<T>` (Drop = `CFRelease`) and
use `?`/`let … else` early returns. Drop order in Rust is reverse declaration order, which
matches the C ladders exactly (innermost released first). For the two pure early-exit ladders,
`let (Some(role), Some(subrole)) = … else { return false }`.

**Silent behaviour change to avoid**: `window_is_sticky` releases `space_list_ref` at
`src/window.c:852`, *before* falling into the `err:` label; with `CfOwned` both releases happen
at scope end in reverse order — same set of releases, different interleaving. Harmless here (no
CF object depends on the other's lifetime), but note it. In `window_level` the labels sit inside
the `if` block, so the `else` branch skips them entirely; translate as two separate functions or
a `match` on the OS version.

#### (b) X-macro-driven serializers
`window_nonax_serialize` (`:121-407`) and `window_serialize` (`:409-711`) are 30-branch
if-ladders whose order defines the JSON field order in the socket protocol.

**Rust**: keep them as straight-line `if flags.contains(...)` ladders writing into a
`impl std::io::Write`. **Do not** refactor into a loop over the property table: the two
functions emit *different* expressions for the same bit and the `did_output` comma logic depends
on the exact ordering. A `did_output: bool` local reproduced verbatim is the safest translation.

#### (c) `FILE *rsp` printf output
Every `fprintf(rsp, …)`. The `FILE*` comes from `fdopen(param1, "w")` on the accepted socket
(`src/event_loop.c:1632`), and is `fflush`ed + `fclose`d there.

**Rust**: the response sink becomes `&mut BufWriter<UnixStream>` (or `&mut dyn io::Write`).
`fclose` on the `fdopen`'d FILE closes the fd; in Rust, let the `UnixStream` drop.
Format-string equivalences that must be checked one by one:

| C | Rust | Trap |
| --- | --- | --- |
| `"%d"` with `uint32_t wid` (`:159`, `:462`) | `{}` on `wid as i32` | **wid > 2³¹−1 prints negative in C.** Printing `u32` would differ. Use `as i32` to stay bit-identical. |
| `"%d"` with `pid_t` (`:166`, `:469`) | `{}` on `i32` | fine |
| `"%.4f"` with `CGFloat` (`f64`) (`:206`, `:503`) | `{:.4}` | Rust and C both round-half-to-even and print `-0.0000` for negative zero. Equivalent. |
| `"%.4f"` with `float opacity` (`:282`, `:580`) | `{:.4}` on `opacity as f64` | must widen to `f64` first, exactly as C's default argument promotion does; formatting the `f32` directly can differ in the 4th decimal. |
| `"%s"` with a possibly-NULL `char*` (`:179`, `:189`, `:479`, `:489`) | see below | The C code guards with `escaped ? escaped : raw` — but `raw` can itself be NULL when `ts_cfstring_copy` fails (`src/misc/helpers.h:368`). `printf("%s", NULL)` is UB that glibc/Apple libc render as `(null)`. Rust must decide; recommend reproducing `"(null)"` only if a test depends on it, otherwise emit `""` and note the divergence. |
| `json_bool(x)` (`src/misc/helpers.h:233`) | `if x {"true"} else {"false"}` | fine |

#### (d) Conditional pre-computation of uninitialised locals
`window_nonax_serialize` declares `connection/pid/sid/level/sub_level` uninitialised
(`:127-131`) and fills them only under the matching flag guards (`:133-153`);
`window_serialize` does the same for `sid/level/sub_level/view/node/is_minimized/is_sticky`
(`:415-456`). Every read is guarded by the same flag set, so no UB is reached, but the compiler
cannot prove it.

**Rust**: `Option<T>` locals filled in the same guards, `.unwrap()` at the read sites (or
`let Some(sid) = sid else { unreachable!() }`). Do **not** compute them unconditionally — some
are expensive round-trips to the window server and the C code deliberately skips them.

#### (e) `flags == 0x0` ⇒ all flags
`src/window.c:125` and `:413`: `if (flags == 0x0) flags |= ~flags;`
**Rust**: `let flags = if flags.is_empty() { WindowProperty::all() } else { flags };` — but note
`~0u64` sets **all 64 bits**, not just the 33 defined ones. `bitflags::all()` sets only the
defined bits. The difference is invisible (nothing tests undefined bits) but a strict
translation should use `WindowProperty::from_bits_retain(!0)`.

#### (f) Function-local `static` buffer + `proc_name`
`src/window.c:173-174`. `proc_name(pid, buf, sizeof buf)` writes a NUL-terminated process name.
**Rust**: `let mut buffer = [0u8; PROC_PIDPATHINFO_MAXSIZE]; libc::proc_name(pid, buffer.as_mut_ptr() as *mut c_void, buffer.len() as u32);`
then `CStr::from_bytes_until_nul`. **`proc_name` can return 0 and leave the buffer untouched**;
the C code then prints whatever the previous call left in the static (a real, observable quirk,
since the buffer is `static` and not re-zeroed). Using a fresh zeroed local changes behaviour in
that error case — flag it, and prefer the fresh local (the quirk is a bug).

#### (g) CF retain/release pairs
Covered in §0.4. The three functions that **return +1 to the caller** are `window_display_uuid`
(`:31`), `window_title` (`:729`), `window_ax_role` (`:989`), `window_ax_subrole` (`:1010`).
**Rust**: return `CfOwned<CFStringRef>` so the +1 is encoded in the type; the three callers in
`window_create` then `into_raw()` it into the struct field.

#### (h) `ts` arena returns
`window_space_list` (`:99`), `window_property_title_ts` (`:719`), `window_title_ts` (`:726`),
`window_role_ts` (`:1006`), `window_subrole_ts` (`:1027`).
**Rust**: `&'arena [u64]` / `&'arena str` from the per-event `Bump`. The `_ts` suffix in the C
name is the ownership contract; keep it in the Rust name.

#### (i) Hand-rolled MIG / `mach_msg`
`src/window.c:930-952`. See §2.2 for the layout. **Rust**: `#[repr(C, packed(4))]` struct,
`unsafe { mach_msg(&mut msg.header, MACH_SEND_MSG | MACH_RCV_MSG, 0x24, 0x30, reply_port, 0, 0) }`,
then read `msg.sub_level`. Reading a field out of a `packed` struct needs
`std::ptr::addr_of!` + `read_unaligned` (or `{ msg.sub_level }` in a block) — a bare field
reference on a packed struct is a compile error in modern Rust.

`NDR_record` is a data symbol: `extern "C" { static NDR_record: NDRRecord; }`.

`msgh_id` is chosen by OS version at `:948` — keep the `workspace_is_macos_tahoe()` branch.

#### (j) Dynamically resolved function pointer
`CGSGetConnectionPortById` (`src/misc/extern.h:4`, tested at `src/window.c:956`).
**Rust**: `static CGS_GET_CONNECTION_PORT_BY_ID: AtomicPtr<c_void>`; at the call site,
`let f = PTR.load(Relaxed); if !f.is_null() { let f: extern "C" fn(i32) -> mach_port_t = transmute(f); … }`.
`transmute` of a function pointer is unavoidable unsafe.

#### (k) CAS on `id_ptr` as a liveness check
Not performed inside `window.c`, but `window_create` establishes the invariant
(`window->id_ptr = &window->id;`, `:1101`) and `window_destroy` breaks it (`id = 0`, `:1138`).
See §7.6.

#### (l) Float→int and int-width traps in this file
* `src/window.c:48`: `int id = CGDisplayGetDisplayIDFromUUID(uuid);` — the API returns
  `CGDirectDisplayID` (`uint32_t`), stored in `int`, then returned as `uint32_t`. Round-trips
  exactly; in Rust keep it `u32` and note the sign laundering is a no-op.
* `src/window.c:75`, `:96`, `:851`, `:884`, `:912`, `:974`: `CFArrayGetCount` returns `CFIndex`
  (`isize`), truncated into `int`. Rust: `as i32` (or `try_into().unwrap_or(i32::MAX)`); a
  silent `as` keeps C behaviour.
* `src/window.c:79`, `:103`: `CFNumberGetValue(id_ref, CFNumberGetType(id_ref), &sid)` writes
  through a `uint64_t*` using *whatever* type the CFNumber reports. If the number is
  `kCFNumberSInt32Type`, only 4 bytes are written and the upper 4 bytes of `sid` keep their
  previous value — `sid` is zero-initialised at `:69` so this is benign there, but in
  `window_space_list` (`:99`) the `ts` buffer is **not** zeroed, so a 32-bit CFNumber leaves
  garbage in the high half. **A Rust translation that reads into a zeroed `u64` changes
  behaviour (for the better).** Flag it; recommend zero-initialising and noting the divergence.
* `src/window.c:812-813`: `CGFloat` (f64) compared against `500.0f`. Exact.

#### (m) Absent patterns
No stretchy buffers (`buf_push`), no intrusive lists, no hash tables, no `setjmp`, no `regex.h`,
no inline asm, no SIMD, no `fork`/`exec`, no Objective-C message sends or blocks in `window.c`.

### 2.7 Comments to carry over
None. `src/window.c` contains no comments.

---

## 3. `src/application.h`

### 3.1 Purpose

Declares `struct application` — yabai's per-process record, pairing the AX application element
with the Carbon PSN, the SkyLight connection id, and the AXObserver used for every window of
that application. It also defines the AX application-notification table and the
`kAXError*` → string table used for debug output.

### 3.2 Types, constants

#### `OBSERVER_CALLBACK(name)` / `observer_callback` — `src/application.h:4-5`
```c
#define OBSERVER_CALLBACK(name) void name(AXObserverRef observer, AXUIElementRef element, CFStringRef notification, void *context)
typedef OBSERVER_CALLBACK(observer_callback);
```
The `AXObserverCallback` signature.
**Rust**: `type ObserverCallback = unsafe extern "C" fn(AXObserverRef, AXUIElementRef, CFStringRef, *mut c_void);`
The macro is only a declaration shorthand; do not reproduce it as a Rust macro.

#### AX application-notification index/bit constants — `src/application.h:7-24`

| Name | Index | Bit |
| --- | --- | --- |
| `AX_APPLICATION_WINDOW_CREATED` | 0 | `0x01` |
| `AX_APPLICATION_WINDOW_FOCUSED` | 1 | `0x02` |
| `AX_APPLICATION_WINDOW_MOVED` | 2 | `0x04` |
| `AX_APPLICATION_WINDOW_RESIZED` | 3 | `0x08` |
| `AX_APPLICATION_WINDOW_TITLE_CHANGED` | 4 | `0x10` |
| `AX_APPLICATION_WINDOW_MENU_OPENED_INDEX` | 5 | *(no bit macro defined)* |
| `AX_APPLICATION_WINDOW_MENU_CLOSED_INDEX` | 6 | *(no bit macro defined)* |
| `AX_APPLICATION_ALL` | — | `0x1F` |

**Gotcha to preserve**: `AX_APPLICATION_ALL` covers only indices 0–4. `application_observe`
(`src/application.c:60`) returns `true` even if the two menu notifications failed to register.
A Rust `bitflags::all()` would include bits 5 and 6 and silently change which applications yabai
considers successfully observed. Define `AX_APPLICATION_ALL` as the explicit `0x1F` constant.

#### `ax_error_str[]` — `src/application.h:26-44`
`static const char *[]`, 16 designated initialisers indexed by the **negated** `AXError` value.
Verified against the SDK's `AXError.h`: `kAXErrorSuccess = 0` and
`kAXErrorFailure = -25200` … `kAXErrorNotEnoughPrecision = -25214`. So the array clang emits has
**25215 entries**: index 0 (`"kAXErrorSuccess"`), indices 1–25199 all `NULL`, and indices
25200–25214 holding the fifteen error names — roughly 200 KiB of mostly-NULL static data.
It is only ever read as `ax_error_str[-result]` at `src/window.c:14` and `src/application.c:52`,
both inside `debug(...)`.
**Rust**: replace it with `fn ax_error_str(error: AXError) -> &'static str` implemented as a
`match`. Not a behaviour change at either call site, and it drops the 200 KiB static.
**Gotcha**: an `AXError` outside the tabulated set indexes a `NULL` entry, which `printf("%s")`
renders as `(null)`; the Rust `match` needs a `_ => "(null)"` arm to stay identical, or an
out-of-range index panics where C printed something.

#### `ax_application_notification_str[]` — `src/application.h:46-55`
`static const char *[7]`, debug strings. **Rust**: `const [&str; 7]` in index order.

#### `ax_application_notification[]` — `src/application.h:57-66`
`static CFStringRef[7]`, statically initialised from the `CFSTR(...)` macros.
**Rust**: `OnceLock<[CFString; 7]>`, same reasoning as §1.2.

#### `struct application` — `src/application.h:68-80`

| Field | C type | Owner of the pointee | Cross-thread |
| --- | --- | --- | --- |
| `ref` | `AXUIElementRef` | **Owned**; `AXUIElementCreateApplication` at `src/application.c:130`, released at `:142` | created/released on whichever thread runs create/destroy; read on the event-loop thread (`src/event_loop.c:377` region) |
| `connection` | `int` | — | written once at `src/application.c:135`, read at `src/event_loop.c:360` |
| `psn` | `ProcessSerialNumber` (two `u32`) | value copy of `process->psn` | read at `src/application.c:107`, `:114`; stored into `wm->focused_window_psn` |
| `pid` | `pid_t` (`i32`) | — | read widely |
| `name` | `char *` | **NOT owned — alias of `process->name`** (`src/application.c:133`). Freed by `process_destroy` (`src/process_manager.c:263`), *after* `application_destroy` in the terminate handler (`src/event_loop.c:319` then `:344`). | read on the event-loop thread |
| `observer_ref` | `AXObserverRef` | **Owned**; created at `src/application.c:45`, released at `:75` — **only inside `if (application->is_observing)`**, so an application whose `AXObserverCreate` succeeded but which was never marked observing would leak. In practice `is_observing` is set unconditionally after a successful create (`:56`). | main thread + event-loop thread |
| `notification` | `uint8_t` | bitmask over indices 0–6 | observe/unobserve only |
| `is_observing` | `bool` | — | observe/unobserve only |
| `is_hidden` | `bool` | — | written at `src/application.c:134` and from the workspace hide/unhide handlers (main thread → event loop); read at `src/window.c:670`, `:685` |
| `ax_retry` | `bool` | — | set at `src/application.c:51` when `kAXErrorCannotComplete`; consumed by `g_window_manager.applications_to_refresh` |

**Rust shape**: `Box`ed, `Box::into_raw`, stored in the window-manager hash table keyed by pid.
`name: *mut c_char` stays a raw alias — making it a `String` would double-free against
`process_destroy`.

### 3.3 Globals / statics
`ax_error_str`, `ax_application_notification_str`, `ax_application_notification`. Static-init,
read-only afterwards, no synchronisation.

### 3.4 Functions
Declarations only (`src/application.h:82-90`). See §4.

### 3.5 Callbacks
None registered here; the `observer_callback` typedef is the shape of the one registered in
`application.c`.

### 3.6 C patterns
* **Macro-defined function-pointer typedef** (`:4-5`) → Rust `type` alias.
* **Sparse designated-initialiser lookup table indexed by a negated error code** (`:26-44`) →
  Rust `match` function.
* **`static CFStringRef[]` initialised from `CFSTR` macros** (`:57-66`) → `OnceLock`.
* **Bit mask in `uint8_t` whose "all" constant deliberately excludes two members** (`:20-24`) →
  explicit constant, not `bitflags::all()`.

### 3.7 Comments to carry over
None.

---

## 4. `src/application.c`

### 4.1 Purpose

Implements the AXObserver plumbing for one application: creating the observer, subscribing to
the seven application-level notifications, and translating every incoming AX notification into
an `event_loop_post` onto yabai's own event queue. Also the small AX/Carbon queries
(`main window`, `focused window`, `frontmost`, `hidden`, `window list`) and the
create/destroy pair.

### 4.2 Types
None.

### 4.3 Globals / statics

| Symbol | Type | Declared | Threads | Synchronisation |
| --- | --- | --- | --- | --- |
| `g_event_loop` | `struct event_loop` (defined `src/yabai.c:34`) | `src/application.c:1` | `event_loop_post` called from the **main thread** here; consumed on the event-loop thread | the queue itself is lock-free (`__sync_bool_compare_and_swap` on `tail->next`, `src/event_loop.c:1736`) |
| `__pending_window_focus` | `volatile bool` (defined `src/event_loop.c:11`) | `src/application.c:2` | **written from the main thread** at `src/application.c:11` with `__ATOMIC_RELEASE`; written from the event-loop thread at `src/event_loop.c:639`; read at `src/event_loop.c:345`-ish with `__ATOMIC_RELAXED` | C11 atomics on a `volatile bool` |
| `g_connection` | `int` | **not declared in this file** — leaks in from an earlier manifest include | read at `src/application.c:135` | none |

**Rust**: `__pending_window_focus` → `static PENDING_WINDOW_FOCUS: AtomicBool`, with
`store(true, Ordering::Release)` / `load(Ordering::Relaxed)` matching the C orderings exactly.
Do not "upgrade" to `SeqCst`; the relaxed load is load-bearing for the front-switch heuristic.

### 4.4 Functions

| Line | Signature | What it does | Thread | Allocates / frees | External symbols |
| --- | --- | --- | --- | --- | --- |
| 6 | `static void application_notification_handler(AXObserverRef, AXUIElementRef element, CFStringRef notification, void *context)` | Dispatches nine AX notifications onto the event queue. | **main run loop** — the observer source is on `CFRunLoopGetMain()` (`:57`). | `CFRetain(element)` for `WINDOW_CREATED` only (`:9`); the retain is balanced in `EVENT_HANDLER(WINDOW_CREATED)`. Nothing else. | `CFEqual`, `CFRetain`, `event_loop_post`, `ax_window_id` (→ `_AXUIElementGetWindow`), `__atomic_store_n`, `__sync_bool_compare_and_swap` |
| 43 | `bool application_observe(struct application *application)` | `AXObserverCreate` + seven `AXObserverAddNotification` with `application` as refcon; sets `is_observing`; adds the observer's run-loop source to the **main** run loop. Returns `(notification & 0x1F) == 0x1F`. | **both**: main thread at startup (`src/window_manager.c:2744`), event-loop thread in steady state (`src/event_loop.c:148`) | nothing heap; takes the observer at +1 into `observer_ref` | `AXObserverCreate`, `AXObserverAddNotification`, `CFRunLoopGetMain`, `AXObserverGetRunLoopSource`, `CFRunLoopAddSource`, `debug` |
| 63 | `void application_unobserve(struct application *application)` | Removes each registered notification, clears `is_observing`, invalidates the source, releases the observer. | both | releases `observer_ref` | `AXObserverRemoveNotification`, `CFRunLoopSourceInvalidate`, `AXObserverGetRunLoopSource`, `CFRelease` |
| 79 | `uint32_t application_main_window(struct application *)` | AX `kAXMainWindow` → window id, 0 if absent. | event-loop thread | releases the window ref | `AXUIElementCopyAttributeValue`, `ax_window_id`, `CFRelease` |
| 91 | `uint32_t application_focused_window(struct application *)` | AX `kAXFocusedWindow` → window id, 0 if absent. | event-loop thread | releases the window ref | same |
| 103 | `bool application_is_frontmost(struct application *)` | `_SLPSGetFrontProcess` then `SameProcess`. | event-loop thread (`src/event_loop.c:657`) | nothing | `_SLPSGetFrontProcess`, `psn_equals` → `SameProcess` |
| 112 | `bool application_is_hidden(struct application *)` | `IsProcessVisible(&psn) == 0`. Deprecated Carbon. | event-loop thread, and main thread inside `application_create` during startup | nothing | `IsProcessVisible` |
| 118 | `CFArrayRef application_window_list(struct application *)` | **Returns +1** the AX `kAXWindows` array, or NULL. Caller must release. | both | CF +1 returned | `AXUIElementCopyAttributeValue` |
| 125 | `struct application *application_create(struct process *process)` | `malloc` + `memset`, `AXUIElementCreateApplication(pid)`, copies psn/pid, **aliases `process->name`**, probes hidden state, resolves the SkyLight connection id. | both | `malloc(sizeof(struct application))`; `ref` at +1 | `malloc`, `memset`, `AXUIElementCreateApplication`, `IsProcessVisible`, `SLSGetConnectionIDForPSN` |
| 140 | `void application_destroy(struct application *application)` | Releases `ref`, frees the struct. **Does not free `name`** (aliased) and **does not** release `observer_ref` (that is `application_unobserve`'s job). | both | releases 1 CF object, frees the struct | `CFRelease`, `free` |

### 4.5 Callbacks registered with the OS

**The AXObserver** — the single most important callback in this reader.

* Registration: `AXObserverCreate(application->pid, application_notification_handler, &application->observer_ref)` at `src/application.c:45`, then seven `AXObserverAddNotification(observer, application->ref, ax_application_notification[i], application)` at `:47`.
* Run-loop attachment: `CFRunLoopAddSource(CFRunLoopGetMain(), AXObserverGetRunLoopSource(observer), kCFRunLoopDefaultMode)` at `:57`. Note this is called from the **event-loop pthread** in steady state, mutating the *main* run loop from another thread — `CFRunLoopAddSource` is documented thread-safe, so this is legal. Rust must keep it.
* The **same observer** also receives the three *window*-level notifications registered by `window_observe` (`src/window.c:10`), with a `struct window *` refcon instead of `struct application *`.
* Thread it fires on: **main run loop**.
* Context pointers and their lifetimes:
  * `struct application *` for indices 0–6. Lifetime: until `application_unobserve` + `application_destroy` (`src/event_loop.c:318-319`). **Never actually dereferenced by the handler** — the handler ignores `context` for those six branches and only uses it for the two miniaturize branches and the destroy branch. That is why a stale application pointer is survivable.
  * `struct window *` for `kAXWindowMiniaturized` / `kAXWindowDeminiaturized` / `kAXUIElementDestroyed`. Lifetime: until `window_unobserve` + `window_destroy`. **Is** dereferenced, at `:36`.
* `kCFRunLoopDefaultMode` (not `kCFRunLoopCommonModes`) — so AX notifications are **not**
  delivered while a modal/tracking run-loop mode is active. Preserve this exactly; using
  `CommonModes` would change when yabai sees window events during drags.

### 4.6 C patterns in `application.c`, and the Rust translation

#### (a) `#pragma clang diagnostic push/ignored "-Wunused-parameter"` — `:4-5`, `:41`
Cosmetic. Rust: `_observer: AXObserverRef` naming.

#### (b) `#pragma clang diagnostic ignored "-Wdeprecated-declarations"` — `:110-111`, `:116`
Wraps `IsProcessVisible`. Rust: the deprecated Carbon Process Manager symbols must be declared
by hand in an `extern "C"` block against `ApplicationServices`; there is no deprecation warning
to silence. Declaration needed:
`fn IsProcessVisible(psn: *const ProcessSerialNumber) -> Boolean;`

#### (c) CFString dispatch chain
`:8-39` is an `if (CFEqual(notification, kAX…)) … else if …` ladder over nine constants.
**Rust**: the cheapest faithful translation keeps `CFEqual` against the `OnceLock` CFStrings.
A tempting optimisation is to convert `notification` to a Rust `&str` and `match` — that is
behaviourally identical (AX notification names are ASCII) but adds a UTF-8 conversion per
notification on the main thread. Recommend keeping `CFEqual`; it is a pointer-compare fast path
for `CFSTR` constants.

#### (d) Pointer-as-integer event payloads
`:12`, `:14`, `:16`, `:18`, `:20`: `(void *)(intptr_t) ax_window_id(element)` — a `uint32_t`
widened to `intptr_t` then reinterpreted as a pointer, unpacked at the other end with
`(uint32_t)(intptr_t) context` (`src/event_loop.c:640`).
**Rust**: `window_id as usize as *mut c_void`, unpacked as `context as usize as u32`. Keep the
two-step cast; `as *mut c_void` directly from `u32` does not compile.

#### (e) `CFRetain` handed to the queue
`:9`: `event_loop_post(&g_event_loop, WINDOW_CREATED, (void *) CFRetain(element), 0)`.
Ownership of the +1 transfers to the event handler.
**Rust**: `CfOwned::retain(element).into_raw() as *mut c_void`, with the handler reconstructing
`CfOwned::from_raw`.

#### (f) CAS on `id_ptr` as a liveness check — `:36`
```c
if (!__sync_bool_compare_and_swap(&window->id_ptr, &window->id, NULL)) return;
```
Full sequential-consistency CAS. Semantics: *the first* observer to claim the window wins; every
later claimant (including the `APPLICATION_TERMINATED` handler at `src/event_loop.c:280`) bails
out. This is the only thing standing between the main thread and a use-after-free of `window`.
**Rust**: `window.id_ptr.compare_exchange(&raw mut window.id, ptr::null_mut(), SeqCst, SeqCst).is_ok()`.
`__sync_bool_compare_and_swap` is `__ATOMIC_SEQ_CST` on both success and failure — do not weaken
it.
**Note the residual race the C accepts**: `window` may already have been freed by the event-loop
thread between the moment the notification was delivered and the CAS. The C code lives with it;
the carried-over comment at `:30-34` documents only the *other* half of the problem (queued
events). Phase 2 should reproduce it and record it as a known defect rather than redesign.

#### (g) `array_count(ax_application_notification)` — `:46`, `:66`
`(int)(sizeof(a)/sizeof(*a))` = 7. Rust: `AX_APPLICATION_NOTIFICATION.len()`.

#### (h) `debug(...)` varargs — `:52`
`src/misc/log.h:6` — a `vfprintf(stdout, …)` gated on `g_verbose`, **not** flushed and **not**
locked. Concurrent `debug` from the main and event-loop threads interleaves at `FILE*` lock
granularity.
**Rust**: a `debug!` macro writing to `io::stdout()` gated on a `static VERBOSE: AtomicBool`.
`io::Stdout` is line-buffered and internally locked, which is *stricter* than C's behaviour —
acceptable, but note the output interleaving may differ under `-V`.

#### (i) `malloc` + `memset(0)` + field assignment — `:127-128`
**Rust**: `Box::new(Application { …Default… })` then `Box::into_raw`. The `memset` guarantees
every unassigned field is zero (`notification`, `is_observing`, `ax_retry` are never assigned in
`application_create`), so the Rust struct must give them `false`/`0` explicitly.

#### (j) `array_count` over a `uint8_t` bitmask with `1 << i`
`:49`, `:67`, `:70` — `i` runs 0..6, so `1 << i` stays within `uint8_t`. Rust: `1u8 << i`.

#### (k) Absent patterns
No goto, no stretchy buffers, no hash tables, no `ts` allocation, no `FILE*` output, no
`fork`/`exec`, no `setjmp`, no regex, no SIMD. The only ObjC-adjacent thing is the deprecated
Carbon call in `application_is_hidden`.

### 4.7 Comments to carry over verbatim

`src/application.c:30-34`:

```c
        //
        // NOTE(asmvik): Flag events that are already queued, but not yet processed,
        // so that they will be ignored; the memory we allocated is still valid and will
        // be freed when this event is handled.
        //
```

This is the only comment in the file. It belongs directly above the `id_ptr` CAS.

---

## 5. `src/process_manager.h`

### 5.1 Purpose

Declares `struct process` (a Carbon-discovered running application before yabai decides to
observe it) and `struct process_manager` (the PSN-keyed table of them plus the Carbon
application-event handler that keeps the table current and tracks front-process switches).

### 5.2 Types

#### `PROCESS_EVENT_HANDLER(name)` / `process_event_handler` — `src/process_manager.h:4-5`
```c
#define PROCESS_EVENT_HANDLER(name) OSStatus name(EventHandlerCallRef ref, EventRef event, void *context)
```
The Carbon `EventHandlerProcPtr` shape.
**Rust**: `type ProcessEventHandler = unsafe extern "C" fn(EventHandlerCallRef, EventRef, *mut c_void) -> OSStatus;`

#### `struct process` — `src/process_manager.h:7-15`

| Field | C type | Owner | Cross-thread |
| --- | --- | --- | --- |
| `psn` | `ProcessSerialNumber` | value | written once before publication; read on both threads |
| `pid` | `pid_t` | — | same |
| `name` | `char *` | **Owned by `struct process`**; `cfstring_copy` (`malloc`) at `src/process_manager.c:44`, freed at `:263`. Aliased by `application->name`. | written once before publication; read on the event-loop thread |
| `ns_application` | `void *` (an `NSRunningApplication *`, retained) | **Owned**; `[… retain]` at `src/workspace.m:30`, `[… release]` at `src/workspace.m:65` | **written with `__ATOMIC_RELEASE` from the main thread (`src/process_manager.c:66`) and from the event-loop thread (`src/event_loop.c:87`); read with `__ATOMIC_RELAXED` from both** |
| `policy` | `int` (`NSApplicationActivationPolicy`) | — | **Never initialised by `process_create`** (`src/process_manager.c:61-67` sets psn/pid/name/terminated/ns_application but not `policy`, and there is no `memset`). First written by `workspace_application_is_observable` (`src/workspace.m:107`/`:110`). Read at `src/workspace.m:216` from the KVO callback on the main thread. **Reading it before `workspace_application_is_observable` runs reads uninitialised heap.** |
| `terminated` | `bool volatile` | — | **written `__ATOMIC_RELEASE` from the main thread (`src/process_manager.c:189`), read `__ATOMIC_RELAXED` from the event-loop thread (`src/event_loop.c:79`)** |

**Rust shape**:

```text
pub struct Process {
    pub psn: ProcessSerialNumber,
    pub pid: pid_t,
    pub name: *mut c_char,                 // owned, freed in process_destroy
    pub ns_application: AtomicPtr<c_void>, // owned NSRunningApplication, retained
    pub policy: i32,                       // MUST be explicitly initialised; see divergence note
    pub terminated: AtomicBool,
}
```

Rust cannot leave `policy` uninitialised without `MaybeUninit`. Initialise it to
`NSApplicationActivationPolicyProhibited` (2) or 0 and **record the divergence** — it fixes a
latent bug. Do not use `MaybeUninit` to be "faithful"; nothing depends on the garbage value.

#### `struct process_manager` — `src/process_manager.h:17-28`

| Field | C type | Notes | Threads |
| --- | --- | --- | --- |
| `process` | `struct table` (`src/misc/hashtable.h:16`) | PSN → `struct process *`. The table **owns a `malloc`'d copy of the key** (`_table_add`, `src/misc/hashtable.h:125-128`) and frees it in `table_remove` (`:143`). It does **not** own the value. | **main thread only** (`src/process_manager.c:162,182,186,190,197,226,257`; `src/event_loop.c:94,158` inside main-queue dispatch blocks; `src/window_manager.c:2740` at startup) |
| `target` | `EventTargetRef` | `GetApplicationEventTarget()` — a process-global singleton, +0 | main thread |
| `handler` | `EventHandlerUPP` | `NewEventHandlerUPP(process_handler)` — **never disposed** | main thread |
| `type[3]` | `EventTypeSpec[3]` | `{kEventClassApplication, kEventAppLaunched/Terminated/FrontSwitched}`. `EventTypeSpec` is `{ UInt32 eventClass; UInt32 eventKind; }`; **this array is passed by pointer to `InstallEventHandler`**, so the Rust type must be `#[repr(C)]`. | main thread |
| `ref` | `EventHandlerRef` | out-param of `InstallEventHandler`; never removed | main thread |
| `front_pid` | `pid_t` | current frontmost pid | written on the **event-loop thread** (`src/event_loop.c:384`), read on the **event-loop thread** (`src/event_loop.c:377`) *and* on the event-loop thread in `event_signal.c:158,170,184`. Initialised on the main thread at `src/process_manager.c:248`. |
| `last_front_pid` | `pid_t` | previous frontmost pid | same |
| `switch_event_time` | `EventTime` (= `double`) | `GetCurrentEventTime()` | written `src/event_loop.c:382`, read `src/event_signal.c:156` — event-loop thread; initialised on main at `src/process_manager.c:250` |
| `finder_psn` | `ProcessSerialNumber` | cached Finder PSN, used to defocus (`src/window_manager.c:1927`, `:2104`, `:2475`) | written on the main thread at startup (`src/process_manager.c:223`), read on the event-loop thread |

**Rust**: `process: HashMap<PsnKey, *mut Process>` where
`PsnKey(u32 /*low*/, u32 /*high*/)` with `Hash` = `low` only (to match `hash_psn`) and `Eq` =
`SameProcess`. **Simpler and equivalent**: derive `Hash`/`Eq` on both words. `SameProcess`
compares both longs, and `hash_psn` only hashes the low long; a derived `Hash` over both words
changes only bucket distribution, which is unobservable. Recommend the derived version and note
it.

### 5.3 Globals / statics
None declared in the header.

### 5.4 Functions
Declarations only (`:30-33`). See §6.

### 5.5 Callbacks
None registered here.

### 5.6 C patterns
* Macro-defined handler typedef (`:4-5`) → Rust `type` alias.
* `volatile` field used as a cross-thread flag (`:14`) → `AtomicBool`.
* `void *` holding a retained Objective-C object (`:12`) → `AtomicPtr<c_void>` plus explicit
  `objc2` `retain`/`release` calls in the `workspace` module.
* Fixed-size array field passed to the OS (`:22`) → `#[repr(C)] [EventTypeSpec; 3]`.

### 5.7 Comments to carry over
None.

---

## 6. `src/process_manager.c`

### 6.1 Purpose

Discovers every running Carbon application at startup and keeps the PSN-keyed table current by
installing a Carbon application-event handler for launch/terminate/front-switch. It filters out
XPC services, blacklisted helper processes and debugger-attached processes, and it provides the
window-server query used to work out which space a given connection's frontmost window lives on.

### 6.2 Types
None declared. Uses `struct kinfo_proc` (`src/process_manager.c:72`) and `ProcessInfoRec`
(`:33`) from the system.

`ProcessInfoRec` on LP64 (from the SDK's `HIServices/Processes.h`) is:
`UInt32 processInfoLength; StringPtr processName; ProcessSerialNumber processNumber;
UInt32 processType; OSType processSignature; UInt32 processMode; Ptr processLocation;
UInt32 processSize; UInt32 processFreeMem; ProcessSerialNumber processLauncher;
UInt32 processLaunchDate; UInt32 processActiveTime; FSRefPtr processAppRef;`
**This struct is passed to `GetProcessInformation`, so the Rust type must be `#[repr(C)]` and
byte-identical, and `processInfoLength` must be set to `size_of::<ProcessInfoRec>()`.** The C
code uses a designated initialiser (`:33`) that zeroes every other field — including
`processName` and `processAppRef`, which the SDK header warns must be NULL or valid.

### 6.3 Globals / statics

| Symbol | Type | Declared | Threads | Synchronisation |
| --- | --- | --- | --- | --- |
| `g_event_loop` | `struct event_loop` | `src/process_manager.c:1` | `event_loop_post` called from the **main thread** (`:183`, `:194`, `:200`) | lock-free queue |
| `g_workspace_context` | `void *` (the `workspace_context` ObjC object, `src/yabai.c:35`) | `src/process_manager.c:2` | read on the main thread (`:191`) and the event-loop thread (`:262`) | none; written once at startup |
| `g_connection` | `int` | **not declared here** — leaks in from an earlier manifest include | read at `:113`, `:119` | none |
| `process_name_blacklist[]` | `static const char *[4]` | `:14-20` | read on the main thread only | none |

`process_name_blacklist` contents, verbatim: `"Übersicht"` (UTF-8, multi-byte), `"Slack Helper (Plugin)"`,
`"Google Chrome Helper (Plugin)"`, `"qlmanage"`.
**Rust**: `const PROCESS_NAME_BLACKLIST: [&str; 4]`. The comparison is `string_equals`
(`strcmp`, `src/misc/helpers.h:254`), i.e. **byte-exact**, so a Rust `==` on `&str` /
`CStr::to_bytes()` is equivalent. Do not case-fold or normalise: `"Übersicht"` is compared as the
raw UTF-8 bytes `C3 9C 62 …`.

### 6.4 Functions

| Line | Signature | What it does | Thread | Allocates / frees | External symbols |
| --- | --- | --- | --- | --- | --- |
| 4 | `static unsigned long hash_psn(void *key)` | `((ProcessSerialNumber *)key)->lowLongOfPSN`. | main | none | none |
| 9 | `static int compare_psn(void *a, void *b)` | `psn_equals` → `SameProcess`. | main | none | `SameProcess` |
| 24 | `static inline pid_t process_pid_for_psn(ProcessSerialNumber psn)` | `GetProcessPID`; returns 0 on failure (pre-initialised). Takes the PSN **by value** but passes `&psn` — so it operates on a stack copy. | main | none | `GetProcessPID` |
| 31 | `static struct process *process_create(ProcessSerialNumber psn, pid_t pid)` | `GetProcessInformation`, `CopyProcessName` → `malloc`'d UTF-8 name; rejects a missing name, an `'XPC!'` process type, and blacklisted names; then `malloc`s and publishes the `struct process` with release stores for `terminated` and `ns_application`. | main | `cfstring_copy` (`malloc`) for `name`; `malloc(sizeof(struct process))`; `CFRelease(process_name_ref)`; `free(process_name)` on every reject path | `GetProcessInformation`, `CopyProcessName`, `cfstring_copy`, `CFRelease`, `string_equals`, `malloc`, `__atomic_store_n`, `workspace_application_create_running_ns_application`, `debug` |
| 70 | `static bool process_is_being_debugged(pid_t pid)` | `sysctl(CTL_KERN, KERN_PROC, KERN_PROC_PID, pid)` → `P_TRACED`. **Ignores `sysctl`'s return value**; on failure it reads `info.kp_proc.p_flag`, which was explicitly zeroed at `:73`, so it returns `false`. | main | stack only | `sysctl` |
| 82 | `uint64_t process_manager_active_space_for_psn(int connection)` | Collects every space of every active display, asks the window server for that connection's windows on those spaces, walks the query iterator and returns the space of the first "real" window. | **event-loop thread** (`src/event_loop.c:360`) | `ts` memory from `display_manager_active_display_list` and `display_space_list` (never freed); releases the two CFArrays and the query/iterator | `display_manager_active_display_list`, `display_space_list`, `cfarray_of_cfnumbers`, `SLSCopyWindowsWithOptionsAndTags`, `CFArrayGetCount`, `SLSWindowQueryWindows`, `SLSWindowQueryResultCopyWindows`, `SLSWindowIteratorAdvance`, `SLSWindowIteratorGetTags/GetAttributes/GetParentID/GetWindowID/GetLevel`, `window_space`, `CFRelease` |
| 151 | `static OSStatus process_handler(EventHandlerCallRef, EventRef event, void *context)` | Carbon handler for launched/terminated/front-switched. Extracts the PSN, then adds to / removes from the table and posts to the event queue. | **main run loop** | `process_create` allocates; `table_add` allocates the key copy; `table_remove` frees the key copy. Nothing frees `struct process` here — that is `process_destroy`, on the event-loop thread. | `GetEventParameter`, `GetEventKind`, `table_add`, `table_remove`, `event_loop_post`, `workspace_application_unobserve`, `__atomic_store_n`, `debug` |
| 208 | `static void process_manager_add_running_processes(struct process_manager *pm)` | `GetNextProcess` loop from `{kNoProcess, kNoProcess}`; creates and tables every process; caches the Finder PSN. | **main** (called from `process_manager_begin` under an `NSAutoreleasePool`) | as `process_create` | `GetNextProcess`, `process_pid_for_psn`, `process_is_being_debugged`, `process_create`, `string_equals`, `table_add`, `debug` |
| 230 | `bool process_manager_begin(struct process_manager *pm)` | Fills the handler/target/type fields, inits the table at capacity 125, enumerates running processes inside an autorelease pool, seeds `front_pid`/`last_front_pid`/`switch_event_time`, installs the Carbon handler. Returns whether `InstallEventHandler` succeeded. | **main** (`src/yabai.c:299`) | `NewEventHandlerUPP` (never disposed); `table_init` `malloc`s the bucket array; `NSAutoreleasePool` alloc/drain | `GetApplicationEventTarget`, `NewEventHandlerUPP`, `table_init`, `[NSAutoreleasePool alloc/init/drain]`, `_SLPSGetFrontProcess`, `GetProcessPID`, `GetCurrentEventTime`, `InstallEventHandler` |
| 255 | `struct process *process_manager_find_process(struct process_manager *pm, ProcessSerialNumber *psn)` | `table_find`. | **main** (all four call sites, §0.2) | none | `table_find` |
| 260 | `void process_destroy(struct process *process)` | Removes the KVO observations, releases the `NSRunningApplication`, frees `name`, frees the struct. | **event-loop thread** (`src/event_loop.c:344`) | frees `name` and the struct; releases the ObjC object | `workspace_application_destroy_running_ns_application`, `free` |

### 6.5 Callbacks registered with the OS

**The Carbon application-event handler.**

* Registration: `InstallEventHandler(pm->target, pm->handler, 3, pm->type, pm, &pm->ref)` at
  `src/process_manager.c:251`, with `pm->target = GetApplicationEventTarget()` (`:232`) and
  `pm->handler = NewEventHandlerUPP(process_handler)` (`:233`).
* Callback: `process_handler` (`:151`).
* Events: `kEventClassApplication` × `{kEventAppLaunched, kEventAppTerminated, kEventAppFrontSwitched}` (`:234-239`).
* Thread it fires on: the **main run loop** (the application event target is serviced by the
  Carbon event dispatcher that `[NSApp run]` pumps).
* Context pointer: `pm` (`&g_process_manager`, `src/yabai.c:28`) — a global with process
  lifetime, never freed. Safe.
* **Never removed.** No `RemoveEventHandler`, no `DisposeEventHandlerUPP`. Rust may leak the
  same way (`Box::leak` the trampoline, or use a plain `extern "C" fn`, which needs no
  allocation at all).

**Rust**: `extern "C" fn process_handler(_call_ref: EventHandlerCallRef, event: EventRef,
context: *mut c_void) -> OSStatus`, passed directly to `NewEventHandlerUPP` (which on modern
macOS is an identity cast of the function pointer). Declare
`fn InstallEventHandler(target: EventTargetRef, handler: EventHandlerUPP, num_types: u32,
list: *const EventTypeSpec, user_data: *mut c_void, out: *mut EventHandlerRef) -> OSStatus;`

### 6.6 C patterns in `process_manager.c`, and the Rust translation

#### (a) Macro-generated function definitions
`static TABLE_HASH_FUNC(hash_psn)` (`:4`), `static TABLE_COMPARE_FUNC(compare_psn)` (`:9`),
`static PROCESS_EVENT_HANDLER(process_handler)` (`:151`). The macro expands to the whole
signature, so the parameter names (`key`, `key_a`, `key_b`, `ref`, `event`, `context`) are
invisible at the definition site.
**Rust**: write the signatures out. The `TABLE_*` pair disappears entirely if the table becomes
a `HashMap` (§5.2).

#### (b) Hash table keyed by a value type, with a malloc'd key copy
`table_init(&pm->process, 125, hash_psn, compare_psn)` (`:240`), `table_add` (`:182`, `:226`),
`table_remove` (`:190`), `table_find` (`:257`).
**Rust**: `HashMap<ProcessSerialNumber, *mut Process>` with `#[derive(Hash, PartialEq, Eq)]` on a
`#[repr(C)] struct ProcessSerialNumber { high_long_of_psn: u32, low_long_of_psn: u32 }`.
Divergence to record: `hash_psn` hashes only `lowLongOfPSN`, and `table_get_bucket` uses
`hash(key) % capacity` with **no rehash on remove** and a 0.75 load factor doubling
(`src/misc/hashtable.h:132-135`). None of that is observable; only iteration order could be, and
the only iteration over this table (`src/window_manager.c:2740`) has order-independent effects —
**except** that `window_manager_begin` calls `application_create`/`window_manager_add_application`
in table order, which determines the order applications are first observed. Not user-visible.

#### (c) `'XPC!'` multi-character character constant
`src/process_manager.c:47`: `process_info.processType == 'XPC!'`. In clang a four-character
constant is a big-endian packing: `'X'<<24 | 'P'<<16 | 'C'<<8 | '!'` = `0x58504321`.
**Rust**: `const XPC_PROCESS_TYPE: u32 = u32::from_be_bytes(*b"XPC!");` — write it as
`0x5850_4321` with the `from_be_bytes` form as documentation-by-construction. Getting the
endianness wrong here silently makes yabai start tracking every XPC service.

#### (d) Cross-thread publication with explicit memory orderings
`:65` `__atomic_store_n(&process->terminated, false, __ATOMIC_RELEASE)`,
`:66` `__atomic_store_n(&process->ns_application, …, __ATOMIC_RELEASE)`,
`:189` `__atomic_store_n(&process->terminated, true, __ATOMIC_RELEASE)`.
**Rust**: `AtomicBool::store(…, Ordering::Release)` / `AtomicPtr::store(…, Ordering::Release)`.
The matching loads in `event_loop.c` are `__ATOMIC_RELAXED` — the C code is technically
under-synchronised (release without a matching acquire), but reproduce it exactly.

#### (e) Explicit compiler barrier
`src/process_manager.c:192`: `__asm__ __volatile__ ("" ::: "memory");` between
`workspace_application_unobserve` and the `event_loop_post` for `APPLICATION_TERMINATED`.
**Rust**: `std::sync::atomic::compiler_fence(Ordering::SeqCst);`. This is a *compiler* barrier
only (no CPU fence), and `compiler_fence` is the exact equivalent. Do **not** substitute
`fence(SeqCst)`, which emits a real barrier.

#### (f) Pointer-to-struct payload handed to another thread
`:183`, `:194`, `:200`: `event_loop_post(&g_event_loop, …, process, 0)`. The `struct process *`
is published to the event-loop thread. For `APPLICATION_TERMINATED` the table entry is removed
first (`:190`), so the main thread can no longer find it — the event-loop thread becomes the sole
owner and frees it in `process_destroy`. For `APPLICATION_LAUNCHED` the table keeps the pointer
and the event-loop thread only reads it.
**Rust**: `*mut Process` as `*mut c_void`. Keep raw pointers; an `Arc<Process>` would change
when the `NSRunningApplication` is released and would not reproduce the "terminated during
launch" path at `src/event_loop.c:79-83`.

#### (g) `goto`-ladder cleanup with **unchecked** CF handles
`src/process_manager.c:114-145`. Differences from the `window.c` ladders that must be preserved:
* `query` (`:119`) and `iterator` (`:120`) are **not NULL-checked** before
  `SLSWindowIteratorAdvance` (`:122`) and `CFRelease` (`:139-140`). A NULL query crashes.
* `CFRelease(query)` happens **before** `CFRelease(iterator)` — the reverse of `window.c`.
* `if (!count) goto out;` (`:117`) skips the query entirely and releases only
  `window_list_ref` and `space_list_ref`.
* `space_list` may be `NULL` when every `display_space_list` returns NULL; then
  `cfarray_of_cfnumbers(NULL, 8, 0, kCFNumberSInt64Type)` builds an empty CFArray. In C this
  declares a zero-length VLA (`CFNumberRef temp[0]`, `src/misc/helpers.h:346`) — technically UB,
  in practice fine.
**Rust**: `CfOwned` + `?`-style early returns reproduce the release set; keep the
`if count == 0` early exit and keep the query/iterator unchecked only if phase 2 wants literal
fidelity. Recommend checking them and recording the divergence — it converts a crash into a
`return 0`.

#### (h) The magic tag/attribute/level predicate
`src/process_manager.c:129-136`:
```c
if (parent_wid == 0) {
    if (level == 0 || level == 3 || level == 8) {
        if (((attributes & 0x2) || (tags & 0x400000000000000)) && (((tags & 0x1)) || ((tags & 0x2) && (tags & 0x80000000)))) {
```
**Rust**: transcribe verbatim. `0x400000000000000` is a `u64` literal (58th bit);
`0x80000000` applied to a `u64` `tags` is bit 31. Writing these as `u64` constants with
underscores is fine; do **not** name them — no comment exists to justify a name, and the rule is
to carry over only existing comments.

#### (i) `sysctl` MIB array
`src/process_manager.c:76`: `int mib[4] = { CTL_KERN, KERN_PROC, KERN_PROC_PID, pid };`
**Rust**: `let mut mib = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PID, pid];`
`sysctl(mib.as_mut_ptr(), 4, &mut info as *mut _ as *mut c_void, &mut size, ptr::null_mut(), 0)`.
`struct kinfo_proc` comes from `libc`; the field path is `info.kp_proc.p_flag` and the mask is
`libc::P_TRACED`. Note `info` is **not** zero-initialised in C (only `kp_proc.p_flag` is,
`:73`), so a Rust `MaybeUninit::zeroed()` is a (benign) divergence.

#### (j) Objective-C autorelease pool
`src/process_manager.c:242-244`:
```c
NSAutoreleasePool *pool = [[NSAutoreleasePool alloc] init];
process_manager_add_running_processes(pm);
[pool drain];
```
Needed because `CopyProcessName` and `NSRunningApplication` autorelease.
**Rust**: `objc2::rc::autoreleasepool(|_| { add_running_processes(pm) })`. `[pool drain]` and
`autoreleasepool`'s drop are equivalent here (`drain` differs from `release` only under GC,
which does not exist on modern macOS).

#### (k) Objective-C object behind a `void *`
`process->ns_application` and `g_workspace_context`. Only `workspace.m` dereferences them.
**Rust**: keep them as `*mut c_void` / `AtomicPtr<c_void>` at this layer; the `workspace` module
casts to `objc2` types.

#### (l) Deprecated Carbon Process Manager API
`GetProcessPID` (`:27`, `:248`), `GetProcessInformation` (`:34`), `CopyProcessName` (`:37`),
`GetNextProcess` (`:211`), `SameProcess` (via `psn_equals`), `IsProcessVisible`
(`src/application.c:114`), `GetCurrentEventTime` (`:250`), plus `kNoProcess` (= 0).
All wrapped by the `-Wdeprecated-declarations` pragma pushed at `:22-23` and **popped at `:253`**
— i.e. it covers almost the entire file, which is why `process_manager_begin` compiles quietly.
**Rust**: hand-written `extern "C"` declarations linked against `ApplicationServices`
(`-framework Carbon` already covers it, `makefile:2`). None of these has a Rust crate binding;
they must go in the `sys` module. `ProcessSerialNumber` must be `#[repr(C)] { high_long_of_psn: u32, low_long_of_psn: u32 }`.

#### (m) `debug(...)` with a UTF-8 process name
`:40`, `:48`, `:55`, `:175`, `:214`, `:222`. `process_name` is a `char *` that may hold invalid
UTF-8 (whatever `CFStringGetCString` produced, which is always valid UTF-8, so in practice fine).
**Rust**: `CStr::to_string_lossy()`.

#### (n) Error paths that leak or exit
* `process_create` frees `process_name` on the XPC and blacklist rejects (`:49`, `:56`) but
  **`CFRelease(process_name_ref)` already happened at `:45`** — correct.
* `process_create` returns NULL when `CopyProcessName` fails (`:39-42`) **without** having
  allocated anything — correct.
* `process_manager_begin` returns `false` if `InstallEventHandler` fails but leaves the table,
  the UPP and every created `struct process` allocated. `main` then calls `error(...)` which
  `exit(EXIT_FAILURE)`s (`src/yabai.c:299-301`, `src/misc/log.h:34`) — so the leak is
  irrelevant. **Rust**: `std::process::exit(1)` after printing to stderr; do **not** convert
  these to `Result` + `?` all the way to `main`, because `error()` exits *before* any unwinding
  and several globals are deliberately never torn down.

#### (o) Absent patterns
No stretchy buffers, no intrusive lists, no `FILE*` output, no `setjmp`, no regex, no SIMD, no
`fork`/`exec`, no blocks. The only ObjC constructs are the autorelease pool and the `void *`
NSRunningApplication.

### 6.7 Comments to carry over verbatim

`src/process_manager.c:98-102` (inside `process_manager_active_space_for_psn`, above the
`if (!space_list) space_list = list;` accumulation):

```c
        //
        // NOTE(asmvik): display_space_list(..) uses a linear allocator,
        // and so we only need to track the beginning of the first list along
        // with the total number of windows that have been allocated.
        //
```

`src/process_manager.c:164-168` (inside `process_handler`, `kEventAppLaunched`, above the early
`return noErr`):

```c
            //
            // NOTE(asmvik): Some garbage applications (e.g Steam) are reported twice with the same PID and PSN for some hecking reason.
            // It is by definition NOT possible for two processes to exist at the same time with the same PID and PSN.
            // If we detect such a scenario we simply discard the dupe notification..
            //
```

These are the only two comments in the file.

---

## 7. Cross-cutting patterns and the recommended Rust model

### 7.1 Ownership model for the three records

`struct window`, `struct application` and `struct process` are **heap records with manual
lifetimes, shared across two threads, deliberately left dangling in queued events and guarded by
a CAS**. Phase 2 must keep them as `Box::into_raw` / `Box::from_raw` raw pointers.

Do not use `Arc`, `Rc`, an arena index or a slotmap:
* `Arc` changes *when* the `AXUIElementRef`, the `NSRunningApplication` and the `char *name` are
  released, which is observable (AX observers stay alive, processes stay retained).
* An index/generation scheme would *fix* the use-after-free the C accepts, changing behaviour in
  the exact paths (`src/application.c:36`, `src/event_loop.c:280`) that the comment at
  `src/application.c:30-34` documents.
* `window->application` is set to `NULL` mid-life (`src/event_loop.c:281`), so it cannot be a
  `&'a Application`.

The Rust modules therefore contain a lot of `unsafe`. Concentrate it: one
`unsafe fn window_create(...) -> *mut Window`, one `unsafe fn window_destroy(window: *mut Window)`,
and safe `&mut Window` methods for everything in between.

### 7.2 Globals

`g_connection`, `g_layer_*_window_level`, `g_verbose`, `__pending_window_focus` are write-once-
at-startup or atomic flags → `AtomicI32` / `AtomicBool` / `OnceLock`.

`g_window_manager`, `g_process_manager`, `g_space_manager`, `g_mouse_state`, `g_event_loop`,
`g_workspace_context` are large mutable structs accessed without any lock from up to three
threads. There is no safe Rust encoding that preserves this. Recommend a single
`struct Globals` behind `static GLOBALS: SyncUnsafeCell<Globals>` (or a hand-rolled
`struct Globals(UnsafeCell<…>); unsafe impl Sync`) with `unsafe fn globals() -> &'static mut Globals`,
and a module-level comment in the *rewrite docs* (not in the code) explaining why. **Do not** wrap
them in `Mutex`: the C code re-enters these structures from AX callbacks while the event-loop
thread holds no lock, and a `Mutex` would deadlock or change ordering.

### 7.3 The AX / SkyLight / Carbon FFI surface used by this reader

`src/misc/extern.h` is already a hand-written FFI header; port it directly to a Rust `sys`
module. From these three files the following are needed:

*SkyLight*: `SLSMainConnectionID`, `SLSCopyManagedDisplayForWindow`, `SLSGetWindowBounds`,
`SLSCopyBestManagedDisplayForRect`, `SLSManagedDisplayGetCurrentSpace`, `SLSCopySpacesForWindows`,
`SLSGetWindowOwner`, `SLSConnectionGetPID`, `SLSWindowIsOrderedIn`, `SLSCopyWindowProperty`,
`SLSGetWindowAlpha`, `SLSGetWindowLevel`, `SLSGetWindowSubLevel`, `SLSWindowQueryWindows`,
`SLSWindowQueryResultCopyWindows`, `SLSWindowIteratorGetCount/Advance/GetParentID/GetWindowID/GetTags/GetAttributes/GetLevel`,
`SLSCopyWindowsWithOptionsAndTags`, `SLSGetConnectionIDForPSN`, `_SLPSGetFrontProcess`.

*Accessibility*: `AXObserverCreate`, `AXObserverAddNotification`, `AXObserverRemoveNotification`,
`AXObserverGetRunLoopSource`, `AXUIElementCreateApplication`, `AXUIElementCopyAttributeValue`,
`AXUIElementIsAttributeSettable`, `AXValueGetValue`, `_AXUIElementGetWindow`.

*Carbon Process Manager / Events*: `GetProcessPID`, `GetProcessInformation`, `CopyProcessName`,
`GetNextProcess`, `SameProcess`, `IsProcessVisible`, `GetApplicationEventTarget`,
`NewEventHandlerUPP`, `InstallEventHandler`, `GetEventParameter`, `GetEventKind`,
`GetCurrentEventTime`.

*Mach*: `mach_msg`, `mig_get_special_reply_port`, `NDR_record`, and the dynamically resolved
`CGSGetConnectionPortById`.

*CoreFoundation / CoreGraphics*: via `core-foundation-sys`, plus `CGDisplayGetDisplayIDFromUUID`,
`CGWindowLevelForKey`.

*libproc*: `proc_name`. *libc*: `sysctl`, `malloc`/`free` (not needed — use `Box`).

Linker: `-framework Carbon -framework Cocoa -framework CoreServices -framework CoreVideo`
`-F/System/Library/PrivateFrameworks -framework SkyLight` (from `makefile:1-2`), emitted from
`build.rs` as `cargo:rustc-link-*`.

### 7.4 Format-string fidelity

The socket protocol is byte-exact JSON assembled by `fprintf`. The traps are catalogued in
§2.6(c). The single riskiest one is `"%d"` applied to a `uint32_t` window id: keep the
`as i32` cast.

### 7.5 `ts` arena lifetime

Every `_ts` function returns memory valid only until the current event handler returns
(`ts_reset()`, `src/event_loop.c:1671`). Encode it as a lifetime (`&'arena str`), not as a raw
pointer, so phase 3's module split cannot accidentally hold one across events.

### 7.6 The `id_ptr` liveness protocol (the single most important invariant)

1. `window_create` (`src/window.c:1101`) sets `id_ptr = &id`.
2. Anyone who wants to *retire* the window CASes `id_ptr` from `&id` to `NULL`
   (`src/application.c:36`, `src/event_loop.c:280`). Exactly one caller wins.
3. Anyone who wants to *use* the window CASes `id_ptr` from `&id` to `&id` — a no-op CAS that
   succeeds only while the window is still live (`src/event_loop.c:647, 681, 731, 833, 877, 930,
   960, 1159, 1240`).
4. `window_destroy` (`src/window.c:1138`) additionally sets `id = 0`, which
   `EVENT_HANDLER(WINDOW_DESTROYED)` (`src/event_loop.c:607`) checks *before* touching anything.

**Rust**: `AtomicPtr<u32>` with `compare_exchange(expected, new, SeqCst, SeqCst)`. Note that in
step 3 the `expected` and `new` are the same pointer, so `compare_exchange` is the right call
(not `compare_exchange_weak`, which may fail spuriously and would change behaviour).

### 7.7 Behaviours a naive translation would silently change — checklist for phase 2

1. `%d` on `uint32_t` window id (§2.6c).
2. `f32` opacity formatted without widening to `f64` (§2.6c).
3. `bitflags::all()` for `AX_APPLICATION_ALL` — would include the two menu bits (§3.2).
4. `bitflags::all()` for `flags |= ~flags` — `!0u64` vs the 33 defined bits (§2.6e).
5. `contains()` vs `intersects()` in `window_check_flag` (§1.2).
6. Zero-initialising the `ts` buffer in `window_space_list` before `CFNumberGetValue` (§2.6l).
7. A fresh zeroed `process_name` buffer instead of the function-local `static` (§2.6f).
8. Initialising `process->policy` (§5.2).
9. NULL-checking `query`/`iterator` in `process_manager_active_space_for_psn` (§6.6g).
10. `'XPC!'` endianness (§6.6c).
11. `compiler_fence` vs `fence` for the inline-asm barrier (§6.6e).
12. `kCFRunLoopDefaultMode` vs `kCFRunLoopCommonModes` for the AX observer source (§4.5).
13. Atomic orderings: every `__ATOMIC_RELEASE` store has a `__ATOMIC_RELAXED` load; keep them.
14. `application->name` and `window->scratchpad` are aliases, not owned — a `String` field
    double-frees.
15. `window_destroy` must set `id = 0` **before** releasing anything else.
16. `error()`/`require()` call `exit()` without unwinding; `std::process::exit` is the match,
    not a `panic!`.

---

## 8. Consolidated list of comments to carry over

Across all six assigned files there are exactly **three** comment blocks. Everything else is
uncommented. Phase 2 must not add any others.

| Location | Block |
| --- | --- |
| `src/application.c:30-34` | `NOTE(asmvik): Flag events that are already queued, but not yet processed, …` (5 lines, `//` box) |
| `src/process_manager.c:98-102` | `NOTE(asmvik): display_space_list(..) uses a linear allocator, …` (5 lines, `//` box) |
| `src/process_manager.c:164-168` | `NOTE(asmvik): Some garbage applications (e.g Steam) are reported twice …` (5 lines, `//` box) |

`src/window.h`, `src/window.c`, `src/application.h` and `src/process_manager.h` contain **no**
comments at all.
