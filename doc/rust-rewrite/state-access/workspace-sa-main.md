# Final Rust signatures — `workspace.h`, `workspace.m`, `sa.h`, `sa.m`, `yabai.c`

Work unit W0b-1. One row per C function, in C source order, per file. Wave 1 pastes the **Rust
signature** column verbatim; nothing in a body may change a parameter list. A disagreement
between a caller in another module and a row here means the fixed point of
`patterns/state-and-ownership.md` §2.1 was computed wrong, not that a signature should be patched
locally.

Derived from `DECISIONS.md` (5, 12, 13, 14, 18, 20, 22, 23, 26, 27, 31, 32, 33, 34, 37),
`patterns/state-and-ownership.md` §1.2-§1.4, §2, §3, `GLOSSARY.md` §1, §2, §4.13, §5.10, §8,
§10, §12, §13, `THREADS.md` §1, §2.2, §3, §4, `patterns/ffi-objc-and-os.md` §16.2, §20-§22,
§25-§26, `patterns/memory-text-and-os-objects.md` §1, §3, and `state-access.tsv`, with every row
re-checked against the C.

Modules: `crate::workspace` (`src/workspace.rs`), `crate::sa` (`src/sa.rs`), and `crate` +
`crate::globals` + `crate::state` (`src/main.rs`, `src/globals.rs`, `src/state.rs`), per
`patterns/idioms-and-conventions.md` §3.

**68 functions are defined across the five files**: 6 in `workspace.h` (the X-macro accessors),
22 in `workspace.m` (11 plain functions plus 11 Objective-C methods), 36 in `sa.m`, 4 in
`yabai.c`. `sa.h` defines none — it declares 21 functions and four data symbols.

---

## 1. How each signature was built

`patterns/state-and-ownership.md` §2.2, in its three steps, without exception:

1. The C parameter list in C order, each pointer rewritten to its handle (§3.1): `uint32_t wid`
   → `WindowId`, `uint64_t sid` → `SpaceId`, `uint32_t did` → `DisplayId`, `pid_t` → `ProcessId`,
   `struct process *` → `&Arc<Process>` (`DECISIONS.md` 22), `void *context` naming the workspace
   observer → `&WorkspaceContext`. A `(pointer, count)` pair derived from one buffer collapses
   into one slice.
2. A manager the C declares keeps the position the C gave it. **None of these five files declares
   a manager parameter**, so step 2 is vacuous for all 68 rows.
3. Every remaining manager of `Managers(f)` is appended after the whole C parameter list, in the
   `src/yabai.c:27-35` order.

**Step 3 appends nothing anywhere in this work unit.** `Managers(f)` is empty for all 68
functions, and this is the single most important fact in the file:

* `src/sa.m` reaches exactly three globals across every row of `state-access.tsv`:
  `g_sa_socket_file`, `g_connection` and the `g_notify_*` pair. All four are `OnceLock` /
  `AtomicBool` statics (`DECISIONS.md` 18, `patterns/state-and-ownership.md` §1.2, §1.3), and
  statics are never parameters (§2.1). The eleven `osax_*` path buffers are the
  `OSAX_PATHS` static of §1.3. So no scripting-addition function takes a manager.
* `src/workspace.m` reaches `g_event_loop` (the `EVENT_SENDER` static, `GLOSSARY.md` §8.1) and
  `g_verbose` (the `VERBOSE` static). `struct process` is an `Arc<Process>` living in the
  `PROCESS_TABLE` static (`DECISIONS.md` 22), not in a manager. So no workspace function takes a
  manager either.
* `src/yabai.c`'s `main` names every manager, but it **constructs** them:
  `THREADS.md` §4.2 step 6 makes `EventLoopOwnedState` a local of `main`, so the managers are
  `main`'s own value and not parameters. `configure_settings_and_acquire_lock` names
  `g_mouse_state` only through `mouse_state_init` (`src/mouse_handler.c:266-272`), which writes
  `modifier`, `action1`, `action2` and `drop_action` — the four fields `DECISIONS.md` 23 puts in
  the `MOUSE_TAP_STATE` static, not in `MouseDragState`. So it takes no manager.

Every manager parameter would be `&mut` (§2.1); the rule is stated for completeness and is
exercised by no row here.

`FILE *rsp` appears in none of these files, so the `Response` of `DECISIONS.md` 28 is absent.

Return types follow `DECISIONS.md` 32: `bool` stays `bool`, and the observable exit-code
integers of `scripting_addition_load` / `_uninstall` / `_install` / `_check` /
`_perform_validation` and of `client_send_message` stay `i32`. `workspace_get_dock_pid` returns
`ProcessId`, whose `ProcessId(0)` keeps the C's "no Dock" meaning that
`src/mission_control.c:79` tests.

---

## 2. Thread context

| Context | What it covers here |
| --- | --- |
| **start-up only** | `src/yabai.c`'s three helpers, `workspace_event_handler_begin` and `-[workspace_context init]`, all of `sa.m`'s install/load path (which runs in a *separate root process*, `yabai --load-sa` / `--uninstall-sa`, never in the daemon) |
| **main run loop** | `main` itself, the nine `workspace_context` selectors, `workspace_application_unobserve` (`src/process_manager.c:191`, the Carbon `kEventAppTerminated` arm) |
| **event loop** | every scripting-addition RPC, and the workspace application helpers reached from `EVENT_HANDLER(APPLICATION_LAUNCHED)` / `(APPLICATION_TERMINATED)` |
| **display link** | `scripting_addition_swap_window_proxy_out` only — its one call site is `src/window_manager.c:580`, inside `window_manager_animate_window_list_thread_proc`, the `CVDisplayLink` output callback (`THREADS.md` §1.6). `scripting_addition_send_bytes` therefore runs there too |
| **any** | the six `workspace_is_macos_*` accessors, `workspace_use_macos_space_workaround`, `workspace_get_dock_pid`, `scripting_addition_is_sip_friendly` |

Four functions are reached from *both* the main thread at start-up and the event-loop thread
afterwards, and are marked `; also main at start-up`:

* `workspace_application_is_observable` — `src/window_manager.c:2741` (inside
  `window_manager_begin`, which `patterns/state-and-ownership.md` §1.4 step 3 runs on main) and
  `src/event_loop.c:121`, `:130`.
* `workspace_application_observe_activation_policy` — `src/window_manager.c:2753` and
  `src/event_loop.c:123`. Its `NSKeyValueObservingOptionInitial` re-enters the KVO selector
  synchronously on the registering thread (`THREADS.md` §4.1), which is why
  `DECISIONS.md` 22 forbids holding the process-table `Mutex` across it.
* `workspace_application_create_running_ns_application` — `src/process_manager.c:66` (main) and
  `src/event_loop.c:87` (event loop).
* `workspace_display_notch_height` — its one caller, `display_bounds_constrained`
  (`src/display.c:141`), is reached from `view_update` (`src/view.c:971`), which
  `state-access/view.md` marks `event loop; also main at start-up`. It mints its
  `MainThreadMarker` with `new_unchecked` at the call site and runs off the main thread in both
  C and Rust (`patterns/ffi-objc-and-os.md` §20.5).

None of this changes a signature, because none of these functions takes a manager — which is
exactly the shape `DECISIONS.md` 20 asks for.

Several scripting-addition RPCs are likewise reachable on main at start-up, through
`window_manager_begin` → `window_manager_add_existing_application_windows` →
`window_manager_create_and_add_window` → `window_manager_adjust_layer`
(`src/window_manager.c:824`) → `scripting_addition_set_layer`. They are listed as `event loop`
because that is where the overwhelming majority of their calls sit, and, again, none of them
takes a manager, so the distinction is not load-bearing for any signature.

---

## 3. `src/workspace.h` — the six X-macro accessors

`workspace.h:12-19` is an X-macro over six `(name, major_version)` pairs, generating one
`static bool _workspace_is_macos_version_<name>` and one `static inline bool
workspace_is_macos_<name>(void)` each. `DECISIONS.md` 31 makes it one `macro_rules!`;
`patterns/idioms-and-conventions.md` §4.3 fixes the generated shapes.

| C name | file:line | thread context | event-loop-owned managers touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `workspace_is_macos_tahoe` | `src/workspace.h:14` | any | — | `pub(crate) fn workspace_is_macos_tahoe() -> bool` | `pub(crate)` |
| `workspace_is_macos_sequoia` | `src/workspace.h:14` | any | — | `pub(crate) fn workspace_is_macos_sequoia() -> bool` | `pub(crate)` |
| `workspace_is_macos_sonoma` | `src/workspace.h:14` | any | — | `pub(crate) fn workspace_is_macos_sonoma() -> bool` | `pub(crate)` |
| `workspace_is_macos_ventura` | `src/workspace.h:14` | any | — | `pub(crate) fn workspace_is_macos_ventura() -> bool` | `pub(crate)` |
| `workspace_is_macos_monterey` | `src/workspace.h:14` | any | — | `pub(crate) fn workspace_is_macos_monterey() -> bool` | `pub(crate)` |
| `workspace_is_macos_bigsur` | `src/workspace.h:14` | any | — | `pub(crate) fn workspace_is_macos_bigsur() -> bool` | `pub(crate)` |

All six are `pub(crate)`: `workspace_is_macos_*` is called from `src/space_manager.c`,
`src/window_manager.c`, `src/window.c`, `src/view.c`, `src/display_manager.c`,
`src/event_loop.c`, `src/message.c` and `src/yabai.c`. The line is `:14` for all six because
that is the `static inline` line inside the `SUPPORT_MACOS_VERSION` definition; the expansion
site is `:19`.

**The flag write has no function of its own.** It is the macro's assignment expansion, written
inline in `workspace_event_handler_begin` at the position of `src/workspace.m:4-6`, exactly where
the C puts it. See §8 for the `patterns/ffi-objc-and-os.md` §20.4 disagreement.

---

## 4. `src/workspace.m` — 22 definitions

`src/workspace.h:21-24` declares `@interface workspace_context : NSObject`. The Rust type is
`WorkspaceContext` (`GLOSSARY.md` §2.1), rebuilt with `objc2::define_class!` carrying
`#[name = "workspace_context"]` so the runtime class name and every selector stay byte-identical
(`patterns/ffi-objc-and-os.md` §20).

### 4.1 Plain functions

| C name | file:line | thread context | event-loop-owned managers touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `workspace_event_handler_begin` | `src/workspace.m:1` | start-up only | — | `pub(crate) fn workspace_event_handler_begin() -> bool` | `pub(crate)` |
| `workspace_use_macos_space_workaround` | `src/workspace.m:17` | any | — | `pub(crate) fn workspace_use_macos_space_workaround() -> bool` | `pub(crate)` |
| `workspace_application_create_running_ns_application` | `src/workspace.m:28` | event loop; also main at start-up | — | `pub(crate) fn workspace_application_create_running_ns_application(process: &Arc<Process>) -> *mut c_void` | `pub(crate)` |
| `workspace_application_destroy_running_ns_application` | `src/workspace.m:33` | event loop | — | `pub(crate) fn workspace_application_destroy_running_ns_application(workspace_context: &WorkspaceContext, process: &Arc<Process>)` | `pub(crate)` |
| `workspace_application_observe_finished_launching` | `src/workspace.m:69` | event loop | — | `pub(crate) fn workspace_application_observe_finished_launching(context: &WorkspaceContext, process: &Arc<Process>)` | `pub(crate)` |
| `workspace_application_observe_activation_policy` | `src/workspace.m:79` | event loop; also main at start-up | — | `pub(crate) fn workspace_application_observe_activation_policy(context: &WorkspaceContext, process: &Arc<Process>)` | `pub(crate)` |
| `workspace_application_unobserve` | `src/workspace.m:89` | main run loop | — | `pub(crate) fn workspace_application_unobserve(workspace_context: &WorkspaceContext, process: &Arc<Process>)` | `pub(crate)` |
| `workspace_application_is_observable` | `src/workspace.m:103` | event loop; also main at start-up | — | `pub(crate) fn workspace_application_is_observable(process: &Arc<Process>) -> bool` | `pub(crate)` |
| `workspace_application_is_finished_launching` | `src/workspace.m:115` | event loop | — | `pub(crate) fn workspace_application_is_finished_launching(process: &Arc<Process>) -> bool` | `pub(crate)` |
| `workspace_display_notch_height` | `src/workspace.m:125` | event loop; also main at start-up | — | `pub(crate) fn workspace_display_notch_height(display_id: DisplayId) -> i32` | `pub(crate)` |
| `workspace_get_dock_pid` | `src/workspace.m:140` | any | — | `pub(crate) fn workspace_get_dock_pid() -> ProcessId` | `pub(crate)` |

Row notes:

* **`workspace_event_handler_begin`.** The C's `void **context` out-parameter writes
  `g_workspace_context`, which is the `WORKSPACE_CONTEXT: OnceLock<Retained<WorkspaceContext>>`
  static (`GLOSSARY.md` §8.1, `patterns/state-and-ownership.md` §1.2). Statics are never
  parameters (§2.1), so the out-parameter disappears and the function fills the static itself.
  The `bool` return is load-bearing — `src/yabai.c:295` turns `false` into
  `error("yabai: could not start workspace context! abort..\n")` — and stays. The function also
  clones the `Sender<Event>` out of `EVENT_SENDER` into `WorkspaceContextIvars`
  (`patterns/ffi-objc-and-os.md` §20.2); the channel exists from `THREADS.md` §4.2 step 5, three
  steps earlier, so nothing is passed in for it. It stores what `init` **returns**, not the
  `alloc` pointer the C stores, and it sets the six version flags *before* the allocation, as
  the C does.
* **The five `Arc<Process>` rows.** `DECISIONS.md` 22 makes `struct process` an `Arc<Process>`,
  and `state-access/window-application-process.md` §"cross-module" already publishes
  `&Arc<Process>` for the three of these that `src/process_manager.c` calls. The two `observe`
  functions need the `Arc` and not just the record: each `addObserver:` mints its refcon with
  `Arc::into_raw(Arc::clone(process))`, one owned strong count per live observation
  (`patterns/ffi-objc-and-os.md` §20.2, `THREADS.md` §6.3 invariant 5). All five therefore take
  `&Arc<Process>`, uniformly.
* **`workspace_context` versus `context`.** The C names this parameter `ws_context` in
  `_destroy_running_ns_application` and `_unobserve`, and `context` in the two `observe`
  functions. `GLOSSARY.md` §10.3 expands `ws_context` to `workspace_context`; `GLOSSARY.md` §12
  lists `context` as a spelling that is already correct and must not be "improved". Applying both
  literally reproduces the C's own asymmetry, and that is what the rows above do.
* **`_create_running_ns_application` returns `*mut c_void`**, because `GLOSSARY.md` §3.4 types
  `Process::ns_application` as `AtomicPtr<c_void>` and the value is stored straight into it with
  a release store (`src/process_manager.c:66`, `src/event_loop.c:87`). The pointer is **owning**:
  objc2 hands back a `Retained<NSRunningApplication>` at `+1` and the function calls
  `Retained::into_raw`, so the C's explicit `[… retain]` has no counterpart to write.
  `_destroy_running_ns_application` takes ownership back exactly once with `Retained::from_raw`,
  which is the `[application release]` of `src/workspace.m:65`.
* **`workspace_display_notch_height` returns `i32`.** The C `int` is compared and subtracted as
  an `int` at `src/display.c:141-143`. See §8 for the `c_int` disagreement.
* **`workspace_get_dock_pid` returns `ProcessId`.** `GLOSSARY.md` §1 maps `pid_t` to `ProcessId`
  with `ProcessId(0)` meaning none, which is exactly the `if (pid && …)` test at
  `src/mission_control.c:79`. The caller passes `process_id.0` to `AXUIElementCreateApplication`
  and `AXObserverCreate`; the C's implicit `pid_t` → `uint32_t` narrowing at
  `src/mission_control.c:76` becomes the explicit `as u32` that `DECISIONS.md` 30 requires, at
  the same point in the expression.

### 4.2 The `workspace_context` class

Eleven methods. `init` is an inherent method; `dealloc` is `Drop`
(`#[unsafe(method(dealloc))]` is a `compile_error!` in objc2 0.6.4, and the macro emits the
`[super dealloc]`); the other nine are `#[unsafe(method(…))]` selectors inside `define_class!`.
All eleven are private to `crate::workspace` — the runtime, not another module, is the caller.

| C name | file:line | thread context | event-loop-owned managers touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `-[workspace_context init]` | `src/workspace.m:154` | start-up only | — | `fn init(this: Allocated<Self>) -> Retained<Self>` | private |
| `-[workspace_context dealloc]` | `src/workspace.m:201` | main run loop | — | `fn drop(&mut self)` | private (`impl Drop for WorkspaceContext`) |
| `-[workspace_context observeValueForKeyPath:ofObject:change:context:]` | `src/workspace.m:209` | main run loop | — | `fn observeValueForKeyPath_ofObject_change_context(&self, key_path: &NSString, object: &AnyObject, change: &NSDictionary, context: *mut c_void)` | private |
| `-[workspace_context didWake:]` | `src/workspace.m:261` | main run loop | — | `fn didWake(&self, notification: &NSNotification)` | private |
| `-[workspace_context didChangeMenuBarHiding:]` | `src/workspace.m:266` | main run loop | — | `fn didChangeMenuBarHiding(&self, notification: &NSNotification)` | private |
| `-[workspace_context didRestartDock:]` | `src/workspace.m:271` | main run loop | — | `fn didRestartDock(&self, notification: &NSNotification)` | private |
| `-[workspace_context didChangeDockPref:]` | `src/workspace.m:276` | main run loop | — | `fn didChangeDockPref(&self, notification: &NSNotification)` | private |
| `-[workspace_context activeDisplayDidChange:]` | `src/workspace.m:281` | main run loop | — | `fn activeDisplayDidChange(&self, notification: &NSNotification)` | private |
| `-[workspace_context activeSpaceDidChange:]` | `src/workspace.m:286` | main run loop | — | `fn activeSpaceDidChange(&self, notification: &NSNotification)` | private |
| `-[workspace_context didHideApplication:]` | `src/workspace.m:291` | main run loop | — | `fn didHideApplication(&self, notification: &NSNotification)` | private |
| `-[workspace_context didUnhideApplication:]` | `src/workspace.m:297` | main run loop | — | `fn didUnhideApplication(&self, notification: &NSNotification)` | private |

Row notes:

* **`init`** registers the eight observers across three notification centres in the C's
  registration order, after calling `NSObject`'s `init`, matching `if ((self = [super init]))`.
  `patterns/ffi-objc-and-os.md` §20.1 has the full centre/name table.
* **`dealloc`** is unreachable — the instance lives in a `OnceLock` in a `static` and a `OnceLock`
  in a `static` is never dropped — but it is written anyway, as the C has it
  (`DECISIONS.md` 2). Its three `removeObserver:` calls go in the body of
  `impl Drop for WorkspaceContext` in the C's order. It is **not** a dead function under
  `DECISIONS.md` 5: it has a definition and the runtime is its caller.
* **The KVO callback's `context`** stays a raw `*mut c_void` and is *borrowed*, never reclaimed:
  `unsafe { &*(context as *const Process) }`. Its two `@try`/`@catch` removals go through
  `remove_observer_swallowing_exception` (§7), and a removal that returns normally retires one
  strong count through `release_kvo_refcon_on_main_queue`. The two `if` blocks are independent,
  not `else if`, and both can run for one notification; that is preserved. `terminated` is read
  with `Ordering::Acquire`, pairing with the `Release` store at `src/process_manager.c:189`.
* **The eight notification handlers** each post one `Event` and nothing else. Per
  `patterns/ffi-objc-and-os.md` §20.2 they send through `self.ivars().event_sender`, not through
  the free `event_loop_post`. Payloads, from `THREADS.md` §3.2: `SystemWoke`,
  `MenuBarHiddenChanged`, `DockDidRestart`, `DockDidChangePref`, `DisplayChanged`, `SpaceChanged`
  carry nothing; `didHideApplication:` sends `Event::ApplicationHidden(ProcessId)` and
  `didUnhideApplication:` sends `Event::ApplicationVisible(ProcessId)`, replacing the C's
  `(void *)(intptr_t) pid` punning.

---

## 5. `src/sa.m` — 36 definitions

`src/sa.h` defines nothing. Its 21 declarations all have definitions in `src/sa.m`; its four data
symbols are covered in §6.

Every row's manager column is `—`: see §1.

### 5.1 The install / load path — a separate root process

`yabai --load-sa` and `yabai --uninstall-sa` `exit` inside `parse_arguments`
(`src/yabai.c:216-220`) before `main` reaches `configure_settings_and_acquire_lock`, so nothing
below runs in the daemon and nothing below can observe daemon state.

| C name | file:line | thread context | event-loop-owned managers touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `scripting_addition_set_path` | `src/sa.m:78` | start-up only | — | `fn scripting_addition_set_path() -> OsaxPaths` | private |
| `scripting_addition_create_directory` | `src/sa.m:96` | start-up only | — | `fn scripting_addition_create_directory() -> bool` | private |
| `scripting_addition_write_file` | `src/sa.m:110` | start-up only | — | `fn scripting_addition_write_file(buffer: &[u8], file: &str, file_mode: &str) -> bool` | private |
| `scripting_addition_prepare_binaries` | `src/sa.m:122` | start-up only | — | `fn scripting_addition_prepare_binaries()` | private |
| `scripting_addition_restart_dock` | `src/sa.m:139` | start-up only | — | `fn scripting_addition_restart_dock()` | private |
| `scripting_addition_set_socket_path` | `src/sa.m:145` | start-up only | — | `fn scripting_addition_set_socket_path() -> bool` | private |
| `scripting_addition_is_installed` | `src/sa.m:162` | start-up only | — | `fn scripting_addition_is_installed() -> bool` | private |
| `scripting_addition_check` | `src/sa.m:173` | start-up only | — | `fn scripting_addition_check() -> i32` | private |
| `scripting_addition_remove` | `src/sa.m:193` | start-up only | — | `fn scripting_addition_remove() -> bool` | private |
| `scripting_addition_install` | `src/sa.m:202` | start-up only | — | `fn scripting_addition_install() -> i32` | private |
| `scripting_addition_request_handshake` | `src/sa.m:239` | start-up only | — | `fn scripting_addition_request_handshake(version: &mut String, attributes: &mut u32) -> bool` | private |
| `scripting_addition_perform_validation` | `src/sa.m:270` | start-up only | — | `fn scripting_addition_perform_validation() -> i32` | private |
| `scripting_addition_is_sip_friendly` | `src/sa.m:301` | any | — | `pub(crate) fn scripting_addition_is_sip_friendly() -> bool` | `pub(crate)` |
| `scripting_addition_is_arm64e_enabled` | `src/sa.m:318` | start-up only | — | `#[cfg(target_arch = "aarch64")] fn scripting_addition_is_arm64e_enabled() -> bool` | private |
| `mach_loader_inject_payload` | `src/sa.m:333` | start-up only | — | `fn mach_loader_inject_payload() -> bool` | private |
| `scripting_addition_uninstall` | `src/sa.m:350` | start-up only | — | `pub(crate) fn scripting_addition_uninstall() -> i32` | `pub(crate)` |
| `scripting_addition_load` | `src/sa.m:369` | start-up only | — | `pub(crate) fn scripting_addition_load() -> i32` | `pub(crate)` |

Row notes:

* **`scripting_addition_set_path` returns `OsaxPaths`** although the C returns `void`.
  `patterns/state-and-ownership.md` §1.3 makes the eleven `char[MAXLEN]` buffers one
  `static OSAX_PATHS: OnceLock<OsaxPaths>` and turns the
  `if (osax_base_dir[0] == 0) scripting_addition_set_path();` guards (`src/sa.m:164`) into
  `OSAX_PATHS.get_or_init(scripting_addition_set_path)`, which requires
  `FnOnce() -> OsaxPaths`. The field spellings are `GLOSSARY.md` §3.29. `MAXLEN` is 512 and
  `snprintf` truncation at that length is observable in the error messages, so the Rust truncates
  to 512 bytes at the same eleven points.
* **`scripting_addition_write_file` collapses `(buffer, size)` into `&[u8]`** — every call site
  passes a length derived from the same buffer (`strlen(sa_plist)`, `strlen(sa_bundle_plist)`,
  `__src_osax_loader_len`, `__src_osax_payload_len`), which is the `(pointer, count)` rule of
  §2.2. The success test stays `fwrite`'s `bytes == 1`, i.e. the whole slice was written.
  `file_mode` is kept: `"w"` and `"wb"` select the same behaviour on macOS, so it decides
  nothing, but no rule in §2.2 removes it and removing it would be improvisation. See §8.
* **`scripting_addition_request_handshake` keeps both out-parameters.** §2.2 collapses only an
  out-parameter *count* paired with a returned buffer; neither of these is one, and
  `DECISIONS.md` 32 keeps the `bool` return a `bool`. `version` is the C's
  `char[SA_SOCKET_BUFF_LEN]` as a `String`; `attributes` is the C's `uint32_t *attrib`, expanded
  per `DECISIONS.md` 37. The Rust scans for the NUL within the bytes actually received and
  bounds-checks that four more follow, where the C reads past a short response — one
  `DEVIATIONS.md` line (`patterns/ffi-objc-and-os.md` §25.4). See §8 for the `OsaxAttrib`
  disagreement behind the `u32`.
* **`scripting_addition_check`, `_install`, `_perform_validation`, `_load`, `_uninstall` return
  `i32`.** These are `DECISIONS.md` 32's observable sentinels: `_install` returns 0, 1 (already
  installed and removal failed) or 2 (a step failed, after `scripting_addition_remove`), and
  `main` hands `_load` / `_uninstall`'s value straight to `exit`.
* **`scripting_addition_is_sip_friendly` is the only `pub(crate)` private-in-C function here** —
  `src/message.c:1304` calls it, which `patterns/idioms-and-conventions.md` §2.5 lists
  explicitly.
* **`scripting_addition_uninstall` checks SIP first and root second**, the opposite order from
  `_load`; that difference is observable in which message the user sees and is kept.
* `scripting_addition_set_socket_path` writes the `SA_SOCKET_FILE` static from `$SUDO_UID` →
  `getpwuid` → `pw_name` after `debug_assert!(getuid() == 0)`; `sscanf(sudo_uid, "%u", &uid)`
  calls libc directly (`DECISIONS.md` 27) and `uid` keeps the `getuid()` value if the parse
  fails.
* `scripting_addition_check` and `scripting_addition_load` each wrap their whole body in
  `objc2::rc::autoreleasepool(|_pool| { … })`; `_load`'s four `goto out`s become early returns
  from the closure (`patterns/ffi-objc-and-os.md` §21.2).

### 5.2 The transport and the nineteen RPCs

| C name | file:line | thread context | event-loop-owned managers touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `scripting_addition_send_bytes` | `src/sa.m:422` | event loop; also display link and start-up | — | `fn scripting_addition_send_bytes(bytes: &[u8]) -> bool` | private |
| `scripting_addition_focus_space` | `src/sa.m:442` | event loop | — | `pub(crate) fn scripting_addition_focus_space(space_id: SpaceId) -> bool` | `pub(crate)` |
| `scripting_addition_create_space` | `src/sa.m:449` | event loop | — | `pub(crate) fn scripting_addition_create_space(space_id: SpaceId) -> bool` | `pub(crate)` |
| `scripting_addition_destroy_space` | `src/sa.m:456` | event loop | — | `pub(crate) fn scripting_addition_destroy_space(space_id: SpaceId) -> bool` | `pub(crate)` |
| `scripting_addition_move_space_to_display` | `src/sa.m:463` | event loop | — | `pub(crate) fn scripting_addition_move_space_to_display(source_space_id: SpaceId, destination_space_id: SpaceId, source_previous_space_id: SpaceId, focus: bool) -> bool` | `pub(crate)` |
| `scripting_addition_move_space_after_space` | `src/sa.m:473` | event loop | — | `pub(crate) fn scripting_addition_move_space_after_space(source_space_id: SpaceId, destination_space_id: SpaceId, focus: bool) -> bool` | `pub(crate)` |
| `scripting_addition_move_window` | `src/sa.m:484` | event loop | — | `pub(crate) fn scripting_addition_move_window(window_id: WindowId, x: i32, y: i32) -> bool` | `pub(crate)` |
| `scripting_addition_set_opacity` | `src/sa.m:493` | event loop | — | `pub(crate) fn scripting_addition_set_opacity(window_id: WindowId, opacity: f32, duration: f32) -> bool` | `pub(crate)` |
| `scripting_addition_set_layer` | `src/sa.m:502` | event loop | — | `pub(crate) fn scripting_addition_set_layer(window_id: WindowId, layer: i32) -> bool` | `pub(crate)` |
| `scripting_addition_set_sticky` | `src/sa.m:510` | event loop | — | `pub(crate) fn scripting_addition_set_sticky(window_id: WindowId, sticky: bool) -> bool` | `pub(crate)` |
| `scripting_addition_set_shadow` | `src/sa.m:518` | event loop | — | `pub(crate) fn scripting_addition_set_shadow(window_id: WindowId, shadow: bool) -> bool` | `pub(crate)` |
| `scripting_addition_focus_window` | `src/sa.m:526` | event loop | — | `pub(crate) fn scripting_addition_focus_window(window_id: WindowId) -> bool` | `pub(crate)` |
| `scripting_addition_scale_window` | `src/sa.m:533` | event loop | — | `pub(crate) fn scripting_addition_scale_window(window_id: WindowId, x: f32, y: f32, width: f32, height: f32) -> bool` | `pub(crate)` |
| `scripting_addition_swap_window_proxy_in` | `src/sa.m:544` | event loop | — | `pub(crate) fn scripting_addition_swap_window_proxy_in(animation_list: &[WindowAnimation]) -> bool` | `pub(crate)` |
| `scripting_addition_swap_window_proxy_out` | `src/sa.m:560` | display link | — | `pub(crate) fn scripting_addition_swap_window_proxy_out(animation_list: &[WindowAnimation]) -> bool` | `pub(crate)` |
| `scripting_addition_order_window` | `src/sa.m:576` | event loop | — | `pub(crate) fn scripting_addition_order_window(a_window_id: WindowId, order: i32, b_window_id: WindowId) -> bool` | `pub(crate)` |
| `scripting_addition_order_window_in` | `src/sa.m:586` | event loop | — | `pub(crate) fn scripting_addition_order_window_in(window_list: &[WindowId]) -> bool` | `pub(crate)` |
| `scripting_addition_move_window_list_to_space` | `src/sa.m:604` | event loop | — | `pub(crate) fn scripting_addition_move_window_list_to_space(space_id: SpaceId, window_list: &[WindowId]) -> bool` | `pub(crate)` |
| `scripting_addition_move_window_to_space` | `src/sa.m:615` | event loop | — | `pub(crate) fn scripting_addition_move_window_to_space(space_id: SpaceId, window_id: WindowId) -> bool` | `pub(crate)` |

Row notes:

* **`scripting_addition_send_bytes` collapses `(bytes, length)` into `&[u8]`** — `length` is the
  `sa_payload_send` macro's running frame length over the same stack buffer
  (`patterns/ffi-objc-and-os.md` §25.3). The one-byte `recv` is the ack that makes every RPC
  synchronous with respect to Dock.app; its return value must be neither dropped nor checked.
  A failed `socket_open` skips the close entirely, as the C's `if` does.
* **Three `(pointer, count)` collapses**, all covered by §2.2's second mechanical consequence:
  `(animation_list, animation_count)` → `&[WindowAnimation]` (both call sites pass
  `context->animation_list, context->animation_count`), and `(window_list, window_count)` →
  `&[WindowId]` in `_order_window_in` (`src/window_manager.c:2535`) and
  `_move_window_list_to_space` (`src/space_manager.c:679`). The `i32` count that goes **on the
  wire** is unaffected: it is packed as `animation_list.len() as i32` / `window_list.len() as i32`,
  keeping the frame byte-identical.
* **`&[WindowId]`, not `&[u32]`**: `GLOSSARY.md` §1 makes `WindowId` the spelling for every
  window id, and `patterns/state-and-ownership.md` §7 turns every `uint32_t *` window buffer into
  a `Vec<WindowId>`. FFI calls inside the body pass `.0` explicitly.
* **`w` / `h` → `width` / `height`** in `_scale_window`, per `GLOSSARY.md` §10.4. `x` and `y` as
  coordinates are unchanged, and `opacity`, `layer`, `order` are `GLOSSARY.md` §12 words that
  stay. `a_wid`/`b_wid` → `a_window_id`/`b_window_id` and `src_sid`/`dst_sid`/`src_prev_sid` →
  `source_space_id`/`destination_space_id`/`source_previous_space_id` are `GLOSSARY.md` §10.2
  rows that name `sa.m:576` and `sa.h:15` by line.
* **`_move_space_after_space` keeps its own function** even though it shares
  `SaOpcode::SpaceMove` with `_move_space_to_display`: it packs a `dummy_sid` of `0` where the
  other packs `source_previous_space_id`, so the frames are byte-identical in shape.
* **`_set_opacity` chooses the opcode from the argument** — `duration > 0.0f` selects
  `SaOpcode::WindowOpacityFade`, otherwise `SaOpcode::WindowOpacity` — and packs all three fields
  in both cases.
* **`_order_window_in` queries SkyLight while packing**, with `ordered_in` declared **once
  outside** the loop and never reset, so a failing `SLSWindowIsOrderedIn` leaves the previous
  window's answer in place. That is reproduced exactly, declaration position included.
* **`_swap_window_proxy_in` / `_out` are the variable-stride encoding**: a skipped animation
  contributes four bytes (a `u32` zero), a live one eight (`window_id` then `proxy.id`).
  `WindowAnimation::skip` is read `Relaxed`, matching `__atomic_load_n(…, __ATOMIC_RELAXED)` at
  `src/sa.m:550` and `:566`; it is atomic precisely because `_out` runs on the display-link
  thread while `_in` runs on the event-loop thread.
* **The bounds check of `DECISIONS.md` 34 turns three C stack-buffer overruns into a `false`
  return**: 512 animations need 4103 bytes, `_order_window_in` overruns past 1022 windows and
  `_move_window_list_to_space` past 1021, against a 4096-byte frame. Three `DEVIATIONS.md` lines.

---

## 6. `src/yabai.c` — 4 definitions

| C name | file:line | thread context | event-loop-owned managers touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `client_send_message` | `src/yabai.c:54` | start-up only (the CLIENT process) | — | `fn client_send_message(arguments: &[String]) -> i32` | private |
| `configure_settings_and_acquire_lock` | `src/yabai.c:128` | start-up only | — | `fn configure_settings_and_acquire_lock() -> bool` | private |
| `parse_arguments` | `src/yabai.c:181` | start-up only | — | `fn parse_arguments(arguments: &[String]) -> Option<String>` | private |
| `main` | `src/yabai.c:261` | main run loop | — | `fn main()` | private (the crate entry point) |

Row notes:

* **`client_send_message` collapses `(argc, argv)` into `&[String]`** — one buffer and its count,
  §2.2 again — and `main` calls it as `client_send_message(&arguments[1..])`, reproducing the C's
  `client_send_message(argc-1, argv+1)`. The body's own `for (int i = 1; …)` then skips the `-m`
  itself, unchanged, so the wire message is byte-identical. `THREADS.md` §2.8: it uses only
  `UnixStream`, `Write`, `Read` and `std::process::exit`, and must name neither
  `EventLoopOwnedState` nor the channel nor any manager type. The return is the C's
  `EXIT_SUCCESS` / `EXIT_FAILURE`, which `parse_arguments` hands to `exit`.
* **`configure_settings_and_acquire_lock` takes no manager.** The twelve globals
  `state-access.tsv` lists for it are the `OnceLock` statics of `DECISIONS.md` 18 —
  `SA_SOCKET_FILE`, `SOCKET_FILE`, `LOCK_FILE`, `CONNECTION`, `PROCESS_ID`, `BOOTSTRAP_PORT`,
  `CV_HOST_CLOCK_FREQUENCY`, the three window levels, plus the two runtime-resolved SkyLight
  pointers — and `g_mouse_state`, which it touches only through `mouse_state_init`, whose four
  writes all land in `MOUSE_TAP_STATE` (`DECISIONS.md` 23). `g_event_bytes` is gone
  (`GLOSSARY.md` §8.1: a zeroed `[u8; 0x100]` local in each of its two users). The `bool` return
  is `fcntl(handle, F_SETLK, &lockfd) != -1`; the lock-file descriptor is deliberately never
  closed (`DECISIONS.md` 34). The `#if 0` block at `src/yabai.c:158-162` is not translated
  (§9).
* **`parse_arguments` returns `Option<String>`** — the `--config` value, last occurrence wins, as
  in C. `patterns/memory-text-and-os-objects.md` §3.1 settles this: `main` performs the single
  `CONFIG_FILE.set(…)` after the loop, with `String::new()` when the flag was absent, because the
  C writes `g_config_file` twice on inputs a user can type (`yabai -c one -c two`, and `yabai -c ""`
  followed by `get_config_file`'s fallback) and a second `set(…).unwrap()` would `Err` and abort.
  Every `--*-service`, `--*-sa`, `--version`, `--help` and `-m` branch still `exit`s inside this
  function. The local spellings are `option` and `value` (`GLOSSARY.md` §10.4).
* **`main` takes no parameters and no managers.** `THREADS.md` §4.2 step 6 makes
  `EventLoopOwnedState` a local built at the position of the C's BSS zeroing, and steps 8-14 pass
  `&mut` fields of it to the `*_begin` functions on the main thread; step 15 **moves** it into
  the event-loop thread, after which no main-thread code may name it. `argc`/`argv` come from
  `std::env::args().collect::<Vec<String>>()`. The C's `#ifndef TESTS` guard
  (`src/yabai.c:260`) disappears: `cargo test` supplies its own harness (`DECISIONS.md` 40).
  The panic hook of `DECISIONS.md` 7 is `main`'s first statement, before `parse_arguments`.

---

## 7. Rust-only helpers this work unit introduces

No C original, so §2.2 step 2 is vacuous and step 3 appends nothing (none of them reaches a
manager). Listed so wave 1 does not invent a second spelling.

| Helper | module | signature | source |
| --- | --- | --- | --- |
| the frame writer | `crate::sa` | `struct ScriptingAdditionPayload { bytes: [u8; SA_SOCKET_BUFF_LEN], length: i16 }` with `fn new() -> Self`, `fn pack(&mut self, value: &[u8]) -> bool`, `fn send(mut self, opcode: SaOpcode) -> bool` | `patterns/ffi-objc-and-os.md` §25.2; replaces the three macros at `src/sa.m:418-420` |
| the eleven path buffers | `crate::sa` | `struct OsaxPaths` with eleven `String` fields | `GLOSSARY.md` §3.29, `patterns/state-and-ownership.md` §1.3 |
| exception-swallowing removal | `crate::workspace` | `fn remove_observer_swallowing_exception(application: &NSRunningApplication, workspace_context: &WorkspaceContext, key_path: &NSString, process: &Process) -> bool` | `patterns/ffi-objc-and-os.md` §22 — the **only** place `objc2::exception::catch` appears, shared with `crate::event_loop`'s two sites at `src/event_loop.c:112`, `:131`. The KVO callback's own two sites take an `&AnyObject` instead of an `&NSRunningApplication` and are otherwise identical. Returns whether the removal came back normally, which is what the refcon accounting turns on |
| refcon retirement | `crate::workspace` | `fn release_kvo_refcon_on_main_queue(process: &Process)` | `patterns/ffi-objc-and-os.md` §20.2, §21.1 — the one place `Arc::from_raw` is written for a KVO refcon; `DECISIONS.md` 20 forbids taking the count on the event-loop thread |
| the version-flag macro | `crate::workspace` | `macro_rules! supported_macos_version_list` generating, per entry, one `static _workspace_is_macos_version_<name>: AtomicBool`, one `pub(crate) fn workspace_is_macos_<name>() -> bool`, and the assignment expansion written **inline** in `workspace_event_handler_begin` | `patterns/idioms-and-conventions.md` §4.3; `paste!`-style concatenation is unavailable (`DECISIONS.md` 11), so both identifiers are spelled out at the entry site |

---

## 8. Judgement calls and the disagreements they settle

Eleven places where the mechanical rule was ambiguous or two documents disagreed. `GLOSSARY.md`
states the precedence: `DECISIONS.md` → `GLOSSARY.md` → `patterns/*.md` → `THREADS.md`.

1. **`WorkspaceContext`, not `workspace_context`, is the Rust type name.** `GLOSSARY.md` §2.1 and
   `patterns/state-and-ownership.md` §1.2 both say `WorkspaceContext`;
   `patterns/ffi-objc-and-os.md` §20-§22 sketches the lowercase `workspace_context`.
   `GLOSSARY.md` outranks `patterns/`. The runtime name stays `workspace_context` through
   `#[name = "workspace_context"]`, which is what `NSNotificationCenter` and the crash logs see.
2. **`workspace_event_handler_begin` loses its `void **context` out-parameter.** The destination
   is the `WORKSPACE_CONTEXT` static and §2.1 says statics are never parameters. The alternative
   — returning `Option<Retained<WorkspaceContext>>` and letting `main` set the static — was
   rejected because it would put the `bool`/`Option` decision in `main` and change the
   `src/yabai.c:295` error path's shape. `THREADS.md` §4.2 step 7 writes the call bare, which
   agrees.
3. **`*mut c_void`, not `*mut NSRunningApplication`, from
   `_create_running_ns_application`.** `GLOSSARY.md` §3.4 types the field `AtomicPtr<c_void>`;
   `patterns/ffi-objc-and-os.md` §21.1 types it `AtomicPtr<NSRunningApplication>`.
   `GLOSSARY.md` wins, and `state-access/window-application-process.md` — the caller's own
   work unit — already published `-> *mut c_void`.
4. **`&Arc<Process>`, not `&Process`, on all five process-taking workspace functions.**
   `patterns/ffi-objc-and-os.md` §21.1 sketches `&Process`;
   `state-access/window-application-process.md` publishes `&Arc<Process>` for the three that
   `src/process_manager.c` calls, and the two `observe` functions need the `Arc` to mint
   `Arc::into_raw(Arc::clone(process))`. Uniform `&Arc<Process>` is the only choice that lets
   every caller and callee agree. The §7 helpers keep `&Process` as
   `patterns/ffi-objc-and-os.md` §22 writes them, because they need only the address; call sites
   pass `&**process`.
5. **`context` stays `context` in the two `observe` functions while `ws_context` becomes
   `workspace_context` in the other two.** `GLOSSARY.md` §10.3 expands `ws_context`;
   `GLOSSARY.md` §12 lists `context` as already correct. Applying both literally reproduces the
   C's asymmetry. Unifying on one spelling would have been an improvisation by a single agent.
6. **`-> i32`, not `-> c_int`, for `workspace_display_notch_height`.**
   `patterns/ffi-objc-and-os.md` §20.5 writes `c_int`; every C `int` in
   `patterns/idioms-and-conventions.md` §2.3 and in the sibling work units
   (`state-access/view.md`, `state-access/window-application-process.md`) is `i32`. Consistency
   across the nine modules wins; the two types are identical on both targets.
7. **`attributes: &mut u32`, not `&mut OsaxAttrib`.** `GLOSSARY.md` §5.10 defines
   `OsaxAttrib(pub u32)` as a flag newtype; `patterns/ffi-objc-and-os.md` §25.1 and the
   **already-committed `build.rs`** (`build.rs:147-168`) generate plain
   `pub const OSAX_ATTRIB_*: u32`. The generated file is what `src/sa.rs` already `include!`s,
   so a newtype here would not compile. `u32` is also the lower-risk choice because
   `scripting_addition_request_handshake` is private to `crate::sa` and no other module sees it.
   **Flagged for the parent: `GLOSSARY.md` §5.10 and `build.rs` disagree and one of them should
   be amended.** (`build.rs` already agrees with `GLOSSARY.md` §4.13 on `SaOpcode` and its
   CamelCase variants, against `patterns/ffi-objc-and-os.md` §25.1's C spellings.)
8. **`file_mode: &str` is kept on `scripting_addition_write_file`**, although `"w"` and `"wb"`
   are identical on macOS and Rust's `File::create` covers both, so the body may not read it.
   §2.2 removes only out-parameter counts, `(pointer, count)` pairs and `struct event_loop *`;
   nothing authorises dropping this one, and dropping it would change the call shape at four
   sites. Wave 1 should either thread it into the open or, if the compiler complains, raise it
   rather than rename the parameter.
9. **`scripting_addition_request_handshake` keeps two out-parameters and a `bool` return**
   rather than becoming `-> Option<(String, u32)>`. `DECISIONS.md` 32 turns nullable *pointers*
   into `Option` and keeps `bool` returns as `bool`; these are out-parameters, not nullable
   pointers, and §2.2's collapse rule is explicitly about a count paired with a returned buffer.
10. **`scripting_addition_set_path` gains an `OsaxPaths` return** although the C returns `void`.
    `patterns/state-and-ownership.md` §1.3 names it as the `OSAX_PATHS.get_or_init` initialiser,
    which fixes the signature; the C's guard-then-fill idiom has no other faithful shape once
    the eleven buffers are one `OnceLock`.
11. **`scripting_addition_swap_window_proxy_out` is marked `display link`, not `event loop`.**
    Its single call site, `src/window_manager.c:580`, is inside
    `window_manager_animate_window_list_thread_proc`, the `CVDisplayLink` output callback. That
    is why `scripting_addition_send_bytes` is marked `event loop; also display link and
    start-up`, and why `WindowAnimation::skip` has to be an `AtomicBool`. It changes no
    signature, but it is the one row in this work unit where the thread column is not the
    obvious one.

---

## 9. Dead functions — not translated

**No function *defined* in these five files is dead.** Every definition has a caller; `-dealloc`
is reached by the Objective-C runtime rather than by yabai code and is written as
`impl Drop for WorkspaceContext`, per `patterns/ffi-objc-and-os.md` §20.1.

What `DECISIONS.md` 5 does remove from these files, none of it a defined function:

| C | site | why |
| --- | --- | --- |
| `hook_nsobject_autorelease()`, `hook_autoreleasepool_drain()`, `hook_autoreleasepool_release()` | `src/yabai.c:158-162`, inside `#if 0` | `#if 0` branch; `src/misc/autorelease.h` is itself dead code and is commented out of `src/manifest.m:49`. One `DEVIATIONS.md` line |
| `PROFILER_END_TRANSLATION_UNIT` | `src/yabai.c:356` | the `PROFILE` machinery of `src/misc/timer.h`. One `DEVIATIONS.md` line |
| `__src_osax_payload_len`, `__src_osax_loader_len` | `src/sa.h:5`, `:7` | replaced by `.len()` on the two `include_bytes!` slices, which also replace the `xxd -i` step and the two generated `.c` files (`patterns/ffi-objc-and-os.md` §26.2). `src/sa.rs` already spells the slices `OSAX_PAYLOAD` and `OSAX_LOADER`, following `GLOSSARY.md`'s SCREAMING_SNAKE rule for file-scope statics rather than §26.2's literal `__src_osax_*` |
| `sa_payload_init`, `pack`, `sa_payload_send` | `src/sa.m:418-420`, `#undef`'d at `:623-625` | the three frame macros become `ScriptingAdditionPayload` (§7) |
| `extern int csr_get_active_config(uint32_t *config)` | `src/sa.m:3` | not a definition; it lives in `src/ffi/libsystem.rs:5` with `CSR_ALLOW_UNRESTRICTED_FS` and `CSR_ALLOW_TASK_FOR_PID` |

`src/sa.h`'s 21 declarations all have definitions in `src/sa.m`, and `src/workspace.h`'s 10
declarations all have definitions in `src/workspace.m`. Neither header declares anything without
a caller.
