# signals-and-rules

Phase-1 mapping for `src/event_signal.h`, `src/event_signal.c`, `src/rule.h`, `src/rule.c`,
`src/message.h`.

Everything below was read in full. Sizes and layouts quoted here were measured by compiling the
struct definitions standalone with `xcrun clang` on this machine (arm64, macOS 15 SDK), not
guessed.

## 0. Threading facts these five files depend on

How the thread attribution was determined: `src/manifest.m` builds one translation unit, so every
symbol is visible everywhere. I traced each entry point backwards to the `pthread_create` that
owns it.

| Thread | Created at | What runs on it that matters here |
| --- | --- | --- |
| main run loop | `src/yabai.c` `main`, ends in `CFRunLoopRun` | Carbon process events, SLS notify procs, `NSWorkspace` observers, `CGEventTap`. None of them call into `event_signal.c` or `rule.c`; they all `event_loop_post`. |
| event-loop pthread | `src/event_loop.c:1718` `pthread_create(&event_loop->thread, NULL, &event_loop_run, event_loop)` | `event_loop_run` (`src/event_loop.c:1647`) pops one event and dispatches to `EVENT_HANDLER_*`. **Every** function in `event_signal.c` and `rule.c` runs here. |
| message-loop pthread | `src/message.c:3042` `pthread_create(&g_message_loop.thread, NULL, &message_loop_run, NULL)` | `message_loop_run` (`src/message.c:3003`) only does `accept()` then `event_loop_post(&g_event_loop, DAEMON_MESSAGE, NULL, sockfd)` (`src/message.c:3009`). It never touches signals or rules. |
| forked children | `src/event_signal.c:64` and `src/event_signal.c:83` | The tail of `event_signal_flush` and then `execvp`. |

Three consequences the phase-2 translator must not lose:

1. **`g_signal_event` and `g_window_manager.rules` are single-threaded state.** `handle_message`
   is reached only through `EVENT_HANDLER(DAEMON_MESSAGE)` (`src/event_loop.c:1614`, calls
   `handle_message` at `src/event_loop.c:1634`), which is on the event-loop pthread, the same
   thread that runs every `event_signal_push`. There is no lock on either and none is needed.
   The atomics in `event_signal_push` are defensive, not load-bearing today.
2. **`event_signal_flush()` runs before `ts_reset()`** — `src/event_loop.c:1670` then
   `src/event_loop.c:1671`. Every `char *` stored in a queued `struct event_signal` points into
   the temporary-storage arena, so the flush *must* happen inside the same event-handler turn.
   A Rust translation that defers the flush, or that resets the arena first, silently reads freed
   storage.
3. **`event_signal_flush` forks before it drains.** The parent zeroes `g_signal_storage.used` at
   `src/event_signal.c:66` and returns; the child at `src/event_signal.c:70-96` reads the
   *pre-fork* copy-on-write snapshot of `used`. The drain is correct only because `fork` snapshots
   the address space. Any translation that shares the queue across the fork boundary (a `Vec`
   behind an `Rc`, a channel, a thread) breaks this.

---

## 1. `src/event_signal.h` (129 lines)

### 1.1 Purpose

Declares the public shape of the user-facing `yabai -m signal` feature: the enumeration of
subscribable events, the string names those events are addressed by on the socket, the
subscription record (`struct signal`), and the queued-notification record (`struct event_signal`).
It is included from `src/manifest.m:66`, before every `.c` file, so all of these types are global
to the unity build.

### 1.2 Types, constants and tables

#### `enum signal_type` — `src/event_signal.h:4-45`

Plain C enum, implicitly `int`, no explicit discriminants, so values run `0..=30`:

Discriminants, in declaration order: `SIGNAL_TYPE_UNKNOWN` 0; `SIGNAL_APPLICATION_LAUNCHED` 1,
`SIGNAL_APPLICATION_TERMINATED` 2, `SIGNAL_APPLICATION_FRONT_SWITCHED` 3,
`SIGNAL_APPLICATION_ACTIVATED` 4, `SIGNAL_APPLICATION_DEACTIVATED` 5, `SIGNAL_APPLICATION_VISIBLE`
6, `SIGNAL_APPLICATION_HIDDEN` 7; `SIGNAL_WINDOW_CREATED` 8, `SIGNAL_WINDOW_DESTROYED` 9,
`SIGNAL_WINDOW_FOCUSED` 10, `SIGNAL_WINDOW_MOVED` 11, `SIGNAL_WINDOW_RESIZED` 12,
`SIGNAL_WINDOW_MINIMIZED` 13, `SIGNAL_WINDOW_DEMINIMIZED` 14, `SIGNAL_WINDOW_TITLE_CHANGED` 15;
`SIGNAL_SPACE_CREATED` 16, `SIGNAL_SPACE_DESTROYED` 17, `SIGNAL_SPACE_CHANGED` 18;
`SIGNAL_DISPLAY_ADDED` 19, `SIGNAL_DISPLAY_REMOVED` 20, `SIGNAL_DISPLAY_MOVED` 21,
`SIGNAL_DISPLAY_RESIZED` 22, `SIGNAL_DISPLAY_CHANGED` 23; `SIGNAL_MISSION_CONTROL_ENTER` 24,
`SIGNAL_MISSION_CONTROL_EXIT` 25; `SIGNAL_DOCK_DID_CHANGE_PREF` 26, `SIGNAL_DOCK_DID_RESTART` 27;
`SIGNAL_MENU_BAR_HIDDEN_CHANGED` 28, `SIGNAL_SYSTEM_WOKE` 29; `SIGNAL_TYPE_COUNT` **30**.

Counted: 1 unknown + 7 application + 8 window + 3 space + 5 display + 2 mission control + 2 dock +
2 (menu bar, system woke) = 30.

Asymmetry to preserve: `g_signal_event` has **30** slots (indices `0..=29`), while
`signal_type_str` has **31** entries (index 30 is `"signal_type_count"`). Indexing
`g_signal_event[SIGNAL_TYPE_COUNT]` would be out of bounds; nothing does it, because
`signal_type_from_string` (`src/event_signal.c:345`) stops at `< SIGNAL_TYPE_COUNT` and
`handle_domain_signal` rejects `SIGNAL_TYPE_UNKNOWN` (`src/message.c:2929-2932`).

#### `static const char *signal_type_str[]` — `src/event_signal.h:47-88`

A designated-initializer table, 31 entries, `static const` in a header inside a unity build, so
exactly one instance exists in the binary. Read from the event-loop pthread only, at
`src/event_signal.c:77` (`debug`), `src/event_signal.c:346` (`signal_type_from_string`) and
`src/event_signal.c:427` (`event_signal_serialize`). Never written. String literals, static
storage, nobody frees them.

#### `#define SIGNAL_PROP_UD 0` / `SIGNAL_PROP_YES 1` / `SIGNAL_PROP_NO 2` — `src/event_signal.h:90-92`

Tri-state for `struct signal::active`. `SIGNAL_PROP_UD` (0) means "the user did not constrain
this" and is also the value a zero-initialised `struct signal` gets, which is what makes
`struct signal signal = {0}` at `src/message.c:2877` correct.

#### `struct event_signal` — `src/event_signal.h:94-102`

96 bytes, alignment 8.

| Field | C type | Owner of the pointee | Threads |
| --- | --- | --- | --- |
| `type` | `enum signal_type` (`int`) | — | written on the event-loop pthread, read in the forked child |
| `arg_name[4]` | `char *` | **temporary-storage arena**, allocated by `ts_alloc_unaligned` (`src/event_signal.c:129` and friends). Nobody frees; `ts_reset()` reclaims the whole arena. | same |
| `arg_value[4]` | `char *` | same arena | same |
| `app` | `char *` | **three different owners depending on the signal type** — see below | same |
| `title` | `char *` | temporary-storage arena, from `window_title_ts` (`src/window.c:724`) | same |
| `active` | `int` | — | same |

`app` ownership, exhaustively:

* `src/event_signal.c:135`, `:183`, `:197`, `:224` — points at `application->name`, a `malloc`ed
  string **owned by `struct application`**, borrowed. Safe only because the application outlives
  the flush.
* `src/event_signal.c:146`, `:209` — `ts_string_copy(...)`, arena-owned. Required at these two
  sites because `application_destroy` (`src/event_loop.c:319`) and `window_destroy`
  (`src/event_loop.c:315`) free the original later *in the same handler*.
* `src/event_signal.c:209` also can be the string literal `"<unknown>"` — static storage — when
  `window->application` was already NULLed at `src/event_loop.c:279`.
* For `SIGNAL_APPLICATION_FRONT_SWITCHED`, every `SIGNAL_SPACE_*`, every `SIGNAL_DISPLAY_*`,
  `SIGNAL_MISSION_CONTROL_*`, `SIGNAL_DOCK_*`, `SIGNAL_MENU_BAR_HIDDEN_CHANGED` and
  `SIGNAL_SYSTEM_WOKE`, **`app`, `title` and `active` are left uninitialised** — `event_signal_push`
  only zeroes `type` and the eight `arg_*` slots (`src/event_signal.c:110-118`). That is not a bug
  today because `event_signal_filter` returns `false` from its `default:` arm
  (`src/event_signal.c:11`) for exactly those types and never reads the fields. A Rust translation
  that zero-initialises everything is strictly safer and observably identical.

Nothing in this struct is passed to the OS, so its layout is free to change.

#### `struct signal` — `src/event_signal.h:104-117`

112 bytes.

| Field | C type | Owner | Threads |
| --- | --- | --- | --- |
| `app` | `char *` | `malloc`ed by `string_copy` at `src/message.c:2893`; freed by `event_signal_destroy` (`src/event_signal.c:364`) | event-loop pthread only |
| `title` | `char *` | `string_copy` at `src/message.c:2901`; freed at `src/event_signal.c:365` | same |
| `app_regex_valid` | `bool` | — | same |
| `title_regex_valid` | `bool` | — | same |
| `app_regex_exclude` | `bool` | — | same |
| `title_regex_exclude` | `bool` | — | same |
| `app_regex` | `regex_t` (32 bytes) | libc owns the `re_guts` behind it; released by `regfree` at `src/event_signal.c:360`, and only if `app_regex_valid` | same |
| `title_regex` | `regex_t` | `regfree` at `src/event_signal.c:361`, gated on `title_regex_valid` | same |
| `active` | `int` | tri-state `SIGNAL_PROP_*` | same |
| `command` | `char *` | `string_copy` at `src/message.c:2923`; freed at `src/event_signal.c:362`; **read in the forked child** at `src/event_signal.c:91` | event-loop pthread + forked child (read-only, post-fork snapshot) |
| `label` | `char *` | `string_copy` at `src/message.c:2891`; freed at `src/event_signal.c:363` | event-loop pthread only |

`regex_t` on macOS is `{ int re_magic; size_t re_nsub; const char *re_endp; struct re_guts *re_g; }`
(32 bytes, from `$SDK/usr/include/_regex.h:113-118`). It holds no self-referential pointers, which
is why `buf_push(g_signal_event[type], *signal)` at `src/event_signal.c:355` can bitwise-copy a
live compiled regex out of a caller's stack frame and have it keep working.

### 1.3 Globals declared here

None. The header only declares prototypes (`src/event_signal.h:119-127`); the storage lives in
`src/yabai.c:27` and `src/yabai.c:32`.

### 1.4 Callbacks registered here

None.

---

## 2. `src/event_signal.c` (454 lines)

### 2.1 Purpose

Implements the whole signal subsystem: the per-event subscriber lists, the filter that decides
whether a subscription matches a fired event, the push path that snapshots an event's environment
variables into a queue, the fork/exec flush path that actually runs the user's shell command, and
the add / remove / list commands reached from the socket.

### 2.2 Globals

Declared `extern` at the top of this file, defined in `src/yabai.c`:

| Symbol | Type | Definition | Initial value | Threads | Synchronisation |
| --- | --- | --- | --- | --- | --- |
| `g_signal_event` | `struct signal *[SIGNAL_TYPE_COUNT]` — 30 stretchy buffers (`src/misc/sbuffer.h`) | `src/yabai.c:27` | all NULL (BSS) | event-loop pthread writes and reads; forked child reads the snapshot | **none** — single writer thread |
| `g_signal_storage` | `struct memory_pool` (`src/misc/memory_pool.h:4-9`) | `src/yabai.c:32` | `memory_pool_init(&g_signal_storage, KILOBYTES(256))` at `src/yabai.c:283` — 256 KiB `mmap` plus one `PROT_NONE` guard page | event-loop pthread writes `used`; forked child reads `memory` and `used` | `used` is `volatile uint64_t` bumped with `__sync_fetch_and_add` at `src/event_signal.c:107`; zeroed non-atomically at `src/event_signal.c:66` |
| `g_process_manager` | `struct process_manager` (`src/process_manager.h:17-28`) | `src/yabai.c:28` | zeroed | read here at `src/event_signal.c:156`, `:158`, `:160`, `:170`, `:172`, `:184`; written on the event-loop pthread at `src/event_loop.c:382-384` and on the main run loop at `src/process_manager.c:248-250` | none |
| `g_display_manager` | `struct display_manager` | `src/yabai.c:29` | zeroed | read at `src/event_signal.c:304-305` | none |
| `g_space_manager` | `struct space_manager` | `src/yabai.c:31` | zeroed | read at `src/event_signal.c:252-253` | none |
| `g_window_manager` | `struct window_manager` | `src/yabai.c:30` | zeroed | read at `src/event_signal.c:210`, `:226` (`focused_window_id`) | none |

`g_signal_storage` capacity in practice: `sizeof(struct event_signal)` is 96, so 256 KiB holds
**2730** queued signals. `event_signal_push` does **not** bounds-check (contrast
`memory_pool_push`, `src/misc/memory_pool.h:29-45`, which wraps): pushing a 2731st signal before a
flush writes into the guard page and the daemon dies with `SIGSEGV`. Unreachable today because
`event_signal_flush` runs after every single event (`src/event_loop.c:1670`), so `used` is at most
a handful of records. Worth keeping in the Rust translation's notes: a `Vec` removes the hazard
entirely.

Also worth naming: `debug` (`src/misc/log.h:6-15`) reads `g_verbose`, defined at `src/yabai.c:51`
and set at `src/yabai.c:248`, during argument parsing, before any thread exists.

There are no function-local statics in this file.

### 2.3 Functions

All six public functions and the two statics run **on the event-loop pthread**, established by:
every `event_signal_push` call site is inside an `EVENT_HANDLER_*` body in `src/event_loop.c`
(lines 173, 223, 261, 309, 378, 381, 385, 421, 487, 548, 595, 623, 672, 698, 748, 870, 921, 941,
976, 990, 1028, 1080, 1089, 1098, 1106, 1114, 1456, 1463, 1470, 1482, 1541, 1558, 1591, 1598,
1611); `event_signal_flush` is called at `src/event_loop.c:1670` from `event_loop_run`; and the
add / remove / list / from_string entry points are reached only through
`handle_domain_signal` (`src/message.c:2864`), itself reached only through
`EVENT_HANDLER(DAEMON_MESSAGE)`.

#### `static bool event_signal_filter(struct event_signal *es, struct signal *signal)` — `src/event_signal.c:8-58`

Returns `true` when the queued event should **not** be delivered to this subscription (it is a
reject predicate, not a match predicate — `event_signal_flush` does `if (...) continue;`).

Thread: event-loop pthread, but also runs inside the **forked child** — `event_signal_flush` calls
it at `src/event_signal.c:81`, after the fork at line 64.

Allocates nothing. External symbols: `regexec` via the inline `regex_match`
(`src/misc/helpers.h:573-579`).

Structure: a `switch` on `es->type` with a `default: return false` that passes everything through
for the 14 types that carry no filterable data. The four groups:

* app only — `LAUNCHED`, `ACTIVATED`, `DEACTIVATED`, `VISIBLE` (`:13-19`).
* app + active — `TERMINATED`, `HIDDEN`, `WINDOW_DESTROYED` (`:20-30`).
* app + title — `WINDOW_CREATED`, `WINDOW_FOCUSED`, `WINDOW_DEMINIMIZED` (`:31-41`).
* app + title + active — `WINDOW_MOVED`, `WINDOW_RESIZED`, `WINDOW_MINIMIZED`,
  `WINDOW_TITLE_CHANGED` (`:42-56`).

The tri-state is the subtle part. `regex_match` returns `REGEX_MATCH_UD` (0) when the subscription
has no regex, `REGEX_MATCH_YES` (1) on a hit, `REGEX_MATCH_NO` (2) on a miss
(`src/misc/macros.h:22-24`). The code builds `regex_match_app = exclude ? YES : NO` and rejects
when `regex_match(...) == regex_match_app`. With no regex compiled the result is `UD`, which
equals neither, so an unset filter never rejects. **Collapsing this to a `bool` loses the
unset case.**

The active check at `:26-27` and `:52-53`:

```c
bool active = signal->active == SIGNAL_PROP_UD;
if (!active) active = es->active == (signal->active == SIGNAL_PROP_YES);
```

`es->active` is an `int` holding 0 or 1 (assigned from a comparison at `:158`, `:160`, `:184`,
`:210`, `:226`); the right-hand side is a `bool` promoted to 0 or 1. Reject when `!active`.

#### `void event_signal_flush(void)` — `src/event_signal.c:60-97`

Drains the queue by forking once, then forking again per (event × matching subscription) and
`execvp`-ing `/usr/bin/env sh -c <command>`.

Thread: entered on the event-loop pthread (`src/event_loop.c:1670`); everything from line 70
onwards executes in a **forked child**.

Sequence, precisely:

1. `:62` — bail if `used == 0`.
2. `:64` `fork()`. Parent (`pid` non-zero, **including `-1` on failure**) zeroes `used` at `:66`
   and returns. On fork failure every queued signal is silently dropped.
3. Child computes `count = used / sizeof(struct event_signal)` at `:70-71` from its COW snapshot.
4. For each queued event: `debug` at `:77` — note this writes to the child's inherited `stdout`
   buffer; `buf_len(g_signal_event[es->type])` at `:76`.
5. For each subscription that survives the filter: `fork()` again at `:83`. The intermediate
   parent `continue`s; the grandchild `setenv`s up to four pairs (`:86-89`) and
   `exit(execvp(exec[0], exec))` at `:92`, with
   `char *exec[] = { "/usr/bin/env", "sh", "-c", signal->command, NULL }` at `:91`.
6. `:96` — the intermediate child `exit(EXIT_SUCCESS)`.

Allocates nothing. Frees nothing. Nobody `wait`s: `signal(SIGCHLD, SIG_IGN)` at `src/yabai.c:151`
makes the kernel reap, which is what makes the spawned commands detached.

External symbols: `fork`, `setenv`, `execvp`, `exit`.

Behaviours a naive translation loses:

* `int pid = fork();` at `:83` **shadows** the `pid_t pid` from `:64`. Harmless, but do not
  "clean it up" into one variable.
* `exit()` (not `_exit()`) in both children. `exit` flushes the stdio buffers inherited from the
  parent, so anything the parent had buffered on `stdout`/`stderr` is written a second time by
  each child. Matching this in Rust means `libc::exit`, not `libc::_exit` and not
  `std::process::exit` (which runs Rust's own at-exit path).
* `execvp` returns `-1` on failure, so `exit(-1)` yields wait status 255.
* Between `fork` and `execvp` the child calls `setenv`, which is not async-signal-safe. It works
  because the forking thread holds no allocator lock at that moment. In Rust, **allocating**
  between fork and exec (building a `CString`, formatting a string, growing a `Vec`) can deadlock
  on the malloc lock held by another thread at fork time. Every `CString` must be built before the
  fork.

#### `void event_signal_push(enum signal_type type, void *context)` — `src/event_signal.c:99-341`

Records one fired event plus its environment variables into `g_signal_storage`, but only if at
least one subscription exists for that type (`:101-102`).

Thread: event-loop pthread, from 35 call sites in `src/event_loop.c`.

Allocates: `ts_alloc_unaligned(128)` per environment name and value (up to 8 per event), and
`ts_string_copy` at `:146` and `:209`. All of it lives in the temporary-storage arena
(`src/misc/ts.h`), reclaimed wholesale by the `ts_reset()` at `src/event_loop.c:1671`. Nobody
frees individually. One slot of `g_signal_storage` is claimed by
`__sync_fetch_and_add(&g_signal_storage.used, size)` at `:107`.

External symbols: `__sync_fetch_and_add`, `snprintf`, `GetCurrentEventTime` (Carbon, `:156`).

`context` is a tagged `void *` whose meaning depends on `type` — the type tag is the `type`
parameter, and the union is implicit:

| Types | `context` is | Decoded at |
| --- | --- | --- |
| `APPLICATION_LAUNCHED`/`ACTIVATED`/`DEACTIVATED`/`VISIBLE`/`TERMINATED`/`HIDDEN` | `struct application *` | `:127`, `:138`, `:175` |
| `WINDOW_*` | `struct window *` | `:189`, `:201`, `:216` |
| `SPACE_CREATED`/`SPACE_DESTROYED` | `uint64_t` space id cast through `uintptr_t` | `:229`, `:243` |
| `DISPLAY_ADDED`/`MOVED`/`RESIZED`/`REMOVED` | `uint32_t` display id cast through `uintptr_t` | `:281`, `:295` |
| `MISSION_CONTROL_ENTER`/`EXIT` | `enum mission_control_mode` cast through `uintptr_t` | `:332` |
| `APPLICATION_FRONT_SWITCHED`, `SPACE_CHANGED`, `DISPLAY_CHANGED`, `DOCK_*`, `MENU_BAR_HIDDEN_CHANGED`, `SYSTEM_WOKE` | `NULL`, state is read from globals | `:170`, `:252`, `:304` |

Environment variables emitted, for the socket contract:

* `YABAI_PROCESS_ID` (`:132`, `:143`, `:169`, `:180`), `YABAI_RECENT_PROCESS_ID` (`:171`)
* `YABAI_WINDOW_ID` (`:194`, `:206`, `:221`)
* `YABAI_SPACE_ID` (`:237`, `:248`, `:268`), `YABAI_RECENT_SPACE_ID` (`:270`),
  `YABAI_SPACE_INDEX` (`:239`, `:273`), `YABAI_RECENT_SPACE_INDEX` (`:275`)
* `YABAI_DISPLAY_ID` (`:289`, `:300`, `:320`), `YABAI_RECENT_DISPLAY_ID` (`:322`),
  `YABAI_DISPLAY_INDEX` (`:291`, `:325`), `YABAI_RECENT_DISPLAY_INDEX` (`:327`)
* `YABAI_MISSION_CONTROL_MODE` (`:337`), value from `mission_control_mode_str`
  (`src/mission_control.c:38-44`)

Format-string details that a Rust rewrite must reproduce byte for byte, because these values go
straight into the user's shell:

* `:133`, `:144`, `:170`, `:172`, `:181` — `"%d"` on `pid_t` (`int32_t`). Fine.
* `:195`, `:207`, `:222` — `"%d"` on `window->id`, a **`uint32_t`**, printed as signed. Rust must
  write `window.id as i32`.
* `:238`, `:249`, `:269`, `:271` — `"%lld"` on a **`uint64_t`** space id, printed as signed. Rust
  must write `sid as i64`.
* `:290`, `:301`, `:321`, `:323` — `"%d"` on a **`uint32_t`** display id, printed as signed. Rust
  must write `did as i32`.
* `arg_size` is 128 and `snprintf` truncates silently. Rust `write!` into a fixed buffer must
  truncate the same way rather than grow.

The `APPLICATION_TERMINATED` arm at `:137-162` is the only place that reads Carbon time:

```c
EventTime dt = GetCurrentEventTime() - g_process_manager.switch_event_time;
if (dt >= 0.05f) { ... } else { ... }
```

`EventTime` is `double`. `0.05f` is a `float` literal promoted to `double`, i.e.
`0.05000000074505806`, **not** `0.05`. Writing `0.05_f64` in Rust changes the threshold in the 9th
decimal; write `f64::from(0.05_f32)`.

#### `enum signal_type signal_type_from_string(const char *str)` — `src/event_signal.c:343-350`

Linear scan from index 1 to 29 comparing against `signal_type_str`; returns `SIGNAL_TYPE_UNKNOWN`
on no match. Starting at 1 means the literal string `"signal_type_unknown"` can never be selected
by a user. Thread: event-loop pthread (`src/message.c:2928`). Allocates nothing. Calls
`string_equals` → `strcmp`.

Note the implicit `int` → `enum signal_type` conversion on `return i`.

#### `void event_signal_add(enum signal_type type, struct signal *signal)` — `src/event_signal.c:352-356`

Removes any existing subscription with the same label, then bitwise-copies `*signal` onto the tail
of `g_signal_event[type]`. Ownership of every pointer and both `regex_t`s **moves** into the
buffer; the caller at `src/message.c:2955` correctly does not destroy its local afterwards.

Thread: event-loop pthread. Allocates through `realloc` inside `buf__grow_f`
(`src/misc/sbuffer.h:22-32`); that block is freed only by `buf_free`, which this subsystem never
calls — the buffers live for the process lifetime.

#### `void event_signal_destroy(struct signal *signal)` — `src/event_signal.c:358-366`

Frees everything a `struct signal` owns: `regfree` both regexes if their `*_valid` flag is set,
then `free` `command`, `label`, `app`, `title`. Does **not** zero the struct afterwards, so calling
it twice is a double free. Called from `src/message.c:2957` (parse failed),
`src/event_signal.c:374` and `:390` (removal).

External symbols: `regfree`, `free`.

#### `bool event_signal_remove_by_index(int index)` — `src/event_signal.c:368-383`

Walks types 1..29 and, within each, the subscription vector, counting a flat `signal_index`. On a
hit: destroy, `buf_del`, return `true`. The numbering is exactly the numbering
`event_signal_list` prints, so the two must keep the same traversal order.

`buf_del` (`src/misc/sbuffer.h:19`) is a **swap-remove**: it copies the last element over the
removed slot and decrements the length. Indices of later subscriptions therefore change after a
removal, and `signal ls` reorders. Rust must use `Vec::swap_remove`, not `Vec::remove`.

A negative `index` never equals a `signal_index` that starts at 0 and only increments, so it
returns `false` — preserved for free if the parameter stays `i32`.

#### `bool event_signal_remove(char *label)` — `src/event_signal.c:385-398`

Same walk, matching on `string_equals(label, ...->label)`. `string_equals`
(`src/misc/helpers.h:254-257`) is `a && b && strcmp(a, b) == 0`, so a NULL label on either side
never matches — subscriptions added without `label=` can never be removed by name.

#### `static void event_signal_serialize(FILE *rsp, struct signal *signal, enum signal_type type, int index)` — `src/event_signal.c:400-429`

Writes one JSON object to the response stream. `TIME_FUNCTION` at `:402` is a no-op unless the
build defines `PROFILE >= 1` (`src/misc/timer.h:136-162`).

Escapes `app`, `title` and `command` with `ts_string_escape` (`src/misc/helpers.h:259`), which
returns **`NULL` when no escaping was needed**, hence the three-way fallbacks at `:424-425`,
`:428`: `escaped ? escaped : raw ? raw : ""`. The escaped copy lands in the temporary-storage
arena.

`json_optional_bool(signal->active)` (`src/misc/helpers.h:225-230`) maps 0 → `"null"`, 1 → `"true"`,
anything else → `"false"`. Note this emits a bare JSON `null` for the common unset case, not the
string `"null"` — the field is not quoted in the format string at `:418`.

Allocates: arena only. External symbols: `fprintf`.

#### `void event_signal_list(FILE *rsp)` — `src/event_signal.c:431-454`

Prints the whole subscription set as a JSON array. The comma logic is worth transcribing exactly:
`event_did_output` guards the separator *between* types (`:441-443`), `j < buf_len(...) - 1` guards
the separator *within* a type (`:447`), and `event_did_output` is only ever set to true
(`:451`). Terminates with `"]\n"`.

Thread: event-loop pthread, from `src/message.c:2973`.

### 2.4 Callbacks registered with the OS or the run loop

**None.** This file registers no `AXObserver`, no `CGEventTap`, no Carbon handler, no
`SLSRegisterConnectionNotifyProc`, no `NSNotification` observer, no dispatch block, no
`CVDisplayLink` and no signal handler. It is purely called into. The signal disposition it relies
on (`SIGCHLD` → `SIG_IGN`) is installed elsewhere, at `src/yabai.c:151`.

### 2.5 Comments present in this file

One block, `src/event_signal.c:148-154` — see section 7.

---

## 3. `src/rule.h` (78 lines)

### 3.1 Purpose

Declares the window-rule type: the matcher (`struct rule`, four optional regexes over app name,
title, role and subrole) and the payload it applies (`struct rule_effects`). Also defines the
six inline bit-flag helpers that every caller uses instead of touching `flags` directly. Included
from `src/manifest.m:68`.

### 3.2 Types and constants

#### `#define RULE_PROP_UD 0` / `RULE_PROP_ON 1` / `RULE_PROP_OFF 2` — `src/rule.h:4-6`

Tri-state for `manage`, `sticky`, `mff`, `fullscreen`. `UD` is the zero value, which is what makes
`struct rule rule = {0}` at `src/message.c:2808` correct and what `rule_combine_effects` tests
against at `src/rule.c:98-101`.

#### `enum rule_flag` — `src/rule.h:8-20`

Bit flags stored in `struct rule::flags`, a `uint16_t`:

`RULE_APP_VALID` 0x001, `RULE_TITLE_VALID` 0x002, `RULE_ROLE_VALID` 0x004,
`RULE_SUBROLE_VALID` 0x008, `RULE_APP_EXCLUDE` 0x010, `RULE_TITLE_EXCLUDE` 0x020,
`RULE_ROLE_EXCLUDE` 0x040, `RULE_SUBROLE_EXCLUDE` 0x080, `RULE_ONE_SHOT` 0x100,
`RULE_ONE_SHOT_REMOVE` 0x200.

The four `*_VALID` bits double as "this `regex_t` has been compiled and must be `regfree`d"
(`src/rule.c:209-212`) — they are the discriminant of an optional.

`RULE_ONE_SHOT_REMOVE` is set by `window_manager_apply_*_rules_to_window`
(`src/window_manager.c:187`, `:211`) and consumed by two sweeps that delete the rule:
`src/event_loop.c:568-577` and `src/window_manager.c:1565-1574`.

#### `enum rule_effects_flag` — `src/rule.h:22-27`

`RULE_FOLLOW_SPACE` 0x01, `RULE_OPACITY` 0x02, `RULE_LAYER` 0x04. Stored in
`struct rule_effects::flags`, a `uint16_t`. These three mark "the user set this field", because
`0.0` opacity and layer `0` (`LAYER_AUTO`) are both meaningful values.

#### `struct rule_effects` — `src/rule.h:29-42`

80 bytes; `scratchpad` at offset 64, `flags` at offset 72.

| Field | C type | Owner | Threads |
| --- | --- | --- | --- |
| `did` | `uint32_t` | — | event-loop pthread only |
| `sid` | `uint64_t` | — | same |
| `opacity` | `float` | — | same |
| `manage`, `sticky`, `mff`, `fullscreen` | `int`, tri-state `RULE_PROP_*` | — | same |
| `layer` | `int` | holds a `CGWindowLevelKey`: `LAYER_AUTO` 0, `LAYER_BELOW` `kCGBackstopMenuLevelKey` 3, `LAYER_NORMAL` `kCGNormalWindowLevelKey` 4, `LAYER_ABOVE` `kCGFloatingWindowLevelKey` 5 (`src/misc/macros.h:42-45`) | same |
| `grid[6]` | `unsigned[6]` | rows, cols, x, y, w, h — parsed with one `sscanf` at `src/message.c:2705-2709` | same |
| `scratchpad` | `char *` | `malloc`ed by `string_copy` at `src/message.c:2632` or `src/rule.c:95`; freed by `rule_destroy` (`src/rule.c:220`) or by `rule_combine_effects` (`src/rule.c:94`) | same |
| `flags` | `uint16_t` | — | same |

#### `struct rule` — `src/rule.h:44-57`

256 bytes.

| Field | C type | Owner | Threads |
| --- | --- | --- | --- |
| `label` | `char *` | `string_copy` at `src/message.c:2618`; freed `src/rule.c:214` | event-loop pthread only |
| `app` | `char *` | `string_copy` at `src/message.c:2639`; freed `src/rule.c:215` | same |
| `title` | `char *` | `string_copy` at `src/message.c:2648`; freed `src/rule.c:216` | same |
| `role` | `char *` | `string_copy` at `src/message.c:2658`; freed `src/rule.c:217` | same |
| `subrole` | `char *` | `string_copy` at `src/message.c:2668`; freed `src/rule.c:218` | same |
| `app_regex` … `subrole_regex` | `regex_t` ×4 | libc; `regfree`d by `rule_destroy` only when the matching `*_VALID` flag is set | same |
| `effects` | `struct rule_effects` | by value | same |
| `flags` | `uint16_t` | — | same |

Note the raw strings are kept **in addition to** the compiled regexes, purely so `rule_serialize`
can echo the user's pattern back (`src/rule.c:8-16`). They are populated even when `regcomp`
fails (`src/message.c:2639-2645`), so `app != NULL` does not imply `RULE_APP_VALID`.

#### Inline flag helpers — `src/rule.h:59-65`

`rule_check_flag` / `rule_clear_flag` / `rule_set_flag`, and the same trio for
`rule_effects_*`. `rule_check_flag` returns `bool` from `r->flags & x`, an implicit non-zero
test.

### 3.3 Globals declared here

None. The rule vector lives in `struct window_manager::rules`
(`src/window_manager.h:85`), a stretchy buffer of `struct rule`, inside the global
`g_window_manager` (`src/yabai.c:30`).

### 3.4 Callbacks

None.

### 3.5 Comments

None.

---

## 4. `src/rule.c` (221 lines)

### 4.1 Purpose

Implements the lifetime and application of window rules: serialising one rule to JSON, folding
several matching rules into one combined effect set, re-applying rules across every known window,
and adding / removing / destroying rules. It owns no storage of its own — the vector lives in
`g_window_manager.rules`.

### 4.2 Globals

Declared `extern` at `src/rule.c:1-2`:

| Symbol | Type | Definition | Threads | Synchronisation |
| --- | --- | --- | --- | --- |
| `g_space_manager` | `struct space_manager` | `src/yabai.c:31` | read/passed at `src/rule.c:47`, `:121`, `:125`, `:164`, `:168` | none |
| `g_window_manager` | `struct window_manager` | `src/yabai.c:30` | `.window` table iterated at `:115`, `:161`; `.rules` read and mutated at `:133`, `:147`, `:178`, `:183`, `:186`, `:196`, `:199` | none |

No statics, no function-local statics.

### 4.3 Functions

Every function here runs **on the event-loop pthread**: the only callers are
`window_manager_query_window_rules` (`src/window_manager.c:32`, reached from
`src/message.c:2857`), `handle_domain_rule` (`src/message.c:2806-2859`),
`window_manager_apply_*_rules_to_window` (`src/window_manager.c:185`, `:209`) and the one-shot
sweeps at `src/event_loop.c:571` and `src/window_manager.c:1568` — all inside `EVENT_HANDLER_*`
bodies.

#### `void rule_serialize(FILE *rsp, struct rule *rule, int index)` — `src/rule.c:4-61`

Writes one rule as a JSON object. Allocates escaped copies of `app`, `title`, `role`, `subrole` in
the temporary-storage arena via `ts_string_escape`; nobody frees them individually. Calls
`display_manager_display_id_arrangement` (`src/display_manager.h:70`) and
`space_manager_mission_control_index` (`src/space_manager.h:62`), both of which hit SkyLight.
External symbols: `fprintf`.

Four formatting traps:

1. `src/rule.c:60` — `(uint32_t)(rule->effects.flags << 16) | (uint32_t)rule->flags`.
   `effects.flags` is `uint16_t`, promoted to `int` before the shift. Today the highest effects
   bit is `RULE_LAYER` 0x04, so the result fits; if a future bit reached 0x8000 this would be
   signed overflow (UB). Rust: `((effects.flags as u32) << 16) | (rule.flags as u32)`.
2. `src/rule.c:53` — `layer_str[rule->effects.layer]`. `layer_str`
   (`src/misc/helpers.h:175-181`) is a 6-entry designated-initializer array with **holes at
   indices 1 and 2** (`kCGMinimumWindowLevelKey`, `kCGDesktopWindowLevelKey`). `%s` with a NULL
   pointer prints `(null)` on macOS. Unreachable via the message parser
   (`src/message.c:2757-2775` only sets 0, 3, 4, 5) but the Rust lookup must decide explicitly
   what an out-of-range layer prints.
3. `src/rule.c:49` — `%.4f` on a `float` promoted to `double`. Rust should format
   `opacity as f64` with `{:.4}`, not the `f32`, to get the same digits in the edge cases.
4. `src/rule.c:55-57` — `%d` on `unsigned grid[6]`. Printed as signed. Rust: cast each to `i32`.

Also `json_bool` at `:48` and `:59` returns `"true"`/`"false"`; `json_optional_bool` at `:50-52`,
`:54` returns `"null"`/`"true"`/`"false"` unquoted.

#### `void rule_combine_effects(struct rule_effects *effects, struct rule_effects *result)` — `src/rule.c:63-111`

Folds one rule's effects into an accumulator. The parameter is named `effects` in the definition
but `rule_effects` in the declaration (`src/rule.h:68`) — cosmetic.

Ordering matters: later rules win, but only for fields they actually set. The `did` and `sid` arms
(`:65-81`) both *overwrite* `RULE_FOLLOW_SPACE` on the result — setting it if this rule had it,
**clearing it otherwise** — so a later `display=` without `^` cancels an earlier `space=^`.

`:93-96` frees the accumulator's `scratchpad` and replaces it with a fresh `string_copy`. The
accumulator is a caller-local `struct rule_effects effects = {0}`
(`src/window_manager.c:175`, `:195`) that is **never destroyed**, so the last `scratchpad` copy
leaks on every rule application that names one. Using `Option<String>` in Rust fixes the leak; say
so rather than reproducing it, since the only observable difference is memory.

Allocates: `string_copy` → `malloc`. Frees: the previous `result->scratchpad`.

#### `void rule_reapply_all(void)` — `src/rule.c:113-129`

Iterates every window in `g_window_manager.window` via the `table_for` macro
(`src/misc/hashtable.h:34-41`) and, for root windows, recomputes title/role/subrole and re-runs
both rule passes with `one_shot_rules = false`.

Allocates: three arena strings per window (`window_title_ts` `src/window.c:724`,
`window_role_ts` `src/window.c:1001`, `window_subrole_ts` `src/window.c:1022`). All reclaimed by
the `ts_reset()` at the end of the handler.

`window->is_eligible` is written at `:124`.

Iteration safety, checked: `window_manager_apply_manage_rules_to_window` and
`window_manager_apply_rules_to_window` can move windows between spaces, change layers, opacity and
scratchpads, and they mutate `wm->managed_window` and the view trees — but the only writers of
`wm->window` itself are `window_manager_add_window` / `window_manager_remove_window`
(`src/window_manager.c:1399-1407`), reachable only from
`window_manager_create_and_add_window` (`src/window_manager.c:1471`),
`EVENT_HANDLER(APPLICATION_TERMINATED)` (`src/event_loop.c:313`) and
`EVENT_HANDLER(WINDOW_DESTROYED)` (`src/event_loop.c:627`). None of those is reachable from here,
so the bucket chains are stable for the duration of the loop.

#### `bool rule_reapply_by_index(int index)` — `src/rule.c:131-143`

Linear scan comparing `i == index`; on a hit, applies the rule unless it is `RULE_ONE_SHOT`, and
returns `true` either way. Returns `false` for an out-of-range or negative index. Note the
asymmetry: a one-shot rule found by index reports success without doing anything.

#### `bool rule_reapply_by_label(char *label)` — `src/rule.c:145-157`

Same, matching `string_equals(rules[i].label, label)` — NULL on either side never matches.

#### `void rule_apply(struct rule *rule)` — `src/rule.c:159-173`

Applies one rule to every matching root window. The three `_ts` calls at `:163` are evaluated as
arguments to `window_manager_rule_matches_window` for **every** root window, matching or not, and
each one allocates a fresh arena string — three arena allocations per root window per call.

`window->is_eligible` written at `:167`.

#### `void rule_add(struct rule *rule)` — `src/rule.c:175-179`

Removes any rule with the same label, then `buf_push(g_window_manager.rules, *rule)` — a bitwise
move of the caller's stack struct, including four `regex_t`s and six owned pointers. Caller at
`src/message.c:2821` correctly does not destroy its local afterwards; the failure path at
`src/message.c:2823` does.

#### `bool rule_remove_by_index(int index)` / `bool rule_remove_by_label(char *label)` — `src/rule.c:181-205`

Find, `rule_destroy`, `buf_del` (swap-remove), return `true`. Same index-reordering consequence as
the signal path: `rule ls` output order changes after a removal.

#### `void rule_destroy(struct rule *rule)` — `src/rule.c:207-221`

`regfree` each regex whose `*_VALID` flag is set, then `free` `label`, `app`, `title`, `role`,
`subrole` and `effects.scratchpad`. Does not clear the flags or NULL the pointers, so a second
call is a double free. External symbols: `regfree`, `free`.

Gotcha for the one-shot sweeps: `src/event_loop.c:568-577` calls `rule_destroy` then `buf_del`
inside a loop, decrementing both the index and the cached length; `src/window_manager.c:1565-1574`
does the same. Any Rust replacement must keep the swap-remove semantics for those loops to stay
correct.

### 4.4 Callbacks

None registered here.

### 4.5 Comments

None in this file.

---

## 5. `src/message.h` (7 lines)

### 5.1 Purpose

The entire public surface of the 3000-line `src/message.c`: parse and execute one socket message,
and start the accept loop. Everything else in `message.c` is `static`. Included from
`src/manifest.m:69`.

### 5.2 Types, constants, globals

None declared here. For phase 2 the two things behind these prototypes that matter:

* `static struct { int sockfd; bool is_running; pthread_t thread; } g_message_loop;` —
  `src/message.c:1-5`. File-local, zero-initialised. `sockfd` and `is_running` are written by
  `message_loop_begin` on the **main thread** before the accept thread starts, then read by
  `message_loop_run` on the **message-loop pthread**. No synchronisation; `is_running` is never
  set back to false anywhere in the tree, so the loop only ends when the process does.
* `struct token { char *text; int length; }` — `src/message.c:254-258`, and
  `struct token_value` — `src/message.c:270-281`, a tagged union
  (`enum token_type` tag + anonymous union of `int` / `float` / `uint32_t` / `char *`).

### 5.3 Functions

#### `void handle_message(FILE *rsp, char *message)` — declared `src/message.h:4`, defined `src/message.c:2979-2999`

Dispatches on the first token to one of seven domain handlers (`config`, `display`, `space`,
`window`, `query`, `rule`, `signal`), or writes an "unknown domain" failure.

Thread: **event-loop pthread**, from `EVENT_HANDLER(DAEMON_MESSAGE)` at `src/event_loop.c:1634`.
This is the fact that makes every signal and rule mutation single-threaded.

`message` points into the temporary-storage arena — allocated at `src/event_loop.c:1623` with
`ts_alloc_unaligned(bytes_to_read)` and filled straight from the socket. The payload is the
client's `argv` joined by NUL bytes, with a trailing extra NUL and a leading `int` byte count
(`client_send_message`, `src/yabai.c:54-81`). `get_token`
(`src/message.c:298-315`) walks to the next NUL and steps over it, and `parse_key_value_pair`
(`src/message.c:440-470`) **writes a `'\0'` into the buffer in place** at `src/message.c:462` to
split `key=value`. Destructive in-place tokenisation of a borrowed buffer.

`rsp` is a `FILE *` from `fdopen(param1, "w")` at `src/event_loop.c:1632`; the handler
`fflush`es and `fclose`s it at `src/event_loop.c:1636-1637`, which closes the socket fd. `rsp` can
legitimately be NULL in some internal paths — `daemon_fail` guards with `if (!rsp) return;`
(`src/message.c:420`).

Allocates: arena strings throughout, plus `malloc` for the strings that become rule / signal
fields. Frees: nothing directly.

#### `bool message_loop_begin(char *socket_path)` — declared `src/message.h:5`, defined `src/message.c:3016-3050`

Creates the `AF_UNIX`/`SOCK_STREAM` socket, `unlink`s any stale path, `bind`s, `chmod 0600`s,
`listen(SOMAXCONN)`s, sets `FD_CLOEXEC`, sets `is_running` and spawns the accept thread.

Thread: **main thread**, from `src/yabai.c:344`, before `CFRunLoopRun`.

External symbols: `socket`, `bind`, `chmod`, `listen`, `unlink`, `fcntl`, `snprintf`,
`pthread_create`.

`FD_CLOEXEC` on the listening socket is load-bearing for `event_signal_flush`: without it every
forked signal command would inherit and hold the listening socket open.

### 5.4 Callbacks

None in the header. The one thread entry point behind it is
`static void *message_loop_run(void *context)` — `src/message.c:3003-3013`, registered by the
`pthread_create` at `src/message.c:3042`, context `NULL` (it reads the file-local
`g_message_loop` instead). It runs for the process lifetime and only ever calls `accept` and
`event_loop_post`.

### 5.5 Comments

None.

---

## 6. C pattern catalogue and the Rust translation for each

Ordered roughly by how much of the translation they decide.

### 6.1 Stretchy buffers (`buf_push` / `buf_del` / `buf_len`)

Sites: `src/event_signal.c:76`, `:101`, `:355`, `:372`, `:375`, `:388`, `:391`, `:439`, `:447`;
`src/rule.c:133`, `:147`, `:178`, `:183`, `:186`, `:196`, `:199`. Implementation:
`src/misc/sbuffer.h:11-32` — a `{int len; int cap; char buf[0];}` header stored immediately before
the elements, grown with `realloc`, never freed in these paths.

**Rust:** `Vec<Signal>` and `Vec<Rule>`.

* `buf_push(b, x)` → `vec.push(x)`.
* `buf_len(b)` → `vec.len()` — but the C type is `int`, and comparisons against user-supplied
  `int` indices are everywhere. Keep public index parameters as `i32` and compare before
  converting.
* **`buf_del(b, x)` → `vec.swap_remove(x)`, never `vec.remove(x)`.** `src/misc/sbuffer.h:19`
  copies the *last* element into slot `x`. Using `Vec::remove` would preserve order and silently
  change the indices that `signal ls` and `rule ls` report, and change which rule the one-shot
  sweeps at `src/event_loop.c:568-577` and `src/window_manager.c:1565-1574` visit next.
* `buf_del`'s value is the **pre-decrement** length (`buf__hdr(b)->len--`), which
  `src/event_loop.c:572` tests as a truthy "did we delete". `Vec::swap_remove` returns the removed
  element instead; those loops need the condition rewritten as `!vec.is_empty()` before the call,
  or restructured with `retain`.

### 6.2 Arena allocation (`ts_alloc_unaligned`, `ts_string_copy`, `ts_string_escape`)

Sites: every `arg_name`/`arg_value` allocation in `event_signal_push`, `src/event_signal.c:146`,
`:209`, `:408-410`; `src/rule.c:13-16`, `:117-119`, `:163`. Implementation `src/misc/ts.h`, an
8 MiB `mmap` (`src/yabai.c:279`) with a `PROT_NONE` guard page, bumped with
`__sync_fetch_and_add`, reset wholesale by `ts_reset()` at `src/event_loop.c:1671`.

**Rust:** do **not** port the arena for these two files. Return owned values:

* `ts_string_escape` → `fn json_escape(s: &str) -> Cow<'_, str>`, `Cow::Borrowed` for the no-escape
  case (which is exactly the C `NULL` return plus the caller's fallback at
  `src/event_signal.c:424-428`, `src/rule.c:42-45`).
* `window_title_ts` / `window_role_ts` / `window_subrole_ts` → functions returning `String`.
* The `arg_name`/`arg_value` pairs → `Vec<(CString, CString)>` or
  `[Option<(CString, CString)>; 4]` owned by the queued signal.

What this changes: the arena is a bump allocator, so the C code's allocations are effectively
free and never fail individually; `ts_assert_within_bounds` (`src/misc/ts.h:28-34`) calls
`exit(EXIT_FAILURE)` with a message on overflow. Rust allocation failure aborts instead. Both are
process death; the message differs. Nobody should depend on it.

The one hard constraint: the C arena is reset *after* the flush. If the queued signal owns its
strings, the ordering constraint disappears — but **only if the queue is converted at the same
time**. Converting one and not the other reintroduces a use-after-free with no compiler warning
because the arena memory stays mapped.

### 6.3 Bump-allocated fork queue (`struct memory_pool g_signal_storage`)

Sites: `src/event_signal.c:62`, `:66`, `:71`, `:74`, `:107-108`.

**Rust:** `Vec<PendingSignal>` in the signal registry. `event_signal_push` becomes
`registry.queue.push(pending)`, `event_signal_flush`'s parent branch becomes `queue.clear()`.

Behaviour notes:

* Drops the 2730-record ceiling and the guard-page crash.
* Drops the `__sync_fetch_and_add`; state the reason in the commit, not in a code comment — the
  only writer is the event-loop pthread.
* The fork snapshot still works: after `libc::fork()`, the child has its own copy-on-write view of
  the `Vec`'s heap block. The child must **not** drop it (it `exit`s), and the parent must clear
  it after the fork, exactly as `src/event_signal.c:66` does.

### 6.4 `fork` / `fork` / `execvp` double-fork detach

Site: `src/event_signal.c:60-97`. Registered signal disposition it depends on:
`signal(SIGCHLD, SIG_IGN)` at `src/yabai.c:151`.

**Rust:** unavoidable `unsafe` around `libc::fork`, `libc::setenv`, `libc::execvp`, `libc::exit`.
`std::process::Command` cannot express this shape: the outer fork exists so that the *first* child
does the filtering work off the event-loop thread's critical path, and the second fork plus
`SIGCHLD` ignore is what detaches the grandchild.

Rules for the child side, none of which the C code needs to state because C has no allocator
surprises here:

* **Build every `CString` before the fork.** `execvp`'s `argv` (`/usr/bin/env`, `sh`, `-c`,
  command) and all four `setenv` name/value pairs. After `fork` in a multi-threaded process, the
  child may hold a locked malloc arena; any allocation can deadlock. yabai has at least three live
  threads at that point.
* Use `libc::exit`, not `libc::_exit` and not `std::process::exit`, to keep the inherited-stdio
  flush behaviour of `src/event_signal.c:92` and `:96`.
* `fork()` returning `-1` must take the **parent** branch, as `if (pid)` does today: queue
  cleared, signals dropped, no error reported.
* Keep the inner `continue` on `pid != 0` (`src/event_signal.c:84`) so the intermediate child keeps
  spawning the remaining commands.
* Build the `argv` array as `[*const c_char; 5]` with a trailing null, matching
  `src/event_signal.c:91`.

Consider `catch_unwind`-free code in the child: a Rust panic after fork would unwind through a
process that has no other threads and run drops on shared state. Keep the child body to raw libc
calls.

### 6.5 POSIX regex (`regcomp` / `regexec` / `regfree`)

Sites: compile `src/message.c:2641`, `:2651`, `:2661`, `:2671` (rules) and `src/message.c:2895`,
`:2903` (signals); execute `src/misc/helpers.h:573-579` called from `src/event_signal.c:18`, `:24`,
`:35`, `:38`, `:47`, `:50` and `src/window_manager.c:94`, `:97`, `:100`, `:103`; free
`src/event_signal.c:360-361`, `src/rule.c:209-212`.

**Rust: bind libc, do not use the `regex` crate.** Recommended shape:

```rust
struct CompiledPosixRegex(Box<libc::regex_t>);   // Drop calls regfree
```

`Box` for a stable address (cheap insurance; macOS's `regex_t` is
`{int re_magic; size_t re_nsub; const char *re_endp; struct re_guts *re_g;}`, 32 bytes, with no
self-referential pointers, so it is in fact memcpy-movable — which is what makes
`buf_push(..., *signal)` at `src/event_signal.c:355` legal today).

Why not the `regex` crate: the patterns come from users' `yabairc` files and are POSIX **extended**
regular expressions (`REG_EXTENDED`). The differences are user-visible:

* `\d`, `\w`, `\s`, `\b` are not ERE. In ERE `\d` matches a literal `d`; the `regex` crate matches
  a digit. Every existing config that wrote `\d` by accident changes meaning.
* ERE has no non-greedy quantifiers, no lookaround, no `(?i)` inline flags; the `regex` crate
  accepts some of these, so patterns that `regcomp` **rejects** today (making `*_regex_valid`
  false and the filter unset) would start compiling and start filtering.
* POSIX is leftmost-longest, Rust is leftmost-first. Irrelevant here, since every call is
  `regexec(re, s, 0, NULL, 0)` — a boolean test with no capture groups.
* `regcomp` is locale-sensitive for `[[:alpha:]]` and friends.

`regexec` needs a NUL-terminated `*const c_char`. `window_title_ts` (`src/window.c:724`) returns
`""` rather than NULL, but `ts_cfstring_copy` (`src/misc/helpers.h:361-370`) **can** return NULL
when `CFStringGetCString` fails, so `es->title` can be NULL and `regexec(re, NULL, ...)` would
crash. Latent bug in C; in Rust make the title an `Option<CString>` and decide explicitly — the
behaviour-preserving choice is to treat `None` the same as `""`.

The tri-state result must survive:

```rust
#[repr(i32)]
enum RegexMatch { Undefined = 0, Yes = 1, No = 2 }
```

so that "no regex configured" still compares unequal to both `Yes` and `No`.

### 6.6 Tri-state `int` properties

`SIGNAL_PROP_UD/YES/NO` (`src/event_signal.h:90-92`) and `RULE_PROP_UD/ON/OFF`
(`src/rule.h:4-6`), both relying on 0 being the zero-initialised default.

**Rust:** `#[derive(Default)] #[repr(i32)] enum SignalProp { #[default] Undefined = 0, Yes = 1, No = 2 }`
and the same for `RuleProp { Undefined, On, Off }`. `json_optional_bool`
(`src/misc/helpers.h:225-230`) becomes a method returning `"null"`, `"true"` or `"false"` — note
the C version maps *anything other than 0 or 1* to `"false"`, so a hypothetical fourth variant
would print `false`, not fail.

### 6.7 Bit flags in a `uint16_t`

`enum rule_flag` / `enum rule_effects_flag` with the six inline helpers at `src/rule.h:59-65`.

**Rust:** `bitflags!` with `RuleFlags: u16` and `RuleEffectsFlags: u16`, keeping the exact
discriminants because `rule_serialize` publishes them over the socket
(`src/rule.c:38`, `:60` — `"flags":"0x%08x"`, effects in the high half). Alternatively plain
associated consts on a newtype, to avoid a dependency; the serialisation constraint is the same
either way.

Do not model the four `*_VALID` bits as bits *and* keep four bare `regex_t` fields — they are an
optional. `Option<CompiledPosixRegex>` replaces both, with the caveat that
`rule_serialize`'s flags word must still report the bit so the JSON is unchanged: derive it from
`regex.is_some()` when serialising.

### 6.8 Tagged `void *` context

`event_signal_push(enum signal_type type, void *context)` (`src/event_signal.c:99`), decoded by a
`switch` on `type` — see the table in section 2.3. Values are cast through `uintptr_t`
(`src/event_signal.c:229`, `:243`, `:281`, `:295`, `:332`) or are raw struct pointers.

**Rust:** replace the pair with one `enum SignalContext` carrying the payload:

```rust
enum SignalContext<'a> {
    Application(&'a Application),
    Window(&'a Window),
    Space(u64),
    Display(u32),
    MissionControl(MissionControlMode),
    None,
}
```

and derive the `SignalType` from the call site as today. This is the single largest readability
win in the file and it removes 35 `void *` casts in `src/event_loop.c`. It does mean the phase-2
translator touches `event_loop.c`'s call sites; if that file is being translated by someone else,
keep `push(SignalType, SignalContext)` as two parameters so the call-site edit is mechanical.

Borrow-checker note: `Application` and `Window` are owned by hash tables inside
`g_window_manager`, and `event_signal_push` is called from handlers that are mutating those very
tables (e.g. `src/event_loop.c:261` immediately before `window_manager_remove_application`). The
honest translation keeps raw pointers here (`*const Application`) and reads through them in
`unsafe`, or copies the two fields actually needed (`pid`, `name`) into the queued record at push
time. **Copying at push time is the better answer** — it is what `src/event_signal.c:146` and
`:209` already do by hand for the two types where the source is about to be freed.

### 6.9 Hash-table iteration (`table_for`)

Sites: `src/rule.c:115`, `:161`. Macro at `src/misc/hashtable.h:34-41`: two nested loops over
`table.buckets[i]` chains, skipping entries with a NULL `value`, and declaring an `int i` in the
caller's scope.

**Rust:** whatever `Table` type phase 2 produces, expose an iterator. But in both call sites the
body calls back into `&mut g_window_manager`, so iterating a borrow of the table while mutating
the manager cannot be expressed safely. Collect first:

```rust
let windows: Vec<*mut Window> = window_manager.window.values().collect();
for window in windows { /* ... */ }
```

This is observably identical (checked in section 4.3: nothing reachable from here inserts into or
removes from `wm.window`) and it removes the aliasing UB that the C version has by construction.
Iteration order is bucket order, i.e. `hash(window_id) % capacity` then insertion order within a
chain — it is not sorted, and `rule_reapply_all` does not depend on order.

### 6.10 `printf` into a `FILE *` response

Sites: `src/event_signal.c:412-428`, `:435`, `:442`, `:447`, `:453`; `src/rule.c:18-60`.

**Rust:** a `Response` newtype over the socket implementing `std::io::Write`, built with
`File::from_raw_fd` / `UnixStream::from_raw_fd` in the `DAEMON_MESSAGE` handler so that dropping
it closes the fd exactly once (C does `fclose` at `src/event_loop.c:1637`). `write!` returns a
`Result`; the C code ignores every `fprintf` return. Swallowing the error with `let _ =` matches;
`unwrap()` does not — a client that hangs up mid-response would abort the daemon.

Numeric formatting differences, all of them silent:

| C | Rust that matches | Rust that silently differs |
| --- | --- | --- |
| `%d` on `uint32_t` | `{}` on `value as i32` | `{}` on the `u32` |
| `%lld` on `uint64_t` | `{}` on `value as i64` | `{}` on the `u64` |
| `%.4f` on `float` | `{:.4}` on `value as f64` | `{:.4}` on the `f32` |
| `0x%08x` | `{:08x}` | `{:#010x}` (adds the `0x` twice) |
| `%s` on NULL | an explicit `"(null)"` or a deliberate choice | `Option` unwrapping |

`json_optional_bool` and `json_bool` emit **unquoted** `null`/`true`/`false` into fields whose
format strings have no quotes (`src/rule.c:28`, `:30-32`, `:34`, `:37`; `src/event_signal.c:418`),
while `label`, `app`, `title`, `sub-layer`, `scratchpad`, `event`, `action` and `flags` **are**
quoted. Get this wrong and every downstream `jq` script breaks.

### 6.11 `ts_string_escape` returning NULL as "unchanged"

`src/misc/helpers.h:259-...`, used at `src/event_signal.c:408-410` and `src/rule.c:13-16` with the
three-way fallback `escaped ? escaped : raw ? raw : ""`.

**Rust:** `Cow<'_, str>` plus `Option`, collapsing to
`rule.app.as_deref().map(json_escape).unwrap_or(Cow::Borrowed(""))`. The escape set is
`"`, `\`, `\b`, `\f`, `\n`, `\r`, `\t` escaped as two characters, and every other byte `<= 0x1f`
expanded to `\u00XX` (6 bytes, hence the `num_replacements += 5`). Note `*cursor >= 0x00` on a
signed `char` means bytes `>= 0x80` are negative and fall through unescaped — UTF-8 passes through
raw. A Rust implementation operating on `char`s instead of bytes would change that for non-ASCII
titles. Operate on bytes.

### 6.12 `goto out` cleanup

Not present in either assigned `.c` file. (It is used next door, e.g.
`src/event_loop.c:256` — mentioned only so phase 2 does not go looking for it here.)

### 6.13 Intrusive linked lists, `setjmp`, inline asm, SIMD, CF retain/release, ObjC message sends, blocks, `AXObserver`, `CGEventTap`, Carbon handlers, `CVDisplayLink`, `SLSRegisterConnectionNotifyProc`, `NSNotification`, dispatch blocks, CAS on `id_ptr`

**None of these appear in `event_signal.h`, `event_signal.c`, `rule.h`, `rule.c` or `message.h`.**
The only atomic is `__sync_fetch_and_add` on `g_signal_storage.used`
(`src/event_signal.c:107`); the only OS-level primitives are `fork`/`setenv`/`execvp`,
`regcomp`/`regexec`/`regfree`, `fprintf` and one Carbon call, `GetCurrentEventTime`
(`src/event_signal.c:156`).

`GetCurrentEventTime` is a Carbon `HIToolbox` symbol returning `EventTime` (`double`, seconds
since boot). Rust: `extern "C" { fn GetCurrentEventTime() -> f64; }` linked against the Carbon
framework, or the `CoreServices`/`HIToolbox` binding the rest of the rewrite settles on. It must
be the same clock as the one `src/event_loop.c:382` and `src/process_manager.c:250` write into
`switch_event_time`, so it cannot be swapped for `Instant` unless all three move together.

### 6.14 Macros that generate code

`TIME_FUNCTION` (`src/misc/timer.h:136-138` when `PROFILE >= 1`, `:151`/`:159` otherwise) at
`src/event_signal.c:402`, `:433`, `src/rule.c:6`. It expands to a
`__attribute((cleanup(END_TIME_BLOCK)))` local — a C RAII guard.

**Rust:** a macro expanding to a guard binding under `#[cfg(feature = "profile")]` and to nothing
otherwise. If the guard is written, name it for what it does on drop, e.g.
`CloseTimeBlockOnDrop`, and bind it to a named variable (`let _close_time_block = ...`), never to
`_`, which drops immediately.

`debug(...)` (`src/misc/log.h:6-15`) at `src/event_signal.c:77` → a `debug!` macro checking
`g_verbose` and writing to `stdout` with **no implicit newline** (the C format strings carry their
own `\n`).

### 6.15 Designated-initializer lookup tables

`signal_type_str` (`src/event_signal.h:47-88`), `layer_str` (`src/misc/helpers.h:175-181`),
`mission_control_mode_str` (`src/mission_control.c:38-44`).

**Rust:** a `const` array indexed by the discriminant, or a `fn as_str(self) -> &'static str`
match. Prefer the `match` for `signal_type_str`, since `signal_type_from_string` is its exact
inverse and a `match` keeps them next to each other. Keep the `"signal_type_count"` entry only if
something indexes 30 — nothing does.

`layer_str` **must** stay an indexed array or an explicit match with the holes handled, because
`src/rule.c:53` indexes it with a raw `int` that is not validated.

### 6.16 Fixed char arrays

`char *exec[]` at `src/event_signal.c:91` is the only one here; `snprintf` into 128-byte arena
slabs (`arg_size`) is the fixed-size pattern. Covered in 6.2 and 6.4.

### 6.17 Ownership transfer by struct copy

`buf_push(g_signal_event[type], *signal)` (`src/event_signal.c:355`) and
`buf_push(g_window_manager.rules, *rule)` (`src/rule.c:178`). The caller's stack struct is
bitwise-copied and the caller then *must not* call the destructor — see `src/message.c:2955` vs
`:2957` and `src/message.c:2821` vs `:2823`.

**Rust:** `Vec::push(value)` by value, with `Drop` on `Signal` and `Rule` doing what
`event_signal_destroy` / `rule_destroy` do. Move semantics make both branches automatic and make
the double-free hazard in `event_signal_destroy` / `rule_destroy` (neither nulls its pointers)
impossible. `rule_destroy` and `event_signal_destroy` then stop being public functions; the
`src/message.c` failure paths become a plain `drop(rule)`.

One catch: `src/event_loop.c:571` and `src/window_manager.c:1568` call `rule_destroy` **followed
by** `buf_del`, i.e. destroy-then-forget. With `Drop`, that becomes a single `swap_remove` whose
returned value is dropped. Make sure the destroy is not written twice.

---

## 7. Comments to carry over verbatim

`src/event_signal.h` — none.
`src/rule.h` — none.
`src/rule.c` — none.
`src/message.h` — none.

`src/event_signal.c` — exactly one block, lines **148-154**, inside the
`SIGNAL_APPLICATION_TERMINATED` arm of `event_signal_push`, immediately above the
`GetCurrentEventTime` comparison at `:156-161`:

```
        //
        // NOTE(asmvik): We always receive an application_front_switched event *before* an application_terminated
        // event. We need to know the difference between a front_switched + application_terminated sequence and a regular
        // application switch followed by a user-initiated termination of the previously focused application. The system
        // events are triggered within an interval that appear to be impossible to match even with a user automated sequence.
        // The dt threshold below is triple the average interval computed, to allow for some leeway.
        //
```

Carry it across unchanged, including the `NOTE(asmvik):` prefix and the blank `//` delimiter
lines, attached to the same code. Nothing else in these five files is commented, and nothing new
should be added.
