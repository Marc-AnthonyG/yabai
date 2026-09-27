# State and ownership

Elaboration of `DECISIONS.md` 12-18 into a form a translator can apply without choosing. Every
statement here is an instruction, not an option. Where an inventory in `files/` or `sweeps/`
proposes something else, that proposal is dead and is not repeated here.

Conventions used throughout, from `DECISIONS.md` 37: C function and type names are kept
(`struct window_manager` becomes `WindowManager`); bindings, fields and parameters are never
abbreviated — `window_manager`, `window_id`, `space_id`, `display_id`, `process_id`,
`process_serial_number`. Rust sketches carry no comments except comments that exist in the C at
the matching place (`DECISIONS.md` 38).

---

## 1. `EventLoopOwnedState`

### 1.1 The struct

One value. Built on the main thread, moved into the event-loop thread, never shared, never
behind a lock, never reachable through a global.

```rust
pub struct EventLoopOwnedState {
    pub signal_event: [Vec<Signal>; SIGNAL_TYPE_COUNT],
    pub process_manager: ProcessManager,
    pub display_manager: DisplayManager,
    pub window_manager: WindowManager,
    pub space_manager: SpaceManager,
    pub signal_storage: Vec<PendingSignal>,
    pub mouse_drag_state: MouseDragState,
    pub mission_control_mode: MissionControlMode,
    pub focus_follows_mouse_suspended_value: FfmMode,
    pub is_menu_open: i32,
}

unsafe impl Send for EventLoopOwnedState {}
```

The `unsafe impl Send` is required because `WindowManager::system_element` is an
`AXUIElementRef` and `View::uuid` is a `CFStringRef`; the justification belongs in `THREADS.md`,
not here and not in a code comment.

The fields are written in the `src/yabai.c:27-35` declaration order, so that this struct and the
table of §1.2 read in the same order. **That field order is not a parameter order.** No signature
is derived from this struct. §2.2 builds every signature from the C parameter list and appends
only the managers the C reached through a global, in the `src/yabai.c:27-35` order; a function
that declares a manager in C keeps it exactly where C put it, which is frequently not this order
— `window_manager_begin(struct space_manager *sm, struct window_manager *wm)`
(`src/window_manager.c:2737`) stays `space_manager, window_manager` even though `g_window_manager`
(`src/yabai.c:30`) is declared before `g_space_manager` (`src/yabai.c:31`). `THREADS.md` §2.3
lists the same fields in a third order for the same reason: nothing depends on it.

`ProcessManager` is the event-loop-visible remainder of `struct process_manager`
(`src/process_manager.h:17-28`) after the table and the Carbon installation are taken out of it:

```rust
pub struct ProcessManager {
    pub front_process_id: i32,
    pub last_front_process_id: i32,
    pub switch_event_time: f64,
    pub finder_process_serial_number: ProcessSerialNumber,
}
```

`MouseDragState` is the event-loop half of `struct mouse_state` (`src/mouse_handler.h:62-81`),
per `DECISIONS.md` 23:

```rust
pub struct MouseDragState {
    pub current_action: MouseMode,
    pub down_location: CGPoint,
    pub last_moved_time: u64,
    pub window_id: Option<WindowId>,
    pub window_frame: CGRect,
    pub ffm_window_id: WindowId,
    pub direction: u8,
    pub feedback_node: Option<(SpaceId, NodeId)>,
}
```

### 1.2 Every global in `src/yabai.c:27-52`

| C global | line | becomes |
|---|---|---|
| `g_signal_event` | `yabai.c:27` | field `signal_event: [Vec<Signal>; SIGNAL_TYPE_COUNT]` |
| `g_process_manager` | `yabai.c:28` | **split.** Field `process_manager: ProcessManager` (§1.1) for `front_pid`/`last_front_pid`/`switch_event_time`/`finder_psn`; `static PROCESS_TABLE: Mutex<Table<ProcessSerialNumber, Arc<Process>>>` for `process` (`DECISIONS.md` 22); `static CARBON_PROCESS_EVENT_INSTALLATION: OnceLock<CarbonProcessEventInstallation>` for `target`/`handler`/`type`/`ref`, which nothing reads after `process_manager_begin` and which exists only to keep the UPP alive for process lifetime as the C does |
| `g_display_manager` | `yabai.c:29` | field `display_manager: DisplayManager` |
| `g_window_manager` | `yabai.c:30` | field `window_manager: WindowManager` |
| `g_space_manager` | `yabai.c:31` | field `space_manager: SpaceManager` |
| `g_signal_storage` | `yabai.c:32` | field `signal_storage: Vec<PendingSignal>` (`DECISIONS.md` 17) |
| `g_mouse_state` | `yabai.c:33` | **split** (`DECISIONS.md` 23). Field `mouse_drag_state: MouseDragState`; `static MOUSE_TAP_STATE: MouseTapState` of atomics (§1.3) |
| `g_event_loop` | `yabai.c:34` | **split.** `static EVENT_SENDER: OnceLock<Sender<Event>>` for the producer half; the `Receiver<Event>` is a local of `event_loop_run`, moved into the thread beside the state. `is_running`, `thread`, `semaphore`, `pool`, `head`, `tail` are **gone**: `is_running` is only ever set `true` and read as the `while` condition (`event_loop.c:1652`, `:1717`), so the loop becomes `for event in receiver` |
| `g_workspace_context` | `yabai.c:35` | `static WORKSPACE_CONTEXT: OnceLock<Retained<WorkspaceContext>>`. Written once on the main thread at `yabai.c:295` (`workspace_event_handler_begin`, `workspace.m:1-15`), before the hand-off point of `DECISIONS.md` 12, and never written again. **The object is then messaged from both threads, directly — not through a main-queue block.** From the event-loop thread: `event_loop.c:104` and `:123` are straight-line statements of `EVENT_HANDLER(APPLICATION_LAUNCHED)` passing it to `workspace_application_observe_finished_launching` / `_activation_policy`, `:115` and `:134` are `[application removeObserver:g_workspace_context …]` inside `@try` blocks of the same handler, and `process_manager.c:262` (`workspace_application_destroy_running_ns_application`) is reached only through `process_destroy`'s single call site, `event_loop.c:344` in `EVENT_HANDLER(APPLICATION_TERMINATED)`. The only two `dispatch_after(…, dispatch_get_main_queue(), …)` blocks in that handler are `event_loop.c:93-96` and `:157-160`, and neither names `g_workspace_context`. From the main thread: `process_manager.c:191`, in the Carbon `kEventAppTerminated` arm, and `window_manager.c:2753`, inside `window_manager_begin`, which §1.4 step 3 runs on main. What makes the type `Send + Sync` is spelled out below the table |
| `g_mission_control_mode` | `yabai.c:37` | field `mission_control_mode: MissionControlMode` |
| `g_cv_host_clock_frequency` | `yabai.c:38` | `static CV_HOST_CLOCK_FREQUENCY: OnceLock<f64>` (`DECISIONS.md` 18) |
| `g_layer_normal_window_level` | `yabai.c:39` | `static LAYER_NORMAL_WINDOW_LEVEL: OnceLock<i32>` |
| `g_layer_below_window_level` | `yabai.c:40` | `static LAYER_BELOW_WINDOW_LEVEL: OnceLock<i32>` |
| `g_layer_above_window_level` | `yabai.c:41` | `static LAYER_ABOVE_WINDOW_LEVEL: OnceLock<i32>` |
| `g_event_bytes` | `yabai.c:42` | **gone.** A zeroed `[u8; 0x100]` local in each of the two functions that use it, `window_manager_make_key_window` (`window_manager.c:1280-1290`) and `window_manager_focus_window_without_raise` (`window_manager.c:1298-1317`). Both `memset` bytes `0..0xf8` before every use and never read `0xf8..0x100`, which stay zero in C and in Rust. One `DEVIATIONS.md` line |
| `g_sa_socket_file` | `yabai.c:44` | `static SA_SOCKET_FILE: OnceLock<String>`. Set exactly once per process mode: `configure_settings_and_acquire_lock` (`yabai.c:135`) in daemon mode, or `scripting_addition_set_socket_path` (`sa.m:158`) in the `--load-sa`-as-root mode, which `exit`s and never reaches the daemon path |
| `g_socket_file` | `yabai.c:45` | `static SOCKET_FILE: OnceLock<String>` |
| `g_config_file` | `yabai.c:46` | `static CONFIG_FILE: OnceLock<String>`, holding the `--config` value or the empty string (`yabai.c:253`). `exec_config_file` (`helpers.h:463`) writes back into the C buffer when it falls back to `get_config_file`; nothing reads the buffer after `yabai.c:348`, so the Rust `exec_config_file` takes a `String` by value and the write-back disappears. One `DEVIATIONS.md` line |
| `g_lock_file` | `yabai.c:47` | `static LOCK_FILE: OnceLock<String>` |
| `g_bs_port` | `yabai.c:49` | `static BOOTSTRAP_PORT: OnceLock<mach_port_t>` (`DECISIONS.md` 18) |
| `g_connection` | `yabai.c:50` | `static CONNECTION: OnceLock<i32>` |
| `g_verbose` | `yabai.c:51` | `static VERBOSE: AtomicBool` (`DECISIONS.md` 18). Written at `yabai.c:246` on main and from `config debug_output` on the event-loop thread, read everywhere |
| `g_pid` | `yabai.c:52` | `static PROCESS_ID: OnceLock<i32>` |

**Why `WORKSPACE_CONTEXT` may be a `static` at all.** A `static` must be `Sync`;
`OnceLock<T>` is `Sync` when `T: Send + Sync`; `Retained<T>` is `Send + Sync` when
`T: Send + Sync`. So `WorkspaceContext` needs both impls:

```rust
unsafe impl Send for WorkspaceContext {}
unsafe impl Sync for WorkspaceContext {}
```

The justification is that the object has nothing to race on. `@interface workspace_context :
NSObject` declares an empty ivar block (`workspace.h:21-23`) and the implementation adds none
(`workspace.m:153-303`): every piece of mutable state the observer touches lives on the `Process`
it receives as the KVO context, which is the `Arc<Process>` of `DECISIONS.md` 22 with its atomic
fields. Every message the five event-loop-thread sites send it is an
`addObserver:forKeyPath:options:context:` or a `removeObserver:forKeyPath:context:`, whose
bookkeeping lives on the observed `NSRunningApplication` rather than on the observer, and which
the C already issues from that thread today. Every callback — `observeValueForKeyPath:ofObject:change:context:`
(`workspace.m:209`) and the eight notification selectors (`:261-301`) — is delivered on the main
run loop, so the object is never re-entered concurrently with itself. The argument belongs in
`THREADS.md` §12 as a numbered invariant, not in a `SAFETY` comment (`DECISIONS.md` 38-39), and
whoever writes it must reconcile it with that section's invariant 14, which currently lists the
`workspace_context` object among the types that carry `PhantomData<*const ()>` so they cannot
travel between threads: the C sends at `event_loop.c:104`, `:115`, `:123`, `:134` and
`process_manager.c:262` mean it must travel.

### 1.3 Every file-scope static

The daemon is a unity build (`src/manifest.m:60-93`), so each of these is a program-wide
singleton.

| C static | site | becomes |
|---|---|---|
| `g_temp_storage` | `misc/ts.h:4-8` | **gone.** §7 |
| `CGSGetConnectionPortById` | `misc/extern.h:4` | `static CGS_GET_CONNECTION_PORT_BY_ID: OnceLock<Option<extern "C" fn(i32) -> mach_port_t>>` (`DECISIONS.md` 18, "the two runtime-resolved SkyLight function pointers"). The `None` case drives the branch at `window.c:956` |
| `SLSPerformAsynchronousBridgedWindowManagementOperation` | `misc/extern.h:5` | `static SLS_PERFORM_ASYNCHRONOUS_BRIDGED_WINDOW_MANAGEMENT_OPERATION: OnceLock<Option<extern "C" fn(*mut c_void) -> i64>>`. `None` drives `space_manager.c:667` and `:688` |
| `g_notify_init` | `misc/notify.h:4` | `static NOTIFY_INIT: AtomicBool`; main thread and event-loop thread both call `notify` |
| `g_notify_img` | `misc/notify.h:5` | `static NOTIFY_IMAGE: OnceLock<Retained<NSImage>>`; retained at `notify.h:23`, never released, deliberate |
| `g_profiler` | `misc/timer.h:16-22` | **gone** (`DECISIONS.md` 5) |
| `g_nsobject_autorelease`, `g_nsautoreleasepool_drain`, `g_nsautoreleasepool_release` | `misc/autorelease.h:3-5` | **gone** (`DECISIONS.md` 5; `autorelease.h` is commented out of `manifest.m:49`) |
| `g_message_loop` | `message.c:1-5` | `static MESSAGE_LOOP: OnceLock<MessageLoop>` holding the listening `UnixListener` and the accept `JoinHandle`. `is_running` is only set `true` and read as the accept loop's condition (`message.c:3005`, `:3041`), so it becomes the `for stream in listener.incoming()` shape and disappears |
| `g_mission_control_observer` | `mission_control.c:46-50` | `static MISSION_CONTROL_OBSERVER: Mutex<Option<MissionControlObserver>>`, owning both CF refs; `is_observing` becomes `Option::is_some`. **Locked from two threads.** The main thread creates it once at `yabai.c:316`; after that `EVENT_HANDLER(DOCK_DID_RESTART)` tears it down and rebuilds it on the event-loop thread, `event_loop.c:1554` then `:1555`, so `mission_control_observe` writes `ref` (`mission_control.c:77`), `observer_ref` (`:80`) and `is_observing` (`:86`) and calls `CFRunLoopAddSource(CFRunLoopGetMain(), …)` (`:87`) from the event-loop thread, and `mission_control_unobserve` clears `is_observing` (`:101`), invalidates the source (`:102`) and releases both refs (`:103-104`) from there too. A `static` must be `Sync`, and `Mutex<T>: Sync` needs `T: Send`, so `MissionControlObserver` carries `unsafe impl Send` — it owns an `AXUIElementRef` and an `AXObserverRef`, neither of which is `Send` automatically. The soundness argument is `THREADS.md` §12, whose invariant 14 is the one that applies: the struct stores no `CFRunLoopSourceRef`, it re-fetches the source with `AXObserverGetRunLoopSource` at each use, and `CFRunLoopAddSource` / `CFRunLoopSourceInvalidate` are documented thread-safe, which is what makes `:87` and `:102` sound off the main thread. `DECISIONS.md` 38: no `SAFETY` comment in the source |
| `kAXExposeShowAllWindows`, `kAXExposeShowFrontWindows`, `kAXExposeShowDesktop`, `kAXExposeExit` | `mission_control.c:52-55` | `OnceLock<CFStringOwned>` each, or one `OnceLock<[CFStringOwned; 4]>` |
| `kAXEnhancedUserInterface` | `misc/helpers.h:171` | `OnceLock<CFStringOwned>` |
| `kAXFullscreenAttribute` | `window.h:4` (not `static`, but the same shape) | `OnceLock<CFStringOwned>` |
| `ax_window_notification` | `window.h:24-29` | `static AX_WINDOW_NOTIFICATION: OnceLock<[CFStringOwned; 3]>`, written in *index* order |
| `ax_application_notification` | `application.h:57-66` | `static AX_APPLICATION_NOTIFICATION: OnceLock<[CFStringOwned; 7]>` |
| `_workspace_is_macos_version_{tahoe,sequoia,sonoma,ventura,monterey,bigsur}` | `workspace.h:13`, ×6 | one `static MACOS_VERSION: OnceLock<MacosVersion>`, set at `workspace.m:4-6`; the six `workspace_is_macos_*` functions read it |
| `ffm_value` | `event_loop.c:1561` | field `focus_follows_mouse_suspended_value: FfmMode` |
| `is_menu_open` | `event_loop.c:1562` | field `is_menu_open: i32` |
| `osax_base_dir` … `osax_bin_loader` (11 × `char[MAXLEN]`) | `sa.m:9-19` | `static OSAX_PATHS: OnceLock<OsaxPaths>` with eleven `String` fields, filled by `scripting_addition_set_path` (`sa.m:77-93`). The `if (osax_base_dir[0] == 0) scripting_addition_set_path();` guards (`sa.m:164` and friends) become `OSAX_PATHS.get_or_init(scripting_addition_set_path)` |
| `sa_plist`, `sa_bundle_plist` | `sa.m:21`, `sa.m:50` | `const &str` |
| `process_name_blacklist` | `process_manager.c:14-20` | `const PROCESS_NAME_BLACKLIST: [&str; 4]` |
| `hash_wm`, `compare_wm` | `window_manager.c:9-17` | `fn hash_window_manager_key(key: &u32) -> u64`; the compare vanishes into `K: PartialEq` (§6) |
| `hash_view`, `compare_view` | `space_manager.c:4-12` | `fn hash_view_key(key: &u64) -> u64`; compare vanishes |
| `hash_psn`, `compare_psn` | `process_manager.c:4-12` | `fn hash_process_serial_number(key: &ProcessSerialNumber) -> u64` returning `lowLongOfPSN`; compare vanishes into a hand-written `PartialEq` that compares both longs, matching `SameProcess` |
| every `*_str[]` / `*_val[]` lookup table | `application.h:26,46,57`, `window.h:17,24,73,80`, `view.h:28,35,101,115,130,138,177`, `display.h:23,30`, `display_manager.h:15,29`, `window_manager.h:33,47,61`, `event_signal.h:47`, `mouse_handler.h:83,93`, `helpers.h:35,173,175`, `mission_control.c:38`, `message.c:283,515,529,539` | `const` arrays, generated together with their enum by one `macro_rules!` per X-macro list (`DECISIONS.md` 31) |
| `static char process_name[...]` | `window.c:173` | a stack `[u8; PROC_PIDPATHINFO_MAXSIZE]` local |
| `static char process_name[...]` | `window_manager.c:935` | a **second, distinct** stack local; the identical name in C is two different buffers |
| `static uint64_t cpu_freq` | `misc/timer.h:38` | **gone** (`DECISIONS.md` 5) |

Non-`static` file-scope globals outside `yabai.c`:

| C global | site | becomes |
|---|---|---|
| `__pending_window_focus` | `event_loop.c:11` | `static PENDING_WINDOW_FOCUS: AtomicBool` |
| `__pending_gesture` | `event_loop.c:12` | `static PENDING_GESTURE: AtomicBool` |
| `__last_gesture_time` | `event_loop.c:13` | `static LAST_GESTURE_TIME: AtomicU64` |
| `__last_cmd_tab_time` | `event_loop.c:14` | `static LAST_CMD_TAB_TIME: AtomicU64` |

`MouseTapState`, the static half of `DECISIONS.md` 23:

```rust
pub struct MouseTapState {
    pub handle: AtomicPtr<__CFMachPort>,
    pub runloop_source: AtomicPtr<__CFRunLoopSource>,
    pub consume_mouse_click: AtomicBool,
    pub drag_detected: AtomicBool,
    pub consumed_event: AtomicPtr<CGEvent>,
    pub modifier: AtomicU8,
    pub action1: AtomicU8,
    pub action2: AtomicU8,
    pub drop_action: AtomicU8,
}

pub static MOUSE_TAP_STATE: MouseTapState = MouseTapState::new();
```

`handle` and `runloop_source` are written only by `mouse_handler_begin` (`mouse_handler.c:274-291`)
and `mouse_handler_end` (`mouse_handler.c:293-303`) — note that `mouse_handler_begin` is called
from the event-loop thread at `window_manager.c:226` as well as from main at `yabai.c:336`, which
is why `runloop_source` joins `handle` in the atomic static rather than staying a plain field.
`action1`, `action2` and `drop_action` hold `MouseMode` discriminants and are written from the
event-loop thread (`message.c` config) and read from the event-loop thread
(`event_loop.c:1137`, `:1139`) and from the tap on main (`mouse_handler.c:119`).

### 1.4 Construction and hand-over

`DECISIONS.md` 12 fixes the order. In `main`:

1. Everything from `yabai.c:129` to `yabai.c:290` runs on the main thread as in C, filling the
   `OnceLock` statics of §1.2 and §1.3.
2. `event_loop_begin` (`yabai.c:291`) creates the `mpsc` channel and installs the `Sender` in
   `EVENT_SENDER`. It does **not** spawn the thread. From here on every callback registered later
   can post, and posts queue.
3. `EventLoopOwnedState::default()` is created, then `workspace_event_handler_begin`,
   `process_manager_begin`, `display_manager_begin`, `mouse_handler_begin`,
   `mission_control_observe`, the `SLSRegisterConnectionNotifyProc` calls, `window_manager_init`,
   `space_manager_begin`, `window_manager_begin` and `update_window_notifications` run **on the
   main thread against `&mut state`**, in exactly the `yabai.c:295-344` order.
4. `std::thread::spawn(move || event_loop_run(state, receiver))` — the position C spawns at
   (`event_loop.c:1718`, reached from `yabai.c:291`) is deliberately abandoned. This removes the
   `ts`-arena and manager-table race of `sweeps/globals-and-ownership.md` §9.5 and settles its
   open question 1 in favour of "startup completes before the thread is spawned"; events posted
   during steps 2-3 are queued, not dropped, and are handled in order. One `DEVIATIONS.md` line.
5. `message_loop_begin` and `exec_config_file` follow, then `[NSApp run]`.

After step 4 no main-thread code may name `state`. That is enforced by construction: the value is
moved, and nothing else holds a reference to it.

Step 3's `mission_control_observe` (`yabai.c:316`) is the only start-up call that is later
repeated on the other thread: `EVENT_HANDLER(DOCK_DID_RESTART)` calls
`mission_control_unobserve` then `mission_control_observe` at `event_loop.c:1554-1555`, on the
event-loop thread, for the rest of the process's life. Neither function takes a manager — both
reach only the `MISSION_CONTROL_OBSERVER` static of §1.3 — so this does not breach the rule
above.

---

## 2. Deriving a function's manager parameters

`DECISIONS.md` 13 requires the parameter sets to be computed once, from the transitive call graph,
before any body is written. This section is that computation.

### 2.1 The rule

For every C function `f`, `Managers(f)` is the least fixed point of

```
Managers(f) = Direct(f) ∪ ⋃ { Managers(g) | g is called by f }
```

where `Direct(f)` is the union of:

* every manager whose field is named literally in `f`'s body, whether reached through a C
  parameter (`wm->focused_window_id`) or through a global (`g_window_manager.insert_feedback`);
* the owning manager of every handle that `f` **resolves**: a `SpaceId` or `(SpaceId, NodeId)`
  resolves through `SpaceManager::view`, a `WindowId` through `WindowManager::window`, a
  `ProcessId` through `WindowManager::application`, a `ProcessSerialNumber` through
  `PROCESS_TABLE` (a static, so it contributes nothing to `Managers`). A handle that `f` only
  passes along or hands to an FFI call contributes nothing;
* `MouseDragState` if `f` names any field of it.

The call graph has cycles (`space_manager_focus_space` → `space_manager_active_space` →
`window_manager_focused_window` → …). Compute strongly connected components first and give every
function in a component the union over the whole component. Do this once, for the whole daemon,
and write the resulting table down before the first body is translated; a later disagreement
between a caller and a callee means the fixed point was computed wrong, not that a signature
should be patched locally.

**Every manager parameter is `&mut`.** No exceptions, no read-only variants. A read-only variant
would make a mixed `&`/`&mut` call impossible at the sites where a read-only callee sits under a
mutating caller — `space_window_list_for_connection` (`space.c:45`, `:58`) reads
`WindowManager::window` and is reached from `window_manager.c:2631`, which already holds
`&mut WindowManager`. Uniform `&mut` reborrows cleanly; a mixed scheme does not.

Statics are never parameters. `CONNECTION`, `VERBOSE`, the three window levels, the paths, the
process table, `MOUSE_TAP_STATE` and the four `__pending_*` atomics are read where they are used.

### 2.2 The order

`DECISIONS.md` 13: "in the C parameter order, with any manager C reached through a global
**appended**". The manager block is never a prefix. `TRANSLATION_PLAN.md` §3.2 spells the same
rule out as the row format of `STATE_ACCESS.md`, which wave 0b generates and wave 1 copies
verbatim into the signatures; this section is that rule applied by hand, and the two must agree
literally, parameter for parameter.

Build the parameter list in three steps.

1. **Start from the C parameter list, in C order**, with each pointer parameter rewritten to its
   handle (§3). `struct window *` becomes a `WindowId`, `struct application *` a `ProcessId`,
   `struct window_node *` a `(SpaceId, NodeId)` pair, `struct view *` a `SpaceId`.
2. **A manager the C declares stays where C declared it.** The declared manager types are
   `struct process_manager *`, `struct display_manager *`, `struct window_manager *`,
   `struct space_manager *`, `struct mouse_state *`, `struct view *` and `struct event_loop *`.
   The first five become the matching `&mut` manager parameter in place, `struct mouse_state *`
   becoming `&mut MouseDragState` — the tap half of `DECISIONS.md` 23 is the `MOUSE_TAP_STATE`
   static of §1.3 and is never a parameter. `struct view *` is the one that changes shape: it
   contributes `&mut SpaceManager` at its own position, immediately followed by the `SpaceId`
   that replaces the pointer, so `view_serialize(FILE *rsp, struct view *view, uint64_t flags)`
   (`view.c:860`) becomes `(response, space_manager, space_id, flags)`. `struct event_loop *`
   disappears entirely — the sender is the `EVENT_SENDER` static of §1.2. A manager that step 2
   has already placed is never placed again by step 3.
   `TRANSLATION_PLAN.md` §3.2 step 4 omits `struct mouse_state *` from its list of six; it is
   included here because `DECISIONS.md` 13 says "in the C parameter order" and the C declares it
   in two signatures (`src/event_loop.c:36`, `src/mouse_handler.c:218`), where it keeps its
   position like any other declared manager.
3. **Append every remaining manager of `Managers(f)` after the whole C parameter list**, in the
   `src/yabai.c:27-35` order: `process_manager` (`:28`), `display_manager` (`:29`),
   `window_manager` (`:30`), `space_manager` (`:31`), `mouse_drag_state` (`:33`). Note
   `window_manager` **before** `space_manager`; that is the declaration order in the C and it is
   the only tie-break there is.

A Rust-only helper with no C original (`view_free_node` of §5.4,
`window_node_collect_subtree_post_order` of §4.4) has no C parameter list, so step 2 is vacuous:
its own handles come first and its managers are appended in the step 3 order.

Three mechanical consequences:

* An out-parameter `int *count` that C pairs with a returned buffer collapses into the returned
  `Vec`'s length and disappears from the signature. A count that is **not** derived from the same
  buffer stays.
* A `(pointer, count)` pair of arguments that C derives from one buffer collapses into one slice:
  `window_manager_animate_window_list(window_list, ts_buf_len(window_list))` becomes one
  `&[WindowCapture]`.
* `FILE *rsp` becomes `response: &mut dyn std::io::Write` and keeps its C position, which is
  first in every function that takes one.

### 2.3 Six worked examples

The first five reach every manager through a global, so for them step 2 is vacuous and the whole
manager block is appended. The sixth shows step 2 doing work.

**(1) `view.c` — `void insert_feedback_show(struct window_node *node)` (`view.c:8-103`)**

`Direct`: `WindowManager::insert_feedback_color` (`view.c:29-37`), `WindowManager::insert_feedback`
(`table_add`, `view.c:43`), and `SpaceManager` because the `node` parameter becomes
`(SpaceId, NodeId)` and is resolved. Callees: `window_level`/`window_sub_level`/
`sls_window_disable_shadow` are pure SkyLight; `update_window_notifications` (`event_loop.c:16`)
reads `WindowManager::window` and `WindowManager::insert_feedback`;
`workspace_is_macos_sequoia`/`_tahoe` read a static.

`Managers = { SpaceManager, WindowManager }`. The C declares neither, so both are appended, in the
`src/yabai.c:27-35` order — `window_manager` (`:30`) before `space_manager` (`:31`).

```rust
pub fn insert_feedback_show(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
)
```

`insert_feedback_destroy` (`view.c:105-119`) has the same set for the same reasons and the same
signature.

**(2) `window.c` — `void window_serialize(FILE *rsp, struct window *window, uint64_t flags)`
(`window.c:409`)**

`Direct`: `WindowManager::managed_window` (`window.c:444`), `WindowManager::focused_window_id`
(`window.c:623`), `MouseDragState::window_id` (`window.c:706`, `window == g_mouse_state.window`),
`WindowManager::window` because the `window` parameter becomes a `WindowId`, and `SpaceManager`
because `view_find_window_node` (`window.c:445`) resolves a view handle.

`Managers = { SpaceManager, WindowManager, MouseDragState }`, none of them declared, so all three
are appended after `flags` in the `src/yabai.c:27-35` order.

```rust
pub fn window_serialize(
    response: &mut dyn std::io::Write,
    window_id: WindowId,
    flags: u64,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
)
```

**(3) `window_manager.c` — `void window_manager_animate_window_list(struct window_capture *window_list, int window_count)` (`window_manager.c:707`)**

`Direct`: `WindowManager::window_animation_duration` (`window_manager.c:709`). Callees:
`window_manager_animate_window_list_async` (`:603`) reads `window_animation_duration` (`:610`),
`window_animation_easing` (`:611`), and locks and mutates `window_animations_table` (`:619-675`);
`window_manager_set_window_frame` (`:730`) resolves a `WindowId`. `WindowCapture::window` becomes
a `WindowId`, so both branches resolve through `WindowManager::window`.

`Managers = { WindowManager }`.

```rust
pub fn window_manager_animate_window_list(
    window_list: &[WindowCapture],
    window_manager: &mut WindowManager,
)
```

The `int window_count` argument is gone: every caller passes `ts_buf_len(window_list)` of the same
buffer (`view.c:378`, `mouse_handler.c:178`, `:215`, `window_manager.c:1868`, `:1879`, `:1920`,
`:2053`).

**(4) `space_manager.c` — `static enum space_op_error space_manager_swap_space_with_space_on_display(uint32_t a_did, uint64_t a_sid, uint32_t b_did, uint64_t b_sid)` (`space_manager.c:745`)**

No manager parameter at all in C. `Direct`: `WindowManager::window_animation_duration`
(`space_manager.c:750-752`, `:796-797`), `SpaceManager::view` (`:760-774`),
`SpaceManager::labels` (`:784-788`). Callees: `display_manager_display_is_animating` is pure
SkyLight; `space_window_list` reaches `WindowManager::window` (`space.c:45`, `:58`);
`view_update` → `display_bounds_constrained` (`display.c:123`) reads `DisplayManager::mode`,
`::top_padding`, `::bottom_padding`; `view_flush` → `window_node_flush` →
`window_manager_animate_window_list` → `WindowManager`; `space_manager_move_window_list_to_space`
→ `SpaceManager` and `WindowManager`.

`Managers = { DisplayManager, SpaceManager, WindowManager }`, all three appended after the four C
parameters, in the `src/yabai.c:27-35` order — `display_manager` (`:29`), `window_manager` (`:30`),
`space_manager` (`:31`).

```rust
fn space_manager_swap_space_with_space_on_display(
    a_display_id: DisplayId,
    a_space_id: SpaceId,
    b_display_id: DisplayId,
    b_space_id: SpaceId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
) -> SpaceOpError
```

**(5) `display_manager.c` — `void display_manager_focus_display(uint32_t did, uint64_t sid)` (`display_manager.c:462`)**

`Direct`: `WindowManager` twice through the global (`display_manager.c:465`, `:468`). Callees:
`window_manager_find_window_on_space_by_rank_filtering_window` and `window_manager_center_mouse`
touch `WindowManager` only; `window_manager_focus_window_with_raise` (`window_manager.c:1324`)
reads `WindowManager::focused_window_psn` and `::focused_window_id` through
`window_manager_focus_window_without_raise` (`:1293-1321`);
`display_manager_set_active_display_id` (`display_manager.c:454`) touches no manager;
`space_manager_active_space` (`space_manager.c:1056`) reaches `WindowManager` via
`window_manager_focused_window` and no other manager; `display_space_id` is pure SkyLight.

`Managers = { WindowManager }` — `DisplayManager` itself is **not** in the set, despite the
function's name and file.

```rust
pub fn display_manager_focus_display(
    display_id: DisplayId,
    space_id: SpaceId,
    window_manager: &mut WindowManager,
)
```

**(6) `window_manager.c` — `void window_manager_begin(struct space_manager *sm, struct window_manager *wm)` (`window_manager.c:2737`)**

The one shape the first five do not show. The C declares two managers, in the order
`space_manager`, `window_manager`. Step 2 keeps both exactly there, even though `src/yabai.c:30`
declares `g_window_manager` before `g_space_manager` — the append order of step 3 never re-sorts
what the C already named. `Direct` adds nothing else that is event-loop-owned — the process table
is the `PROCESS_TABLE` static (`window_manager.c:2740`, `DECISIONS.md` 22) and
`WORKSPACE_CONTEXT` is the static of §1.2 (`window_manager.c:2753`), and statics are never
parameters.

`Managers = { SpaceManager, WindowManager }`, both already placed by step 2, so step 3 appends
nothing.

```rust
pub fn window_manager_begin(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
)
```

---

## 3. Handles

`DECISIONS.md` 14: no long-lived pointers into owned collections. Every `struct view *`,
`struct window_node *`, `struct window *`, `struct application *` and `struct process *` that
outlives a single expression becomes a handle and is re-resolved.

### 3.1 The handle types

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct WindowId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ProcessId(pub i32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SpaceId(pub u64);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DisplayId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct NodeId(pub u32);

pub const ROOT_NODE_ID: NodeId = NodeId(0);
```

Rules that go with them:

* The inner field is `pub` and every FFI call passes it explicitly: `SLSGetWindowLevel(connection,
  window_id.0, &mut level)`. No `Deref`, no `From<u32>`, no implicit anything — a `DisplayId` must
  never silently become a `WindowId`.
* `WindowId(0)`, `SpaceId(0)` and `DisplayId(0)` keep their C meaning of "none"
  (`DECISIONS.md` 32). `Default` therefore gives the C zero-initialised value, which
  `view_create`'s `memset` and `window_manager_init` rely on.
* `NodeId` is an index into `View::nodes` (§5). `ROOT_NODE_ID` is always valid for a live `View`.
* A node handle that crosses a `View` boundary is the pair `(SpaceId, NodeId)`. Node identity
  comparisons in the C — `a_node != b_node` at `event_loop.c:1189`, `:1295`,
  `window_manager.c:1852`, `:1968` — compare the **pair**, because the two nodes can come from
  different views. Comparisons that the C performs inside one tree —
  `parent->left == node` (`view.c:215`), `parent->right == node` (`view.c:220`),
  `node->zoom == node->parent` (`view.c:289`), `child->zoom == parent` (`view.c:677`),
  `child->left->zoom == child` (`view.c:696`), `child->right->zoom == child` (`view.c:706`) —
  compare bare `NodeId` / `Option<NodeId>`.
* A `ProcessSerialNumber` is a `#[repr(C)] struct ProcessSerialNumber { high_long_of_psn: u32,
  low_long_of_psn: u32 }` with hand-written `PartialEq`/`Hash`: `PartialEq` compares both longs
  (matching `SameProcess`), `Hash` feeds only `low_long_of_psn` (matching `hash_psn`,
  `process_manager.c:4-7`).

### 3.2 Every C pointer field, and what it becomes

| C field | site | ownership in C | Rust |
|---|---|---|---|
| `g_window_manager.managed_window[wid] -> struct view *` | `window_manager.c:296` | borrowed | `Table<WindowId, SpaceId>`. The lookup at `window_manager_find_managed_window` returns a `SpaceId`; the caller resolves it through `SpaceManager::view`. A destroyed view leaves a stale `SpaceId` that resolves to a miss instead of a dangling read |
| `g_window_manager.insert_feedback[wid] -> struct window_node *` | `view.c:43`, removed `view.c:108`, read `event_loop.c:28`, `:948` | borrowed | `Table<WindowId, (SpaceId, NodeId)>`. The key stays `node->window_order[0]`, not the node |
| `struct scratchpad::window` | `window_manager.h:71`, set `window_manager.c:2499`, compared `:2511` | borrowed | `window_id: WindowId`. The comparison `wm->scratchpad_window[i].window == window` becomes `scratchpad.window_id == window_id` |
| `struct scratchpad::label` | `window_manager.h:70`, allocated `message.c:596`, freed `window_manager.c:2514` | **owned** | `label: String`, moved in by `window_manager_set_scratchpad_for_window`, dropped by the `swap_remove` at `window_manager.c:2515` |
| `struct window::scratchpad` | `window.h:105`, aliased `window_manager.c:2502`, cleared `:2512` | **aliases the scratchpad label** | `scratchpad: Option<String>`, a **clone** of the label taken at the same statement that pushes the `Scratchpad`. Not `Rc<str>`, not a borrow. `window->scratchpad` is only ever read for the `"scratchpad"` JSON property and for `string_equals`, so a clone is byte-identical, and it removes the requirement that `window_destroy` must not free it. `window_manager_remove_scratchpad_for_window` sets the field to `None` before the `swap_remove`, exactly as `window_manager.c:2512-2515` does |
| `struct window::application` | `window.h:88`, nulled `event_loop.c:281` | borrowed, nullable | `application: Option<ProcessId>`. `None` models `event_loop.c:281`; the null checks at `event_loop.c:611` and `event_signal.c:209` become `if let Some(..)` |
| `struct application::name` | `application.h:73`, aliased at `application.c:133` | **aliases `process->name`** | `name: Arc<str>`, cloned from `Process::name: Arc<str>` at `application.c:133`. This reproduces the alias exactly, deletes the "`application_destroy` must run before `process_destroy`" dependency (`event_loop.c:319` before `:344`), and lets `window_manager_rule_matches_window` (`window_manager.c:94`) read the name while `&mut WindowManager` is live. The two defensive `ts_string_copy`s at `event_signal.c:146` and `:209` stay as `to_string()`, because `DECISIONS.md` 17 requires queued signals to own their strings |
| `struct process::name` | `process_manager.h:10`, `cfstring_copy` at `process_manager.c:44`, freed `:263` | **owned** | `name: Arc<str>`. `cfstring_copy` returning NULL makes `process_create` return `None` early, rather than letting `string_equals(NULL, ..)` at `process_manager.c:54` dereference it. One `DEVIATIONS.md` line |
| `struct process::ns_application` | `process_manager.h:11` | owned, retained `workspace.m:30`, released `:65` | `AtomicPtr<c_void>` on the `Arc<Process>` (`DECISIONS.md` 22) |
| `g_mouse_state.window` | `mouse_handler.h:77`, set `event_loop.c:1128`, nulled `:305`, `:619`, `:1228`, `:1242` | borrowed | `MouseDragState::window_id: Option<WindowId>`. The four identity comparisons become `mouse_drag_state.window_id == Some(window_id)` |
| `g_mouse_state.feedback_node` | `mouse_handler.h:80`, set `event_loop.c:1330`, nulled `:1193`, `:1336` | borrowed, **never cleared when the node is freed** | `MouseDragState::feedback_node: Option<(SpaceId, NodeId)>`, and it **is** cleared when the node's slot is freed (§5.4). One `DEVIATIONS.md` line; this is `sweeps/globals-and-ownership.md` §9.1 |
| `struct window_node::parent` / `left` / `right` / `zoom` | `view.h:155-158` | `left`/`right` owned, `parent`/`zoom` borrowed | `Option<NodeId>` each (§5) |
| `struct view::root` | `view.h:204` | owned | gone; the root is always `ROOT_NODE_ID` (§5.2) |
| `struct window_capture::window` | `view.h:52` | borrowed | `window_id: WindowId` |
| `struct window_animation::window` | `view.h:69`, set `window_manager.c:621`, dereferenced `:694` | borrowed, never cleared if the window dies mid-animation | `window_id: WindowId`, re-resolved at `window_manager.c:694` |
| `g_window_manager.window_animations_table[wid] -> struct window_animation *` | `window_manager.c:673` | borrowed **interior pointer** into a `malloc`'d array freed on another thread | `Table<WindowId, (Arc<AnimationContext>, usize)>` behind the existing mutex (`DECISIONS.md` 24). The table holds a strong reference, so the context outlives the CVDisplayLink callback |
| `g_window_manager.applications_to_refresh[i]` | `window_manager.h:84`, pushed `window_manager.c:1716`, popped `:1733`, `:1739`, `event_loop.c:266` | borrowed | `Vec<ProcessId>` |
| `g_space_manager.view[sid] -> struct view *` | `space_manager.c:112` | owned by the table | `Table<SpaceId, View>` — the value **is** the `View`, not a pointer to it. `remove` yields the `View` by value so the two re-keying sites can move it (§4.5) |
| `g_window_manager.window[wid] -> struct window *` | `window.c:1095` | owned by the table | `Table<WindowId, Window>` |
| `g_window_manager.application[pid] -> struct application *` | `application.c:127` | owned by the table | `Table<ProcessId, Application>` |
| `g_process_manager.process[psn] -> struct process *` | `process_manager.c:61` | owned by the table | `Table<ProcessSerialNumber, Arc<Process>>` behind the static `Mutex` (`DECISIONS.md` 22) |
| `struct space_label::label`, `struct display_label::label` | `space_manager.h:7`, `display_manager.h:38` | owned, from `message.c:596` | `String` |
| `struct rule`'s five `char *` and four `regex_t` | `rule.h` | owned | `Option<String>` ×5 and four `Regex` values whose `Drop` calls `regfree` (`DECISIONS.md` 26). `rule_destroy` becomes that `Drop`; `buf_del` becomes `Vec::swap_remove`, which moves the `Regex` exactly as `sbuffer.h:19` memcpy-moves the `regex_t` |

---

## 4. The five borrow-checker recipes

These are the only shapes in the daemon where the handle discipline of §3 is not enough on its
own. Every aliasing site in the C reduces to one of them. Apply the recipe as written; do not
invent a sixth.

### 4.1 R1 — hold a record while calling something that needs its manager

**Rule.** Resolve a handle at the point of use. Copy the `Copy` fields you need into locals
**before** any call that takes the owning manager. Never hold the resolved reference across such
a call; re-resolve after it.

**C** — `window_did_receive_focus`, `event_loop.c:36-71`. `focused_window` and `window` are two
live `struct window *` out of `wm->window`, `&mut wm` is live, `ms` is live, and the body reaches
`g_window_manager` again at `event_loop.c:40` and `:57`.

```c
struct window *focused_window = window_manager_find_window(wm, wm->focused_window_id);
if (focused_window && focused_window != window && window_space(focused_window->id) == window_space(window->id)) {
    window_manager_set_window_opacity(wm, focused_window, g_window_manager.normal_window_opacity);
}
```

**Rust.**

```rust
fn window_did_receive_focus(
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
    window_id: WindowId,
    space_manager: &mut SpaceManager,
) {
    let focused_window_id = window_manager.focused_window_id;
    let normal_window_opacity = window_manager.normal_window_opacity;
    let active_window_opacity = window_manager.active_window_opacity;

    if window_manager.window.find(&focused_window_id).is_some()
        && focused_window_id != window_id
        && window_space(focused_window_id) == window_space(window_id)
    {
        window_manager_set_window_opacity(window_manager, focused_window_id, normal_window_opacity);
    }

    window_manager_set_window_opacity(window_manager, window_id, active_window_opacity);

    if window_manager.focused_window_id != window_id {
        if mouse_drag_state.ffm_window_id != window_id {
            window_manager_center_mouse(window_manager, window_id);
        }
        window_manager.last_window_id = window_manager.focused_window_id;
    }

    window_manager.focused_window_id = window_id;

    let Some(window) = window_manager.window.find(&window_id) else { return };
    let Some(process_id) = window.application else { return };
    let Some(application) = window_manager.application.find(&process_id) else { return };
    window_manager.focused_window_psn = application.psn;

    mouse_drag_state.ffm_window_id = WindowId(0);

    let Some(space_id) = window_manager.managed_window.find(&window_id).copied() else { return };
    let Some(view) = space_manager.view.find_mut(&space_id) else { return };
    let Some(node_id) = view_find_window_node(view, window_id) else { return };

    let node = view.node_mut(node_id);
    if node.window_count <= 1 { return; }

    for index in 0..node.window_count as usize {
        if node.window_order[index] != window_id { continue; }
        node.window_order.copy_within(0..index, 1);
        node.window_order[0] = window_id;
        break;
    }
}
```

The parameter order is §2.2 applied: `wm` and `ms` are declared by the C at positions one and
two (`event_loop.c:36`) and stay there, `window` becomes `window_id` in position three, and
`SpaceManager` is appended because it is reached only through a resolution the C did not need —
`window_manager_find_managed_window` (`window_manager.c:283-286`) returns the C's stored
`struct view *` straight out of `wm->managed_window`, whereas §3.2 stores a `SpaceId` there and
the view has to be looked up in `space_manager.view`.

Four points a translator must not slide past:

* `focused_window != window` in C is a **pointer** comparison that is only reached when
  `focused_window` is non-NULL. The `is_some()` test reproduces the NULL half and the id
  comparison reproduces the other half; `focused_window_id` may legitimately be `WindowId(0)`,
  which misses the table.
* `window->application->psn` (`event_loop.c:54`) dereferences a possibly-NULL
  `window->application`. Under §3.2 it is `Option<ProcessId>`, so the `None` arm returns early.
  One `DEVIATIONS.md` line.
* `view_find_window_node` returns NULL at `view.c:618` and `event_loop.c:60` immediately reads
  `node->window_count` through it. The `let … else { return }` is the safe equivalent. One
  `DEVIATIONS.md` line.
* The last `&mut View` is taken **after** the last call that needs `&mut WindowManager`, so the
  two `&mut` parameters never conflict. That ordering is the whole recipe.

Same recipe, same shape: `window.c:1037` (`CFEqual(value, window->application->ref)` inside a
function holding `&mut window`) — read `application.ref` into a `CFBorrowed` local first; and
`event_loop.c:797-798` (`g_mouse_state.window_frame.size = g_mouse_state.window->frame.size`) —
read the size into a local, then write the field.

### 4.2 R2 — iterate a table while the body mutates that table or another manager

**Rule.** Collect the keys first, in bucket order, then loop re-resolving. Never call anything
that takes the owning manager from inside an iterator over one of its tables.

**C** — `window_manager_set_window_opacity_enabled`, `window_manager.c:232-240`:

```c
table_for (struct window *window, wm->window, {
    if (window_manager_is_window_eligible(window)) {
        window_manager_set_opacity(wm, window, enabled ? window->opacity : 1.0f);
    }
})
```

**Rust.**

```rust
pub fn window_manager_set_window_opacity_enabled(window_manager: &mut WindowManager, enabled: bool) {
    window_manager.enable_window_opacity = enabled;

    for window_id in window_manager.window.keys_in_bucket_order() {
        let Some(window) = window_manager.window.find(&window_id) else { continue };
        if !window_manager_is_window_eligible(window) { continue; }
        let opacity = if enabled { window.opacity } else { 1.0 };
        window_manager_set_opacity(window_manager, window_id, opacity);
    }
}
```

`keys_in_bucket_order` (§6) reproduces `table_for`'s order exactly, which matters wherever a
bounded output buffer truncates the walk. The `find` inside the loop can miss, because the body
of some of these loops removes entries; in C that is a use-after-free waiting to happen, in Rust
it is a `continue`.

**The harder variant** is `space_manager_set_layout_for_all_spaces`, `space_manager.c:259-274`,
where the body calls `window_manager_validate_and_check_for_windows_on_space`
(`window_manager.c:2625`), whose first statement is `space_manager_find_view`
(`space_manager.c:102-113`) — a function that **inserts on demand** and can therefore rehash the
very table being walked. Same recipe, and it is the reason `space_manager_find_view` may never be
called while a `&mut View` from the same table is live:

```rust
let space_ids: Vec<SpaceId> = space_manager
    .view
    .iter()
    .filter(|(_, view)| !view.flags.contains(ViewFlags::LAYOUT))
    .map(|(space_id, _)| *space_id)
    .collect();

for space_id in space_ids {
    ...
}
```

Also R2: `rule.c:115-128` and `rule.c:161-172` (`table_for` over `wm->window` calling
`window_manager_apply_*`), `window_manager.c:764-772` and `:810-818` (`purify_window` /
`set_opacity` per window), `event_loop.c:18-33` (`update_window_notifications`, whose fixed
`uint32_t window_list[1024]` becomes a `Vec<WindowId>` — one `DEVIATIONS.md` line for the removed
overflow), and every `table_for` over `sm->view` at `space_manager.c:262`, `:279`, `:291`, `:303`,
`:315`, `:327`, `:339`, `:349`, `:1160`.

### 4.3 R3 — snapshot a list of handles, then mutate

**Rule.** Where C builds an array of pointers into a collection and then mutates that collection
while walking the array, build a `Vec` of **handles** instead and re-resolve each one in the
second loop. Do not deduplicate, do not sort, do not skip; the snapshot keeps C's order and C's
repetitions.

**C** — `EVENT_HANDLER(APPLICATION_TERMINATED)`, `event_loop.c:272-316`. Two snapshots at once:
`window_manager_find_application_windows` (`window_manager.c:1424-1436`) returns
`struct window **` pointing into `wm->window`, and the loop at `event_loop.c:313-315` removes and
frees entries of that very table; `view_list` (`event_loop.c:275`) is an array of
`struct view *` that the same loop can invalidate.

**Rust.**

```rust
let window_ids: Vec<WindowId> = window_manager_find_application_windows(window_manager, process_id);
let mut space_ids: Vec<SpaceId> = Vec::new();

for window_id in window_ids {
    ...
    if let Some(space_id) = window_manager.managed_window.find(&window_id).copied() {
        view_remove_window_node(space_manager, space_id, window_id, display_manager, window_manager, mouse_drag_state);
        window_manager_remove_managed_window(window_manager, window_id);
        if let Some(view) = space_manager.view.find_mut(&space_id) {
            view.flags.insert(ViewFlags::IS_DIRTY);
        }
        space_ids.push(space_id);
    }
    ...
}

for space_id in space_ids {
    if !space_is_visible(space_id) { continue; }
    let Some(view) = space_manager.view.find(&space_id) else { continue };
    if !view_is_dirty(view) { continue; }
    window_node_flush(space_manager, window_manager, space_id, ROOT_NODE_ID);
    if let Some(view) = space_manager.view.find_mut(&space_id) {
        view.flags.remove(ViewFlags::IS_DIRTY);
    }
}
```

`space_ids` may contain the same `SpaceId` several times — C pushes once per managed window
(`event_loop.c:301`), and the second flush is a no-op because `VIEW_IS_DIRTY` was cleared by the
first. Keep the duplicates; deduplicating changes nothing observable but is a change nobody asked
for.

Also R3: `event_loop.c:191-243`, `:439-485`, `:502-546` (the other three
`ts_alloc_list(struct view *, …)` snapshots), `window_manager.c:2540-2577` and `:2579-2623`
(`view_find_window_list` borrows the view, then the loop calls `view_remove_window_node` /
`view_add_window_node` on it — snapshot `Vec<WindowId>`), and `window_manager.c:2016-2029`
(`swap_window` iterates `a_node->window_list` while calling
`window_manager_remove_managed_window` and `space_manager_move_window_to_space` — copy the
`window_list[..window_count]` slice into a `Vec<WindowId>` first).

### 4.4 R4 — view code that re-enters the window manager during tree teardown

**Rule.** Split teardown into two phases. Phase 1 walks the arena and produces a post-order
`Vec<(NodeId, Vec<WindowId>)>`. Phase 2 replays that vector in order against
`&mut SpaceManager` + `&mut WindowManager` + `&mut MouseDragState`, unmanaging the windows,
destroying the feedback overlay and freeing the slot for each entry. The post-order is not an
implementation detail: it is the order in which the C issues `SLSOrderWindow`,
`SLSReleaseWindow` and `update_window_notifications`.

**C** — `window_node_destroy`, `view.c:335-346`:

```c
static void window_node_destroy(struct window_node *node)
{
    if (node->left)  window_node_destroy(node->left);
    if (node->right) window_node_destroy(node->right);

    for (int i = 0; i < node->window_count; ++i) {
        window_manager_remove_managed_window(&g_window_manager, node->window_list[i]);
    }

    insert_feedback_destroy(node);
    free(node);
}
```

**Rust.**

```rust
fn window_node_collect_subtree_post_order(view: &View, node_id: NodeId, out: &mut Vec<NodeId>) {
    let node = view.node(node_id);
    if let Some(left) = node.left { window_node_collect_subtree_post_order(view, left, out); }
    if let Some(right) = node.right { window_node_collect_subtree_post_order(view, right, out); }
    out.push(node_id);
}

pub fn window_node_destroy(
    space_id: SpaceId,
    node_id: NodeId,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let mut node_ids = Vec::new();
    let Some(view) = space_manager.view.find(&space_id) else { return };
    window_node_collect_subtree_post_order(view, node_id, &mut node_ids);

    for node_id in node_ids {
        let window_ids = {
            let view = space_manager.view.find(&space_id).unwrap();
            let node = view.node(node_id);
            node.window_list[..node.window_count as usize].to_vec()
        };

        for window_id in window_ids {
            window_manager_remove_managed_window(window_manager, window_id);
        }

        insert_feedback_destroy(space_id, node_id, window_manager, space_manager);
        view_free_node(space_id, node_id, space_manager, mouse_drag_state);
    }
}
```

`view_free_node` is the single funnel of §5.4: it sets the slot to `None`, pushes the id onto the
free list, and clears `mouse_drag_state.feedback_node` if it named that node. `insert_feedback_destroy`
must run before the free because it keys `WindowManager::insert_feedback` on
`node.window_order[0]`, which is only readable while the slot is still `Some`.

`view_clear` (`view.c:1017-1031`) is the same recipe applied to `root.left` and `root.right`,
followed by unmanaging the root's own windows, `insert_feedback_destroy(root)`, and
`self.nodes[0] = Some(WindowNode::default())` — the in-place reset of §5.2, not a free.
`view_destroy` (`view.c:1033-1042`) is the same recipe applied to `ROOT_NODE_ID`, after which the
`View` is dropped by the caller (`event_loop.c:988-989`); `View` has no `Drop` impl, exactly as
`view_destroy` does not `free(view)`.

`window_node_capture_windows` (`view.c:358-372`) is the read-only twin: it reaches
`window_manager_find_window` from inside a `&View` walk. Same split — collect
`Vec<(NodeId, WindowId)>` with `&View`, then build the `Vec<WindowCapture>` with
`&mut WindowManager`.

### 4.5 R5 — two views held across a remove and a re-add

**Rule.** `Table::remove` returns the value. Take both views **out** of the table, mutate them as
owned values, then put them back under the new keys. Never hold two `&mut View` from the same
table.

**C, the space swap** — `space_manager_swap_space_with_space_on_display`,
`space_manager.c:760-774`:

```c
struct view *a_view = table_find(&g_space_manager.view, &a_sid);
struct view *b_view = table_find(&g_space_manager.view, &b_sid);

table_remove(&g_space_manager.view, &a_sid);
table_remove(&g_space_manager.view, &b_sid);

a_view->sid = b_sid;
b_view->sid = a_sid;

CFStringRef tmp = a_view->uuid;
a_view->uuid    = b_view->uuid;
b_view->uuid    = tmp;

table_add(&g_space_manager.view, &a_sid, b_view);
table_add(&g_space_manager.view, &b_sid, a_view);
```

**Rust.**

```rust
let Some(mut a_view) = space_manager.view.remove(&a_space_id) else {
    return SpaceOpError::InvalidSrc;
};
let Some(mut b_view) = space_manager.view.remove(&b_space_id) else {
    space_manager.view.add(a_space_id, a_view);
    return SpaceOpError::InvalidDst;
};

a_view.space_id = b_space_id;
b_view.space_id = a_space_id;

std::mem::swap(&mut a_view.uuid, &mut b_view.uuid);

space_manager.view.add(a_space_id, b_view);
space_manager.view.add(b_space_id, a_view);
```

The two early returns replace the unconditional dereference of a possibly-NULL `table_find` at
`space_manager.c:766` (`sweeps/globals-and-ownership.md` §9.6). The first one must put `a_view`
back so the table is never left short of a view. Two `DEVIATIONS.md` lines. The
`window_animation_duration` save/restore around the whole function
(`space_manager.c:750-752`, `:796-797`) becomes a `RestoreWindowAnimationDurationOnDrop` guard, so
the early returns restore it.

**C, the display add** — `space_manager_handle_display_add`, `space_manager.c:1150-1197`. A VLA
snapshot of `struct view *` plus each view's **borrowed** `CFStringRef`, entries nulled in pairs
as they match, and the matched view removed and re-added under a new sid while the snapshot is
still live.

**Rust.**

```rust
let Some(space_list) = display_space_list(display_id) else { return };

let mut candidate_space_ids: Vec<Option<SpaceId>> = space_manager
    .view
    .keys_in_bucket_order()
    .into_iter()
    .map(Some)
    .collect();

for space_id in space_list {
    let Some(uuid) = SLSSpaceCopyName(connection(), space_id) else { continue };

    for candidate in candidate_space_ids.iter_mut() {
        let Some(candidate_space_id) = *candidate else { continue };

        let matches = match space_manager.view.find(&candidate_space_id) {
            Some(view) => match &view.uuid {
                Some(view_uuid) => CFEqual(view_uuid, &uuid),
                None => false,
            },
            None => false,
        };
        if !matches { continue; }

        *candidate = None;

        let mut view = space_manager.view.remove(&candidate_space_id).unwrap();

        if let Some(label) = space_manager_get_label_for_space(space_manager, view.space_id) {
            label.space_id = space_id;
        }

        view.space_id = space_id;
        view.uuid = Some(uuid.clone());

        space_manager.view.add(space_id, view);
        break;
    }
}

space_manager.current_space_id = space_manager_active_space(window_manager);
space_manager.last_space_id = space_manager.current_space_id;
```

Snapshot the **sids**, not the uuids: re-resolving for the comparison keeps the retain counts
identical to C, which holds the uuid borrowed (`space_manager.c:1158`, `:1172`) and releases the
old one at `:1182`. Nulling the entry before the remove (`space_manager.c:1178-1179`) is what
makes both the C borrow and the Rust re-resolution safe, and it is what stops a re-keyed view
matching a later space in the same pass. `CFRelease(view->uuid)` at `:1182` is the drop of the old
`Option<CFStringOwned>` when the field is overwritten; `CFRetain(uuid)` at `:1189` is the clone.

**C, the window swap** — `window_manager_swap_window`, `window_manager.c:1952-2054`, holds
`a_view`, `b_view`, `a_node` and `b_node` at once, where `a_view` and `b_view` may be the same
view. Handles dissolve it: carry `(a_space_id, a_node_id)` and `(b_space_id, b_node_id)`, and note
that `a_node == b_node` at `window_manager.c:1968` is a comparison of nodes that can come from
different views, so it becomes

```rust
if (a_space_id, a_node_id) == (b_space_id, b_node_id) {
```

Every other `struct view *`-pair site reduces the same way: `window_manager_warp_window`
(`window_manager.c:1854-1944`), `window_manager_stack_window` (`:1805-1828`), and the mouse drop
paths `event_loop.c:1172-1224` / `:1278-1338` with `mouse_handler.c:153-230`, whose
`mouse_drop_*` functions carry two `struct view *` and two `struct window *` today.

Each of them keeps its own C parameter order under §2.2 step 2, so they do **not** all get the
same manager block. `mouse_drop_action_stack` (`mouse_handler.c:133`) declares
`(struct window_manager *wm, struct view *src_view, struct window *src_window, struct view *dst_view, struct window *dst_window)`,
so the first `struct view *` brings `&mut SpaceManager` in at its own position and the second
brings only its handle:

```rust
pub fn mouse_drop_action_stack(
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    source_space_id: SpaceId,
    source_window_id: WindowId,
    destination_space_id: SpaceId,
    destination_window_id: WindowId,
)
```

`mouse_drop_no_target` (`mouse_handler.c:218`) declares `sm` **before** `wm`, and both stay there:
the `&mut SpaceManager` is already in place from the `sm` parameter, so `src_view` and `dst_view`
contribute only their `SpaceId`s, and nothing is appended.

```rust
pub fn mouse_drop_no_target(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
    source_space_id: SpaceId,
    destination_space_id: SpaceId,
    window_id: WindowId,
    node_space_id: SpaceId,
    node_id: NodeId,
)
```

`node_space_id` is redundant at the only call site — `event_loop.c:1221` passes `a_node`, which
came out of `src_view` — but the node handle is a pair everywhere (§3.1) and the rule does not
make exceptions for what a caller happens to pass.

`mouse_drop_action_swap` (`:153`), `mouse_drop_action_warp` (`:181`) and
`mouse_drop_try_adjust_bsp_grid` (`:232`) follow `mouse_drop_action_stack`'s shape, with their
`struct window_node *` parameters becoming `(SpaceId, NodeId)` pairs in place and their trailing
`split`, `child` and `info` parameters unchanged at the end of the C list.

---

## 5. The BSP index arena

`DECISIONS.md` 15. The tree is an index arena owned by its `View`.

### 5.1 Storage

```rust
pub struct View {
    pub uuid: Option<CFStringOwned>,
    pub space_id: SpaceId,
    pub nodes: Vec<Option<WindowNode>>,
    pub free_node_ids: Vec<NodeId>,
    pub insertion_point: WindowId,
    pub layout: ViewType,
    pub split_type: WindowNodeSplit,
    pub top_padding: i32,
    pub bottom_padding: i32,
    pub left_padding: i32,
    pub right_padding: i32,
    pub window_gap: i32,
    pub auto_balance: u32,
    pub flags: ViewFlags,
}

#[derive(Default)]
pub struct WindowNode {
    pub area: Area,
    pub parent: Option<NodeId>,
    pub left: Option<NodeId>,
    pub right: Option<NodeId>,
    pub zoom: Option<NodeId>,
    pub window_list: [WindowId; NODE_MAX_WINDOW_COUNT],
    pub window_order: [WindowId; NODE_MAX_WINDOW_COUNT],
    pub window_count: i32,
    pub ratio: f32,
    pub split: WindowNodeSplit,
    pub child: WindowNodeChild,
    pub insert_dir: i32,
    pub feedback_window: Option<FeedbackWindow>,
}
```

`window_count` stays `i32`, matching `view.h:162`; the array index sites cast with
`as usize` at each use (`DECISIONS.md` 30). `insert_dir` stays a bare `i32` holding the
`macros.h:26-31` constants, because `DIR_NORTH` is `360`, not `0`, and `0` is the live
"no insertion direction" sentinel (`DECISIONS.md` 32).

Four accessors, and only four:

```rust
impl View {
    pub fn node(&self, node_id: NodeId) -> &WindowNode;
    pub fn node_mut(&mut self, node_id: NodeId) -> &mut WindowNode;
    pub fn find_node(&self, node_id: NodeId) -> Option<&WindowNode>;
    pub fn find_node_mut(&mut self, node_id: NodeId) -> Option<&mut WindowNode>;
}
```

`node`/`node_mut` unwrap the slot and are used for every walk that arrived through a tree edge
(`parent`, `left`, `right`, `zoom`, `ROOT_NODE_ID`), where the slot is live by construction.
`find_node`/`find_node_mut` are used for exactly the two handles that escape the tree —
`WindowManager::insert_feedback` (read at `event_loop.c:28`, `:948`) and
`MouseDragState::feedback_node` (read at `event_loop.c:1191`, `:1295`) — where a stale handle is
possible and must become a miss.

### 5.2 `NodeId(0)` is always the root

`view_create` (`view.c:986-1015`) allocates the view and its root node together. In Rust it pushes
one `Some(WindowNode::default())`, so `ROOT_NODE_ID` is `NodeId(0)` for the life of the `View`,
and `nodes[0]` is `Some` for the life of the `View`.

Consequences, all mechanical:

* `struct view::root` disappears as a field. Every `view->root` becomes `ROOT_NODE_ID`.
* The two in-place resets keep the allocation and keep the id, which is what the C `memset`
  idiom buys:
  * `view_remove_window_node`'s root branch, `view.c:656`:
    `*view.node_mut(ROOT_NODE_ID) = WindowNode::default();`
  * `view_clear`, `view.c:1028`: the same statement.
* The `if (view->root)` guards at `view.c:1018` and `view.c:1034` are always true and disappear.
  One `DEVIATIONS.md` line.
* `NodeId(0)` is never pushed onto `free_node_ids`.

### 5.3 Allocation

```rust
impl View {
    pub fn allocate_node(&mut self) -> NodeId {
        match self.free_node_ids.pop() {
            Some(node_id) => {
                self.nodes[node_id.0 as usize] = Some(WindowNode::default());
                node_id
            }
            None => {
                self.nodes.push(Some(WindowNode::default()));
                NodeId((self.nodes.len() - 1) as u32)
            }
        }
    }
}
```

`window_node_split` (`view.c:277-322`) calls it twice, left then right, matching the C `malloc`
order. The free list is LIFO; nothing observable depends on which index a new node gets, because
every comparison in the tree is an id comparison (§5.6) and no id is ever printed or sent.

### 5.4 Freeing, and the two handles that escape

One funnel, and every free goes through it:

```rust
pub fn view_free_node(
    space_id: SpaceId,
    node_id: NodeId,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    if mouse_drag_state.feedback_node == Some((space_id, node_id)) {
        mouse_drag_state.feedback_node = None;
    }

    let Some(view) = space_manager.view.find_mut(&space_id) else { return };
    view.nodes[node_id.0 as usize] = None;
    view.free_node_ids.push(node_id);
}
```

Two rules make the free list safe to reuse:

1. **`WindowManager::insert_feedback` is always cleaned before the slot is freed.**
   `insert_feedback_destroy` keys the table on `node.window_order[0]`, so it must run while the
   slot is still `Some`. `view_remove_window_node` calls it for `node` at `view.c:717`; the port
   additionally re-points the entry when `child`'s overlay migrates to `parent` (§5.5), which the
   C forgets. One `DEVIATIONS.md` line — this is `files/view-and-tests.md` §2.6-4.
2. **`MouseDragState::feedback_node` is cleared here**, which the C never does anywhere
   (`sweeps/globals-and-ownership.md` §9.1). One `DEVIATIONS.md` line.

`FeedbackWindow` owns its SLS window and its `CGContextRef` and releases both in `Drop`, in the C
order of `view.c:114-116`: `SLSOrderWindow(connection, id, 0, 0)`, `CGContextRelease(context)`,
`SLSReleaseWindow(connection, id)`. `insert_feedback_destroy` therefore becomes: `table_remove`,
then `update_window_notifications` when the macOS version calls for it, then
`drop(node.feedback_window.take())` — the `take` is the `memset` at `view.c:117`. The migration at
`view.c:682` is `parent.feedback_window = child.feedback_window.take()`, a move, so no `Drop`
fires. This also closes the leak of `files/view-and-tests.md` §2.6 item 4 and
`sweeps/globals-and-ownership.md` §9.4: freeing a slot releases its overlay even when
`insert_dir` and `feedback_window.id` disagree. One `DEVIATIONS.md` line.

### 5.5 `view_remove_window_node`, case three, in full

`view.c:661-727`, the sibling lift. Every field read must be copied into a local before the first
write, because `parent`, `child`, `child.left` and `child.right` are four distinct slots of one
`Vec`. Cases one and two (`view.c:626-651` and `view.c:653-659`) are left out of the sketch; they
are covered by §5.2 and by the last bullet below.

```rust
pub fn view_remove_window_node(
    space_manager: &mut SpaceManager,
    space_id: SpaceId,
    window_id: WindowId,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    mouse_drag_state: &mut MouseDragState,
) -> Option<NodeId> {
    let window_zoom_persist = space_manager.window_zoom_persist;
    let auto_balance = {
        let view = space_manager.view.find(&space_id)?;
        view.auto_balance
    };

    let node_id = view_find_window_node(space_manager.view.find(&space_id)?, window_id)?;

    let (parent_id, child_id, child_window_list, child_window_order, child_window_count,
         child_parent, child_zoom, child_left, child_right, child_left_zoom, child_right_zoom,
         child_insert_dir, child_split, child_child, parent_parent) = {
        let view = space_manager.view.find(&space_id)?;
        let node = view.node(node_id);
        let parent_id = node.parent?;
        let parent = view.node(parent_id);
        let child_id = if parent.right == Some(node_id) { parent.left? } else { parent.right? };
        let child = view.node(child_id);
        (
            parent_id, child_id,
            child.window_list, child.window_order, child.window_count,
            child.parent, child.zoom, child.left, child.right,
            child.left.map(|left| view.node(left).zoom).flatten(),
            child.right.map(|right| view.node(right).zoom).flatten(),
            child.insert_dir, child.split, child.child,
            parent.parent,
        )
    };

    let parent_zoom = if !window_zoom_persist {
        None
    } else if child_zoom.is_none() {
        None
    } else if child_zoom == Some(parent_id) {
        parent_parent
    } else {
        Some(ROOT_NODE_ID)
    };

    {
        let view = space_manager.view.find_mut(&space_id)?;
        let parent = view.node_mut(parent_id);
        parent.window_list[..child_window_count as usize]
            .copy_from_slice(&child_window_list[..child_window_count as usize]);
        parent.window_order[..child_window_count as usize]
            .copy_from_slice(&child_window_order[..child_window_count as usize]);
        parent.window_count = child_window_count;
        parent.left = None;
        parent.right = None;
        parent.zoom = parent_zoom;
    }

    if child_insert_dir != 0 {
        {
            let view = space_manager.view.find_mut(&space_id)?;
            let feedback_window = view.node_mut(child_id).feedback_window.take();
            let parent = view.node_mut(parent_id);
            parent.feedback_window = feedback_window;
            parent.insert_dir = child_insert_dir;
            parent.split = child_split;
            parent.child = child_child;
        }
        let key = {
            let view = space_manager.view.find(&space_id)?;
            view.node(parent_id).window_order[0]
        };
        window_manager.insert_feedback.remove(&key);
        window_manager.insert_feedback.add(key, (space_id, parent_id));
        insert_feedback_show(space_id, parent_id, window_manager, space_manager);
    }

    if child_parent.is_some() && !(child_left.is_none() && child_right.is_none()) {
        let left_zoom = if !window_zoom_persist {
            None
        } else if child_left_zoom.is_none() {
            None
        } else if child_left_zoom == Some(child_id) {
            Some(parent_id)
        } else {
            Some(ROOT_NODE_ID)
        };
        let right_zoom = if !window_zoom_persist {
            None
        } else if child_right_zoom.is_none() {
            None
        } else if child_right_zoom == Some(child_id) {
            Some(parent_id)
        } else {
            Some(ROOT_NODE_ID)
        };

        {
            let view = space_manager.view.find_mut(&space_id)?;
            view.node_mut(parent_id).left = child_left;
            view.node_mut(parent_id).right = child_right;
            if let Some(left) = child_left {
                let left_node = view.node_mut(left);
                left_node.parent = Some(parent_id);
                left_node.zoom = left_zoom;
            }
            if let Some(right) = child_right {
                let right_node = view.node_mut(right);
                right_node.parent = Some(parent_id);
                right_node.zoom = right_zoom;
            }
            if !window_zoom_persist {
                window_node_clear_zoom(view, parent_id);
            }
        }
        window_node_update(space_manager, window_manager, space_id, parent_id);
    }

    insert_feedback_destroy(space_id, node_id, window_manager, space_manager);
    view_free_node(space_id, child_id, space_manager, mouse_drag_state);
    view_free_node(space_id, node_id, space_manager, mouse_drag_state);

    if auto_balance != SPLIT_NONE {
        window_node_balance(space_manager, space_id, ROOT_NODE_ID, auto_balance);
        view_update(display_manager, space_manager, window_manager, space_id);
        return Some(ROOT_NODE_ID);
    }

    Some(parent_id)
}
```

Five things that are easy to get wrong:

* The guard at `view.c:689` is `window_node_is_intermediate(child) && !window_node_is_leaf(child)`.
  `window_node_is_intermediate` tests `child->parent != NULL` (`view.c:205`) — `child_parent`, not
  `parent_parent` — and it is always `Some(parent_id)` here; `!window_node_is_leaf` tests
  `!(left == NULL && right == NULL)` (`view.c:210`). Write both halves, in that order, against
  `child_parent`, `child_left` and `child_right`. Inside the branch both grandchildren are
  `Some`, because the tree only ever creates children in pairs (`view.c:317-318`).
* `child`'s own `left`/`right` slots are **not** freed: they were re-parented onto `parent`. Only
  `child_id` and `node_id` are freed, matching `view.c:718-719`.
* `child`'s grandchildren are read (`child_left_zoom`, `child_right_zoom`) **before** any write,
  because `parent.zoom` is written first and `parent` may be `child.left.zoom`'s target.
* The returned `NodeId` identity is observable: `mouse_handler.c:181` compares
  `src_node_rm != src_node_add`, so returning `ROOT_NODE_ID` under auto-balance rather than
  `parent_id` (`view.c:721-725`) must be reproduced exactly.
* `assert(removed_entry)` / `assert(removed_order)` in case one (`view.c:642-643`) become
  `debug_assert!` (`DECISIONS.md` 33), and the two `memmove`s at `view.c:632`, `:637` become
  `copy_within`, not `copy_from_slice`.
* `DisplayManager` is in the parameter block only because the auto-balance branch calls
  `view_update` (`view.c:723`), which reaches `display_bounds_constrained` (`display.c:123`) and
  its three `g_display_manager` reads. `view_clear` (`view.c:1017-1031`) and case two
  (`view.c:653-659`) call `view_update` too and take it for the same reason;
  `window_node_destroy` and `view_destroy` do not call `view_update` and do not take it.

### 5.6 Pointer identity becomes `NodeId` identity

| C | site | Rust |
|---|---|---|
| `node->parent->left == node` | `view.c:215` | `view.node(parent_id).left == Some(node_id)` |
| `node->parent->right == node` | `view.c:220` | `view.node(parent_id).right == Some(node_id)` |
| `node->zoom == node->parent` | `view.c:289` | `node.zoom == node.parent` |
| `child->zoom == parent` | `view.c:677` | `child_zoom == Some(parent_id)` |
| `child->left->zoom == child` | `view.c:696` | `child_left_zoom == Some(child_id)` |
| `child->right->zoom == child` | `view.c:706` | `child_right_zoom == Some(child_id)` |
| `node == view->root` | `view.c:653` | `node_id == ROOT_NODE_ID` |
| `window->id == node->zoom->window_list[0]`-style zoom tests | `window.c:637`, `:645` | resolve `node.zoom` through `view.node(..)` and compare ids |
| `a_node == b_node` (possibly different views) | `window_manager.c:1852`, `:1968`, `event_loop.c:1189`, `:1295` | `(a_space_id, a_node_id) == (b_space_id, b_node_id)` |
| `src_node_rm != src_node_add` | `mouse_handler.c:181`, `:211` | `(space_id, removed) != (space_id, added)` |
| `node->left == NULL && node->right == NULL` | `view.c:210` | `node.left.is_none() && node.right.is_none()` — both, not either |

`view_find_min_depth_leaf_node` (`view.c:525-539`) is a breadth-first search over a
`struct window_node *list[256]` that writes `list[++j]` twice per iteration and runs off the end
at `i == 127`. It becomes a `VecDeque<NodeId>` seeded with the start node, popped from the front,
pushing `left` then `right`, returning `None` when it empties. One `DEVIATIONS.md` line for the
removed stack overflow; the C's 256-iteration cap disappears with it, so a tree deeper than the
C could search now returns a leaf instead of reading past the array.

`view_find_window_list` (`view.c:819-838`) becomes a `Vec<WindowId>` built with `push`; the
hand-rolled capacity doubling and the `ts_expand` that can under-grow for a 32-window leaf
(`files/view-and-tests.md` §2.6-2) both disappear. One `DEVIATIONS.md` line.

`view_stack_window_node` (`view.c:730-749`) gains the `NODE_MAX_WINDOW_COUNT` bound that
`mouse_handler.c:139` already applies at one of its three call sites: an early return when
`node.window_count + 1 >= NODE_MAX_WINDOW_COUNT`, rather than a slice panic. One
`DEVIATIONS.md` line.

---

## 6. The `Table` port

`DECISIONS.md` 16: `misc/hashtable.h` becomes a safe generic `Table` that keeps the C hash
functions, the bucket iteration order and the "add does not overwrite" rule.
`std::collections::HashMap` is not used for any of these.

### 6.1 Shape

```rust
pub struct Table<K, V> {
    count: i32,
    capacity: i32,
    max_load: f32,
    hash: fn(&K) -> u64,
    buckets: Vec<Vec<(K, V)>>,
}
```

A `Vec` per bucket, not a linked list. `table_get_bucket` (`hashtable.h:75-86`) walks the chain to
its end and `_table_add` appends there (`hashtable.h:125-129`), so the chain is append-at-tail;
`Vec::push` is that, and `Vec::remove` is `table_remove`'s splice (`hashtable.h:146`). **Never
`swap_remove` inside a bucket** — it would reorder the chain and therefore reorder iteration.

`K: PartialEq` replaces `table_compare_func` outright. All three C comparators are equality:
`compare_wm` on `uint32_t` (`window_manager.c:14`), `compare_view` on `uint64_t`
(`space_manager.c:9`), and `compare_psn` = `psn_equals`, which compares both longs
(`process_manager.c:9`). The hash stays an explicit `fn` field so `hash_psn`'s
"hash the low long only" (`process_manager.c:4-7`) survives unchanged.

### 6.2 API

```rust
impl<K: PartialEq, V> Table<K, V> {
    pub fn new(capacity: i32, hash: fn(&K) -> u64) -> Table<K, V>;

    pub fn add(&mut self, key: K, value: V);
    pub fn find(&self, key: &K) -> Option<&V>;
    pub fn find_mut(&mut self, key: &K) -> Option<&mut V>;
    pub fn remove(&mut self, key: &K) -> Option<V>;

    pub fn len(&self) -> i32;

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)>;
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)>;
    pub fn values(&self) -> impl Iterator<Item = &V>;
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V>;
}

impl<K: PartialEq + Clone, V> Table<K, V> {
    pub fn keys_in_bucket_order(&self) -> Vec<K>;
}
```

Behaviour of each, against the C:

* **`new`** mirrors `table_init` (`hashtable.h:46-55`): `count = 0`, the given `capacity`,
  `max_load = 0.75f`, `buckets` a `Vec` of `capacity` empty `Vec`s. Call sites, verbatim:
  `Table::new(150, hash_window_manager_key)` ×7 (`window_manager.c:2727-2733`),
  `Table::new(23, hash_view_key)` (`space_manager.c:1213`),
  `Table::new(125, hash_process_serial_number)` (`process_manager.c:240`).
* **`add` does not overwrite.** If the key is present, the existing value is left alone and the
  new one is dropped (`hashtable.h:120-123`). Every "replace" in the daemon is therefore an
  explicit `remove` then `add`, and the port keeps that pair everywhere the C has it:
  `space_manager.c:763-774`, `:1181-1190`, `window_manager.c:662`/`:673`,
  and the new one introduced in §5.5. `_table_add`'s "existing bucket whose value is NULL" branch
  (`hashtable.h:121-122`) is **dead** — no call site in the daemon ever stores a NULL value — so
  it is not translated (`DECISIONS.md` 5); the same goes for `table_for`'s
  `if (!bucket->value) continue;` (`hashtable.h:37`) and for `table_free` (`hashtable.h:57-73`),
  which has no caller.
* **The load factor and the rehash stay bit-identical.** After a successful insert,
  `let load = (1.0f32 * self.count as f32) / self.capacity as f32;` and rehash when
  `load > self.max_load` (`hashtable.h:132-134`). `table_rehash` (`hashtable.h:88-115`) doubles
  the capacity and walks the old buckets in index order, draining each chain head-to-tail and
  pushing into the new bucket — reproduce that walk exactly, because it is what determines the
  chain order afterwards, and the chain order is what `iter` yields.
* **`remove` returns the value.** This is what makes §4.5 work: the two re-keying sites
  (`space_manager.c:763-774`, `:1181-1190`) move the `View` out and put the same `View` back
  under a different key, with no clone and no `Box`.
* **`len`** is `self.count`, an `i32`, matching `table.count` as read at `window_manager.c:1427`
  and `space_manager.c:1157`.
* **`iter`/`values` yield bucket index ascending, then chain order** — `table_for`
  (`hashtable.h:34-41`) exactly. This is not incidental: `update_window_notifications`
  (`event_loop.c:18-33`) fills a bounded buffer from one of these walks, and
  `window_manager_find_application_windows` (`window_manager.c:1424-1436`) hands the order on to
  `event_loop.c:276`.
* **`keys_in_bucket_order`** is the R2 primitive (§4.2): the same order as `iter`, materialised
  into a `Vec<K>` so the loop body can take `&mut` on the table or on another manager. It is the
  only sanctioned way to write a `table_for` whose body mutates.

### 6.3 The eight instances

| C table | init | Rust type |
|---|---|---|
| `wm->application` | `window_manager.c:2727` | `Table<ProcessId, Application>` |
| `wm->window` | `window_manager.c:2728` | `Table<WindowId, Window>` |
| `wm->managed_window` | `window_manager.c:2729` | `Table<WindowId, SpaceId>` |
| `wm->window_lost_focused_event` | `window_manager.c:2730` | `Table<WindowId, ()>` — the `(void *)(intptr_t) 1` sentinel of `window_manager.c:1391` disappears; `find(..).is_some()` is `window_manager.c:1381` |
| `wm->application_lost_front_switched_event` | `window_manager.c:2731` | `Table<ProcessId, ()>` — same, `window_manager.c:1366`, `:1376` |
| `wm->window_animations_table` | `window_manager.c:2732` | `Table<WindowId, (Arc<AnimationContext>, usize)>`, behind the mutex of `DECISIONS.md` 24 |
| `wm->insert_feedback` | `window_manager.c:2733` | `Table<WindowId, (SpaceId, NodeId)>` |
| `sm->view` | `space_manager.c:1213` | `Table<SpaceId, View>` |
| `pm->process` | `process_manager.c:240` | `Table<ProcessSerialNumber, Arc<Process>>` inside the static `Mutex` of `DECISIONS.md` 22 |

The two sets keep `Table`, not `HashSet`: `DECISIONS.md` 16 takes tables out of the standard
collections wholesale, and a second collection type would be a second set of iteration-order
rules to reason about for no gain.

---

## 7. `ts`, `ts_buf`, `memory_pool` and `buf_*`

`DECISIONS.md` 17: all four are replaced, not ported. `misc/ts.h` and the `ts_buf_*` half of
`misc/sbuffer.h` are deleted outright; `misc/memory_pool.h` is deleted outright; the `buf_*` half
of `misc/sbuffer.h` becomes `Vec`.

### 7.1 `ts` — one shape at a time

The arena's whole contract is "freed at the end of the current event handler" — `ts_reset()` runs
exactly once per dequeued event, at `event_loop.c:1671`, after `event_signal_flush()`. `Vec` and
`String` give that for free by dropping at the end of the handler, so `ts_init` (`yabai.c:279`)
and `ts_reset` both disappear along with the 8 MiB `mmap`, the `PROT_NONE` guard page and the
`exit(EXIT_FAILURE)` on overflow (`ts.h:28-34`).

| shape | C sites | replacement |
|---|---|---|
| `ts_alloc_list(T, n)` filled, returned with an `int *count` out-parameter | `space.c:35` (`space_window_list_for_connection`), `display.c:237` (`display_space_list`), `display_manager.c:401` (`display_manager_active_display_list`), `window.c:99` (`window_space_list`), `view.c:824` (`view_find_window_list`), `window_manager.c:1427` (`window_manager_find_application_windows`), `window_manager.c:1553` (`window_manager_add_application_windows`) | return `Vec<T>` by value, or `Option<Vec<T>>` where the C's NULL return is load-bearing. The `int *count` out-parameter disappears (§2.2). `struct window **` becomes `Vec<WindowId>` |
| `ts_alloc_list(struct view *, n)` used as a snapshot of the space table | `event_loop.c:191`, `:275`, `:439`, `:502` | `Vec<SpaceId>` — recipe R3 (§4.3) |
| `ts_alloc_list(pthread_t, n)` | `window_manager.c:615` | gone; the proxy builders run under `std::thread::scope` (`DECISIONS.md` 24), so `threads` and `thread_count` disappear with the `pthread_join` loop at `window_manager.c:680` |
| `ts_resize(ptr, old, new)`, only ever shrinking | `space.c:71` | `window_list.truncate(window_count)`. The "must be the arena's newest allocation" assert (`ts.h:91`) has no analogue and is not needed |
| `ts_expand(ptr, old, increment)` | `view.c:828` | gone; `Vec::push`. The hand-rolled capacity doubling at `view.c:823-834` grows by one doubling per *node* while a leaf can contribute 32 ids, so it can write past the allocation. One `DEVIATIONS.md` line |
| `ts_alloc_unaligned` for a string copy | `helpers.h:283` (the JSON escaper), `helpers.h:364` (`ts_cfstring_copy`), `helpers.h:390` (`ts_string_copy`) | `String`. `ts_cfstring_copy` becomes `fn ts_cfstring_copy(string: CFStringRef) -> Option<String>` — `None` where `CFStringGetCString` fails and C returns NULL (`helpers.h:366-368`), which `view.c:878` already checks. `ts_string_copy(s)` becomes `s.to_string()` |
| the four `*_ts` accessors that return arena strings | `window.c:713` (`window_property_title_ts`), `:724` (`window_title_ts`), `:1001` (`window_role_ts`), `:1022` (`window_subrole_ts`); consumed at `rule.c:117-119`, `:163`, `window_manager.c:1442-1444`, `window.c:186`, `:486`, `:510`, `:518`, `event_signal.c:198`, `:225`, `display.c:40`, `view.c:877` | return `String` by value. Where C returns `ts_string_copy("")` (`window.c:717`, `:726`, `:1004`, `:1027`) return `String::new()` |
| `ts_alloc_unaligned` for the daemon message buffer | `event_loop.c:1623` | a `Vec<u8>` of `bytes_to_read`, read from the accepted socket. Tokens are `(start, length)` ranges over it (`DECISIONS.md` 27) and it becomes text with `from_utf8_lossy` where it is stored (`DECISIONS.md` 28) |
| `ts_alloc_unaligned(128)` × up to 8 per queued signal | `event_signal.c:129-130`, `:140-141`, `:164-167`, `:177-178`, `:191-192`, `:203-204`, `:218-219`, `:232-235`, `:245-246`, `:258-266`, `:284-287`, `:297-298`, `:310-318`, `:334-335` | `PendingSignal` owns `[Option<(String, String)>; 4]` for the name/value pairs and `Option<String>` for `app` and `title` (`DECISIONS.md` 17, "queued signals own their strings"). The two defensive `ts_string_copy`s at `event_signal.c:146` and `:209` stay as `to_string()`; so do the eleven sites that today store a borrowed `char *` |

### 7.2 The three arena-concatenation sites

`window_manager.c:73-88`, `process_manager.c:93-106` and `window_manager.c:1589-1602` each walk
every display, call `display_space_list` per display, keep only the **first** returned pointer and
sum the counts, treating the separate arena allocations as one flat array. Each carries the
`NOTE(asmvik)` comment that says so (`window_manager.c:78-82`, `process_manager.c:98-102`,
`window_manager.c:1594-1598`), and the first of those three differs from the other two in its last
word ("spaces" versus "windows") — `files/window-manager.md` §4 requires both texts to be carried
over verbatim, so they must not be deduplicated.

All three become the same thing:

```rust
let mut space_list: Vec<SpaceId> = Vec::new();

for display_id in display_list {
    let Some(list) = display_space_list(display_id) else { continue };
    space_list.extend(list);
}
```

with the comment kept at the `extend`, which is where `if (!space_list) space_list = list;` was
(`window_manager.c:84-85`, `process_manager.c:104-105`, `window_manager.c:1600-1601`). `space_count`
disappears into `space_list.len()`. The `continue` on a `None` list reproduces `if (!list) continue;`
exactly, including the case where every display returns nothing and the C leaves `space_list` NULL —
which `window_manager.c:1603` then tests (`return space_list ? … : NULL;`) and becomes
`if space_list.is_empty() { return None; }`.

### 7.3 `ts_buf_*`

`ts_buf__grow_f` (`sbuffer.h:51-69`) bumps the arena in place without asserting that the buffer is
still at the tail; it is the most fragile thing in the codebase and it is replaced by `Vec`
wholesale.

| C | sites | Rust |
|---|---|---|
| `ts_buf_push(*window_list, capture)` behind a `struct window_capture **` out-parameter | `view.c:358` (signature), `view.c:365` | `fn window_node_capture_windows(…, out: &mut Vec<WindowCapture>)`. The double pointer existed only because the macro reallocates |
| `ts_buf_len(window_list)` as the count argument of `window_manager_animate_window_list` | `view.c:378`, `mouse_handler.c:178`, `:215`, `window_manager.c:1868`, `:1879`, `:1920`, `:2053` | collapses into `&[WindowCapture]` (§2.2). `ts_buf_len(NULL) == 0` (`sbuffer.h:45`) is why `view.c:378` guards with `if (window_list)`; that guard becomes `if !window_list.is_empty()` |
| `ts_buf_push` / `ts_buf_len` / `ts_buf_del` on a `uint32_t *` | `window_manager.c:1652`, `:1657`, `:1676`, `:1694`, `:1714` | `Vec<WindowId>` with `push`, `len`, `swap_remove`. `window_manager.c:1694` discards `ts_buf_del`'s value and `break`s immediately, so `swap_remove(index); break;` is exact. `app_window_list_len` is deliberately re-read at the top of each `element_id` iteration (`window_manager.c:1676`) and must stay re-read, because the inner loop shrinks the vector |

### 7.4 `memory_pool`

| instance | C | Rust |
|---|---|---|
| `g_event_loop.pool` | `KILOBYTES(512)` at `event_loop.c:1707`, pushed once per post at `:1689` | gone. `std::sync::mpsc` carrying a typed `Event` whose variants own their payloads (`DECISIONS.md` 19). `memory_pool_push`'s wrap-to-the-start when the pool fills (`memory_pool.h:39-42`), which silently overwrites still-queued events after roughly 16 000 pending posts, is **not** reproduced. One `DEVIATIONS.md` line |
| `g_signal_storage` | `KILOBYTES(256)` at `yabai.c:323`, bumped with a bare `__sync_fetch_and_add` at `event_signal.c:107`, reset to zero in the parent after the fork at `event_signal.c:66` | field `signal_storage: Vec<PendingSignal>` on `EventLoopOwnedState`. `event_signal_push` (`event_signal.c:99-341`) becomes a `push`; `if (!g_signal_storage.used) return;` (`event_signal.c:62`) becomes `if signal_storage.is_empty() { return; }`; `g_signal_storage.used = 0` (`event_signal.c:66`) becomes `signal_storage.clear()` in the parent, at the same point |

`event_signal_flush` (`event_signal.c:60-97`) additionally has to move work across the fork:
`DECISIONS.md` 25 requires the regex filter verdict, the argv and the environment to be computed
in the **parent**, so `event_signal_filter` (`event_signal.c:20-58`) and its two `regexec` calls
run before the first `fork`, and the `setenv` + `execvp` pair at `event_signal.c:87-92` becomes a
prebuilt environment block handed to `execve`. The storage shape above is what this document
fixes; the fork mechanics belong to the signals and threads documents.

### 7.5 `buf_*`

`Vec<T>` everywhere, and **`buf_del` is `Vec::swap_remove`, never `Vec::remove`** — `sbuffer.h:19`
writes the last element over index `x` and decrements the length. Using `Vec::remove` would change
which rules are evaluated, which scratchpad is found first and which label a lookup returns.
`buf_free` is never called anywhere in the daemon, so the C leaks all of these at exit; the Rust
`Vec`s drop, which is not observable.

The six live buffers: `wm->rules`, `wm->applications_to_refresh`, `wm->scratchpad_window`,
`sm->labels`, `dm->labels`, `g_signal_event[t]`.

**The two sites that use `buf_del`'s value.** `buf_del(b, x)` is a comma expression whose value is
`buf__hdr(b)->len--`, i.e. the length **before** the decrement. Both users are the one-shot rule
pruner, duplicated:

* `event_loop.c:568-577`, inside `EVENT_HANDLER(WINDOW_CREATED)`
* `window_manager.c:1565-1574`, inside `window_manager_add_application_windows`

```c
int rule_len = buf_len(wm->rules);
for (int i = 0; i < rule_len; ++i) {
    if (rule_check_flag(&wm->rules[i], RULE_ONE_SHOT_REMOVE)) {
        rule_destroy(&wm->rules[i]);
        if (buf_del(wm->rules, i)) {
            --i;
            --rule_len;
        }
    }
}
```

The value is the pre-decrement length, which is at least 1 whenever index `i` was in range — so
the branch is **unconditionally taken** and the `if` is not a condition, it is C plumbing. In Rust:

```rust
let mut rule_length = window_manager.rules.len();
let mut index = 0;
while index < rule_length {
    if window_manager.rules[index].flags.contains(RuleFlag::OneShotRemove) {
        window_manager.rules.swap_remove(index);
        rule_length -= 1;
        continue;
    }
    index += 1;
}
```

`continue` without incrementing `index` is the `--i; ++i` round trip, so the element swapped into
the hole is re-examined. `rule_destroy` is the `Drop` of the value `swap_remove` returns; C
destroys before the swap and Rust drops after it, which is unobservable because `rule_destroy`
only frees memory and calls `regfree`.

**Every other `buf_del` site**, all of which discard the value:

| site | shape | Rust |
|---|---|---|
| `space_manager.c:175` (`space_manager_remove_label_for_space`) | `free(label); buf_del; return true;` | `space_manager.labels.swap_remove(index); return true;` — the `String` drops |
| `space_manager.c:191` (`space_manager_set_label_for_space`, replacing an existing label) | same | same |
| `display_manager.c:53`, `:69` | same, on `dm->labels` | same |
| `rule.c:186` (`rule_remove_by_index`), `rule.c:199` (`rule_remove_by_label`) | `rule_destroy; buf_del; return true;` | `window_manager.rules.swap_remove(index); return true;` |
| `event_signal.c:375` (`event_signal_remove_by_index`), `:391` (`event_signal_remove`) | `event_signal_destroy; buf_del; return true;` | `signal_event[signal_type].swap_remove(index); return true;` |
| `event_loop.c:266` (`EVENT_HANDLER(APPLICATION_TERMINATED)`) | `buf_del; break;` — the only removal with no index fixup, because it breaks | `applications_to_refresh.swap_remove(index); break;` |
| `window_manager.c:1733`, `:1739` | `buf_del(wm->applications_to_refresh, refresh_index); result = true;` — the swap invalidates the **caller's** loop index | `applications_to_refresh.swap_remove(refresh_index); result = true;` |
| `window_manager.c:2515` (`window_manager_remove_scratchpad_for_window`) | `window->scratchpad = NULL; free(label); buf_del;` | set `window.scratchpad = None` first (§3.2), then `scratchpad_window.swap_remove(index)`; the `String` drops |

`window_manager.c:1733`/`:1739`'s caller is `space_manager_refresh_application_windows`
(`space_manager.c:1133-1148`), which compensates for the swap by decrementing both the bound and
the index:

```rust
pub fn space_manager_refresh_application_windows(
    space_manager: &mut SpaceManager,
    window_manager: &mut WindowManager,
) -> bool {
    let mut refresh_count = window_manager.applications_to_refresh.len();
    if refresh_count == 0 { return false; }

    let window_count = window_manager.window.len();
    let mut index = 0;
    while index < refresh_count {
        let process_id = window_manager.applications_to_refresh[index];
        if window_manager_add_existing_application_windows(space_manager, window_manager, process_id, index as i32) {
            refresh_count -= 1;
            continue;
        }
        index += 1;
    }

    window_count != window_manager.window.len()
}
```

`window_count` is snapshotted before the loop (`space_manager.c:1137`) and compared after it
(`space_manager.c:1147`); keep both.

Finally, the `for (int i = 0; i < buf_len(x); ++i)` loops that re-evaluate `buf_len` every
iteration (`window_manager.c:30`, `:176`, `:200`, `:2433`, `:2510`, `space_manager.c:147`, `:159`,
`:171`, `:187`, `:784`, `rule.c:133`, `:147`, `:183`, `:196`, `display_manager.c:25`, `:37`, `:49`,
`:65`, `event_signal.c:372`, `:388`, `:439`) never mutate the buffer inside the body except at the
`return`/`break` sites listed above, so `for index in 0..vec.len()` with a recomputed bound is not
required; a plain `while index < vec.len()` is the safe uniform choice and matches the C literally.
