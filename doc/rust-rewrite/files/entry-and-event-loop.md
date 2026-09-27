# Phase 1 map — entry point and event loop

Reader: `entry-and-event-loop`.
Sources read in full: `src/yabai.c`, `src/event_loop.h`, `src/event_loop.c`, `src/mission_control.c`, `src/manifest.m`.
Supporting sources read for thread attribution and type detail (not owned by this reader):
`src/misc/macros.h`, `src/misc/memory_pool.h`, `src/misc/ts.h`, `src/misc/log.h`, `src/misc/timer.h`,
`src/misc/helpers.h`, `src/misc/extern.h`, `src/misc/hashtable.h`, `src/misc/sbuffer.h`, `src/misc/notify.h`,
`src/application.c` (observer callback), `src/process_manager.c` (Carbon handler), `src/mouse_handler.c` (event tap),
`src/message.c` (message loop), `src/event_signal.c` (signal flush), `src/workspace.m` (NSNotification handlers),
`src/window_manager.c`, `src/window.h`, `src/view.h`, `makefile`.

---

## 0. Thread model, and how each attribution was determined

Five kinds of execution context exist in the daemon. Every "which thread" claim below resolves to one of these.

| Tag | What it is | Where it is created | How I determined membership |
| --- | --- | --- | --- |
| **MAIN** | The process main thread running the CFRunLoop / NSRunLoop started by `[NSApp run]` (`src/yabai.c:350`) | process start | Anything whose run-loop source is added with `CFRunLoopGetMain()`, any Carbon `InstallEventHandler`, any `NSNotificationCenter` observer installed before `[NSApp run]`, any `dispatch_get_main_queue()` block. |
| **EVENTLOOP** | One pthread running `event_loop_run` | `pthread_create` at `src/event_loop.c:1718`, from `event_loop_begin`, called on MAIN at `src/yabai.c:291` | Single consumer of the event queue. **Every `EVENT_HANDLER_*` body in `src/event_loop.c` runs here and only here**, because the only call sites are the X-macro switch at `src/event_loop.c:1665` and the one direct call `EVENT_HANDLER_WINDOW_DESTROYED(window, 0)` at `src/event_loop.c:965` (itself inside a handler). |
| **MSGLOOP** | One pthread running `message_loop_run` | `pthread_create` at `src/message.c:3042`, from `message_loop_begin`, called on MAIN at `src/yabai.c:344` | It only does `accept()` then `event_loop_post(..., DAEMON_MESSAGE, NULL, sockfd)` (`src/message.c:3006-3009`). It never touches manager state. |
| **CVLINK / PROXY** | `CVDisplayLink` output thread (`src/window_manager.c:701`) and short-lived proxy-builder pthreads (`src/window_manager.c:666`) | window animation only | Not producers or consumers of the event queue. Listed only because they exist and phase 2 must not assume "two threads". |
| **CHILD** | `fork()`ed children | `src/misc/helpers.h:477` (`exec_config_file`, forked from MAIN at `src/yabai.c:348`) and `src/event_signal.c:64` / `:83` (`event_signal_flush`, forked from EVENTLOOP at `src/event_loop.c:1670`) | `fork()` call sites. Both immediately `execvp("/usr/bin/env", …)` or `exit()`. |

**Producers of events** (all call `event_loop_post`, which is MPSC-safe):

* MAIN — Carbon process events (`src/process_manager.c:183,194,200`), AX observer callbacks (`src/application.c:9-38`), SLS connection notify proc (`src/mission_control.c:10-22`), mission-control AX observer (`src/mission_control.c:62-68`), CGEventTap (`src/mouse_handler.c:34,44,61,67`), display reconfiguration callback (`src/display.c:9-15`), NSWorkspace/NSNotification/NSDistributedNotification observers (`src/workspace.m:232-300`), and `dispatch_after` blocks onto the main queue (`src/event_loop.c:93,157,1478,1516,1520`).
* MSGLOOP — `DAEMON_MESSAGE` only.
* EVENTLOOP — re-posts to itself (`src/event_loop.c:167,917`, and `src/window_manager.c:1467`).

**Consumer**: EVENTLOOP only.

Everything the managers own (`g_window_manager`, `g_space_manager`, `g_display_manager`, `g_process_manager` tables, views, windows) is effectively single-threaded on EVENTLOOP, with a small set of explicitly atomic cross-thread flags listed in §3.2. This is the single most important fact for the Rust port: **the manager graph does not need `Arc<Mutex<…>>`; it needs to live behind one owner on the event-loop thread.**

---

## 1. `src/yabai.c`

### 1.1 Purpose

Defines every global the unity build shares, parses the command line, does one-time process configuration
(lock file, private-framework symbol resolution, cached window levels, signal disposition), starts each
subsystem in a fixed order, then hands the main thread to `[NSApp run]`. It is textually last in the unity
build (`src/manifest.m:97`) so that it can define globals other translation units declared `extern`.

### 1.2 `#define` constants

| Name | Value | Line | Notes for Rust |
| --- | --- | --- | --- |
| `SA_SOCKET_PATH_FMT` | `"/tmp/yabai-sa_%s.socket"` | 1 | `%s` is `$USER`. |
| `SOCKET_PATH_FMT` | `"/tmp/yabai_%s.socket"` | 2 | same |
| `LCFILE_PATH_FMT` | `"/tmp/yabai_%s.lock"` | 3 | same |
| `SCRPT_ADD_LOAD_OPT` | `"--load-sa"` | 5 | |
| `SCRPT_ADD_UNINSTALL_OPT` | `"--uninstall-sa"` | 6 | |
| `SERVICE_INSTALL_OPT` | `"--install-service"` | 7 | |
| `SERVICE_UNINSTALL_OPT` | `"--uninstall-service"` | 8 | |
| `SERVICE_START_OPT` | `"--start-service"` | 9 | |
| `SERVICE_RESTART_OPT` | `"--restart-service"` | 10 | |
| `SERVICE_STOP_OPT` | `"--stop-service"` | 11 | |
| `CLIENT_OPT_LONG` / `CLIENT_OPT_SHRT` | `"--message"` / `"-m"` | 12-13 | |
| `CONFIG_OPT_LONG` / `CONFIG_OPT_SHRT` | `"--config"` / `"-c"` | 14-15 | |
| `DEBUG_VERBOSE_OPT_LONG` / `_SHRT` | `"--verbose"` / `"-V"` | 16-17 | |
| `VERSION_OPT_LONG` / `_SHRT` | `"--version"` / `"-v"` | 18-19 | |
| `HELP_OPT_LONG` / `_SHRT` | `"--help"` / `"-h"` | 20-21 | |
| `MAJOR` / `MINOR` / `PATCH` | `7` / `1` / `25` | 23-25 | Printed as `yabai-v7.1.25`. Rust: keep as three `u32` consts, **not** `env!("CARGO_PKG_VERSION")`, so the string format is byte-identical (`--version` output is parsed by `makefile:45-46` and `scripts/install.sh`). |

There are no structs, enums, unions, typedefs or X-macros defined in this file.

### 1.3 Globals (all defined here; every other `.c` reaches them via `extern`)

| Name | Type | Initial value | Threads | Synchronisation today |
| --- | --- | --- | --- | --- |
| `g_signal_event` (27) | `struct signal *[SIGNAL_TYPE_COUNT]` | zero (BSS) | EVENTLOOP writes (message handlers), EVENTLOOP + CHILD read (`src/event_signal.c:76,80`) | none; CHILD reads a post-`fork` copy |
| `g_process_manager` (28) | `struct process_manager` | zero | MAIN writes (`src/process_manager.c:182,190,223`), EVENTLOOP reads/writes (`src/event_loop.c:377-385`) | **none** — genuine data race on the `process` hash table between the Carbon handler on MAIN and the handlers on EVENTLOOP. Preserve the structure, but this is a bug worth recording. |
| `g_display_manager` (29) | `struct display_manager` | zero | EVENTLOOP | none needed |
| `g_window_manager` (30) | `struct window_manager` | zero | EVENTLOOP; `update_window_notifications` also reads `window` / `insert_feedback` tables on EVENTLOOP only | none needed |
| `g_space_manager` (31) | `struct space_manager` | zero | EVENTLOOP | none needed |
| `g_signal_storage` (32) | `struct memory_pool` | zero, then `memory_pool_init(…, KILOBYTES(256))` at `yabai.c:283` | EVENTLOOP (`event_signal_push` uses `__sync_fetch_and_add` on `.used`, `src/event_signal.c:107`), CHILD reads | atomic add on `.used` only |
| `g_mouse_state` (33) | `struct mouse_state` | zero, then `mouse_state_init` at `yabai.c:155` | `.modifier` is `volatile uint8_t` read on MAIN in the event tap; everything else EVENTLOOP | `volatile` only |
| `g_event_loop` (34) | `struct event_loop` | zero, then `event_loop_begin` at `yabai.c:291` | see §3 | lock-free |
| `g_workspace_context` (35) | `void *` (really `workspace_context *`, an ObjC object) | NULL, set by `workspace_event_handler_begin` at `yabai.c:295` | MAIN (KVO callbacks), EVENTLOOP (`src/event_loop.c:104,115,123,134`) | none; the ObjC object is `alloc`+`init`, never released |
| `g_mission_control_mode` (37) | `enum mission_control_mode` | `MISSION_CONTROL_MODE_INACTIVE` (0) | written EVENTLOOP only (`src/event_loop.c:1455,1462,1469,1476,1542`), read EVENTLOOP (`mission_control_is_active`) | none needed |
| `g_cv_host_clock_frequency` (38) | `double` | `CVGetHostClockFrequency()` at 144 | written MAIN once, read CVLINK (`src/window_manager.c:545`) | publication-before-thread-start |
| `g_layer_normal_window_level` (39) | `int` | `CGWindowLevelForKey(LAYER_NORMAL)` at 145 | written MAIN once, read EVENTLOOP | same |
| `g_layer_below_window_level` (40) | `int` | `CGWindowLevelForKey(LAYER_BELOW)` at 146 | same | same |
| `g_layer_above_window_level` (41) | `int` | `CGWindowLevelForKey(LAYER_ABOVE)` at 147 | same | same |
| `g_event_bytes` (42) | `uint8_t *` | `malloc(0x100)` + `memset 0` at 141-142 | EVENTLOOP (`src/window_manager.c:1280-1317` builds a 0xf8-byte SLPS event record in it) | none; single-threaded in practice. **Never freed.** |
| `g_sa_socket_file` (44) | `char[512]` (`MAXLEN`) | `""`, formatted at 135 | MAIN writes, EVENTLOOP reads (`src/sa.m:247,429`); also rewritten at `src/sa.m:158` | none |
| `g_socket_file` (45) | `char[512]` | `""`, formatted at 136 | MAIN only | — |
| `g_config_file` (46) | `char[4096]` | `""`, may be set by `-c` at 253 | MAIN only | — |
| `g_lock_file` (47) | `char[512]` | `""`, formatted at 137 | MAIN only | — |
| `g_bs_port` (49) | `mach_port_t` | set by `task_get_special_port` at 156 | MAIN writes once, EVENTLOOP reads (`src/window_manager.c:440`) | publication |
| `g_connection` (50) | `int` | `SLSMainConnectionID()` at 143 | MAIN writes once, read by MAIN and EVENTLOOP everywhere | publication |
| `g_verbose` (51) | `bool` | `false`, set by `-V` at 248 | written MAIN, read from every thread via `debug()` (`src/misc/log.h:9`) | none |
| `g_pid` (52) | `pid_t` | `getpid()` at 140 | MAIN | — |

Two more file-scope globals live in `src/misc/extern.h:4-5` but are **assigned here** (`yabai.c:148-149`):
`CGSGetConnectionPortById` (`mach_port_t (*)(int)`) and
`SLSPerformAsynchronousBridgedWindowManagementOperation` (`int64_t (*)(void *)`), both `static`, both NULL until
`macho_find_symbol` resolves them, both read on EVENTLOOP (`src/window.c:946,956`, `src/space_manager.c:667,672,688,693`).
Both may legitimately stay NULL, and every call site null-checks.

### 1.4 Functions

#### `static int client_send_message(int argc, char **argv)` — `yabai.c:54-124`

Serialises `argv[1..argc-1]` into a length-prefixed NUL-separated blob, connects to the daemon's UNIX socket and
streams the reply to stdout/stderr. Runs on **MAIN**, in the *client* invocation of the binary (no daemon state exists).

Wire format, which the Rust client must reproduce byte for byte:

```
[int32 message_length][arg1]\0[arg2]\0…[argN]\0\0
```

`message_length` (line 65) starts at `argc` and accumulates `strlen(argv[i])` for `i` in `1..argc`. That means it
equals `sum(len) + (argc-1) NULs + 1 terminating NUL`, i.e. exactly the number of bytes after the header. The
`int` header is **not** counted in `message_length` but **is** sent (`send(..., sizeof(int)+message_length, 0)`, line 96).
Note `argl[0]` (line 66) is never written and never read.

Allocation: `malloc(sizeof(int)+message_length)` at line 73, freed at line 101 after `send`. `int argl[argc]` at
line 66 is a VLA on the stack. `char rsp[BUFSIZ]` at 106.

Reply protocol: read until EOF; if the **first byte of a chunk** equals `FAILURE_MESSAGE[0]` (`'\x07'`,
`src/misc/macros.h:18`) the exit code flips to `EXIT_FAILURE`, output switches to stderr *for the rest of the call*,
and that one leading byte is skipped (`rsp + 1`, line 114). Subsequent chunks are printed whole even though
`output` is still stderr. This per-chunk (not per-message) behaviour must be preserved.

External symbols: `getenv`, `strlen`, `malloc`, `memcpy`, `free`, `snprintf`, `socket`/`connect` (via
`socket_open`/`socket_connect`), `send`, `shutdown`, `read`, `fprintf`, `fflush`, `close`.

Error paths call `error(…)` (`src/misc/log.h:27`) which prints to stderr and `exit(EXIT_FAILURE)` — leaking
`message` and the fd on purpose.

#### `static inline bool configure_settings_and_acquire_lock(void)` — `yabai.c:128-178`

One-time process setup; returns whether the advisory write lock on `/tmp/yabai_$USER.lock` was taken.
Runs on **MAIN** before any other thread exists. Wrapped in
`#pragma clang diagnostic ignored "-Wdeprecated-declarations"` (126-127 / 179) because of `NSApplicationLoad`,
`CGSetLocalEventsSuppressionInterval` and `CGEnableEventStateCombining`.

Order matters and must be preserved exactly:

1. `getenv("USER")` → three `snprintf`s into `g_sa_socket_file`, `g_socket_file`, `g_lock_file` (135-137).
2. `NSApplicationLoad()` (139) — required before touching AppKit from a non-bundled binary; also creates `NSApp`, which `main` later calls `run` on.
3. `g_pid = getpid()` (140).
4. `g_event_bytes = malloc(0x100); memset(…, 0, 0x100)` (141-142).
5. `g_connection = SLSMainConnectionID()` (143).
6. `g_cv_host_clock_frequency = CVGetHostClockFrequency()` (144).
7. Three `CGWindowLevelForKey` calls (145-147).
8. Two `macho_find_symbol` lookups into SkyLight (148-149). The second is a mangled C++ symbol:
   `__ZL54SLSPerformAsynchronousBridgedWindowManagementOperationP47SLSAsynchronousBridgedWindowManagementOperation`.
9. `signal(SIGCHLD, SIG_IGN)` and `signal(SIGPIPE, SIG_IGN)` (151-152).
10. `CGSetLocalEventsSuppressionInterval(0.0f)`, `CGEnableEventStateCombining(false)` (153-154).
11. `mouse_state_init(&g_mouse_state)` (155).
12. `task_get_special_port(mach_task_self(), TASK_BOOTSTRAP_PORT, &g_bs_port)` (156).
13. `#if 0` block (158-162) — three disabled `hook_*` calls; `src/misc/autorelease.h` is also commented out in the manifest. Dead code; do not port.
14. `open(g_lock_file, O_CREAT|O_WRONLY|O_CLOEXEC, 0600)` (164) then `fcntl(handle, F_SETLK, &lockfd)` with
    `struct flock { l_start=0, l_len=0, l_pid=g_pid, l_type=F_WRLCK, l_whence=SEEK_SET }` (169-177).
    **The fd is deliberately never closed** — closing any fd on the file would drop the lock. In Rust the `File`
    must be `std::mem::forget`ed or stored in a `static`, never dropped.

External symbols: `getenv`, `snprintf`, `NSApplicationLoad`, `getpid`, `malloc`, `memset`, `SLSMainConnectionID`,
`CVGetHostClockFrequency`, `CGWindowLevelForKey`, `signal`, `CGSetLocalEventsSuppressionInterval`,
`CGEnableEventStateCombining`, `task_get_special_port`, `mach_task_self`, `open`, `fcntl`.

#### `static void parse_arguments(int argc, char **argv)` — `yabai.c:181-258`

Dispatches the "do one thing and exit" options, then scans the rest for `-V` / `-c`. Runs on **MAIN**.
Called only when `argc > 1` (`yabai.c:263`), so the unguarded `argv[1]` at line 183 is safe.

Behaviour worth pinning down:

* Only `argv[1]` is tested against `--help/-h`, `--version/-v`, `--message/-m`, `--load-sa`, `--uninstall-sa` and
  the four `--*-service` options (183-241). `yabai -V -m window --focus next` therefore does **not** go to the
  client path; it falls into the trailing loop and dies with `"yabai: '-m' is not a valid option!"`.
* `--message` recurses as `client_send_message(argc-1, argv+1)` (212), so the client's `argv[0]` is the literal
  `"-m"`/`"--message"` and is skipped by the `i = 1` loop.
* The trailing loop (243-257) starts at `i = 1`, so if `argv[1]` was `-V` it is reprocessed — harmless.
* `-c` with no following argument calls `error(…)` → exit 1 (252).
* Anything unrecognised calls `error(…)` → exit 1 (255).
* Help text is a single `fprintf` with `%d.%d.%d` substituted three times at the tail (185-200). Reproduce verbatim.

External symbols: `fprintf`, `exit`, `snprintf`.

#### `int main(int argc, char **argv)` — `yabai.c:261-353`, guarded by `#ifndef TESTS` (260/354)

The startup sequence. Runs on **MAIN**. `-DTESTS` (see `tests/makefile:10`) compiles this out; the Rust crate
does not need that switch unless the test harness is ported.

Exact order (each failure is fatal):

1. `parse_arguments` if `argc > 1` (263-265).
2. `is_root()` → `require("yabai: running as root is not allowed! abort..\n")`. **`require` exits with `EXIT_SUCCESS`** (`src/misc/log.h:38-46`), not failure. Same for the next two checks. This matters for launchd's `KeepAlive` semantics.
3. `ax_privilege()` (271) — pops the accessibility prompt via `AXIsProcessTrustedWithOptions`.
4. `SLSGetSpaceManagementMode(SLSMainConnectionID()) == 1` (275) — "displays have separate spaces".
5. `ts_init(MEGABYTES(8))` (279) — 8 MiB temp arena, `error` on failure.
6. `memory_pool_init(&g_signal_storage, KILOBYTES(256))` (283).
7. `configure_settings_and_acquire_lock()` (287).
8. `event_loop_begin(&g_event_loop)` (291) — **starts EVENTLOOP**.
9. `workspace_event_handler_begin(&g_workspace_context)` (295) — sets the `workspace_is_macos_*` flags and installs the NSNotification observers.
10. `process_manager_begin(&g_process_manager)` (299).
11. `display_manager_begin(&g_display_manager)` (303).
12. `mouse_handler_begin(&g_mouse_state, MOUSE_EVENT_MASK)` (307).
13. Version-gated notification wiring (311-334), detailed in §1.5.
14. `window_manager_init`, `space_manager_begin`, `window_manager_begin` (336-338).
15. `update_window_notifications()` on Sequoia/Tahoe (340-342) — **calls a `static` function defined in `src/event_loop.c:16`**; legal only because of the unity build.
16. `message_loop_begin(g_socket_file)` (344) — **starts MSGLOOP**.
17. `exec_config_file(g_config_file, sizeof(g_config_file))` (348) — `fork()` + `execvp("/usr/bin/env", …)` from MAIN.
18. `[NSApp run]` (350) — never returns in practice; `return 0` at 352 is unreachable.

Note that steps 9-16 all run **after** EVENTLOOP is live, so events can already be queued and handled while the
managers are still being constructed.

#### `PROFILER_END_TRANSLATION_UNIT` — `yabai.c:356`

Expands to a `_Static_assert` on `__COUNTER__` when `PROFILE >= 2`, and to nothing otherwise
(`src/misc/timer.h:149,154,162`). No Rust equivalent needed.

### 1.5 OS callbacks registered from this file

| Registration | Line | Callback | Fires on | Context | Context lifetime |
| --- | --- | --- | --- | --- | --- |
| `signal(SIGCHLD, SIG_IGN)` | 151 | — | — | — | process lifetime; auto-reaps the `fork`ed config/signal children |
| `signal(SIGPIPE, SIG_IGN)` | 152 | — | — | — | process lifetime; keeps a client disconnect from killing the daemon |
| `mission_control_observe()` | 316 | `mission_control_notification_handler` (`src/mission_control.c:59`) | MAIN | `NULL` | — |
| `SLSRegisterConnectionNotifyProc(g_connection, connection_handler, 1327, NULL)` | 322 | `connection_handler` (`src/mission_control.c:7`) | MAIN | `NULL` | — |
| … `, 1328, NULL)` | 323 | same | MAIN | `NULL` | — |
| … `, 1204, NULL)` (pre-Monterey path) | 326 | same | MAIN | `NULL` | — |
| … `, 808, NULL)` | 329 | same | MAIN | `NULL` | — |
| … `, 1202, NULL)` | 330 | same | MAIN | `NULL` | — |
| … `, 804, NULL)` (Sequoia/Tahoe) | 333 | same | MAIN | `NULL` | — |

Version gating (311-334), verbatim in behaviour:

* Monterey **or** Ventura **or** Sonoma **or** Sequoia **or** Tahoe → `mission_control_observe()`; and within that,
  Ventura **or** Sonoma **or** Sequoia **or** Tahoe → register 1327 and 1328.
* Otherwise (Big Sur) → register 1204.
* Always → register 808 and 1202.
* Sequoia **or** Tahoe → additionally register 804.

Note the asymmetry: on **Monterey only**, `mission_control_observe()` runs but neither 1327/1328 nor 1204 is
registered, so `MISSION_CONTROL_ENTER` never fires there and only the AX Expose notifications drive mission-control
state. Preserve this; it is not obviously intentional but it is the current behaviour.

The `connection_handler` symbol is `static` in `src/mission_control.c` and referenced here — unity build again.

`[NSApp run]` (350) is itself the registration of the main run loop; every MAIN callback in the table above and in
§3.5/§4.4 is delivered by it.

---

## 2. `src/event_loop.h`

### 2.1 Purpose

Declares the event taxonomy as an X-macro list, the two queue node types, and the two public entry points
(`event_loop_begin`, `event_loop_post`). It is included before any `.c` (`src/manifest.m:65`) so the enum is visible
to every producer.

### 2.2 Macros

**`EVENT_HANDLER(event_type)` — line 4**

```c
#define EVENT_HANDLER(event_type) void EVENT_HANDLER_##event_type(void *context, int param1)
```

Every handler therefore has the uniform signature `void (*)(void *context, int param1)`. `context` is used as
*either* an owning pointer, a borrowed pointer, or an integer stuffed through `(void *)(intptr_t)` — see the
ownership table in §3.6. `param1` is used by exactly two handlers (`MOUSE_DOWN` reads the modifier byte,
`DAEMON_MESSAGE` reads the socket fd).

**`EVENT_TYPE_LIST` — lines 6-46**

X-macro list of 41 entries, expanded twice:

* `src/event_loop.h:50-52` — into `enum event_type` members, so the numeric values are the list order:
  `APPLICATION_LAUNCHED = 0` … `DAEMON_MESSAGE = 40`.
* `src/event_loop.c:1665-1667` — into the dispatch `switch`.

Full list in order: `APPLICATION_LAUNCHED`, `APPLICATION_TERMINATED`, `APPLICATION_FRONT_SWITCHED`,
`APPLICATION_VISIBLE`, `APPLICATION_HIDDEN`, `WINDOW_CREATED`, `WINDOW_DESTROYED`, `WINDOW_FOCUSED`,
`WINDOW_MOVED`, `WINDOW_RESIZED`, `WINDOW_MINIMIZED`, `WINDOW_DEMINIMIZED`, `WINDOW_TITLE_CHANGED`,
`SLS_WINDOW_ORDERED`, `SLS_WINDOW_DESTROYED`, `SLS_SPACE_CREATED`, `SLS_SPACE_DESTROYED`, `SPACE_CHANGED`,
`DISPLAY_ADDED`, `DISPLAY_REMOVED`, `DISPLAY_MOVED`, `DISPLAY_RESIZED`, `DISPLAY_CHANGED`, `MOUSE_DOWN`,
`MOUSE_UP`, `MOUSE_DRAGGED`, `MOUSE_MOVED`, `MISSION_CONTROL_SHOW_ALL_WINDOWS`,
`MISSION_CONTROL_SHOW_FRONT_WINDOWS`, `MISSION_CONTROL_SHOW_DESKTOP`, `MISSION_CONTROL_ENTER`,
`MISSION_CONTROL_CHECK_FOR_EXIT`, `MISSION_CONTROL_EXIT`, `DOCK_DID_RESTART`, `MENU_OPENED`, `MENU_CLOSED`,
`MENU_BAR_HIDDEN_CHANGED`, `DOCK_DID_CHANGE_PREF`, `SYSTEM_WOKE`, `DAEMON_MESSAGE`.

(That is 40 entries; the enum has no explicit count member and nothing depends on the numeric values crossing a
process boundary, so the exact ordinals are internal.)

### 2.3 Types

**`enum event_type` — 48-53.** Underlying type is implementation-defined `int`. Rust: `#[repr(C)] enum` is not
needed — nothing passes it to the OS. A plain Rust enum is correct.

**`struct event` — 55-61**

| Field | C type | Ownership | Cross-thread |
| --- | --- | --- | --- |
| `type` | `enum event_type` | value | written by producer with `__ATOMIC_RELEASE` (`event_loop.c:1690`), read by EVENTLOOP with `__ATOMIC_RELAXED` (1664) |
| `param1` | `int` | value | same (1691 / 1665) |
| `context` | `void *` | **varies per event type — see §3.6** | same (1692 / 1665) |
| `next` | `struct event *` | points into the same ring-buffer pool; never freed | written by producer (1693, and CAS at 1698), read by EVENTLOOP (1660) |

The node itself is **not heap-allocated**: it comes from `memory_pool_push(&event_loop->pool, …)`
(`event_loop.c:1689`), a bump allocator over a 512 KiB `mmap` that **wraps around and reuses memory** when full
(`src/misc/memory_pool.h:39-42`). See the hazard note in §6.

**`struct event_loop` — 63-71**

| Field | C type | Ownership | Cross-thread |
| --- | --- | --- | --- |
| `is_running` | `bool` | value | written once on MAIN at `event_loop.c:1717` **before** `pthread_create`; read by EVENTLOOP at 1652. Never set to `false` anywhere in the tree — the loop is infinite. |
| `thread` | `pthread_t` | value | written on MAIN at 1718; never joined |
| `semaphore` | `sem_t *` | owned by the kernel (named POSIX semaphore, immediately `sem_unlink`ed) | `sem_post` from every producer thread, `sem_wait` from EVENTLOOP |
| `pool` | `struct memory_pool` | owns an `mmap` region; never unmapped | `.used` CAS'd by producers (`memory_pool.h:36,40`) |
| `head` | `struct event *` | borrows into `pool` | **written only by EVENTLOOP** (CAS at 1662), read by EVENTLOOP |
| `tail` | `struct event *` | borrows into `pool` | written by producers (CAS at 1700), read by producers (1697) |

`is_running`/`thread` are plain, non-atomic, non-volatile: safe only because of the
before-`pthread_create` publication.

### 2.4 Prototypes

```c
bool event_loop_begin(struct event_loop *event_loop);                                            // 73
void event_loop_post(struct event_loop *event_loop, enum event_type type, void *context, int param1); // 74
```

No `event_loop_end` exists.

---

## 3. `src/event_loop.c`

### 3.1 Purpose

Holds the body of every event handler (the entire reactive core of yabai: application lifecycle, window lifecycle,
space/display changes, mouse drag-and-drop tiling, mission-control state, and the daemon message entry point) plus
the lock-free MPSC queue and the consumer thread that drives them. Structurally it is "one giant `switch`" split
into 40 `static` functions by an X-macro.

### 3.2 Globals and file-scope statics

| Name | Line | Type | Initial | Threads | Synchronisation |
| --- | --- | --- | --- | --- | --- |
| `__pending_window_focus` | 11 | `volatile bool` | `false` | **written MAIN** (`src/application.c:11`, release) and **EVENTLOOP** (422, 638, release); **read EVENTLOOP** (369, relaxed) | `__atomic_*` |
| `__pending_gesture` | 12 | `volatile bool` | `false` | **written MAIN** (`src/mouse_handler.c:76,78`, release); read EVENTLOOP (1351, relaxed) | `__atomic_*` |
| `__last_gesture_time` | 13 | `volatile uint64_t` | `0` | **written MAIN** (`src/mouse_handler.c:79`, release); read EVENTLOOP (1352, relaxed) | `__atomic_*` |
| `__last_cmd_tab_time` | 14 | `volatile uint64_t` | `0` | **written MAIN** (`src/mission_control.c:24`, release); read EVENTLOOP (362, relaxed) | `__atomic_*` |
| `ffm_value` | 1561 | `static enum ffm_mode` | `0` | EVENTLOOP only (1570, 1581) | none |
| `is_menu_open` | 1562 | `static int` | `0` | EVENTLOOP only (1567, 1578, 1580, 1583) | none |

`extern` declarations at lines 1-10 pull in `g_event_loop`, `g_process_manager`, `g_display_manager`,
`g_space_manager`, `g_window_manager`, `g_mouse_state`, `g_mission_control_mode`, `g_connection`,
`g_workspace_context`, `g_layer_below_window_level` — all defined in `src/yabai.c`.

Rust mapping for the four `__`-prefixed flags: `static PENDING_WINDOW_FOCUS: AtomicBool = AtomicBool::new(false);`
etc., with `Ordering::Release` on stores and `Ordering::Relaxed` on loads, matching the C exactly. They are true
cross-thread state and are the **only** shared mutable state in this file.

### 3.3 Helper functions

#### `static void update_window_notifications(void)` — 16-34

Rebuilds the SkyLight per-window notification subscription list. **EVENTLOOP** (called at 246, 340, 599, 632) and
**MAIN** (called once at `src/yabai.c:341`) and **EVENTLOOP** again via `src/view.c:45,111`.

`uint32_t window_list[1024] = {0}` on the stack (19). **There is no bound check** on `window_count` — more than
1024 tracked windows overflows the stack buffer. On Sequoia/Tahoe it iterates every entry of
`g_window_manager.window`; otherwise only `g_window_manager.insert_feedback`, taking `node->window_order[0]`.

Rust: `let mut window_list = [0u32; 1024]` plus an explicit `.min(1024)` guard is the obvious safe translation, but
that **changes behaviour** in the overflow case (from UB to truncation). Recommend a `SmallVec<[u32; 1024]>` or a
reused `Vec<u32>` sized to the table, and note the deviation in the phase-2 notes.

External symbols: `SLSRequestNotificationsForWindows`.

#### `static void window_did_receive_focus(struct window_manager *wm, struct mouse_state *ms, struct window *window)` — 36-71

Applies opacity to old/new focus, optionally centres the mouse, updates `focused_window_id` /
`focused_window_psn` / `last_window_id` / `ffm_window_id`, and rotates the window to the front of its stack node's
`window_order` with a `memmove` (66). **EVENTLOOP** only (called at 420, 671, 1010, 1062).

Note line 40 reads `g_window_manager.normal_window_opacity` through the global while the same object is also the
`wm` parameter — harmless, but a Rust `&mut self` method must not try to take a second borrow.

Allocates nothing. External symbols: `memmove`, plus yabai-internal `window_space`, `window_manager_*`,
`view_find_window_node`.

### 3.4 Event handlers

All 40 run on **EVENTLOOP**. All are wrapped by one `#pragma clang diagnostic ignored "-Wunused-parameter"`
spanning 73-74 through 1645; `APPLICATION_FRONT_SWITCHED` additionally sits inside
`ignored "-Wdeprecated-declarations"` (347-348 / 424) for `GetCurrentEventTime`.

| Handler | Lines | One-line behaviour | Allocates / frees | Notable externals |
| --- | --- | --- | --- | --- |
| `APPLICATION_LAUNCHED` | 75-248 | Waits for `ns_application`, `finishedLaunching` and an observable activation policy (re-posting itself via `dispatch_after` 0.1 s if not ready), then creates+observes the `struct application`, adopts its windows and batch-flushes the affected views | `application_create` (146) owned by `g_window_manager`; `ts_alloc_list(struct view *, window_count)` (191) — temp arena, reclaimed by `ts_reset()` at 1671; `application_destroy` on the failure path (152) | `dispatch_after`, `dispatch_time`, `dispatch_get_main_queue`, ObjC `observationInfo` / `removeObserver:forKeyPath:context:`, `@try/@catch` |
| `APPLICATION_TERMINATED` | 250-345 | Tears down the application and all of its windows, unmanages them, emits signals, then `process_destroy(process)` at the shared `out:` label | **takes ownership of `context`** (`struct process *`) and frees it at 344; frees each `struct window` via `window_destroy` (315); `ts_alloc_list` (275) | `buf_len`/`buf_del` on `applications_to_refresh`, `__sync_bool_compare_and_swap` on `window->id_ptr` (280) |
| `APPLICATION_FRONT_SWITCHED` | 349-423 | Records the activated/deactivated app, optionally suppresses the space-switch animation, refreshes unresolved windows, then focuses the app's focused window | none beyond `ts` | `GetCurrentEventTime`, `AXUIElementCopyAttributeValue(app->ref, CFSTR("__fence"), &dummy)` (366) — a deliberate synchronous AX round-trip used as a fence; `SLSSpaceSetFrontPSN` |
| `APPLICATION_VISIBLE` | 426-488 | Clears `is_hidden`, re-tiles the app's windows, batch flush | `ts_alloc_list` (439) | — |
| `APPLICATION_HIDDEN` | 490-549 | Sets `is_hidden`, untiles and purifies the app's windows, batch flush | `ts_alloc_list` (502) | — |
| `WINDOW_CREATED` | 551-601 | Turns a retained `AXUIElementRef` into a tracked `struct window`, applies+expires one-shot rules, tiles it | **consumes the `+1` `CFRetain` from `src/application.c:9`**: `CFRelease(context)` on each of the four early returns (554, 557, 560, 563); on the success path the ref is handed to `window_create` inside `window_manager_create_and_add_window` (`src/window_manager.c:1440`) which owns it, and the `if (!window) return;` at 566 is correct because that function already destroyed the window (and its ref) | `ax_window_id`, `ax_window_pid`, `CFRelease` |
| `WINDOW_DESTROYED` | 603-634 | Untiles, signals, removes from scratchpad/table, unobserves and frees the window | **frees `context`** via `window_destroy` (629) | — |
| `WINDOW_FOCUSED` | 636-673 | Clears `__pending_window_focus`, validates liveness, optionally forces the space, then `window_did_receive_focus` | — | `__sync_bool_compare_and_swap` liveness check (647), `SLSSpaceSetFrontPSN` |
| `WINDOW_MOVED` | 675-723 | Debounces against the cached origin, updates `frame.origin`, and re-flushes the node when the AX origin drifted ≥1.5 pt from the layout | — | `CGPointEqualToPoint`, `CGRectEqualToRect` |
| `WINDOW_RESIZED` | 725-827 | Same as above for the full frame, plus native-fullscreen enter/exit transitions and role/subrole refresh | `CFRelease` old `role`/`subrole` (772, 775) then re-`copy` | `CGRectEqualToRect`, `CFRelease` |
| `WINDOW_MINIMIZED` | 829-871 | Sets `WINDOW_MINIMIZE`, refreshes movable/resizable/role/subrole, untiles | `CFRelease` role/subrole (853, 856) | — |
| `WINDOW_DEMINIMIZED` | 873-922 | Inverse; re-tiles if the window landed on the active space, replays a lost focus event | `CFRelease` role/subrole (897, 900) | — |
| `WINDOW_TITLE_CHANGED` | 924-942 | Replaces `window->title` | `CFRelease` old title (937) | — |
| `SLS_WINDOW_ORDERED` | 944-950 | Re-orders an insert-feedback overlay above its window | — | `SLSOrderWindow` |
| `SLS_WINDOW_DESTROYED` | 952-966 | Liveness-checks and **tail-calls `EVENT_HANDLER_WINDOW_DESTROYED(window, 0)`** directly (965) | delegates | — |
| `SLS_SPACE_CREATED` | 968-978 | Materialises a view for space types 0 and 4 | `space_manager_find_view` may allocate a `struct view` | `SLSSpaceGetType` |
| `SLS_SPACE_DESTROYED` | 980-992 | Removes the label, removes the table entry, `view_destroy` + `free(view)` (988-989) | **frees the `struct view`** | `free` |
| `SPACE_CHANGED` | 994-1029 | Rotates `last/current_space_id`, re-applies menubar alpha, refreshes app windows, validates and flushes the new view | — | `SLSSetMenuBarInsetAndAlpha` |
| `DISPLAY_CHANGED` | 1031-1081 | Same as `SPACE_CHANGED` for displays, with two "ignore this event" guards (1034, 1046) | — | `SLSSetMenuBarInsetAndAlpha` |
| `DISPLAY_ADDED` | 1083-1090 | `space_manager_handle_display_add` + `window_manager_handle_display_add_and_remove` | — | — |
| `DISPLAY_REMOVED` | 1092-1099 | Drops the display label and re-homes windows onto the main display | — | — |
| `DISPLAY_MOVED` | 1101-1107 | Marks every space invalid | — | — |
| `DISPLAY_RESIZED` | 1109-1115 | Marks the display's spaces invalid | — | — |
| `MOUSE_DOWN` | 1117-1152 | Latches the window under the cursor, the modifier and the resize handle quadrant; `goto out` → `CFRelease(context)` | **consumes the `CFRetain` from `src/mouse_handler.c:34`** at 1151 | `CGEventGetLocation`, `CGEventGetIntegerValueField`, `CGRectGetMidX/Y`, `CFRelease` |
| `MOUSE_UP` | 1154-1233 | Resolves the drop: stack / swap / warp in four directions, or "no target", or a BSP grid adjust; three-label cleanup `err:`/`res:`/`out:` | consumes the `CFRetain` at 1232 | `CGEventGetLocation`, `CFRelease` |
| `MOUSE_DRAGGED` | 1235-1343 | Moves or resizes the dragged window (throttled to ~14.8 Hz for resize) and updates the insert-feedback overlay | consumes the `CFRetain` at 1244 (early path) or 1342 | `CGEventGetLocation`, `CFRelease`, `scripting_addition_move_window` |
| `MOUSE_MOVED` | 1345-1450 | Focus-follows-mouse: autofocus (forwarding to an `AXSheet`/`AXDrawer` child) or autoraise (suppressed when a floating window would be occluded), or display focus when over empty desktop | consumes the `CFRetain` at 1449; `CFRelease(window_list)` (1393) and `CFRelease(role)` (1385) | `SLSCopyAssociatedWindows`, `CFArrayGetCount`, `CFArrayGetValueAtIndex`, `CFNumberGetValue`, `CFEqual`, `CGRectContainsRect`, `CFRelease` |
| `MISSION_CONTROL_SHOW_ALL_WINDOWS` | 1452-1457 | Sets mode 2 and signals | — | — |
| `MISSION_CONTROL_SHOW_FRONT_WINDOWS` | 1459-1464 | Sets mode 3 and signals | — | — |
| `MISSION_CONTROL_SHOW_DESKTOP` | 1466-1471 | Sets mode 4 and signals | — | — |
| `MISSION_CONTROL_ENTER` | 1473-1483 | Sets mode 1 and arms the 0.1 s exit poll | schedules a `dispatch_after` block | `dispatch_after` |
| `MISSION_CONTROL_CHECK_FOR_EXIT` | 1485-1526 | Polls the on-screen window list for the Dock's unnamed layer-18 window; re-arms at 0.1 s while present, posts `MISSION_CONTROL_EXIT` at 0 s once gone | `CGWindowListCopyWindowInfo` released at 1525; early `return` at 1487 happens **before** the copy, so no leak | `CGWindowListCopyWindowInfo`, `CFArrayGetCount`, `CFDictionaryGetValue`, `CFNumberGetType`, `CFNumberGetValue`, `CFEqual`, `CFRelease`, `dispatch_after` |
| `MISSION_CONTROL_EXIT` | 1528-1543 | Re-applies menubar alpha, reconciles window positions the Dock moved, signals, resets the mode to 0 | — | `SLSSetMenuBarInsetAndAlpha` |
| `DOCK_DID_RESTART` | 1545-1559 | Re-installs the mission-control AX observer on Monterey+ | `mission_control_unobserve` releases two CF refs; `mission_control_observe` creates two | see §4 |
| `MENU_OPENED` | 1564-1573 | Depth-counts open menus; on the first, saves and disables FFM | — | — |
| `MENU_CLOSED` | 1575-1585 | Decrements; on zero restores FFM; clamps negative to zero | — | — |
| `MENU_BAR_HIDDEN_CHANGED` | 1587-1592 | Marks spaces invalid, signals | — | — |
| `DOCK_DID_CHANGE_PREF` | 1594-1599 | Marks spaces invalid, signals | — | — |
| `SYSTEM_WOKE` | 1601-1612 | Re-applies active opacity and re-centres the mouse on the focused window | — | — |
| `DAEMON_MESSAGE` | 1614-1644 | Reads the `int` length prefix and the body off `param1`, `fdopen`s the fd for writing, runs `handle_message`, flushes and closes | `ts_alloc_unaligned(bytes_to_read)` (1623); `fdopen`/`fclose` **take and close the fd** (1632, 1637); the `return` at 1639 deliberately skips `socket_close` | `read`, `fdopen`, `fflush`, `fclose`, `socket_close` |

### 3.5 Callbacks registered from this file

This file registers no OS callback directly. It schedules five GCD blocks onto the **main queue**, which is the
only way EVENTLOOP hands work back to MAIN:

| Site | Delay | Captured context | Lifetime of the capture | Block body |
| --- | --- | --- | --- | --- |
| 93-96 | 0.1 s | `__block ProcessSerialNumber psn = process->psn` (92) — copied **by value**, deliberately, so a dead `process` cannot be dereferenced; the block re-looks-up by PSN | block-owned copy | re-post `APPLICATION_LAUNCHED` |
| 157-160 | 0.1 s | same pattern (156) | block-owned copy | re-post `APPLICATION_LAUNCHED` |
| 1478-1480 | 0.1 s | none | — | post `MISSION_CONTROL_CHECK_FOR_EXIT` |
| 1516-1518 | 0.1 s | none | — | post `MISSION_CONTROL_CHECK_FOR_EXIT` |
| 1520-1522 | 0.0 s | none | — | post `MISSION_CONTROL_EXIT` |

`0.1f * NSEC_PER_SEC` is evaluated in `float`: `0.1f` is `0.100000001490116…`, times `1000000000` rounds to exactly
`100000000.0f`, so the delay is 100 ms. A Rust translation using `Duration::from_millis(100)` is exact.

### 3.6 `context` ownership per event type (the table phase 2 will need most)

| Event | `context` holds | Producer | Who releases |
| --- | --- | --- | --- |
| `APPLICATION_LAUNCHED` | `struct process *` (borrowed; owned by `g_process_manager.process`) | `src/process_manager.c:183`, `src/workspace.m:232,256`, self-repost 95/159 | nobody (table owns it) |
| `APPLICATION_TERMINATED` | `struct process *` (**moved**; already removed from the table at `src/process_manager.c:190`) | `src/process_manager.c:194` | the handler, `process_destroy` at 344 |
| `APPLICATION_FRONT_SWITCHED` | `struct process *` (borrowed) | `src/process_manager.c:200`, self-repost 167 | nobody |
| `APPLICATION_VISIBLE` / `APPLICATION_HIDDEN` | `(void *)(intptr_t) pid_t` — **an integer, not a pointer** | `src/workspace.m:294,300` | — |
| `WINDOW_CREATED` | `AXUIElementRef`, **+1 retain owned by the event** | `src/application.c:9` (`CFRetain(element)`) | the handler (4 early `CFRelease`s) or `window_create` on success |
| `WINDOW_DESTROYED` | `struct window *` (**moved**) | `src/application.c:38`, and 965 | the handler, `window_destroy` at 629 |
| `WINDOW_FOCUSED` / `WINDOW_MOVED` / `WINDOW_RESIZED` / `WINDOW_TITLE_CHANGED` | `(void *)(intptr_t) uint32_t wid` | `src/application.c:12,14,16,18`, 917, `src/window_manager.c:1467` | — |
| `WINDOW_MINIMIZED` / `WINDOW_DEMINIMIZED` | `struct window *` (borrowed — it is the AXObserver's `refcon`) | `src/application.c:24,26` | nobody |
| `SLS_WINDOW_ORDERED` / `SLS_WINDOW_DESTROYED` | `(void *)(intptr_t) uint32_t wid` | `src/mission_control.c:19,22` | — |
| `SLS_SPACE_CREATED` / `SLS_SPACE_DESTROYED` | `(void *)(intptr_t) uint64_t sid` | `src/mission_control.c:13,16` | — |
| `SPACE_CHANGED` / `DISPLAY_CHANGED` | `NULL` | `src/workspace.m:283,288` | — |
| `DISPLAY_ADDED/REMOVED/MOVED/RESIZED` | `(void *)(intptr_t) uint32_t did` | `src/display.c:9-15` | — |
| `MOUSE_DOWN/UP/DRAGGED/MOVED` | `CGEventRef`, **+1 retain owned by the event**; `param1` = modifier byte for DOWN/MOVED | `src/mouse_handler.c:34,44,61,67` | the handler, at its `out:`/`err:` label |
| `MISSION_CONTROL_*` | `NULL` | `src/mission_control.c:10,62-68`, 1479/1517/1521 | — |
| `DOCK_DID_RESTART` / `DOCK_DID_CHANGE_PREF` / `MENU_BAR_HIDDEN_CHANGED` / `SYSTEM_WOKE` / `MENU_CLOSED` | `NULL` | `src/workspace.m:263-278`, `src/application.c:22` | — |
| `MENU_OPENED` | `(void *)(intptr_t) wid` — **computed but never read** by the handler | `src/application.c:20` | — |
| `DAEMON_MESSAGE` | `NULL`; `param1` = accepted socket fd, **ownership moved to the handler** | `src/message.c:3009` | the handler, via `fclose` (1637) or `socket_close` (1643) |

Rust translation: this begs for `enum Event { ApplicationLaunched(ProcessRef), WindowCreated(RetainedAxUiElement),
MouseDown { event: RetainedCGEvent, modifier: u8 }, DaemonMessage(OwnedFd), … }` — a tagged union with the
ownership encoded in the payload type, so `Drop` replaces the hand-written `CFRelease`/`free`/`close` paths. That is
a *shape* change but not a *behaviour* change, and it removes an entire class of the leaks the C version is prone to.

### 3.7 Queue machinery

#### `static void *event_loop_run(void *context)` — 1647-1682

Runs on **EVENTLOOP**. Structure:

```
while (is_running) {
    pool = [[NSAutoreleasePool alloc] init];
    for (;;) {
        profile_begin();
        do { head = load(head); next = load(head->next); if (!next) goto empty; }
        while (!CAS(&head, head, next));
        switch (load(next->type)) { … EVENT_HANDLER_x(load(next->context), load(next->param1)); … }
        event_signal_flush();
        ts_reset();
        profile_end_and_print();
    }
empty:
    [pool drain];
    sem_wait(semaphore);
}
```

Points that must survive translation:

* **Dummy-head MPSC queue.** `head` is a sentinel node; the node actually dequeued is `head->next`, and the old
  `head` becomes garbage (never reclaimed — the pool ring reclaims it eventually).
* **Single consumer.** The `do…while(!CAS)` on `head` (1658-1662) can never fail; it is a leftover. A Rust port with
  a real channel does not need it.
* **`ts_reset()` after every event** (1671) resets the 8 MiB temp arena to zero used, so every `ts_alloc*` pointer
  from inside a handler dies at the end of that handler. This is the arena discipline the whole codebase relies on.
* **`event_signal_flush()` after every event** (1670) `fork()`s if anything was pushed.
* **One autorelease pool per drain cycle**, not per event: it is created when the loop wakes and drained only when
  the queue runs dry (1653 / 1677). Under sustained event pressure the pool grows unbounded. Preserve the shape:
  in Rust, `objc2::rc::autoreleasepool` around the inner drain loop.
* `switch` has no `default:` arm; all 40 enum values are generated by the X-macro.
* `profile_begin()` / `profile_end_and_print()` expand to nothing (plus a stray `;`) unless `-DPROFILE>=1`
  (`src/misc/timer.h:157-158`).

#### `void event_loop_post(struct event_loop *, enum event_type, void *context, int param1)` — 1684-1703

Runs on **any producer thread**. Allocates a node from the ring, fills it with release stores, emits a compiler
barrier (`__asm__ __volatile__ ("" ::: "memory")`, 1694), then:

```c
do { tail = load_relaxed(&tail); success = CAS(&tail->next, NULL, new_tail); } while (!success);
CAS(&tail, tail, new_tail);
sem_post(semaphore);
```

Standard Michael-Scott enqueue with a helping-free swing of `tail`. Note the helper CAS at 1700 is unconditional
and may fail benignly.

#### `bool event_loop_begin(struct event_loop *)` — 1705-1721

Runs on **MAIN**. `memory_pool_init(pool, KILOBYTES(512))` → 512 KiB + one `PROT_NONE` guard page
(`src/misc/memory_pool.h:21-24`). Then
`sem_open("yabai_event_loop_semaphore", O_CREAT, 0600, 0)` immediately followed by
`sem_unlink("yabai_event_loop_semaphore")` (1709-1710) — a named semaphore used as an anonymous one, because
`sem_init` is unimplemented on macOS. Allocates the sentinel node, sets `tail = head`, sets `is_running = true`,
`pthread_create`. Returns `false` on pool or semaphore failure; **leaks the pool mapping in the semaphore-failure
case** (and `main` exits anyway).

Note the `sem_unlink` race: if two yabai instances start simultaneously they can share/steal the name. The lock
file makes this practically unreachable.

External symbols: `mmap`, `mprotect`, `getpagesize`, `sem_open`, `sem_unlink`, `sem_wait`, `sem_post`,
`pthread_create`, `__sync_bool_compare_and_swap`, `__atomic_load_n`, `__atomic_store_n`.

---

## 4. `src/mission_control.c`

### 4.1 Purpose

Bridges two OS notification sources into the event queue: the private SkyLight per-connection notify proc (space
create/destroy, window ordered/destroyed, mission-control enter, cmd-tab), and the Dock's accessibility
`AXExpose*` notifications. Also owns the `mission_control_mode` enum, its string table and the
`mission_control_is_active()` predicate used across `space_manager.c` and `display_manager.c`.

Included at `src/manifest.m:81`, i.e. **before** `event_loop.c`, which is why `mission_control_is_active` (a
`static inline` at line 108) is visible to the handlers.

### 4.2 Types and constants

**`enum mission_control_mode` — 29-36**

| Variant | Value |
| --- | --- |
| `MISSION_CONTROL_MODE_INACTIVE` | 0 |
| `MISSION_CONTROL_MODE_SHOW` | 1 |
| `MISSION_CONTROL_MODE_SHOW_ALL_WINDOWS` | 2 |
| `MISSION_CONTROL_MODE_SHOW_FRONT_WINDOWS` | 3 |
| `MISSION_CONTROL_MODE_SHOW_DESKTOP` | 4 |

Ordering oddity to be aware of: line 2 declares `extern enum mission_control_mode g_mission_control_mode;`
**before** the enum is defined at line 29. Legal C (incomplete enum type completed later in the same TU); in Rust
this simply disappears.

**`static const char *mission_control_mode_str[]` — 38-44.** Designated-initialiser array indexed by the enum:
`"inactive"`, `"show"`, `"show-all-windows"`, `"show-front-windows"`, `"show-desktop"`. Read from
`src/event_signal.c:338` on **EVENTLOOP** and **CHILD**. Rust: a `const [&str; 5]` or a method on the enum.

**Anonymous struct `g_mission_control_observer` — 46-50**

| Field | C type | Owner | Threads |
| --- | --- | --- | --- |
| `ref` | `AXUIElementRef` | owned (`AXUIElementCreateApplication` at 77, `CFRelease` at 104) | written MAIN at startup **and EVENTLOOP** via `DOCK_DID_RESTART` (`src/event_loop.c:1554-1555`) |
| `observer_ref` | `AXObserverRef` | owned (`AXObserverCreate` at 80, `CFRelease` at 103) | same |
| `is_observing` | `bool` | value | same |

This struct is **genuinely touched from two threads with no synchronisation** — `mission_control_observe()` runs on
MAIN at `src/yabai.c:316` and on EVENTLOOP at `src/event_loop.c:1555`. It also calls
`CFRunLoopAddSource(CFRunLoopGetMain(), …)` (87) from EVENTLOOP in the restart case, which is legal for CFRunLoop
but worth recording.

**`static CFStringRef kAXExpose*` — 52-55.** Four `CFSTR` literals: `"AXExposeShowAllWindows"`,
`"AXExposeShowFrontWindows"`, `"AXExposeShowDesktop"`, `"AXExposeExit"`. `CFSTR` constants are immortal; no
release. Rust: `objc2_core_foundation::CFString` statics, or `ns_string!`-style `static CFStringRef` created once
in a `OnceLock`.

### 4.3 Functions

#### `static CONNECTION_CALLBACK(connection_handler)` — 7-26

Expands (via `src/misc/extern.h:1`) to:

```c
void connection_handler(uint32_t type, void *data, size_t data_length, void *context, int cid)
```

Fires on **MAIN** (delivered by the main run loop; registered from MAIN at `src/yabai.c:322-333` before
`[NSApp run]`). Wrapped in `-Wunused-parameter` pragmas (5-6 / 27) because `data_length`, `context` and `cid` are ignored.

Dispatch by `type`:

| `type` | Action |
| --- | --- |
| 1204 | post `MISSION_CONTROL_ENTER`, context `NULL` |
| 1327 | `memcpy(&sid, data, 8)` → post `SLS_SPACE_CREATED` with `(void*)(intptr_t) sid` |
| 1328 | `memcpy(&sid, data, 8)` → post `SLS_SPACE_DESTROYED` |
| 808 | `memcpy(&wid, data, 4)` → post `SLS_WINDOW_ORDERED` |
| 804 | `memcpy(&wid, data, 4)` → post `SLS_WINDOW_DESTROYED` |
| 1202 | `__atomic_store_n(&__last_cmd_tab_time, read_os_timer(), __ATOMIC_RELEASE)` — no event posted |

The `memcpy`s exist because `data` has no alignment guarantee. Rust must use `read_unaligned` on a raw pointer
inside `unsafe`, not a `&u64` cast. **`data_length` is never validated** — a short payload would over-read. Recommend
keeping the read but adding a `data_length >= size_of::<…>()` guard and noting the deviation.

Allocates nothing (the event node comes from the ring). External symbols: `memcpy`, `__atomic_store_n`,
`mach_absolute_time`/`AbsoluteToNanoseconds` via `read_os_timer`.

#### `static OBSERVER_CALLBACK(mission_control_notification_handler)` — 59-70

Expands (via `src/application.h:4`) to:

```c
void mission_control_notification_handler(AXObserverRef observer, AXUIElementRef element, CFStringRef notification, void *context)
```

Fires on **MAIN** (the observer's run-loop source is added to `CFRunLoopGetMain()` at line 87). `CFEqual`s the
notification against the four `kAXExpose*` constants and posts `MISSION_CONTROL_SHOW_ALL_WINDOWS`,
`MISSION_CONTROL_SHOW_FRONT_WINDOWS`, `MISSION_CONTROL_SHOW_DESKTOP` or `MISSION_CONTROL_EXIT`, all with `NULL`
context. Also wrapped in `-Wunused-parameter` pragmas (57-58 / 71).

External symbols: `CFEqual`.

#### `void mission_control_observe(void)` — 73-91

Idempotent (`is_observing` guard). Gets the Dock's pid via `workspace_get_dock_pid()`, creates an
`AXUIElementRef` for it, creates an `AXObserverRef`, adds the four notifications with a `NULL` refcon, and adds the
observer's run-loop source to the **main** run loop in `kCFRunLoopDefaultMode`.

**Leak on the failure path:** `AXUIElementCreateApplication` (77) is called unconditionally, but if `pid == 0`,
the ref is NULL-or-leaked, and if `AXObserverCreate` fails the `ref` is never released and `is_observing` stays
`false`, so the next call leaks another one. Preserve the control flow but a Rust `CFRetained<AXUIElement>` makes
the leak disappear — record that as an intentional improvement.

Runs on **MAIN** (`src/yabai.c:316`) and on **EVENTLOOP** (`src/event_loop.c:1555`).

External symbols: `AXUIElementCreateApplication`, `AXObserverCreate`, `AXObserverAddNotification`,
`AXObserverGetRunLoopSource`, `CFRunLoopAddSource`, `CFRunLoopGetMain`.

#### `void mission_control_unobserve(void)` — 93-106

Removes the four notifications, clears `is_observing`, invalidates the run-loop source, releases the observer and
the element. Called only from `DOCK_DID_RESTART` (`src/event_loop.c:1554`), i.e. **EVENTLOOP**.

External symbols: `AXObserverRemoveNotification`, `CFRunLoopSourceInvalidate`, `AXObserverGetRunLoopSource`,
`CFRelease`.

#### `static inline bool mission_control_is_active(void)` — 108-111

`g_mission_control_mode != MISSION_CONTROL_MODE_INACTIVE`. Read on **EVENTLOOP** from
`src/event_loop.c:1119,1156,1237,1348,1487`, `src/space_manager.c:803,853,893,987,1013,1039,1064`,
`src/display_manager.c:485`.

### 4.4 Callback registry for this file

| Registration site | Callback | Thread | Context pointer | Lifetime |
| --- | --- | --- | --- | --- |
| `src/yabai.c:322,323,326,329,330,333` — `SLSRegisterConnectionNotifyProc` | `connection_handler` | MAIN | `NULL` | process lifetime; never unregistered |
| `src/mission_control.c:81-84` — `AXObserverAddNotification` ×4 | `mission_control_notification_handler` | MAIN | `NULL` (the `refcon` argument) | until `mission_control_unobserve` |
| `src/mission_control.c:87` — `CFRunLoopAddSource(CFRunLoopGetMain(), …)` | delivers the above | MAIN | — | until `CFRunLoopSourceInvalidate` at 102 |

---

## 5. `src/manifest.m`

### 5.1 Purpose

The whole build. It is the only file passed to the compiler (`makefile:12`, `makefile:66-68`): it `#include`s the
system headers, then every yabai header, then every yabai `.c`/`.m` in a fixed order, producing one translation
unit. Nothing else in `src/` is compiled separately.

### 5.2 Contents

* **System headers, 1-8**: `objc/objc-runtime.h`, `Carbon/Carbon.h`, `Cocoa/Cocoa.h`, `CoreVideo/CoreVideo.h`,
  `mach/mach_time.h`, `mach-o/dyld.h`, `mach-o/swap.h`, `bootstrap.h`.
* **SIMD headers, 10-14**: `emmintrin.h` on `__x86_64__`, `arm_neon.h` on `__arm64__`. Used by `src/misc/helpers.h`
  string routines. Rust: `core::arch::x86_64` / `core::arch::aarch64`, or plain safe code if the phase-2 owner of
  `helpers.h` decides the SIMD is not load-bearing.
* **libc/POSIX headers, 16-43**: notably `regex.h` (25), `execinfo.h` (26), `semaphore.h` (39), `pthread.h` (40),
  `spawn.h` (42), `libproc.h` (43).
* **yabai `misc` headers, 45-59**, with `HASHTABLE_IMPLEMENTATION` defined around `hashtable.h` (56-58) — an
  stb-style single-header library. `//#include "misc/autorelease.h"` at **line 49** is commented out.
* **`osax/common.h`, 61** — the one header shared with the scripting addition, which stays in C.
* **yabai headers, 63-78** in dependency order: `view.h`, `sa.h`, `event_loop.h`, `event_signal.h`, `workspace.h`,
  `rule.h`, `message.h`, `display.h`, `space.h`, `window.h`, `process_manager.h`, `application.h`,
  `display_manager.h`, `space_manager.h`, `window_manager.h`, `mouse_handler.h`.
* **yabai implementations, 80-97**: `sa.m`, `mission_control.c`, `event_loop.c`, `event_signal.c`, `workspace.m`,
  `rule.c`, `message.c`, `display.c`, `space.c`, `view.c`, `window.c`, `process_manager.c`, `application.c`,
  `display_manager.c`, `space_manager.c`, `window_manager.c`, `mouse_handler.c`, `yabai.c`.

### 5.3 What the unity build implies for the Rust crate

1. **`static` is not "private" here.** Symbols declared `static` in one `.c` are freely used by any file included
   later. Concretely, within this reader's scope:
   * `connection_handler` (`mission_control.c:7`, static) ← used by `yabai.c:322-333`.
   * `mission_control_is_active` (`mission_control.c:108`, static inline) ← used by `event_loop.c`,
     `space_manager.c`, `display_manager.c`.
   * `mission_control_mode_str` (`mission_control.c:38`, static) ← used by `event_signal.c:338`.
   * `update_window_notifications` (`event_loop.c:16`, static) ← used by `yabai.c:341` and `view.c:45,111`.
   * `EVENT_HANDLER_WINDOW_DESTROYED` (static) ← used by `EVENT_HANDLER_SLS_WINDOW_DESTROYED` at `event_loop.c:965`.
   * `CGSGetConnectionPortById` / `SLSPerformAsynchronousBridgedWindowManagementOperation`
     (`misc/extern.h:4-5`, static) ← assigned in `yabai.c:148-149`, read in `window.c` / `space_manager.c`.

   In Rust these become `pub(crate)` items, which is the natural mapping, but **phase 2 must not assume that a
   `static` C function is module-private** — it must grep before deciding visibility.

2. **Include order is initialisation order** only for `static inline` visibility, not for runtime. Rust has no
   ordering constraint; the only thing to preserve is `main`'s runtime sequence (§1.4).

3. **`#define HASHTABLE_IMPLEMENTATION`** is a build-time switch with no Rust analogue.

4. **Build flags to reproduce in `build.rs` / `.cargo/config.toml`** (`makefile:4`, `:27`):
   `-arch x86_64 -arch arm64` (universal — Rust needs `lipo` over two target builds, or `cargo-lipo`-style
   post-processing), `-mmacosx-version-min=11.0`, `-fno-objc-arc`, `-fvisibility=hidden`,
   `-sectcreate __TEXT __info_plist assets/Info.plist`, `-F/System/Library/PrivateFrameworks`, and
   `-framework Carbon -framework Cocoa -framework CoreServices -framework CoreVideo -framework SkyLight`.
   The `__info_plist` section is required for the accessibility prompt to name the binary; it maps to
   `-C link-arg=-Wl,-sectcreate,__TEXT,__info_plist,assets/Info.plist`.

5. **`osax` stays C**: `makefile:30-36` compiles `payload.m` and `loader.m` for `-arch x86_64 -arch arm64e`, then
   `xxd -i` them into `payload_bin.c` / `loader_bin.c`, which are linked into the daemon (`makefile:11-12`). The
   Cargo build script must reproduce exactly that, and expose the two byte arrays (and their lengths) to Rust.

6. `PROFILE` is undefined in every makefile target except `tests/makefile:10` (`-DPROFILE=1`), so all profiling
   macros compile away in the shipping binary.

---

## 6. C pattern catalogue → Rust

Each entry: the pattern, where it appears in **these** files, the recommended translation, and what a naive
translation would silently change.

### 6.1 X-macro generating an enum and a dispatch switch

*Sites:* `event_loop.h:6-46` (list), `event_loop.h:50-52` (enum), `event_loop.c:1665-1667` (switch),
`event_loop.h:4` (`EVENT_HANDLER` signature macro), also `workspace.h:4-19` (`SUPPORTED_MACOS_VERSION_LIST`).

*Rust:* a plain `enum Event` with a payload per variant (see §3.6) and a `match`. No macro needed; the
exhaustiveness the X-macro bought is what `match` gives for free. Do **not** reach for `macro_rules!` here — it
would recreate the indirection without buying anything.

*Silent change to watch:* the C `switch` has no `default`, so an out-of-range `type` is a no-op. With a Rust enum
the state is unrepresentable, which is strictly better; but if phase 2 keeps a numeric wire value anywhere, it must
reject unknown discriminants explicitly rather than transmuting.

### 6.2 Lock-free MPSC queue over a wrapping bump allocator

*Sites:* `event_loop.h:55-71`, `event_loop.c:1684-1721`, `misc/memory_pool.h:29-45`.

*Rust:* `crossbeam_channel::unbounded::<Event>()` (or `std::sync::mpsc`) with the `Sender` cloned into every
producer. The semaphore disappears — `recv()` blocks, `try_recv()` drains. The drain/park structure becomes:

```rust
loop {
    autoreleasepool(|_| { while let Ok(event) = rx.try_recv() { handle(event); flush_signals(); ts_reset(); } });
    match rx.recv() { Ok(event) => { /* handle, then continue draining */ } Err(_) => break }
}
```

*Silent changes to watch:*

* **The C pool wraps.** `memory_pool_push` (`memory_pool.h:39-42`) resets `used` to `size` and returns the base
  pointer when the 512 KiB fills, so after ~16 384 queued `struct event`s it **overwrites live queue nodes**. A
  channel cannot do this. The Rust version is therefore *more* correct under burst load; say so in the phase-2
  notes rather than trying to reproduce the corruption.
* **Unbounded memory.** The C ring is a hard 512 KiB cap; an unbounded channel is not. If that matters, use
  `crossbeam_channel::bounded(16_384)` with a blocking send — but that changes producer behaviour (an AX callback
  on MAIN would block), so unbounded is the right default.
* **Node reclamation.** The C version never frees an event node; dropping the Rust `Event` runs the payload's
  `Drop`, which is what replaces the hand-written `CFRelease`/`free`. That means an event **dropped without being
  handled** now releases its payload, whereas C would leak it. Equivalent-or-better; not observable.

### 6.3 `goto out` / multi-label cleanup

*Sites:* `event_loop.c:258,343-345` (`APPLICATION_TERMINATED`), `1119-1151` (`MOUSE_DOWN`),
`1156-1232` (`MOUSE_UP`, three labels `err:` / `res:` / `out:` with fallthrough), `1237-1342` (`MOUSE_DRAGGED`),
`1347-1449` (`MOUSE_MOVED`), `event_loop.c:1661` (`goto empty` out of a nested loop).

*Rust:* the cleanup is always "release the owned CF object" or "destroy the owned struct" → make it the payload's
`Drop` and use early `return`. The `MOUSE_UP` ladder needs care: `err:` sets `window = NULL` then **falls through**
to `res:` which sets `current_action = NONE` then falls through to `out:`. Model it as a small `enum Outcome` or as
a labelled block:

```rust
'body: { … break 'body; … }   // then the tail runs unconditionally
```

*Silent change to watch:* fallthrough between labels. `goto res` skips the `window = NULL` assignment but
`goto err` does not skip `current_action = NONE`. Transcribing each `goto` as an early `return` without replaying
the tail assignments changes mouse state.

### 6.4 CF retain/release pairing across a thread boundary

*Sites:* producer retains (`application.c:9`, `mouse_handler.c:34,44,61,67`), consumer releases
(`event_loop.c:554,557,560,563,1151,1232,1244,1342,1449`); intra-handler pairs at 772/775, 853/856, 897/900, 937
(release-then-replace), 1385, 1393, 1525.

*Rust:* `objc2-core-foundation`'s `CFRetained<T>` (or `core-foundation` crate's `TCFType`), moved through the
channel. `CFRetained::from_raw` at the producer (taking the +1 the C code created), `Drop` at the consumer. The
release-then-replace pattern (`if (window->role) CFRelease(window->role); window->role = window_ax_role(window);`)
becomes a plain assignment to an `Option<CFRetained<CFString>>` field.

*Silent change to watch:* the "Get" vs "Copy/Create" rule. `CFDictionaryGetValue` (1496, 1499, 1502) and
`CFArrayGetValueAtIndex` (1377, 1494) return **borrowed** references that must **not** be released; `window_role`
(1381) and `CGWindowListCopyWindowInfo` (1489) return **owned** ones that must. Wrapping a borrowed pointer in
`CFRetained` without retaining it is an over-release and an instant crash.

### 6.5 Pointer-sized integers stuffed into `void *`

*Sites:* `event_loop.c:428,492,639,677,727,926,946,954,970,982,1085,1094,1103,1111`,
`mission_control.c:13,16,19,22`, `event_loop.c:1456,1463,1470,1482,1541` (`(void*)(uintptr_t)` of an enum).

*Rust:* typed enum payloads (`WindowFocused(u32)`, `SpaceCreated(u64)`, `DisplayAdded(u32)`).

*Silent changes to watch:*

* `event_loop.c:946` and `:954`: `uint32_t wid = (uint64_t)(intptr_t) context;` — a `u64` truncated to `u32`.
  Producers only ever put a `u32` in, so the truncation is a no-op today, but a Rust `as u32` must be written
  deliberately, not `try_into().unwrap()`.
* `event_loop.c:428,492`: `(pid_t)(intptr_t) context` — `pid_t` is `i32`. Sign matters if a pid ever exceeded
  `i32::MAX` (it cannot on macOS).
* `event_loop.c:1134`: `uint8_t mod = (uint8_t) param1;` — the modifier byte is passed in an `int` and truncated.

### 6.6 CAS on `window->id_ptr` as a liveness check

*Sites:* `event_loop.c:280` (store `NULL` — "claim the window"), and the read-only probes at
`647, 681, 731, 833, 877, 930, 960, 1159, 1240`. The producer side is `application.c:36`.

The idiom is `__sync_bool_compare_and_swap(&window->id_ptr, &window->id, &window->id)` — compare the pointer field
against `&window->id` and write the same value back. It answers "has anybody NULLed this out since the event was
queued?" without a lock. `window->id_ptr` is declared `uint32_t *volatile` (`src/window.h:92`).

*Rust:* `struct Window { id: u32, alive: AtomicBool }` with `compare_exchange(true, false, …)` for the claim at 280
/ `application.c:36`, and `load(Relaxed)` for the probes. Keeping a self-referential raw pointer in Rust is painful
and buys nothing — the pointer identity is never used, only its NULL-ness.

*Silent change to watch:* the C probe is a **read-modify-write** (a full CAS), so it has release/acquire semantics
on x86 and a `ldaxr/stlxr` pair on arm64. Replacing it with a relaxed load weakens the ordering. Use
`Ordering::AcqRel` on the claim and `Ordering::Acquire` on the probes to stay conservative.

### 6.7 Arena / temp allocation (`ts_alloc*`)

*Sites:* `event_loop.c:191,275,439,502` (`ts_alloc_list(struct view *, n)`), `event_loop.c:1623`
(`ts_alloc_unaligned(bytes_to_read)`), reset at `event_loop.c:1671`.

*Rust:* a `bumpalo::Bump` owned by the event-loop thread and `reset()` after each event, with
`bumpalo::collections::Vec` for the view lists; or, simpler and probably better, a reused `Vec<&mut View>` /
`Vec<usize>` scratch buffer cleared per event. The arena in C exists to avoid `malloc`; a `Vec` with retained
capacity achieves the same.

*Silent changes to watch:*

* `ts_assert_within_bounds` (`misc/ts.h:28-34`) **`exit(EXIT_FAILURE)`s the daemon** on overflow. `DAEMON_MESSAGE`
  passes an attacker-controlled `bytes_to_read` straight into `ts_alloc_unaligned` (1623), so a malformed message
  on the socket kills yabai. A Rust `Vec::with_capacity(n)` would instead try to allocate. Either cap the length
  explicitly or reproduce the abort — but do not silently allocate 4 GiB.
* The arena is reset **between events**, so no `ts` pointer may escape a handler. Rust lifetimes enforce this for
  free; phase 2 will find places where the C code relied on it implicitly.
* `ts_reset()` (`misc/ts.h:100-103`) is a **non-atomic** store to a `volatile uint64_t` that other threads CAS
  (`ts.h:59,68`). Today only EVENTLOOP allocates from it inside handlers, but `ts_*` is also used by
  `message.c` helpers. Worth a sweep note.

### 6.8 Stretchy buffers (`buf_*`) and pointer-keyed hash tables (`table_*`)

*Sites:* `event_loop.c:264-268` (`buf_len` + `buf_del` over `applications_to_refresh`), `568-577`
(`buf_len`/`buf_del` over `rules` with manual index fix-up), `387-393`; `table_for` at `16-31`, `table_find` at
`948, 983`, `table_remove` at `987`.

*Rust:* `Vec<T>` and `HashMap<K, V>` / `hashbrown`. `buf_del` is **swap-remove**
(`misc/sbuffer.h:19`: `b[x] = b[len-1]; len--`), so it is `Vec::swap_remove`, not `Vec::remove` — order is not
preserved and the loops at 264-268 / 568-577 depend on that.

*Silent changes to watch:*

* `event_loop.c:570-576` deletes while iterating and compensates with `--i; --rule_len;`. With `swap_remove` the
  element swapped into slot `i` is re-examined next iteration — correct only because `--i` is inside the
  `if (buf_del(...))`. `Vec::retain` is **not** an equivalent rewrite: `rule_destroy` must run for removed
  elements, and `retain`'s closure order differs. Use an explicit `while i < len` loop with `swap_remove`.
* `table_for` (`misc/hashtable.h:34-41`) **declares `int i` in the caller's scope** and skips buckets with a NULL
  `value`. Iteration order is bucket order, i.e. unspecified. Nothing in these files depends on it, but
  `update_window_notifications` (16-34) produces a list whose *order* is passed to
  `SLSRequestNotificationsForWindows` — if SkyLight ever cared, a `HashMap` iteration would differ. It does not
  appear to.

### 6.9 `printf` into a `FILE *` response stream

*Sites:* `event_loop.c:1632` (`fdopen(param1, "w")`), `1636-1637` (`fflush` + `fclose`);
`yabai.c:114,117` (client side, `fprintf` + `fflush` per chunk); `misc/log.h` (`debug`/`warn`/`error`/`require`).

*Rust:* `std::os::unix::io::OwnedFd` → `std::fs::File` → `BufWriter<File>`, passed as `&mut dyn Write` to
`handle_message`. `fclose` maps to dropping the `BufWriter` (which flushes) and then the `File` (which closes).

*Silent changes to watch:*

* `fdopen` **takes ownership of the fd**; `fclose` closes it. The `return` at 1639 skipping `socket_close` is
  therefore correct, and a Rust translation that both drops the `File` and closes the fd double-closes.
* If `fdopen` fails, the fd is closed by `socket_close(param1)` at 1643, which does `shutdown(SHUT_RDWR)` **then**
  `close` (`misc/helpers.h:198-202`). The success path does **not** shutdown, only close — the client sees EOF
  either way, but a faithful port should keep the distinction.
* `error()`/`require()` are variadic and call `exit`. In Rust: `eprint!` + `std::process::exit(1)` / `exit(0)`.
  Note `require` exits **0**.
* `debug()` checks `g_verbose` first (`misc/log.h:9`) and writes to **stdout**, unbuffered-ish (no explicit flush
  except in `debug_message`). `debug_message` (`misc/log.h:49-61`) walks a NUL-separated blob printing each
  segment — that is the daemon-message tracing at `event_loop.c:1633`.

### 6.10 Fixed-size char arrays and `snprintf`

*Sites:* `yabai.c:44-47` (`char[512]` ×3, `char[4096]`), `yabai.c:85` (`char socket_file[MAXLEN]`),
`yabai.c:106` (`char rsp[BUFSIZ]`), `event_loop.c:19` (`uint32_t window_list[1024]`).

*Rust:* `String` / `PathBuf` for the paths (they are only ever written once and read as C strings by
`socket_connect`), `[u8; BUFSIZ]` or a `Vec<u8>` for `rsp`.

*Silent changes to watch:*

* Every one of these ends up at a syscall that wants a NUL-terminated `char *`. Use `CString`, and remember
  `socket_connect` (`misc/helpers.h:189-196`) `snprintf`s into `sun_path[104]` — a path longer than 103 bytes is
  **silently truncated**, not rejected. A Rust `SocketAddr::from_pathname` **errors** instead. `$USER` would have to
  be absurd for this to matter, but it is a behaviour difference.
* `g_config_file` is 4096 but `MAXLEN` is 512 — do not unify them.

### 6.11 Bit flags

*Sites:* `event_loop.c:1144-1147` (`HANDLE_LEFT/TOP/RIGHT/BOTTOM` OR'd into `g_mouse_state.direction`,
`misc/macros.h:36-40`), `window_set_flag`/`window_clear_flag`/`window_check_flag` throughout (`src/window.h:108-117`),
`view_set_flag`/`view_clear_flag`/`view_is_dirty` (`src/view.h:197-221`).

*Rust:* `bitflags!` for `WindowFlags`, `ViewFlags`, `ResizeHandle`. The `view_is_dirty`/`view_is_invalid` helpers
are already functions, so they become methods.

*Silent change to watch:* `g_mouse_state.direction` is `uint8_t` and `HANDLE_ABS = 0x10` also lives in that space;
`bitflags` with a `u8` repr keeps that exact.

### 6.12 Objective-C message sends, blocks, `@try/@catch`

*Sites:* `event_loop.c:92-96` and `156-160` (`__block` capture + `dispatch_after` + block literal),
`event_loop.c:112-117` and `131-136` (`@try { … } @catch (NSException * __unused exception) {}`),
`event_loop.c:113-115,132-134` (`[application observationInfo]`, `[application removeObserver:forKeyPath:context:]`),
`event_loop.c:1653,1677` (`[[NSAutoreleasePool alloc] init]` / `[pool drain]`),
`yabai.c:139` (`NSApplicationLoad()`), `yabai.c:350` (`[NSApp run]`).

*Rust:*

* Message sends → `objc2` + `objc2-foundation` / `objc2-app-kit` (`msg_send!`), all inside `unsafe`.
* `NSAutoreleasePool` → `objc2::rc::autoreleasepool(|pool| { … })`. Note the C code keeps the pool across many
  events and drains on queue-empty; the closure form maps cleanly onto the inner drain loop.
* `dispatch_after` → the `dispatch2` crate, or a raw `dispatch_after_f` with a `Box::into_raw`'d context. The
  `__block ProcessSerialNumber psn` capture is a **by-value copy** made precisely so the block never touches a
  possibly-freed `struct process` — in Rust, `move` a `ProcessSerialNumber` value into the closure.
* `@try/@catch` → `objc2::exception::catch` (which needs the `catch-all` feature) or a tiny C shim in the same
  build script that already compiles `osax`. **There is no safe way to let an ObjC exception unwind through Rust
  frames.** Both sites here swallow the exception entirely, so `let _ = catch(|| …);` is faithful.
* `[NSApp run]` → `unsafe { NSApplication::sharedApplication(mtm).run() }` from the main thread, with
  `MainThreadMarker` obtained at startup.

*Silent changes to watch:*

* The build uses `-fno-objc-arc` (`makefile:4`): every object here is manually managed. `objc2`'s `Retained<T>` is
  ARC-like and will release on drop. Re-audit each ObjC object's ownership rather than transcribing.
* `[application observationInfo]` returning non-nil is the guard before `removeObserver:` — KVO throws if the
  observer was never registered. Keep the guard **and** the `@catch`; dropping either changes behaviour.

### 6.13 `fork` / `exec`, signals

*Sites (reachable from these files):* `exec_config_file` at `yabai.c:348` → `fork` at `misc/helpers.h:477`,
`execvp("/usr/bin/env", …)`; `event_signal_flush()` at `event_loop.c:1670` → nested `fork`s at
`src/event_signal.c:64,83`; `signal(SIGCHLD, SIG_IGN)` / `signal(SIGPIPE, SIG_IGN)` at `yabai.c:151-152`.

*Rust:* `std::process::Command` is the obvious answer for `exec_config_file` (it is a plain fork+exec of
`/usr/bin/env sh [-c] <file>`), and it handles the `SIGCHLD` reaping question differently — with
`SIGCHLD` set to `SIG_IGN`, `Child::wait` returns `ECHILD`. Since the C code never waits, spawn and
`std::mem::forget` the `Child`, or keep `SIG_IGN` and ignore the wait error.

*Silent changes to watch:*

* `fork()` in a multithreaded process is only async-signal-safe until `exec`. `event_signal_flush` forks from
  EVENTLOOP **while MAIN is running a CFRunLoop**, then calls `setenv` and `execvp` in the child
  (`event_signal.c:86-92`) — `setenv` is not async-signal-safe. This is a real latent hazard in the C code.
  `Command::env()` + `spawn()` uses `posix_spawn` where possible and is strictly safer. Record the deviation.
* Rust's standard library installs its own `SIGPIPE` disposition (`SIG_IGN`) at startup, matching `yabai.c:152`.
  `SIGCHLD` must still be set explicitly, via `libc::signal` or the `signal-hook` crate, **before** any spawn.
* `std::process::Command` sets `CLOEXEC` on everything by default; the C `fork`+`execvp` inherits every non-CLOEXEC
  fd, including the lock-file fd (which **is** `O_CLOEXEC`, `yabai.c:164`) and the listening socket (which **is**
  `FD_CLOEXEC`, `message.c:3039`). No accepted client fd is CLOEXEC, so a config script forked while a message is
  in flight inherits it today. Behaviour difference; almost certainly an improvement.

### 6.14 Float/integer conversion hazards (the subtlest thing in this file)

*Sites:* `event_loop.c:363`, `event_loop.c:1266`, `event_loop.c:1353` — all the same shape:

```c
float dt = ((float) read_os_timer() - last_time) * (1000.0f / (float)read_os_freq());
```

`read_os_timer()` (`misc/helpers.h:149-154`) returns **nanoseconds since boot** as `uint64_t`; `read_os_freq()`
returns `1000000000`. Both operands are converted to `float` (24-bit mantissa) **before** the subtraction.

At 1 hour of uptime the value is ≈3.6e12 ns, where a `float` ULP is ≈262 144 ns ≈ 0.26 ms. At 12 days of uptime
(≈1e15 ns) the ULP is ≈67 ms — larger than the 67.67 ms drag throttle at `event_loop.c:1267`. So:

* `MOUSE_DRAGGED`'s resize throttle degrades to "always allow" or "always block" on long uptimes.
* `APPLICATION_FRONT_SWITCHED`'s 1500 ms cmd-tab window (364) and `MOUSE_MOVED`'s 1250 ms gesture window (1354)
  quantise badly.

**A Rust port that computes this in `f64` or in integer nanoseconds silently changes behaviour** — in the right
direction, but observably (drag-resize will feel different). Phase 2 must either reproduce the `f32` arithmetic
exactly (`(timer as f32 - last as f32) * (1000.0f32 / freq as f32)`) or make the fix deliberately and record it.
Recommendation: reproduce the `f32` arithmetic in phase 2, and list "compute deltas in integer nanoseconds" as a
phase-3 candidate.

Other conversions in these files:

* `event_loop.c:1269-1270`: `int dx = point.x - g_mouse_state.down_location.x;` — `CGFloat` (f64) truncated
  **toward zero**, so `-0.9` → `0`. Rust `as i32` on `f64` truncates toward zero too **and saturates** instead of
  being UB for out-of-range values. Same result for any realistic coordinate.
* `event_loop.c:1000,1052,1533`: `float alpha = space_is_fullscreen(…) ? 1.0f : g_window_manager.menubar_opacity;`
  and the `!= 1.0f` comparisons at 999/1051/1532 — exact float equality against a literal. Keep `f32` and `!=`.
* `event_loop.c:1506`: `uint64_t layer = 0; CFNumberGetValue(layer_ref, CFNumberGetType(layer_ref), &layer);` —
  writes however many bytes the number's native type occupies (usually 4) into an 8-byte zero-initialised slot,
  then compares against `18`. In Rust: zero a `u64`, pass `&mut … as *mut _ as *mut c_void`, keep the
  `CFNumberGetType` round-trip. Passing `kCFNumberSInt64Type` instead would make `CFNumberGetValue` convert rather
  than reinterpret — usually the same answer, but not the same call.
* `event_loop.c:1373`: `int window_count = CFArrayGetCount(window_list);` — `CFIndex` (i64) to `int`.

### 6.15 Struct layout handed to the OS

*Sites:* `yabai.c:169-175` (`struct flock` passed to `fcntl(F_SETLK)`), `misc/helpers.h:191-195`
(`struct sockaddr_un`), `misc/helpers.h:206-222` (`mach_msg` header + OOL descriptor),
`window_manager.c:1280-1317` (the 0xf8-byte SLPS event record built in `g_event_bytes`).

*Rust:* use the `libc` crate's definitions (`libc::flock`, `libc::sockaddr_un`) rather than redeclaring them —
`#[repr(C)]` hand-rolls are a standing source of padding bugs. The SLPS record is a raw byte array and stays one.

*Silent change to watch:* `struct flock` field order differs between platforms; `libc`'s macOS definition is
authoritative. Do not use the `nix` crate's higher-level lock API without checking it emits `F_SETLK` (not
`flock(2)`, which is a different lock namespace and would not interoperate with an old C yabai).

### 6.16 Designated-initialiser lookup tables

*Sites:* `mission_control.c:38-44` (`mission_control_mode_str`), `misc/helpers.h:173-181` (`bool_str`,
`layer_str` — the latter indexed by `kCGBackstopMenuLevelKey` etc., which are **negative and non-contiguous**…
actually `LAYER_BELOW/NORMAL/ABOVE` are small non-negative key constants, but the array is sparse).

*Rust:* `impl MissionControlMode { fn as_str(self) -> &'static str }` with a `match`. Sparse designated-initialiser
arrays have NULL holes in C; a `match` with an explicit arm per variant is both safer and clearer.

### 6.17 `volatile` used as "atomic"

*Sites:* `event_loop.c:11-14`, `misc/memory_pool.h:8`, `misc/ts.h:7`, `src/window.h:92`,
`src/mouse_handler.h:71`.

`volatile` alone guarantees nothing about atomicity or ordering; the code pairs it with `__atomic_*` /
`__sync_*` builtins, which is what actually provides the guarantee. In Rust the `volatile` disappears entirely and
the type becomes `AtomicBool` / `AtomicU64` / `AtomicU8`. Match the orderings literally (§3.2). `g_mouse_state.modifier`
is `volatile uint8_t` **without** any atomic builtin at its read sites — that one is a plain racy read today;
`AtomicU8` with `Relaxed` is the closest faithful mapping.

### 6.18 Compiler barriers and inline asm

*Sites:* `event_loop.c:1694` (`__asm__ __volatile__ ("" ::: "memory")`), `process_manager.c:192` (same),
`misc/timer.h:30,61` (`mrs cntvct_el0` / `mrs cntfrq_el0`, arm64 only, profiling builds only).

*Rust:* the barrier becomes `core::sync::atomic::compiler_fence(Ordering::SeqCst)` — but in the channel-based
translation of §6.2 it is unnecessary, because the channel's own synchronisation subsumes it. The `mrs` reads only
exist under `PROFILE`, which the shipping build never sets; drop them.

### 6.19 `regex.h`

Included at `manifest.m:25` and used by `misc/helpers.h:573` / `src/rule.c`. Not used in this reader's files, but
the crate-level decision belongs in the sweep: POSIX ERE via `regex.h` is **not** the same dialect as the Rust
`regex` crate (backreferences, `\b`, leftmost-longest vs leftmost-first). `regex` is leftmost-first by default,
POSIX is leftmost-longest. For rule matching, only "does it match at all" is asked, so the dialects agree in
practice — but the syntax accepted differs and users have regexes in their `yabairc`. Flag for whoever owns
`rule.c`: the safest option is the `regex` crate with the POSIX-ish subset documented, or binding `libc`'s
`regcomp`/`regexec` directly.

### 6.20 Patterns *not* present in these files

For completeness, so phase 2 does not go looking: no `setjmp`/`longjmp`, no intrusive linked lists other than the
event queue, no tagged unions, no function-pointer tables (the two SkyLight function pointers in `extern.h` are
singletons, not a table), no SIMD in these four files (the SIMD headers in `manifest.m:10-14` serve
`misc/helpers.h`), no varargs beyond `misc/log.h`.

---

## 7. Comments to carry over verbatim

Only these comments exist in the assigned files. Everything else is code. Phase 2 carries these across and adds
nothing.

### `src/yabai.c`

**None.** (Line 200 is a URL inside the `--help` string literal, not a comment. Lines 158-162 are an `#if 0`
block containing three disabled `hook_*` calls — dead code, not a comment; do not port it.)

### `src/event_loop.h`

**None.**

### `src/mission_control.c`

**None.**

### `src/manifest.m`

* **49**: `//#include "misc/autorelease.h"` — a commented-out include. It has no Rust equivalent; drop it (record
  in the phase-2 notes that `src/misc/autorelease.h` is unused by the build).

### `src/event_loop.c`

| Lines | Comment |
| --- | --- |
| 22 | `// NOTE(asmvik): Subscribe to all windows because of window_destroyed (and ordered) notifications` |
| 27 | `// NOTE(asmvik): Subscribe to windows that have a feedback_border because of window_ordered notifications` |
| 106-109 | `//` / `// NOTE(asmvik): Do this again in case of race-conditions between the previous check and key-value observation subscription.` / `// Not actually sure if this can happen in practice..` / `//` |
| 125-128 | identical block, second occurrence |
| 140-143 | `//` / `// NOTE(asmvik): If we somehow receive a duplicate launched event due to the subscription-timing-mess above,` / `// simply ignore the event..` / `//` |
| 185 | inline `/* if (g_window_manager.window_origin_mode == WINDOW_ORIGIN_CURSOR) */` on the trailing `else` |
| 201-209 | `//` / `// @cleanup` / `//` / `// :AXBatching` / `//` / `// NOTE(asmvik): Batch all operations and mark the view as dirty so that we can perform a single flush,` / `// making sure that each window is only moved and resized a single time, when the final layout has been computed.` / `// This is necessary to make sure that we do not call the AX API for each modification to the tree.` / `//` |
| 227-234 | `//` / `// @cleanup` / `//` / `// :AXBatching` / `//` / `// NOTE(asmvik): Flush previously batched operations if the view is marked as dirty.` / `// This is necessary to make sure that we do not call the AX API for each modification to the tree.` / `//` |
| 288-296 | same "Batch all operations" block (2nd of 3) |
| 321-328 | same "Flush previously batched" block (2nd of 3) |
| 448-456 | same "Batch all operations" block (3rd of 3) |
| 469-476 | same "Flush previously batched" block (3rd of 3) |
| 510-518 | same "Batch all operations" block (4th) |
| 530-537 | same "Flush previously batched" block (4th) |
| 586 | inline `/* if (g_window_manager.window_origin_mode == WINDOW_ORIGIN_CURSOR) */` |
| 1217 | `/* silence compiler warning.. */` (inside `case MOUSE_DROP_ACTION_NONE:`) |
| 1323 | `/* silence compiler warning.. */` (inside `case MOUSE_DROP_ACTION_NONE:`) |
| 1365-1369 | `//` / `// NOTE(asmvik): Look for a window with role AXSheet or AXDrawer` / `// and forward focus to it because we are not allowed to focus the main` / `// window in these cases.` / `//` |
| 1400-1405 | `//` / `// NOTE(asmvik): If any **floating** window would be fully occluded by` / `// autoraising the window below the cursor we do not actually perform the` / `// focus change, as it is likely that the user is trying to reach for the` / `// smaller window that sits on top of the window we would otherwise raise.` / `//` |

Note on the two `/* silence compiler warning.. */` comments: a Rust `match` on an enum needs the
`MouseDropAction::None => {}` arm for exhaustiveness, so the comment still applies and should be carried. The
`@cleanup` / `:AXBatching` markers are the author's own cross-reference tags and appear in other files too
(`view.c`, `window_manager.c`) — keep the tag text exactly so the cross-references keep working.

---

## 8. Recommended shape for phase 2 (these files only)

* `src/yabai.c` → `src/main.rs` (arg parsing, startup sequence) + `src/client.rs` (`client_send_message`) +
  `src/globals.rs` (the `extern` surface). The globals are the hard part: 22 of them, mostly single-threaded on
  EVENTLOOP. Suggested split for phase 2, to be refined in phase 3:
  * the four manager structs + `g_mouse_state` + `g_mission_control_mode` + `g_signal_event` → one
    `struct Yabai` owned by the event-loop thread, passed as `&mut self` to every handler;
  * the read-only-after-init values (`g_connection`, `g_pid`, `g_bs_port`, the three layer levels,
    `g_cv_host_clock_frequency`, the four path strings, `g_verbose`) → `OnceLock` / `static` initialised in `main`;
  * the four `__`-prefixed flags → `AtomicBool`/`AtomicU64` statics;
  * `g_event_bytes` → a `[u8; 0x100]` inside the window manager;
  * `g_workspace_context` → an `objc2` `Retained<WorkspaceContext>` held by `main`.
* `src/event_loop.h` + `src/event_loop.c` → `src/event_loop.rs` (the `Event` enum, `EventSender`, the run loop) and
  the 40 handlers as `impl Yabai` methods in the same module for phase 2. That module will be ~1800 lines, which
  is exactly what phase 3 is for.
* `src/mission_control.c` → `src/mission_control.rs`.
* `src/manifest.m` → deleted; replaced by `Cargo.toml`, `build.rs` (osax compilation + `xxd`-equivalent embedding +
  the `__info_plist` link arg) and the module tree.

---

## 9. Open questions for the orchestrator

1. **`f32` timer arithmetic (§6.14).** Reproduce bug-for-bug in phase 2, or fix now and note it? My recommendation
   is reproduce, then fix in phase 3 with a changelog line — but it is a user-visible feel change either way.
2. **Event queue: ring buffer or channel (§6.2).** A channel drops the wraparound-corruption hazard and the
   semaphore. I recommend the channel. Confirm that "behaviour identical" tolerates this.
3. **`update_window_notifications`'s 1024-window stack array (§3.3).** Truncate, grow, or reproduce the overflow?
4. **`DAEMON_MESSAGE` trusts the length prefix (§6.7).** A malformed message currently kills the daemon via
   `ts_assert_within_bounds`. Keep the abort, or cap and reject?
5. **`g_process_manager` is raced between MAIN and EVENTLOOP (§1.3).** Rust will not let this compile as-is. Options:
   move the Carbon handler's table insert onto EVENTLOOP by posting a "process appeared" event, or wrap the process
   table in a `Mutex`. The first is a behaviour change (ordering), the second is not. Needs a decision before the
   `process_manager.c` translator starts.
6. **Monterey-only gap in notification registration (§1.5).** Preserve as-is, or is it a bug to fix?
7. **Universal binary.** Does the build stay `cargo build` + `lipo` of two targets, or does the project adopt a
   `cargo xtask`? This affects `build.rs`'s osax step, which must produce a fat `arm64e`+`x86_64` payload
   regardless of which host slice Cargo is building.
8. **`regex.h` dialect (§6.19).** Not my file, but the decision is crate-wide and affects user configs.
