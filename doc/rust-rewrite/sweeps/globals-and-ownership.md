# Sweep: globals, object graph, allocators and lifetimes

Phase-1 cross-cutting sweep. No Rust is written here; this is the ownership contract that
phase 2 has to reproduce.

Everything below was read in full: `src/yabai.c`, `src/application.c`, `src/process_manager.c`,
`src/window.c`, `src/view.c`, `src/window_manager.c`, `src/space_manager.c`, `src/display.c`,
`src/display_manager.c`, `src/space.c`, `src/event_loop.c`, `src/event_signal.c`, `src/rule.c`,
`src/mouse_handler.c`, `src/mission_control.c`, `src/workspace.m`, `src/sa.m`, every header in
`src/misc/`, and the ownership-relevant parts of `src/message.c`.

The daemon is a unity build (`src/manifest.m:60-93`), so *every* `static` at file scope is a
program-wide singleton, not a per-object-file private. That distinction matters throughout.

---

## 1. Thread inventory (only what ownership depends on)

| thread | created at | owns / touches |
|---|---|---|
| **main run-loop** | the process main thread; `[NSApp run]` at `yabai.c:350` | AX observer callbacks (`application.c:6`, `mission_control.c:59`), the Carbon process handler (`process_manager.c:151`, installed `process_manager.c:251`), the CGEventTap callback (`mouse_handler.c:21`, installed `mouse_handler.c:278`), the CG display-reconfig callback (`display.c:6`), the SkyLight connection callback (`mission_control.c:7`), all `NSWorkspace`/KVO callbacks (`workspace.m:209-301`), and every `dispatch_after(… dispatch_get_main_queue() …)` block (`event_loop.c:93`, `:157`, `:1478`, `:1516`, `:1520`) |
| **event-loop thread** | `pthread_create` at `event_loop.c:1718`, body `event_loop_run` at `event_loop.c:1647` | **owns the entire object graph**: `g_window_manager`, `g_space_manager`, `g_display_manager`, all views/nodes/windows/applications, `g_mouse_state`'s drag fields, the ts arena, `g_signal_storage` |
| **message-accept thread** | `pthread_create` at `message.c:3042`, body `message.c:3003` | only `accept()` + `event_loop_post`. Touches nothing else |
| **CVDisplayLink thread** | started `window_manager.c:702`, body `window_manager_animate_window_list_thread_proc` at `window_manager.c:537` | `struct window_animation_context` (owns it, frees it at `window_manager.c:592-593`), and `g_window_manager.window_animations_table` **under `g_window_manager.window_animations_lock`** (`window_manager.c:577`, `:589`) |
| **proxy-build workers** | `pthread_create` at `window_manager.c:666`, body `window_manager.c:507` | one `struct window_animation` element each; joined at `window_manager.c:680` before anything else reads them |
| **forked children** | `fork()` at `event_signal.c:64` and `:83`, `helpers.h:477` | copy-on-write snapshot; `execvp` immediately. Never returns |

Two facts that the rest of this document leans on:

1. **The process table is main-thread confined.** `g_process_manager.process` is only ever
   `table_add`/`table_remove`/`table_find`/`table_for`-ed from the main thread
   (`process_manager.c:182`, `:190`, `:226`, `:240`, `:257`; `event_loop.c:94` and `:158` are
   inside `dispatch_get_main_queue()` blocks; `window_manager.c:2740` runs on main during
   startup). The event-loop thread only ever dereferences a `struct process *` that was handed
   to it through an event.
2. **The single-consumer FIFO event queue is load-bearing for memory safety, not just for
   ordering.** See §4.2.

---

## 2. Global inventory

### 2.1 Definitions in `src/yabai.c:27-52`

| global | type | defined | `extern`-ed in | written by | read by |
|---|---|---|---|---|---|
| `g_signal_event` | `struct signal *[SIGNAL_TYPE_COUNT]` (array of stretchy buffers) | `yabai.c:27` | `event_signal.c:1` | event thread (`event_signal.c:355`, `:375`, `:391`) | event thread + forked child (`event_signal.c:76`, `:80`) |
| `g_process_manager` | `struct process_manager` | `yabai.c:28` | `event_signal.c:3`, `event_loop.c:2`, `window_manager.c:5` | main (table), event thread (`front_pid`/`last_front_pid`/`switch_event_time` at `event_loop.c:382-384`) | both |
| `g_display_manager` | `struct display_manager` | `yabai.c:29` | `display_manager.c:1`, `event_signal.c:4`, `event_loop.c:3`, `view.c:2` | event thread | event thread |
| `g_window_manager` | `struct window_manager` | `yabai.c:30` | `display_manager.c:2`, `event_signal.c:6`, `event_loop.c:5`, `space_manager.c:1`, `message.c:10`, `window.c:1`, `rule.c:2`, `view.c:4` | event thread; **`window_animations_table` also by the CVDisplayLink thread** | event thread, CVDisplayLink thread, main thread during startup |
| `g_space_manager` | `struct space_manager` | `yabai.c:31` | `event_signal.c:5`, `event_loop.c:4`, `message.c:9`, `rule.c:1`, `view.c:3` | event thread (+ main at `yabai.c:341`) | event thread |
| `g_signal_storage` | `struct memory_pool` (256 KiB, `yabai.c:323`) | `yabai.c:32` | `event_signal.c:2` | event thread (`event_signal.c:107`, reset `:66`) | forked child (`event_signal.c:74`) |
| `g_mouse_state` | `struct mouse_state` | `yabai.c:33`, init `yabai.c:163` | `event_loop.c:6`, `message.c:11`, `window_manager.c:6` | **main thread** (`mouse_handler.c:37`, `:38`, `:52-54`, `:60`) **and** event thread (`event_loop.c:1128-1147`, `:1228-1230`, …) | both |
| `g_event_loop` | `struct event_loop` | `yabai.c:34` | every `.c` that posts | all producer threads (lock-free) | event thread |
| `g_workspace_context` | `void *` (a `workspace_context *`) | `yabai.c:35` | `process_manager.c:2`, `event_loop.c:9`, `window_manager.c:4` | main (`yabai.c:333`) | main + event thread |
| `g_mission_control_mode` | `enum mission_control_mode` | `yabai.c:37` | `mission_control.c:2`, `event_loop.c:7` | event thread | event thread |
| `g_cv_host_clock_frequency` | `double` | `yabai.c:38` | `window_manager.c:7` | main (`yabai.c:145`) | CVDisplayLink thread (`window_manager.c:545`) |
| `g_layer_normal_window_level` / `g_layer_below_window_level` / `g_layer_above_window_level` | `int` | `yabai.c:39-41` | `window.c:2-4`, `event_loop.c:10` | main (`yabai.c:146-148`) | event thread |
| `g_event_bytes` | `uint8_t *` (0x100, `yabai.c:143`) | `yabai.c:42` | `window_manager.c:2` | event thread only (`window_manager.c:1280-1290`, `:1298-1317`) | idem |
| `g_sa_socket_file` / `g_socket_file` / `g_config_file` / `g_lock_file` | `char[MAXLEN]` / `char[4096]` | `yabai.c:44-47` | `sa.m:7` | main at startup | event thread (`sa.m:432`) |
| `g_bs_port` | `mach_port_t` | `yabai.c:49` | `window_manager.c:1` | main (`yabai.c:164`) | event thread + CVDisplayLink thread (`window_manager.c:440`) |
| `g_connection` | `int` (SkyLight main connection id) | `yabai.c:50` | 8 files | main (`yabai.c:144`) | all threads |
| `g_verbose` | `bool` | `yabai.c:51` | `log.h:4`, `message.c:12` | main at startup, event thread (`config debug_output`) | all |
| `g_pid` | `pid_t` | `yabai.c:52` | — | main (`yabai.c:142`) | main |

**Rust plan for this block.** `g_connection`, `g_pid`, `g_bs_port`, the three layer levels,
`g_cv_host_clock_frequency` and the four path strings are write-once-at-startup, read-many →
`OnceLock<T>` (or a `Settings` struct in a `OnceLock`). `g_verbose` → `AtomicBool`.
`g_event_bytes` is a scratch buffer touched only by the event thread → a field on the
event-thread-owned state, not a global. The five managers plus `g_mission_control_mode` and the
event-thread half of `g_mouse_state` become fields of **one owned `Yabai` struct** that the
event-loop thread holds by value (§7.1).

### 2.2 File-scope statics (unity build ⇒ program-wide singletons)

| static | defined | notes |
|---|---|---|
| `g_temp_storage` | `misc/ts.h:4-8` | the ts arena. `{void *memory; uint64_t size; volatile uint64_t used;}`. §6.1 |
| `CGSGetConnectionPortById` | `misc/extern.h:4` | function pointer resolved at `yabai.c:148` via `macho_find_symbol`. May stay NULL → branch at `window.c:956` |
| `SLSPerformAsynchronousBridgedWindowManagementOperation` | `misc/extern.h:5` | resolved at `yabai.c:149`. NULL-check drives three different space-move strategies (`space_manager.c:667`, `:688`) |
| `g_notify_init`, `g_notify_img` | `misc/notify.h:4-5` | `g_notify_img` is a retained `NSImage *` (`notify.h:23`), never released — deliberate singleton |
| `g_profiler` | `misc/timer.h:16-22` | only exists under `PROFILE >= 1`; not in the default build |
| `g_nsobject_autorelease`, `g_nsautoreleasepool_drain`, `g_nsautoreleasepool_release` | `misc/autorelease.h:3-5` | **dead**: `autorelease.h` is commented out of `manifest.m:49` and the three hooks sit inside `#if 0` at `yabai.c:158-162`. Do not port |
| `g_message_loop` | `message.c:1-5` | `{int sockfd; bool is_running; pthread_t thread;}` |
| `g_mission_control_observer` | `mission_control.c:46-50` | `{AXUIElementRef ref; AXObserverRef observer_ref; bool is_observing;}`; owns both CF refs, released at `mission_control.c:103-104` |
| `kAXExposeShowAllWindows` / `ShowFrontWindows` / `ShowDesktop` / `Exit` | `mission_control.c:52-55` | `CFSTR` constants |
| `kAXEnhancedUserInterface` | `misc/helpers.h:171` | `CFSTR` constant |
| `_workspace_is_macos_version_{tahoe,sequoia,sonoma,ventura,monterey,bigsur}` | `workspace.h:13` (macro-expanded ×6) | six `static bool`, all written once at `workspace.m:4-6`, read everywhere. → one `OnceLock<MacosVersion>` in Rust |
| `ffm_value`, `is_menu_open` | `event_loop.c:1561-1562` | the menu-open FFM suspend latch; event thread only |
| `osax_base_dir` … `osax_bin_loader` (11 × `char[MAXLEN]`) | `sa.m:9-19` | filled by `scripting_addition_set_path()` (`sa.m:77-93`) |
| `sa_plist`, `sa_bundle_plist` | `sa.m:21`, `sa.m:50` | `char[]` literals |
| `process_name_blacklist` | `process_manager.c:14-20` | const table |
| every `*_str[]` / `*_val[]` lookup table | `application.h:26,46,57`, `window.h:17,24,73,80`, `view.h:28,35,101,115,130,138,177`, `display.h:23,30`, `display_manager.h:15,29`, `window_manager.h:33,47,61`, `event_signal.h:47`, `mouse_handler.h:83,93`, `helpers.h:35,173,175`, `mission_control.c:38`, `message.c:283,515,529,539` | pure const data → Rust `const` arrays / `match` |

### 2.3 Non-`static` file-scope globals that are not `g_`-prefixed

| global | defined | why it matters |
|---|---|---|
| `__pending_window_focus` | `event_loop.c:11` (`volatile bool`) | written by the AX observer on **main** (`application.c:11`) and by the event thread (`event_loop.c:422`, `:638`); read on the event thread (`event_loop.c:369`). All accesses use `__atomic_*` → `AtomicBool` |
| `__pending_gesture` | `event_loop.c:12` | written by the event tap on **main** (`mouse_handler.c:76`, `:78`), read on the event thread (`event_loop.c:1351`) → `AtomicBool` |
| `__last_gesture_time` | `event_loop.c:13` | main → event thread (`mouse_handler.c:79`, `event_loop.c:1352`) → `AtomicU64` |
| `__last_cmd_tab_time` | `event_loop.c:14` | SkyLight connection callback on **main** (`mission_control.c:24`) → event thread (`event_loop.c:362`) → `AtomicU64` |
| `kAXFullscreenAttribute` | `window.h:4` — `const CFStringRef`, **not** `static` | a `CFSTR` constant |

### 2.4 Function-local statics

| static | site | Rust plan |
|---|---|---|
| `static char process_name[PROC_PIDPATHINFO_MAXSIZE]` | `window.c:173` (inside `window_nonax_serialize`) | a stack `[u8; N]` or `String`; there is no reason for it to be static |
| `static char process_name[PROC_PIDPATHINFO_MAXSIZE]` | `window_manager.c:935` (inside `window_manager_window_connection_is_jankyborders`) | same. Note: a *second, distinct* buffer despite the identical name, because each is function-local |
| `static uint64_t cpu_freq` | `misc/timer.h:38` | x86-only, PROFILE-only. Skip |

---

## 3. The object graph

```
                                 ┌──────────────────────────┐
  main thread                    │      g_event_loop        │   (§6.2)
  ─────────────                  │  MPSC intrusive list in  │
   Carbon/AX/CG/tap  ──post──►   │  a 512 KiB ring pool     │  ──► event-loop thread
   callbacks                     └──────────────────────────┘

  ┌────────────────────┐  table<PSN -> Process*>
  │ g_process_manager  │──────────────► struct process (malloc, process_manager.c:61)
  │  (main-thread only)│                 ├─ name      : char*  (malloc, :44)   ◄──┐ ALIAS
  └────────────────────┘                 ├─ psn, pid, policy                      │
                                         ├─ terminated      : atomic bool          │
                                         └─ ns_application  : atomic id (retained) │
                                                                                   │
  ┌────────────────────┐  table<pid -> Application*>                               │
  │  g_window_manager  │──────────────► struct application (malloc, application.c:127)
  │                    │                 ├─ ref          : AXUIElementRef (owned) │
  │                    │                 ├─ observer_ref : AXObserverRef  (owned) │
  │                    │                 ├─ name         : char* ───────borrowed──┘
  │                    │                 └─ psn, pid, connection, flags
  │                    │
  │   table<wid -> Window*>  ──────────► struct window (malloc, window.c:1095)
  │                    │                 ├─ application : struct application*  (borrowed, nullable)
  │                    │                 ├─ ref         : AXUIElementRef (owned)
  │                    │                 ├─ id, id_ptr  : the liberation CAS  (§4)
  │                    │                 ├─ role/subrole/title : CFStringRef (owned)
  │                    │                 └─ scratchpad  : char* ──borrowed─┐
  │                    │                                                   │
  │   table<wid -> View*>  (managed_window) ─────borrowed────┐             │
  │   table<wid -> 1>      (window_lost_focused_event)       │             │
  │   table<pid -> 1>      (application_lost_front_switched) │             │
  │   table<wid -> WindowNode*> (insert_feedback) ──────┐    │             │
  │   table<wid -> WindowAnimation*> (+mutex)  ───────┐ │    │             │
  │   rules                  : buf<struct rule>      │ │    │             │
  │   applications_to_refresh: buf<Application*> ────┼─┼────┼─borrowed     │
  │   scratchpad_window      : buf<struct scratchpad>│ │    │             │
  │        └─ .label : char* (owned) ────────────────┼─┼────┼─────────────┘
  │        └─ .window: struct window* ───borrowed────┼─┼────┤
  │   system_element : AXUIElementRef (owned)        │ │    │
  └──────────────────┘                               │ │    │
                                                     │ │    │
  ┌────────────────────┐  table<sid -> View*>        │ │    │
  │  g_space_manager   │───────────► struct view (malloc, view.c:988)
  │   labels : buf<space_label{sid, char* owned}>    │ │    │
  └────────────────────┘             ├─ uuid : CFStringRef (owned, view.c:995)
                                     ├─ sid, layout, paddings, flags
                                     └─ root : struct window_node*  (owns the tree)
                                             │
                                             ▼
                        struct window_node (malloc, view.c:279/282/991)
                          ├─ parent / left / right : window_node*   (tree edges)
                          ├─ zoom                  : window_node*   (aliases parent or view->root)
                          ├─ window_list[32] / window_order[32] : uint32_t  (wids, NOT pointers)
                          ├─ area, ratio, split, child, insert_dir
                          └─ feedback_window {uint32_t id; CGContextRef ctx} (owned SLS window) ◄┘
                                     ▲                                                     ▲
                                     └───── g_mouse_state.feedback_node (borrowed) ────────┘

  ┌────────────────────┐
  │   g_mouse_state    │  window : struct window*  (borrowed, nulled on destroy)
  │                    │  feedback_node : struct window_node*  (borrowed, NOT nulled on free)
  └────────────────────┘

  ┌────────────────────┐
  │  g_signal_event[]  │  buf<struct signal> per signal type; each owns app/title/command/label
  └────────────────────┘  char* and two regex_t
```

### 3.1 Edge-by-edge ownership table

| edge | owner of the pointee | allocated | freed | what stops dangling use today |
|---|---|---|---|---|
| `g_process_manager.process[psn] -> struct process` | the table | `process_manager.c:61` | `process_destroy` at `event_loop.c:344` (event thread), after the main thread already did `table_remove` at `process_manager.c:190` | ownership *transfers* into the `APPLICATION_TERMINATED` event; FIFO ordering guarantees any earlier `APPLICATION_LAUNCHED`/`FRONT_SWITCHED` event carrying the same pointer is drained first (§4.2) |
| `process->name -> char*` | `struct process` | `cfstring_copy` at `process_manager.c:44` | `process_manager.c:263` | — |
| `application->name -> char*` | **nothing — it aliases `process->name`** (`application.c:133`) | — | — | `application_destroy` (`event_loop.c:319`) runs strictly before `process_destroy` (`event_loop.c:344`) in the same handler. `SIGNAL_APPLICATION_TERMINATED` / `SIGNAL_WINDOW_DESTROYED` defensively `ts_string_copy` the name (`event_signal.c:146`, `:209`) because the flush happens after the handler returns |
| `g_window_manager.application[pid] -> struct application` | the table | `application.c:127` | `application.c:143`, called from `event_loop.c:319` and the two observe-failure paths `event_loop.c:152` / `window_manager.c:2749` | `window_manager_remove_application` at `event_loop.c:262` precedes the free |
| `window->application -> struct application` | borrowed | — | — | every window of an application is destroyed in the same `APPLICATION_TERMINATED` handler before `application_destroy`; windows that lose the CAS get `window->application = NULL` (`event_loop.c:281`) and all later readers null-check (`event_loop.c:611`, `event_signal.c:209`) |
| `g_window_manager.window[wid] -> struct window` | the table | `window.c:1095` | `window.c:1143` via `window_destroy`, from `event_loop.c:315` (app terminated), `event_loop.c:629` (window destroyed), `window_manager.c:1450`/`:1462` (rejected at creation) | the **`id_ptr` CAS** (§4) plus `window_manager_remove_window` immediately before the free |
| `window->{role,subrole,title}` `CFStringRef` | `struct window` | `window.c:1103-1105`, refreshed at `event_loop.c:772-776`, `:853-857`, `:897-901`, `:937-939` | `window.c:1139-1141` | old value `CFRelease`d before the refresh |
| `window->ref` `AXUIElementRef` | `struct window` | `CFRetain`ed by the caller: `application.c:9`, `window_manager.c:1561`, `:1639`; or handed over un-retained from `_AXUIElementCreateWithRemoteToken` (`window_manager.c:1680`, transferred at `:1701`) | `window.c:1142` | the `WINDOW_CREATED` handler `CFRelease`s `context` on every early-out (`event_loop.c:554,557,560,563`) |
| `window->scratchpad -> char*` | **aliases `wm->scratchpad_window[i].label`** (`window_manager.c:2502`) | `message.c:596` (`malloc`) | `window_manager.c:2514`, after `window->scratchpad = NULL` at `:2512` | `window_manager_remove_scratchpad_for_window` is called from both destroy paths (`event_loop.c:312`, `:626`). Note `window_destroy` deliberately does **not** free it |
| `wm->scratchpad_window[i].window -> struct window` | borrowed | — | — | removed from the buf when the window dies (`event_loop.c:312`, `:626`) |
| `wm->applications_to_refresh[i] -> struct application` | borrowed | pushed `window_manager.c:1716` | popped `window_manager.c:1733`/`:1739`, or `event_loop.c:266` on app termination | — |
| `g_space_manager.view[sid] -> struct view` | the table | `view.c:988` via `view_create`, called at `space_manager.c:107` and `:1225` | **only** `event_loop.c:988-989` (`view_destroy` then `free`) on `SLS_SPACE_DESTROYED`. Never freed anywhere else | `view_destroy -> window_node_destroy -> window_manager_remove_managed_window` (`view.c:341`) evicts every `managed_window` entry that pointed at this view before the `free` |
| `view->uuid` `CFStringRef` | `struct view` | `SLSSpaceCopyName` at `view.c:995`, re-taken at `space_manager.c:1188` | `view.c:1040`; old one released at `space_manager.c:1182`; **swapped, not released**, at `space_manager.c:769-771` | — |
| `view->root -> struct window_node` | `struct view` | `view.c:991` | `view.c:1036` (`window_node_destroy`). Reset in place (`memset`) rather than freed by `view_clear` (`view.c:1028`) and by the `node == view->root` branch of `view_remove_window_node` (`view.c:656`) | — |
| `node->{left,right}` | the parent node | `view.c:279`, `:282` | recursively `view.c:345`; spliced-out pair freed at `view.c:718-719` | — |
| `node->parent` | back-edge, non-owning | set `view.c:313-314`, re-pointed `view.c:691`, `:701` | — | the tree is only ever mutated on the event thread |
| `node->zoom` | non-owning; always `node->parent`, `view->root`, `node->parent->parent`, or NULL | `view.c:285-291`, `:673-679`, `:692-708`, `window_manager.c:2346`, `:2375` | — | rewritten on every structural change; cleared wholesale by `window_node_clear_zoom` (`view.c:348`) when `window_zoom_persist` is off |
| `node->feedback_window.{id,context}` | the node; an SLS window + CGContext | `view.c:18`, `:26` | `view.c:115-116` | `insert_feedback_destroy` is called from `window_node_destroy` (`view.c:344`), `view_clear` (`view.c:1027`), `view_remove_window_node` (`view.c:655`, `:717`), `view_add_window_node_with_insertion_point` (`view.c:776`), `window_manager_set_window_insertion` (`window_manager.c:1762`, `:1770`), and the mouse handlers (`event_loop.c:1192`, `:1298`, `:1335`). **`view.c:718` frees `child` without having destroyed its feedback window** unless `child->insert_dir` was set — see §9.4 |
| `g_window_manager.insert_feedback[wid] -> struct window_node*` | borrowed, keyed by `node->window_order[0]` | `view.c:43` | `view.c:108` | the node's own `insert_feedback_destroy` removes the entry. Read at `event_loop.c:28` and `event_loop.c:948` |
| `g_window_manager.managed_window[wid] -> struct view*` | borrowed | `window_manager.c:296` | `window_manager.c:290`, and en-masse from `view.c:341` / `view.c:1024` during teardown | removal ordering (see the `view` row) |
| `g_mouse_state.window -> struct window*` | borrowed | `event_loop.c:1128` | nulled at `event_loop.c:305`, `:619`, `:1228`, `:1242` | the two destroy handlers explicitly compare and null it |
| `g_mouse_state.feedback_node -> struct window_node*` | borrowed | `event_loop.c:1330` | nulled at `event_loop.c:1193`, `:1336` | **nothing.** No destroy path clears it — see §9.1 |
| `g_window_manager.window_animations_table[wid] -> struct window_animation*` | borrowed; it is an **interior pointer** into `context->animation_list` | `window_manager.c:673` | `window_manager.c:584` (CVDisplayLink thread) and `:662` (event thread) | `window_animations_lock` (`window_manager.h:84`) held at `:577-589` and `:619-675`. The array itself is freed at `window_manager.c:592` while the lock is *not* held — safe only because every entry was removed under the lock at `:584` |
| `animation->window -> struct window*` | borrowed | `window_manager.c:621` | — | **nothing clears it if the window dies mid-animation.** Benign today because the CVDisplayLink callback only uses `.wid` and `.proxy`; `.window` is dereferenced solely on the event thread at `window_manager.c:694`, before the link starts |
| `window_animation_context` | the CVDisplayLink callback | `window_manager.c:605`, `:609` | `window_manager.c:592-593` | — |
| `wm->rules` | `buf<struct rule>` **by value** | pushed `rule.c:178` | `rule_destroy` + `buf_del` at `rule.c:186`, `:198`, `window_manager.c:1568-1569`, `event_loop.c:571-572` | each rule owns 5 `char*` and 4 `regex_t`; `buf_del` swap-moves the struct (`sbuffer.h:19`), memcpy-moving `regex_t` |
| `g_signal_event[t]` | `buf<struct signal>` **by value** | pushed `event_signal.c:355` | `event_signal_destroy` + `buf_del` at `:374-375`, `:390-391` | same shape as rules |
| `sm->labels`, `dm->labels` | `buf<space_label>` / `buf<display_label>`, each owning a `char*` | `space_manager.c:196`, `display_manager.c:73` | `space_manager.c:174`/`:190`, `display_manager.c:52`/`:68` | the `char*` comes straight from `message.c:596` `malloc`, ownership transferred by the setter |
| `process->ns_application` | `struct process`; a retained `NSRunningApplication` | `workspace.m:30` (`retain`) | `workspace.m:65` (`release`) inside `process_destroy` | KVO observers are removed first (`workspace.m:56-62`, and eagerly at `process_manager.c:191`) |

---

## 4. The two invariants that make the C safe

### 4.1 The `id_ptr` liberation CAS

`struct window` carries `uint32_t *volatile id_ptr` (`window.h:92`), initialised to `&window->id`
at `window.c:1101`.

* **Claim** (exactly one winner): `application.c:36`
  ```c
  if (!__sync_bool_compare_and_swap(&window->id_ptr, &window->id, NULL)) return;
  event_loop_post(&g_event_loop, WINDOW_DESTROYED, window, 0);
  ```
  The AX destroyed-notification, which fires on the **main thread**, claims the window and hands
  ownership to the queued `WINDOW_DESTROYED` event. The comment at `application.c:30-34` spells
  out the intent: events already queued for this window are thereby flagged as ignorable, and the
  allocation stays valid until the destroy event is handled.
* **Second claimer**: `event_loop.c:280`, inside `APPLICATION_TERMINATED`. If the CAS fails the
  window has already been claimed by AX, so the handler only does `window->application = NULL`
  and skips the free.
* **Peek** (a no-op CAS used purely as a liveness test): `event_loop.c:647`, `:681`, `:731`,
  `:833`, `:877`, `:930`, `:960`, `:1159`, `:1240`.

The protocol is a one-shot "liberation" flag, not a refcount. Rust equivalent: replace
`uint32_t *volatile id_ptr` with `AtomicU32 live_id` (0 = claimed), claim = `swap(0) != 0`,
peek = `load(Relaxed) != 0`. Identical semantics, no pointer-typed atomic, no `&window->id`
self-reference to keep stable.

### 4.2 The FIFO event queue is a memory-safety device

Three raw pointers travel through the queue as `void *context`:

| event | payload | who owns it while queued |
|---|---|---|
| `APPLICATION_LAUNCHED` / `APPLICATION_FRONT_SWITCHED` | `struct process *` | still the main-thread table — **borrowed** |
| `APPLICATION_TERMINATED` | `struct process *` | the event — ownership **transferred** (`process_manager.c:190` removed it from the table first) |
| `WINDOW_DESTROYED` | `struct window *` | the event — ownership **transferred** by the CAS |
| `WINDOW_CREATED` | `CFRetain`ed `AXUIElementRef` (`application.c:9`) | the event — one retain, released on every path |
| `MOUSE_*` | `CFRetain`ed `CGEventRef` (`mouse_handler.c:34,44,61,67`) | the event — released at `event_loop.c:1151`, `:1232`, `:1244`, `:1342`, `:1449` |
| everything else | an integer stuffed into a pointer (`(void *)(intptr_t) wid`, `sid`, `did`, `pid`, mode) | nothing |

A borrowed `struct process *` in a queued `APPLICATION_LAUNCHED` stays valid *because* the
matching `APPLICATION_TERMINATED` (which frees it) is necessarily behind it in the same FIFO.
The two `dispatch_after` re-posts at `event_loop.c:93-96` and `:157-160` deliberately capture
the **PSN by value** (`__block ProcessSerialNumber psn = process->psn`) and re-look-up
(`process_manager_find_process`) instead of capturing the pointer — because a main-queue block
*can* run after the termination has been processed. A Rust port that captures the process itself
in that closure reintroduces a use-after-free.

The same reasoning covers `workspace.m:212-213` and `:237-238`, where the KVO callback reads
`process->terminated` before posting; the observer is removed at `process_manager.c:191` *before*
the terminated event is posted, so no callback can fire after the free.

---

## 5. The BSP tree, precisely

`struct window_node` is `view.h:152-167`. Leaves carry up to `NODE_MAX_WINDOW_COUNT = 32`
window **ids** (not pointers) in two parallel arrays: `window_list` (stack order, left to right)
and `window_order` (MRU order, `window_order[0]` is the visible one).

Structural mutations, all on the event thread:

| operation | site | what it does to pointers |
|---|---|---|
| split | `window_node_split`, `view.c:277-322` | allocates two children, copies the leaf's window arrays into one of them, re-points `node->{left,right}`, recomputes `zoom` |
| remove | `view_remove_window_node`, `view.c:621-728` | the hard one. Lifts the sibling's contents into the parent, re-parents the grandchildren (`view.c:691`, `:701`), re-derives three `zoom` pointers (`:673-679`, `:692-698`, `:702-708`), migrates the feedback window (`:682-686`), then `free(child); free(node);` (`:718-719`) |
| root special case | `view.c:653-659` | `memset`s the root node in place instead of freeing it |
| stack push | `view_stack_window_node`, `view.c:730-749` | `memmove` inside the two fixed arrays |
| swap contents | `window_node_swap_window_list`, `view.c:399-419` | swaps the arrays between two nodes in possibly different views, clears both `zoom`s |
| rotate | `view.c:469-492` | swaps `left`/`right` and flips `split`, recursively |
| mirror | `view.c:494-507` | swaps `left`/`right` where `split == axis`. Parent pointers stay correct because both children share the parent |
| balance / equalize | `view.c:223-275` | only touches `ratio` |
| destroy | `window_node_destroy`, `view.c:335-346` | post-order free; **calls back into `g_window_manager` at `view.c:341`** |

Two pointers escape the tree and are stored globally:
`g_window_manager.insert_feedback` (`view.c:43`) and `g_mouse_state.feedback_node`
(`event_loop.c:1330`). Both are bare `struct window_node *`, so they implicitly carry "which
view" with them. An arena-index port must make that explicit (§7.3).

---

## 6. Allocators

### 6.1 `ts` temporary storage — `src/misc/ts.h`

* **Shape**: one `mmap`ed region, `MEGABYTES(8)` requested at `yabai.c:319`, rounded up to page
  size, with a `PROT_NONE` guard page mapped after it (`ts.h:23`). Bump pointer in
  `volatile uint64_t used`.
* **Allocation**: `ts_alloc_aligned` (`ts.h:52`) CAS-loops on `used` with explicit alignment;
  `ts_alloc_list(T, n)` (`ts.h:49`) is `ts_alloc_aligned(__alignof__(T), sizeof(T)*n)`;
  `ts_alloc_unaligned` (`ts.h:66`) is a plain `__sync_fetch_and_add` with **no alignment at all** —
  used for all string copies (`helpers.h:283`, `:364`, `:390`) and for the daemon message buffer
  (`event_loop.c:1623`).
* **Overflow**: `ts_assert_within_bounds` (`ts.h:28`) prints and `exit(EXIT_FAILURE)`s. The guard
  page is the backstop.
* **Reset**: exactly one site — `ts_reset()` at `event_loop.c:1671`, once per dequeued event,
  *after* `event_signal_flush()` at `:1670`. So the arena lifetime is **one event handler
  invocation**.
* **Who allocates from it**: the event-loop thread (every `*_ts` helper, `display_space_list`,
  `space_window_list*`, `display_manager_active_display_list`, `view_find_window_list`,
  `window_manager_find_application_windows`, `window_manager_add_application_windows`,
  the `ts_alloc_list(struct view *, …)` snapshots at `event_loop.c:191`, `:275`, `:439`, `:502`,
  and `ts_alloc_list(pthread_t, …)` at `window_manager.c:615`) **and the main thread during
  startup** (`yabai.c:340-342` reach `display_manager_active_display_list` and
  `space_window_list` while the event thread is already running and already calling `ts_reset`).
  That is a real race; see §9.5.

**Three things do depend on ts layout, contrary to a naive "it's just scratch" reading:**

1. **Contiguity of consecutive allocations.** Three call sites walk every display, call
   `display_space_list` per display, keep only the *first* returned pointer, and sum the counts —
   treating the separate allocations as one flat array. The comment is repeated verbatim at
   `window_manager.c:78-82`, `process_manager.c:98-102` and `window_manager.c:1594-1598`
   ("`display_space_list(..)` uses a linear allocator, and so we only need to track the beginning
   of the first list"). Implementation sites: `window_manager.c:84-85`,
   `process_manager.c:104-105`, `window_manager.c:1600-1601`.
2. **"Must be the most recent allocation"** for `ts_expand` (`ts.h:75-86`, asserts
   `ptr == memory + used - old_size`) and `ts_resize` (`ts.h:89-98`, same assert). Live users:
   `view_find_window_list` at `view.c:828`, `space_window_list_for_connection` at `space.c:71`.
3. **`ts_buf` grows in place without copying** (`sbuffer.h:51-69`): when the buffer already
   exists it merely bumps `g_temp_storage.used` and returns the *same* header, with **no assert**
   that the buffer is still at the arena tail. Any interleaved ts allocation silently corrupts.
   Live users: `window_node_capture_windows` (`view.c:365`) and
   `window_manager_add_existing_application_windows` (`window_manager.c:1652`). Both happen to be
   safe today only because nothing between the pushes allocates from ts.

**Rust mapping**: all three tricks *disappear* rather than needing to be reproduced. Plain
`Vec<T>` / `String` for every `ts_*` result; the flat-array trick becomes "build one
`Vec<u64>` and `extend` it per display" (which is what the C is emulating); `ts_expand`/`ts_resize`
become `Vec::reserve` / `Vec::truncate`; `ts_buf_push` becomes `Vec::push`. Nothing in the
codebase holds a ts pointer past `ts_reset`, so no lifetime is lost. Confirmed by inspection of
every `ts_*` call site.

### 6.2 `memory_pool` — `src/misc/memory_pool.h`

Two instances, both `mmap` + guard page:

| instance | size | push | reset |
|---|---|---|---|
| `g_event_loop.pool` | `KILOBYTES(512)` (`event_loop.c:1707`) | `memory_pool_push` at `event_loop.c:1689`, one `struct event` per post | never. `memory_pool_push` **wraps to the start of the pool** when full (`memory_pool.h:39-42`) — a ring that silently overwrites still-queued events after ~16 k pending posts |
| `g_signal_storage` | `KILOBYTES(256)` (`yabai.c:323`) | **not** via `memory_pool_push`; `event_signal_push` does a bare `__sync_fetch_and_add` at `event_signal.c:107`, so there is no wrap and an overrun hits the guard page | `g_signal_storage.used = 0` in the **parent** after the fork, `event_signal.c:66` |

`event_signal_flush` (`event_signal.c:60-97`) forks; the child walks the pool as a flat
`struct event_signal[]` (`event_signal.c:70-74`) and forks again per matching subscriber. The
child inherits the ts arena too, which is why `es->arg_name[i]` / `arg_value[i]` / `title` may
point into ts (`event_signal.c:129-130`, `:198`, …) and why `es->app` is `ts_string_copy`-ed
precisely in the two cases where the owner dies inside the handler (`event_signal.c:146`, `:209`).

**Rust mapping**: the event queue becomes a real MPSC channel carrying an owning `enum Event`
(§7.9) — this removes the ring-overwrite hazard, which is a behaviour *improvement* that should
be called out explicitly rather than smuggled in. `g_signal_storage` becomes a
`Vec<PendingSignal>` on the event thread, drained by `flush`.

### 6.3 Stretchy buffers — `src/misc/sbuffer.h`

`buf_*` is a `malloc`/`realloc` header-before-data vector (`sbuffer.h:4-32`). `buf_del`
(`sbuffer.h:19`) is a **swap-remove**. Live `buf` globals: `wm->rules`, `wm->applications_to_refresh`,
`wm->scratchpad_window`, `sm->labels`, `dm->labels`, `g_signal_event[t]`.

`buf_free` is never called anywhere in the daemon; all of these leak at exit by design.

Three loops iterate a buf while swap-removing from it, with manual index fixup:
`window_manager.c:1565-1574`, `event_loop.c:568-577` (one-shot rule pruning) and
`space_manager.c:1138-1146` (refresh list). A fourth removes without fixup and `break`s
(`event_loop.c:264-269`).

**Rust mapping**: `Vec<T>` + `swap_remove`. The iterate-and-remove loops need index-based
`while i < v.len()` because the body needs `&mut Yabai`.

### 6.4 Hashtable — `src/misc/hashtable.h`

Separate-chaining table, `malloc`ed buckets, `max_load = 0.75`, doubling rehash
(`hashtable.h:88-115`). Keys are copied into a `malloc`ed block sized by `sizeof(*key)` at the
`table_add` macro (`hashtable.h:29`), values are stored as bare `void *` and **never owned**.
Two behaviours matter:

* `_table_add` on an existing key with a non-NULL value is a **no-op** (`hashtable.h:120-123`),
  not an overwrite. Every "replace" in the codebase is therefore an explicit
  `table_remove` + `table_add` pair (e.g. `space_manager.c:763-774`, `:1181-1190`).
* `table_remove` frees key and bucket but never shrinks the bucket array.

Eight instances: `pm->process` (key `ProcessSerialNumber`, hash on `lowLongOfPSN`,
`process_manager.c:4-12`), `sm->view` (key `uint64_t`, `space_manager.c:4-12`), and six in
`window_manager` all keyed by `uint32_t` (`window_manager.c:9-17`, init at `:2727-2733`).
Two of them are **sets**, storing `(void *)(intptr_t) 1` as a presence marker
(`window_manager.c:1376`, `:1391`).

**Rust mapping**: `HashMap<K, V>` for the five real maps, `HashSet<K>` for
`window_lost_focused_event` and `application_lost_front_switched_event`.

### 6.5 `malloc` / `free`

Complete inventory of frees in the daemon (grep verified): `application.c:143`,
`display_manager.c:52`, `:68`, `event_signal.c:362-365`, `process_manager.c:49`, `:56`, `:263`,
`:264`, `rule.c:94`, `:214-218`, `:220`, `event_loop.c:989`, `view.c:345`, `:718`, `:719`,
`space_manager.c:174`, `:190`, `window_manager.c:162`, `:592`, `:593`, `:2514`, `window.c:1143`,
`yabai.c:101`. Everything else is deliberately leaked for process lifetime.

### 6.6 CoreFoundation retain/release

Conventions the C follows and Rust must preserve:

* `SLSCopy*`, `CFUUIDCreate*`, `AXUIElementCopyAttributeValue` out-params, `CGWindowListCopyWindowInfo`,
  `SLSWindowQueryWindows`, `SLSWindowQueryResultCopyWindows`, `SLSSpaceCopyName`,
  `cfarray_of_cfnumbers` → **owned**, must be released. The `goto err/out/free` ladders in
  `window.c:71-111`, `:872-897`, `:963-987`, `space.c:17-81`, `display.c:218-252`,
  `process_manager.c:112-145` exist purely to get this right.
* `CFArrayGetValueAtIndex`, `CFDictionaryGetValue`, `window_role()`/`window_subrole()`
  (`window.c:996`, `:1017`) → **borrowed**, must *not* be released.

That second rule is violated once: `event_loop.c:1381-1385` calls `window_role(child)` (borrowed)
and then `CFRelease(role)` — an over-release. See §9.2.

**Rust mapping**: a minimal RAII newtype is the right call here, not the whole
`core-foundation` crate. The private SkyLight/AX surface in `misc/extern.h` (92 declarations) has
no crate coverage anyway, so the FFI layer is hand-written regardless. Recommend a single
`CFOwned<T>(NonNull<T>)` with `Drop = CFRelease`, plus a plain `CFBorrowed<'a, T>` alias for
get-rule results. That makes `window_role()` returning a borrow a *type error* to release, which
is exactly the bug above. Pulling in `core-foundation` only buys `CFString`/`CFArray`/`CFNumber`
conversions, which `ts_cfstring_copy` (`helpers.h:361`) and `cfarray_of_cfnumbers`
(`helpers.h:344`) already do by hand; keep those hand-written for byte-identical behaviour and
use the crate, if at all, only for `CFString` ↔ `String`.

### 6.7 Objective-C, `-fno-objc-arc`

The build is explicitly non-ARC (`makefile:4`, `:21`, `:24`, `:27`). Manual retain/release:

* `workspace_context` is `[workspace_context alloc]` + `[ws_context init]` at `workspace.m:8-11`,
  stored in `g_workspace_context`, never released — a singleton.
* `process->ns_application` is `retain`ed at `workspace.m:30`, `release`d at `workspace.m:65`.
* `g_notify_img` is `retain`ed at `notify.h:23`, never released.
* `[operation release]` after `SLSPerformAsynchronousBridgedWindowManagementOperation`
  (`space_manager.c:673`, `:694`).

**Autorelease pools exist in exactly four places**:

| pool | scope |
|---|---|
| `event_loop.c:1653` … `:1677` | wraps one *drain cycle* of the event loop — created before the inner `for(;;)`, drained when the queue empties |
| `process_manager.c:242-244` | around `process_manager_add_running_processes` |
| `window_manager.c:2739-2756` | around `window_manager_begin` |
| `notify.h:31`, `:47` | around one notification |
| `sa.m` (`[pool drain]` at `sa.m:414`) | around `scripting_addition_load` |

**Pools are missing on every other callback thread.** The CVDisplayLink callback
(`window_manager.c:537`), the proxy-build workers (`window_manager.c:507`), the message-accept
thread (`message.c:3003`) and the CGEventTap callback (`mouse_handler.c:21`) have none. They are
currently CF-only (no ObjC autoreleasing calls), which is why it has not bitten. Phase 2 must
keep that property or add pools.

**Rust mapping**: `objc2` with explicit `retain`/`release` semantics, and an explicit
`autoreleasepool(|_| { … })` at exactly the five sites above — same scopes, same boundaries.
Do not let a Rust ObjC crate's implicit pooling change *when* things drain; the event-loop pool
draining only when the queue empties is observable behaviour under memory pressure.

---

## 7. Proposed Rust ownership model

The guiding rule, and the single most important decision in this document:

> **Every long-lived `struct view *`, `struct window_node *`, `struct window *`,
> `struct application *` and `struct process *` local becomes a handle — `sid: u64`,
> `(sid, NodeId)`, `wid: u32`, `pid: i32` — and is re-resolved on each use.**

That one change makes the overwhelming majority of §8 compile without restructuring the logic.
The cost is extra hash lookups, which the C already pays in many of these functions anyway
(`space_manager_find_view` is called per-operation at `window_manager.c:1001`, `:1015`, `:1029`,
`:1040`, `:1062`, `:1079`, `:1096`, `:1107`, `:1118`, `:1129`, `:1140`, `:1159`).

### 7.1 One owned root, `&mut self` everywhere

```
struct Yabai {                     // owned by the event-loop thread, moved into its closure
    process_manager: ProcessManagerView,  // only the event-thread-visible fields
    display_manager: DisplayManager,
    window_manager:  WindowManager,
    space_manager:   SpaceManager,
    mouse:           MouseDragState,
    mission_control_mode: MissionControlMode,
    signal_storage:  Vec<PendingSignal>,
}
```

Every `foo(struct space_manager *sm, struct window_manager *wm, …)` signature — which is really
"give me two fields of the same god-object" — becomes `impl Yabai { fn foo(&mut self, …) }`.
`window_manager.h` alone has 34 such double-manager signatures (`window_manager.h:110-113`,
`:130-142`, `:180-183`, `:187-190`, `:210-213`); collapsing them removes the two-mutable-borrow
problem at every one of those call sites for free.

### 7.2 Processes, applications, windows

```
processes:    HashMap<Psn, Box<Process>>        // main thread only, behind a raw static or a Mutex
applications: HashMap<i32 /*pid*/, Application>
windows:      HashMap<u32 /*wid*/, Box<Window>>
```

`Box<Window>` rather than `Window` because the AX observer registration stores the window address
as the `void *` context (`window.c:10`) and that address must survive map rehashing. `Window` then
holds `pid: Option<i32>` instead of `application: *mut Application` (the `Option` models
`event_loop.c:281`), and `live_id: AtomicU32` instead of `id_ptr` (§4.1).

`Window::app_name` is the one place worth deviating: make `Process::name` and
`Application::name` both `Arc<str>` and clone the `Arc` at `application.c:133`. That models the
existing alias exactly, removes the free-ordering dependency, and — critically — lets
`window_manager_rule_matches_window` (`window_manager.c:94`) read the app name while `&mut self`
is live, which it otherwise cannot.

### 7.3 The BSP tree: index arena, not `Option<Box<Node>>`

```
struct View {
    nodes: Vec<Option<WindowNode>>,   // slot arena
    free:  Vec<NodeId>,
    root:  NodeId,
    …
}
struct WindowNode {
    parent: Option<NodeId>, left: Option<NodeId>, right: Option<NodeId>, zoom: Option<NodeId>,
    window_list: [u32; 32], window_order: [u32; 32], window_count: usize,
    area: Area, ratio: f32, split: Split, child: Child, insert_dir: i32,
    feedback_window: Option<FeedbackWindow>,
}
```

Why arena and not `Option<Box<Node>>` + `parent: *mut Node`:

* `view_remove_window_node` (`view.c:661-719`) lifts a sibling's contents into its parent and
  re-parents both grandchildren, then frees two interior nodes. With `Box` that is a sequence of
  `take`/`replace` gymnastics *plus* raw parent pointers — i.e. all of the unsafety of the arena
  with none of the ergonomics.
* `node->zoom` legitimately aliases `node->parent`, `node->parent->parent` and `view->root`
  (`view.c:673-708`). With `Box` that must be a raw pointer; with indices it is `Option<NodeId>`
  and the equality tests at `window.c:637`, `:645`, `window_manager.c:2338`, `:2367` stay exactly
  as cheap.
* `window_node_find_prev_leaf` / `find_next_leaf` / `window_node_fence` (`view.c:439-467`,
  `:509-523`) walk *up* through `parent` and then down. Indices make that a plain loop.
* `window_node_destroy` calls back into `&mut Yabai` mid-teardown (`view.c:341`). With an arena
  the fix is mechanical: collect the wids into a `Vec<u32>` first, free the slots, then apply the
  manager mutation. Behaviour-identical, because `managed_window` removal order is irrelevant.

The arena forces two globals to become explicit about *which view* they refer to, which is a
genuine improvement in expressiveness over the C:

* `g_window_manager.insert_feedback`: `HashMap<u32 /*wid*/, (u64 /*sid*/, NodeId)>`
* `g_mouse_state.feedback_node`: `Option<(u64, NodeId)>` — and this one must now be invalidated
  on view destruction, which the C forgets to do (§9.1).

Node identity comparisons (`a_node == b_node` at `window_manager.c:1852`, `:1968`,
`event_loop.c:1189`, `:1295`) must compare `(sid, NodeId)` pairs, not bare `NodeId`, because the
two nodes can come from different views.

### 7.4 Temp storage

Plain `Vec<T>` / `String`. Verified: no ts allocation outlives `ts_reset` (`event_loop.c:1671`),
and the three layout tricks in §6.1 are emulations of things `Vec` does natively. The one
signature change worth noting: `display_space_list(did, &count)` returning a ts pointer becomes
`fn display_space_list(did: u32) -> Vec<u64>`, and the three "concatenate by contiguity"
call sites become `for did in displays { space_list.extend(display_space_list(did)) }`.

### 7.5 CoreFoundation and the private frameworks

`CFOwned<T>` + `CFBorrowed<'a, T>` as in §6.6. The ~92 private declarations in
`misc/extern.h:6-98` become one `extern "C"` block plus two lazily-resolved function pointers
(`OnceLock<Option<fn…>>` mirroring `misc/extern.h:4-5` / `yabai.c:148-149`), with
`macho_find_symbol` (`misc/macho_dlsym.h`) ported as-is because it walks the Mach-O symtab of an
already-loaded image and has no crate equivalent.

### 7.6 Callbacks that only get a `void *`

| callback | current context | Rust |
|---|---|---|
| AX application observer (`application.c:6`, registered `application.c:47`) | `struct application *` | a `*mut Application` from the `Box`ed map value, or better the `pid` as `usize`. It only reads nothing and posts events, so the `pid` suffices except for the destroyed branch |
| AX window observer (`window.c:10`) | `struct window *` | `*mut Window` from the `Box`. The claim CAS then runs on `(*ctx).live_id` — the only place the raw pointer is dereferenced on the main thread, and it is sound because the `Box` outlives the registration (`window_unobserve` at `event_loop.c:628` precedes `window_destroy` at `:629`) |
| mission-control observer (`mission_control.c:59`) | `NULL` | `null_mut()` |
| Carbon process handler (`process_manager.c:151`, installed `:251`) | `&g_process_manager` | main-thread-confined; a `static` raw pointer set once, or a `thread_local` `RefCell`. Do **not** reach for a `Mutex` here — it would change the locking story for a table that has no contention today |
| CGEventTap (`mouse_handler.c:21`, installed `:278`) | `&g_mouse_state` | `Arc<MouseTapState>` leaked into the registration; see §7.7 |
| CG display reconfiguration (`display.c:6`, registered `display_manager.c:505`) | `NULL` | `null_mut()` |
| SkyLight connection notify (`mission_control.c:7`, registered `yabai.c:314-326`) | `NULL` | `null_mut()` |
| CVDisplayLink output (`window_manager.c:537`, set `:701`) | `struct window_animation_context *` | `Box::into_raw(AnimationContext)`, reclaimed with `Box::from_raw` at the `t == 1.0` branch (`window_manager.c:591-596`) |

The rule the C already obeys and Rust should enforce by construction: **no main-thread callback
ever touches the object graph.** They only CFRetain, read an atomic, compute an id, and post.
Enforce it by giving those callbacks no access to `Yabai` at all — the event-loop thread owns it
outright, so there is no `Mutex<Yabai>` to accidentally lock from a callback.

### 7.7 `g_mouse_state` must be split in two

`struct mouse_state` (`mouse_handler.h:62-81`) is currently one struct written from two threads:

| field | written by | read by |
|---|---|---|
| `handle` | main (`mouse_handler.c:278`, `:302`) | main (`:28-29`) — already `__atomic` |
| `consume_mouse_click`, `drag_detected`, `consumed_event` | **main only** (`:37-38`, `:46-54`, `:60`) | main only |
| `modifier` | event thread (config) | main (`:36`, `:65`) — declared `volatile uint8_t` |
| `action1`, `action2`, `drop_action` | event thread (config) | event thread (`event_loop.c:1137`, `:1139`) and `drop_action` at `mouse_handler.c:119` |
| `window`, `window_frame`, `down_location`, `direction`, `current_action`, `last_moved_time`, `ffm_window_id`, `feedback_node` | event thread only | event thread only |

Split into `MouseTapState { handle, consume_mouse_click, drag_detected, consumed_event,
modifier: AtomicU8 }` behind an `Arc` (main thread + one atomic read), and
`MouseDragState { window_id: Option<u32>, window_frame, down_location, direction,
current_action, last_moved_time, ffm_window_id, feedback_node: Option<(u64, NodeId)> }` as a
field of `Yabai`. `action1`/`action2`/`drop_action` are event-thread config → plain fields on
`Yabai`. This removes a genuine data race, not just a borrow-checker complaint.

### 7.8 Rules, signals, labels

* `rules: Vec<Rule>` with `swap_remove`; `Rule` owns `Option<String>` ×5 and four compiled
  patterns.
* `signal_event: [Vec<Signal>; SIGNAL_TYPE_COUNT]`.
* `space_labels: Vec<SpaceLabel>` / `display_labels: Vec<DisplayLabel>`, each owning a `String`;
  the setters take ownership exactly as `space_manager.c:196` and `display_manager.c:73` do.
* **Regex**: the C uses POSIX ERE (`regcomp(..., REG_EXTENDED)` at `message.c:2641`, `:2651`,
  `:2661`, `:2671`, `:2895`, `:2903`; `regexec` at `helpers.h:577`). The `regex` crate is
  leftmost-first, not POSIX leftmost-longest, and differs on a handful of bracket-expression and
  anchoring details. For a behaviour-identical port, **FFI to libc `regcomp`/`regexec`** with a
  `Drop` calling `regfree`. Note that `Regex` then must not be `Copy`, and `Vec::swap_remove`
  moves it — which is fine for `regex_t` (opaque struct holding one pointer), and is exactly what
  the C already does at `sbuffer.h:19`. This needs a decision (see §10).

### 7.9 The event queue

```
enum Event {
    ApplicationLaunched(Psn),            // re-resolve, do NOT carry the pointer
    ApplicationTerminated(Box<Process>), // ownership transfer, matches process_manager.c:190-194
    ApplicationFrontSwitched(Psn),
    WindowCreated(CFOwned<AXUIElementRef>),
    WindowDestroyed(WindowClaim),        // the claimed Box<Window>
    WindowMinimized(*mut Window), …      // peeked, not claimed
    MouseDown { event: CFOwned<CGEventRef>, modifier: u8 }, …
    SpaceChanged, DisplayAdded(u32), DaemonMessage(RawFd), …
}
```

`std::sync::mpsc` / `crossbeam` unbounded channel replaces both the intrusive list and the
semaphore (`event_loop.c:1689-1702`, `:1678`). Two deliberate deviations to declare:

* `ApplicationLaunched`/`FrontSwitched` carry the **PSN**, not the borrowed pointer. The C carries
  the pointer and relies on FIFO ordering (§4.2); carrying the PSN and re-resolving on the main
  thread's table is strictly safer and, because the table is main-confined, requires the handler
  to be handed a resolved `&Process` or a copy of the fields it needs. This is the one place where
  a faithful transposition and a sound one differ; flag it in the phase-2 file map.
* The 512 KiB ring's silent overwrite (`memory_pool.h:39-42`) is not reproduced.

---

## 8. Aliasing the borrow checker will reject

Ordered roughly by how much work each costs. "Fix" entries assume the handle discipline of §7.

| # | site | the C aliasing | Rust fix |
|---|---|---|---|
| 1 | `view.c:341` (`window_node_destroy`) and `view.c:1024` (`view_clear`) | mutating `g_window_manager.managed_window` from inside a `&mut View` teardown | collect wids into a `Vec<u32>`, free slots, then apply to `self.window_manager` |
| 2 | `view.c:43` / `view.c:108` (`insert_feedback_show`/`_destroy`) | `&mut WindowNode` inserted into `g_window_manager.insert_feedback`, *and* `g_window_manager.insert_feedback_color` read in the same expression (`view.c:29-37`) | store `(sid, NodeId)`; read the colour into a local before touching the node |
| 3 | `view.c:44-46`, `:110-112` | `update_window_notifications()` iterates `g_window_manager.window` from inside a view mutation | hoist to the caller, or take the node index and re-enter `self` after dropping the node borrow |
| 4 | `view.c:661-719` (`view_remove_window_node`) | `parent`, `child`, `node`, `child->left`, `child->right`, `view->root` all live simultaneously as mutable | arena indices; each field write is `self.nodes[id]` |
| 5 | `space_manager.c:760-774` | `a_view`/`b_view` obtained by `table_find`, then the table is `table_remove`d and `table_add`ed **while both pointers are still used** at `:766-791` | `HashMap::remove` both to owned `View`s, mutate, re-`insert` under the swapped keys |
| 6 | `space_manager.c:1157-1190` | VLA snapshot of `struct view *` / `CFStringRef` taken from the table, then the table mutated inside the loop while the snapshot is dereferenced (`:1176-1190`) | snapshot `Vec<(u64 /*sid*/, CFOwned<CFString>)>`, then `remove`/`insert` by sid |
| 7 | `event_loop.c:191-243`, `:275-337`, `:439-485`, `:502-546` | `ts_alloc_list(struct view *, n)` snapshots of view pointers, the graph is mutated, then the snapshot is dereferenced to flush | snapshot `Vec<u64>` of sids; re-resolve in the flush loop |
| 8 | `event_loop.c:272-316` | `window_manager_find_application_windows` returns `struct window **` (`window_manager.c:1424-1436`) into `wm->window`, then the loop *removes and frees entries of that very table* at `:313-315` | `Vec<u32>` of wids; `self.windows.remove(&wid)` per iteration |
| 9 | `event_loop.c:1172-1224`, `:1278-1338` (MOUSE_UP / MOUSE_DRAGGED) | `src_view` + `dst_view` (two `&mut View`, possibly the same one), `a_node` + `b_node`, `g_mouse_state.window`, and `&mut g_window_manager` all live at once | `(src_sid, dst_sid, a_node_id, b_node_id, mouse_wid)` handles; each `mouse_drop_*` becomes `&mut self` + handles |
| 10 | `mouse_handler.c:153-179`, `:181-216`, `:218-230` | the `mouse_drop_*` signatures literally take two `struct view *` and two `struct window_node *` and a `struct window_manager *` | same |
| 11 | `window_manager.c:2016-2029` (`swap_window`) | iterating `a_node->window_list` (borrow of view A) while calling `window_manager_remove_managed_window` / `add_managed_window` (`&mut wm`) and `space_manager_move_window_to_space` | copy the `window_list` slice into a small stack array first |
| 12 | `window_manager.c:1854-1944` (`warp_window`) | `a_view`/`b_view`/`a_node`/`b_node`, plus `view_remove_window_node` frees `a_node` and the code then uses `a_node_rm`/`a_node_add` and compares against `a_node_add->parent` (`:1916`) | arena: freed slots are `None`, comparisons are index equality |
| 13 | `window_manager.c:1805-1828` (`stack_window`) | `a_view` held across `space_manager_untile_window(b_view, b)` which can mutate *either* view | handles |
| 14 | `window_manager.c:2540-2577`, `:2579-2623` | `view_find_window_list(view, …)` borrows the view; the loop then calls `view_remove_window_node(view, …)` / `view_add_window_node(view, …)` on it | `Vec<u32>` snapshot |
| 15 | `rule.c:115-128` (`rule_reapply_all`), `rule.c:161-172` (`rule_apply`) | `table_for` over `g_window_manager.window` while the body calls `window_manager_apply_*` which takes `&mut` everything | snapshot `Vec<u32>` of wids, then loop resolving each |
| 16 | `window_manager.c:232-240`, `:764-772`, `:810-818` | `table_for` over `wm->window` while the body calls `window_manager_set_opacity(wm, …)` / `purify_window(wm, …)` | same snapshot pattern |
| 17 | `window_manager.c:176-192`, `:200-216` | iterating `wm->rules` by index while `rule_set_flag(&wm->rules[i], …)` and accumulating `effects`; then `window_manager_apply_*_rule_effects_to_window(sm, wm, …)` | index loop, take the effects out, apply after the loop (which is what the C already does at `:192`/`:216`) |
| 18 | `window_manager.c:1565-1574`, `event_loop.c:568-577`, `space_manager.c:1138-1146` | swap-remove from a `buf` while iterating with manual index fixup, and the body needs `&mut wm` | `while i < len` index loop, `swap_remove`, no `--i` needed if you skip the increment |
| 19 | `window_manager.c:673` | `table_add(&g_window_manager.window_animations_table, …, &context->animation_list[i])` — an interior pointer into a `malloc`ed array, stored globally, freed on another thread | `Arc<AnimationContext>` or an owning `HashMap<u32, AnimationHandle>` where the handle is `(Arc<Mutex<AnimationContext>>, usize)`; keep the existing mutex, just make it guard the whole context |
| 20 | `window_manager.c:631-663` | `existing_animation` (a pointer into *another* context's array, possibly owned by a running CVDisplayLink thread) mutated (`:633`) and read (`:635-658`), then removed and its proxy destroyed (`:662-663`) | same `Arc` scheme; the `skip` flag stays an `AtomicBool` |
| 21 | `event_loop.c:36-71` (`window_did_receive_focus`) | takes `&mut wm` *and* `&mut ms` *and* `&mut window`, then reaches `g_window_manager` again at `:40` | `&mut self` + `wid` |
| 22 | `event_loop.c:305`, `:619`, `window_manager.c:1430`, `:2511` | pointer-identity comparisons (`g_mouse_state.window == window`, `window->application == application`, `scratchpad_window[i].window == window`) | id comparisons (`wid`, `pid`) |
| 23 | `event_loop.c:797-798` | `g_mouse_state.window_frame.size = g_mouse_state.window->frame.size` — writes one field of `ms` from a pointer stored in `ms` | read into a local first |
| 24 | `message.c:2607-2612` + `message.c:440-470` (`parse_key_value_pair`) | the daemon message buffer is a ts allocation (`event_loop.c:1623`) that the parser **mutates in place** (`*token = '\0'` at `message.c:464`) while `key`/`value` alias into it | parse into `Vec<(String, String, bool)>`, or index ranges over an immutable `&str` |
| 25 | `window.c:1037` | `CFEqual(value, window->application->ref)` inside a function that takes `&mut window` | resolve `app.ref` into a local `CFBorrowed` first |
| 26 | `space.c:45`, `:58` | `space_window_list_for_connection` reaches into `g_window_manager` from what looks like a pure SkyLight query | `&self` on `Yabai`; note it is called from paths that already hold `&mut self` (`window_manager.c:2631`) — so it must become `&self` or take a `&HashMap` |
| 27 | `view.c:133`, `:144-146`, `:153`, `:229`, `:233`, `:285`, `:673`, `:710`, `:786-791` | `view.c` reads `g_space_manager.{window_placement, split_type, split_ratio, window_zoom_persist, window_insertion_point}` and `g_window_manager.focused_window_id` from inside `&mut View` methods | pass a small `Copy` `ViewDefaults` struct into the tree functions, or make them `&mut self` methods on `Yabai` |

---

## 9. Latent bugs found while tracing lifetimes

These are all present in the C today. Phase 2 has to decide, per item, whether to reproduce the
bug or fix it — silently "fixing" them by virtue of Rust's rules is a behaviour change that
should be recorded.

**9.1 `g_mouse_state.feedback_node` can dangle.** Set at `event_loop.c:1330` to a node inside a
view. It is nulled at `event_loop.c:1193` and `:1336`, but *nothing* clears it when the node is
freed — not `view_remove_window_node` (`view.c:718-719`), not `window_node_destroy`
(`view.c:345`), not `view_clear` (`view.c:1028`). A drag that straddles a window closing on the
drop target dereferences freed memory at `event_loop.c:1191-1192`. The arena port turns this into
a stale-index read of a `None` slot, i.e. a graceful no-op — which is a fix, not a transposition.

**9.2 Over-release of a borrowed `CFStringRef`.** `event_loop.c:1381-1385`:
```c
CFTypeRef role = window_role(child);   // window.c:996 — returns window->role, NOT retained
if (!role) continue;
bool valid = CFEqual(role, kAXSheetRole) || CFEqual(role, kAXDrawerRole);
CFRelease(role);                       // over-release
```
Every other reader of `window_role`/`window_subrole` treats the result as borrowed
(`window.c:1003`, `:1024`, `:1050-1056`, `:1068-1074`, `:1086-1090`). The `CFOwned`/`CFBorrowed`
split in §6.6 makes this a compile error.

**9.3 Three fixed-size buffers with unchecked writes.**
* `view_find_min_depth_leaf_node` (`view.c:525-539`): `struct window_node *list[256]`, written as
  `list[++j]` twice per iteration for up to 256 iterations — `j` reaches ~512. Stack buffer
  overflow on any view with a deep-enough tree.
* `update_window_notifications` (`event_loop.c:18-33`): `uint32_t window_list[1024]`, filled from
  `table_for` over `g_window_manager.window` with no bound. Overflows past 1024 tracked windows.
* `window_manager_notify_jankyborders` (`window_manager.c:441-455`): `uint32_t proxy_wid[512]`
  and `real_wid[512]` filled from `animation_count` with no bound.
* `window_manager_set_window_layer` (`window_manager.c:849-867`): `check_list[window_count]` is a
  VLA written as `check_list[check_count++]` inside a loop whose bound is `check_count` itself —
  it can exceed `window_count`.

All four become `Vec` in Rust and stop being bugs.

**9.4 `view_remove_window_node` can leak an SLS feedback window.** At `view.c:717-719` it calls
`insert_feedback_destroy(node)` but frees `child` without destroying `child`'s feedback window —
unless `child->insert_dir` was truthy, in which case the window is migrated to `parent` at
`view.c:682`. `insert_feedback_destroy` never clears `insert_dir` (`view.c:105-119`), so the two
fields *can* disagree. In Rust, `FeedbackWindow` should own its SLS window with a `Drop` impl,
which fixes this structurally.

**9.5 The ts arena is raced during startup.** The event-loop thread starts at `yabai.c:331` and
calls `ts_reset()` per event (`event_loop.c:1671`). The main thread then runs
`window_manager_init` / `space_manager_begin` / `window_manager_begin` / `update_window_notifications`
(`yabai.c:340-344`), all of which allocate from ts (via `display_manager_active_display_list`,
`display_space_list`, `space_window_list_for_connection`, `window_title_ts`, …) and hold those
pointers across calls. A concurrently handled event resets `used` to 0 and the next allocation
overwrites them. `used` is atomic so allocation itself is race-free; the *reset* is not. The same
window exists for `g_window_manager`'s tables, which the main thread populates at
`window_manager.c:2740-2755` while the event thread may already be servicing queued events.
Rust's ownership model removes this by construction, since the arena becomes per-handler locals
and the manager graph is owned by one thread — but that means startup must either move into the
event thread or complete before the thread is spawned. **This is a real ordering decision for
phase 2**, not a free win.

**9.6 `space_manager_swap_space_with_space_on_display` can null-deref.** `space_manager.c:760-761`
uses `table_find` (which returns NULL for an unknown sid) rather than `space_manager_find_view`
(which creates on demand), then dereferences unconditionally at `:766`.

**9.7 `EVENT_HANDLER(WINDOW_DESTROYED)` reads possibly-freed memory.** `window_destroy` sets
`window->id = 0` and then `free(window)` (`window.c:1138`, `:1143`); the handler's guard at
`event_loop.c:606` (`window->id == 0`) is therefore a read of freed memory in the double-destroy
case. The `id_ptr` CAS makes the double-destroy unreachable through the AX path, and
`SLS_WINDOW_DESTROYED` (`event_loop.c:952-966`) only *peeks* before calling the handler directly
— so it is currently unreachable, but the guard itself is not the thing making it safe. The
`AtomicU32 live_id` port should claim (swap) rather than peek in the SLS path to make this
explicit.

**9.8 Event-pool wraparound.** `memory_pool_push` (`memory_pool.h:39-42`) resets `used` to `size`
and returns `pool->memory` when the 512 KiB pool fills, overwriting the oldest still-queued
`struct event`. At `sizeof(struct event)` ≈ 32 bytes that is ~16 k pending events. Under a storm
(e.g. a thousand windows resizing) this silently corrupts the queue.

---

## 10. Open questions for phase 2

1. **Startup ordering (§9.5).** Does `window_manager_init` / `space_manager_begin` /
   `window_manager_begin` / `update_window_notifications` move onto the event-loop thread
   (a behaviour change in *when* the first events are serviced), or does the event thread start
   only after they complete (a behaviour change in *whether* early events are dropped)? The C does
   neither and races. This needs a call before any file is transposed, because it decides whether
   `Yabai` is constructed on the main thread and moved, or constructed inside the thread closure.
2. **POSIX regex.** FFI to `regcomp`/`regexec` for byte-identical matching, or the `regex` crate
   with its leftmost-first semantics? Rules and signals are user-facing config, so a semantic
   change here is visible. Recommendation: FFI.
3. **Event-queue bound.** Reproduce the 512 KiB ring's silent overwrite (bounded channel that
   drops), or use an unbounded channel? Recommendation: unbounded, documented as a fix.
4. **`ApplicationLaunched` payload.** Carry the raw `*mut Process` (faithful, relies on FIFO
   ordering for soundness — §4.2) or the `Psn` and re-resolve (sound, but the handler then needs
   main-thread access to the process table, or the fields copied into the event)? This affects
   `process_manager.rs` and `event_loop.rs` signatures directly.
5. **How far to fix §9.** Each of 9.1–9.8 is a place where the natural Rust is *safer* than the C.
   Blanket-fixing is the sane default, but the rewrite's stated goal is behaviour-identical, so
   each needs to be listed in the phase-2 file map as an intentional deviation.
6. **`core-foundation` crate vs hand-rolled `CFOwned<T>`.** The private SkyLight/AX surface is
   hand-written regardless; is it worth the dependency for `CFString`/`CFArray` conversions alone?
   Recommendation: hand-rolled `CFOwned`/`CFBorrowed`, no crate.
