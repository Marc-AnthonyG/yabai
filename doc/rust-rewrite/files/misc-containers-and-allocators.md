# misc: containers, allocators and process-wide utilities

Phase-1 map for the reader `misc-containers-and-allocators`.

Files covered (all read in full):

| file | lines | include site in `src/manifest.m` | include guard |
|---|---|---|---|
| `src/misc/macros.h` | 47 | `manifest.m:46` | yes (`MACROS_H`) |
| `src/misc/memory_pool.h` | 47 | `manifest.m:47` | yes (`MEMORY_POOL_H`) |
| `src/misc/ts.h` | 105 | `manifest.m:48` | yes (`TS_H`) |
| `src/misc/notify.h` | 50 | `manifest.m:50` | **no** |
| `src/misc/log.h` | 63 | `manifest.m:51` | yes (`LOG_H`) |
| `src/misc/timer.h` | 165 | `manifest.m:53` | yes (`TIMER_H`) |
| `src/misc/macho_dlsym.h` | 80 | `manifest.m:54` | **no** |
| `src/misc/sbuffer.h` | 71 | `manifest.m:55` | yes (`SBUFFER_H`) |
| `src/misc/hashtable.h` | 156 | `manifest.m:56-58` (`HASHTABLE_IMPLEMENTATION` defined then undefined around it) | yes, plus a separate `#ifdef HASHTABLE_IMPLEMENTATION` implementation block at `hashtable.h:45` |
| `src/misc/autorelease.h` | 98 | `manifest.m:49` — **commented out**; its three hook calls are inside `#if 0` at `yabai.c:158-162` | **no** |

These ten headers are the substrate every other module sits on. Almost all of them are
*header-only definitions in a unity build*: `manifest.m` includes each one exactly once, so
`static` at file scope here means "one instance for the whole program", not "one per object
file". Phase 2 must not reproduce `static` as Rust `mod`-private state if the C `static` was
actually a program-wide singleton — in every case below it was.

---

## 0. Cross-cutting: the thread inventory these files live in

Everything in section 4 of each file below refers to this list. How each was determined is
stated once here rather than repeated.

| name used in this document | what it is | evidence |
|---|---|---|
| **main run-loop thread** | the process main thread; runs `main()` then `[NSApp run]` | `yabai.c:261-353`, `[NSApp run]` at `yabai.c:350` |
| **event-loop pthread** | single consumer of the lock-free event queue | created `event_loop.c:1718` (`pthread_create(&event_loop->thread, NULL, &event_loop_run, event_loop)`), body `event_loop.c:1647-1682` |
| **message-loop pthread** | blocking `accept()` loop on the unix socket | created `message.c:3042`, body `message.c:3003-3012`; it only does `accept()` + `event_loop_post(..., DAEMON_MESSAGE, NULL, sockfd)` (`message.c:3009`) |
| **CVDisplayLink thread** | per-animation display-link output callback | registered `window_manager.c:701`, callback `window_manager_animate_window_list_thread_proc` at `window_manager.c:535` |
| **window-proxy pthreads** | short-lived, one per animated window, joined immediately | created `window_manager.c:666`, joined `window_manager.c:679-681`, body `window_manager.c:506-531` |
| **forked children** | `fork()` without `exec` in `event_signal_flush` (`event_signal.c:64`, `event_signal.c:85`) and `exec_config_file` (`helpers.h:477`) | they read parent memory (including temp storage) before `execvp` |

Two facts that matter for every allocator below:

1. **The event-loop pthread starts at `yabai.c:291`, but the main thread keeps initialising
   until `yabai.c:350`.** `window_manager_init`/`space_manager_begin`/`window_manager_begin`
   (`yabai.c:336-338`) and `update_window_notifications` (`yabai.c:341`) run on the main thread
   *while the event-loop thread is already draining events*. So "event-loop thread only" is
   never strictly true for anything reachable from those calls — see the concrete races noted
   per file.
2. **All OS callbacks (AXObserver, CGEventTap, SLS notify procs, NSNotification, Carbon) fire on
   the main run-loop thread** and do nothing but `event_loop_post`. Verified for the mouse tap:
   `CGEventTapCreate` at `mouse_handler.c:278` and `CFRunLoopAddSource(CFRunLoopGetMain(), ...)`
   at `mouse_handler.c:288`, both inside `mouse_handler_begin`. So the only *producers* of
   events are the main thread and the message-loop pthread; the only *consumer* is the
   event-loop pthread.

---

## 1. `src/misc/macros.h`

### 1.1 Purpose

A flat list of `#define`s: size literals, tiny arithmetic helpers, and the magic integer
constants that the rest of the daemon uses as directions, resize handles, window layers, regex
tri-state results and the client/daemon failure marker. It contains no types, no state and no
code.

### 1.2 Every definition

Arithmetic / size macros:

| line | macro | expansion | notes |
|---|---|---|---|
| 4 | `KILOBYTES(value)` | `((value) * 1024ULL)` | result is `unsigned long long` |
| 5 | `MEGABYTES(value)` | `(KILOBYTES(value) * 1024ULL)` | |
| 6 | `GIGABYTES(value)` | `(MEGABYTES(value) * 1024ULL)` | **never used** anywhere in `src/` |
| 8 | `array_count(a)` | `(int)(sizeof((a)) / sizeof(*(a)))` | explicit cast to `int`; 15 uses |
| 9 | `min(a, b)` | `((a) < (b) ? (a) : (b))` | double evaluation of both args; never used outside this header |
| 10 | `max(a, b)` | `((a) > (b) ? (a) : (b))` | used at `sbuffer.h:24`, `sbuffer.h:54`, `window_manager.c:353-354` |
| 11 | `add_and_clamp_to_zero(a, b)` | `(((a) + (b) <= 0) ? 0 : (a) + (b))` | 5 uses, all `space_manager.c:221,367-370` |
| 12 | `in_range_ii(a, b, c)` | `(((a) >= (b)) && ((a) <= (c)))` | 8 uses |
| 13 | `in_range_ie(a, b, c)` | `(((a) >= (b)) && ((a) < (c)))` | 1 use, `display_manager.c:207` |
| 14 | `in_range_ei(a, b, c)` | `(((a) > (b)) && ((a) <= (c)))` | 2 uses, `message.c:1356`, `message.c:1365` |
| 15 | `in_range_ee(a, b, c)` | `(((a) > (b)) && ((a) < (c)))` | **never used** |
| 16 | `lerp(a, t, b)` | `(((1.0-t)*a) + (t*b))` | arguments are **not parenthesised**; 4 uses, `window_manager.c:560-563` |

Constants:

| line | macro | value | meaning / where it lands |
|---|---|---|---|
| 18 | `FAILURE_MESSAGE` | `"\x07"` (BEL) | first byte of a daemon response marks failure. Written `message.c:424` (`fprintf(rsp, FAILURE_MESSAGE)`), read `yabai.c:111` (`rsp[0] == FAILURE_MESSAGE[0]`) |
| 20 | `MAXLEN` | `512` | fixed `char` array size; 28 uses, incl. `g_sa_socket_file`, `g_socket_file`, `g_lock_file` (`yabai.c:44-47`) |
| 22-24 | `REGEX_MATCH_UD` / `_YES` / `_NO` | `0` / `1` / `2` | tri-state returned by `regex_match` (`helpers.h:573-579`) |
| 26-29 | `DIR_NORTH` / `_EAST` / `_SOUTH` / `_WEST` | `360` / `90` / `180` / `270` | degrees, not an enum; 11 uses each |
| 31 | `STACK` | `111` | a **fifth value in the same namespace as `DIR_*`**, stored in the same `insert_dir` field (`event_loop.c:1305,1308`, `view.c:78`, `view.c:773`, `message.c:1143`) |
| 33-34 | `TYPE_ABS` / `TYPE_REL` | `0x1` / `0x2` | argument-kind tag returned by `message.c:475-477`, consumed `space_manager.c:218,220,361,366`, `window_manager.c:311,314,337` |
| 36-40 | `HANDLE_TOP`/`_BOTTOM`/`_LEFT`/`_RIGHT`/`_ABS` | `0x01`/`0x02`/`0x04`/`0x08`/`0x10` | bit flags OR-ed together (`message.c:494,496`, `event_loop.c:1145`, `mouse_handler.c:244`), and also **compared for equality** with `HANDLE_ABS` (`window_manager.c:374,401`) |
| 42-45 | `LAYER_AUTO`/`_BELOW`/`_NORMAL`/`_ABOVE` | `0` / `kCGBackstopMenuLevelKey` (3) / `kCGNormalWindowLevelKey` (4) / `kCGFloatingWindowLevelKey` (5) | fed to `CGWindowLevelForKey` at `yabai.c:145-147`, and used as **array indices** into the sparse designated-initialiser table `layer_str[]` at `helpers.h:175-181` |

No structs, enums, unions, typedefs, X-macro lists, globals, functions or callbacks in this file.

### 1.3 Rust translation

```rust
pub const fn kilobytes(v: u64) -> u64 { v * 1024 }
pub const fn megabytes(v: u64) -> u64 { kilobytes(v) * 1024 }

pub const FAILURE_MESSAGE: u8 = 0x07;
pub const MAXLEN: usize = 512;
```

- `min`/`max` → `std::cmp::min`/`max` for integers, `f32::max` for the float sites. Do **not**
  write a macro: the C macro's double evaluation is not relied on anywhere, and `max(1,
  frame.size.width + dx * x_mod)` (`window_manager.c:353`) is an `int` vs `float` comparison
  whose ternary result type is `float` — in Rust write `(frame.size.width + dx * x_mod).max(1.0)`.
- `lerp` → `fn lerp(a: f64, t: f64, b: f64) -> f64 { (1.0 - t) * a + t * b }`. **Trap:** the call
  sites pass `float mt` (`window_manager.c:550`) and `CGFloat` frame components; the literal
  `1.0` promotes the whole expression to `double`, and the result is stored into
  `proxy.tx`/`ty`/`tw`/`th`. Take the widening seriously — computing in `f32` gives different
  pixel positions on long animations.
- `array_count` → `.len() as i32` at the call sites; the `(int)` cast is load-bearing where the
  result is compared against an `int` loop counter.
- `in_range_*` → four small `const fn`s, or `(b..=c).contains(&a)` etc. Keep the four distinct
  names so the inclusive/exclusive ends stay visible.
- `add_and_clamp_to_zero` → `fn add_and_clamp_to_zero(a: i32, b: i32) -> i32 { (a + b).max(0) }`
  — note the C version clamps on `<= 0`, so a result of exactly 0 is also produced by the
  clamp branch; identical output, so `.max(0)` is safe.
- `DIR_*` + `STACK` → **one** `#[repr(i32)] enum InsertDirection { North = 360, East = 90, South
  = 180, West = 270, Stack = 111 }`. They genuinely share a field (`view.c:773` compares
  `leaf->insert_dir == STACK`), so a single enum is the faithful shape, not two.
- `HANDLE_*` → `bitflags!` (crate `bitflags`), because `message.c:494` builds `TOP | LEFT`.
  Keep an explicit `== Handle::ABS` comparison for `window_manager.c:374,401`, which tests
  equality with the whole bit set, not containment.
- `TYPE_ABS`/`TYPE_REL` → a two-variant enum; they are never OR-ed despite the hex spelling.
- `REGEX_MATCH_*` → `enum RegexMatch { Undefined, Yes, No }`. **Trap:** `event_signal.c:17,23,34`
  do `int regex_match_app = signal->app_regex_exclude ? REGEX_MATCH_YES : REGEX_MATCH_NO;` and
  then compare the tri-state for equality — an `Undefined` result deliberately matches neither
  branch. A `bool` translation silently changes filtering.
- `LAYER_*` → `#[repr(i32)] enum WindowLayer { Auto = 0, Below = 3, Normal = 4, Above = 5 }`
  with the numbers pinned to the `CGWindowLevelKey` enum
  (`CGWindowLevel.h:22-28`: `kCGBaseWindowLevelKey = 0` … `kCGFloatingWindowLevelKey = 5`).
  **Trap:** `layer_str[]` (`helpers.h:175-181`) is a sparse 6-element array with `NULL` holes at
  indices 1 and 2. Translating it as a `Vec`/array indexed by the layer value reproduces the
  holes; translate it as a `match` on the enum instead. Also note `LAYER_AUTO == 0 ==
  kCGBaseWindowLevelKey`, so "auto" is not a distinct sentinel at the CoreGraphics level — it is
  only distinguished by yabai's own checks (`window_manager.c:822,832`).
- `FAILURE_MESSAGE` is used as a **format string** at `message.c:424`
  (`fprintf(rsp, FAILURE_MESSAGE)`). In Rust write the raw byte, not a format call.
- `MAXLEN` backs fixed `char[512]` globals. See §6 "fixed char arrays".

---

## 2. `src/misc/memory_pool.h`

### 2.1 Purpose

A lock-free bump allocator over an `mmap`-ed region with a guard page, used as a **ring**: when
the bump pointer would run past the end it wraps back to offset 0 and starts overwriting.
Nothing is ever individually freed and the mapping is never unmapped. It backs the event queue's
node storage and the pending-signal storage.

### 2.2 Types

`struct memory_pool` (`memory_pool.h:4-9`):

| field | C type | owner / lifetime | cross-thread |
|---|---|---|---|
| `memory` | `void *` | owned by the pool; `mmap`-ed at `memory_pool.h:21`, **never** `munmap`-ed | written once on the main thread at init, read by every thread that allocates |
| `size` | `uint64_t` | — | written once at init, read-only afterwards |
| `used` | `volatile uint64_t` | — | **yes** — CAS-ed by every producer thread (`memory_pool.h:36,40`), and plainly written by the event-loop thread for `g_signal_storage` (`event_signal.c:66`, `event_signal.c:105`) |

### 2.3 Instances

| instance | declared | size | initialised | who pushes |
|---|---|---|---|---|
| `struct event_loop.pool` | `event_loop.h:68` | `KILOBYTES(512)` (`event_loop.c:1707`) | main thread, `event_loop_begin` | every event producer: main run-loop thread (all OS callbacks) and the message-loop pthread (`message.c:3009`), via `event_loop_post` → `memory_pool_push` (`event_loop.c:1689`) |
| `g_signal_storage` | `yabai.c:32` | `KILOBYTES(256)` (`yabai.c:283`) | main thread, `main` | **never through `memory_pool_push`** — `event_signal_push` does its own `__sync_fetch_and_add(&g_signal_storage.used, size)` at `event_signal.c:105` (event-loop thread only), and `event_signal_flush` resets `used = 0` at `event_signal.c:66` |

`event_loop.pool` hands out `sizeof(struct event)` = 24 bytes (`event_loop.h:56-61`:
`enum` 4 + `int` 4 + `void*` 8 + `struct event*` 8), so ~21845 events fit before the ring wraps.

### 2.4 Functions

**`bool memory_pool_init(struct memory_pool *pool, uint64_t size)`** — `memory_pool.h:11-27`.
Rounds `size` up to a page multiple, `mmap`s `size + page_size` anonymous R/W private, then
`mprotect`s the trailing page `PROT_NONE` as a guard. Returns `pool->memory != MAP_FAILED`.
Runs on the **main run-loop thread only** (`yabai.c:283` and `event_loop.c:1707`, both before
`[NSApp run]`). Allocates the mapping; nothing ever frees it. External symbols: `getpagesize`,
`mmap`, `mprotect` (and the `MAP_ANON|MAP_PRIVATE`, `PROT_*` constants).

**`void *memory_pool_push(struct memory_pool *pool, uint64_t size)`** — `memory_pool.h:29-45`.
CAS loop: relaxed-load `used`; if `used + size < pool->size` publish `used + size` and return
`memory + used`; otherwise publish `size` and return `memory` (wrap to the start). Runs on
**every producer thread**. Allocates nothing beyond bumping the counter. External symbols:
`__atomic_load_n`, `__sync_bool_compare_and_swap`.

No callbacks registered in this file.

### 2.5 Rust translation

```rust
pub struct MemoryPool {
    memory: *mut u8,
    size: u64,
    used: AtomicU64,
}
unsafe impl Sync for MemoryPool {}
```

- `mmap`/`mprotect`/`getpagesize` → `libc::mmap`, `libc::mprotect`, `libc::getpagesize`, all in
  one `unsafe` block. Do not reach for `memmap2`: the guard page is created by mapping one page
  *past* the usable size and then protecting it, which `memmap2` does not express directly.
- `used` → `AtomicU64`. `__sync_bool_compare_and_swap` is a **full sequentially-consistent
  barrier** in GCC/clang legacy atomics, so translate it as
  `compare_exchange(old, new, Ordering::SeqCst, Ordering::Relaxed)`, not `Relaxed`. The paired
  `__atomic_load_n(..., __ATOMIC_RELAXED)` is `load(Ordering::Relaxed)`.
- Return type: `*mut MaybeUninit<Event>`, not `&mut Event`. The memory is uninitialised on first
  pass and *stale, possibly still-referenced* memory after a wrap.

**Behaviour a naive translation would silently change**

- **The wrap is a silent data-corruption path, not an error.** When the pool is full,
  `memory_pool_push` returns `pool->memory` and the next `event_loop_post` overwrites event
  nodes that the consumer may still be walking (`event_loop.c:1651-1653` reads `head->next`).
  A Rust `Vec`-backed queue, or a version that returns `Err` on exhaustion, changes observable
  behaviour under load. Reproduce the wrap.
- The bound is **strict** (`new_used < pool->size`, `memory_pool.h:35`), so the final byte of the
  mapping is never handed out. Off-by-one differences change where the wrap happens.
- `pool->memory + pool->size` and `pool->memory + used` (`memory_pool.h:24,37,41`) are
  arithmetic on `void *` — a GNU extension with byte granularity. In Rust: `self.memory.add(n)`
  on a `*mut u8`.
- Alignment is *never* enforced. It holds only because the sole caller always asks for the same
  24-byte size starting at offset 0. If phase 2 ever pushes a differently sized type through the
  same pool, the returned pointer can be misaligned; keep `size` a multiple of 8 or add an
  explicit alignment step and note the change.
- `memory_pool_init` does not check `mprotect`'s return value.
- `g_signal_storage.used` is manipulated **outside** this file by two different mechanisms —
  an atomic add (`event_signal.c:105`) and a plain store of 0 (`event_signal.c:66`). If the
  Rust type makes `used` private, `event_signal` needs explicit accessors that keep exactly
  those two operations, including the non-atomic reset.
- `event_signal_push` does **no bounds check at all** (`event_signal.c:103-106`): with enough
  queued signals it walks off the end into the guard page and takes SIGBUS. That is current
  behaviour; do not "fix" it silently in phase 2 — flag it.

---

## 3. `src/misc/ts.h`

### 3.1 Purpose

The process-wide temporary-storage arena: a second `mmap`-ed bump allocator, 8 MiB
(`yabai.c:279`), reset to zero once per event at the bottom of the event-loop iteration
(`event_loop.c:1671`). Everything scratch — window lists, space lists, escaped JSON strings,
socket message bodies, signal environment strings — is allocated here and never freed
individually.

### 3.2 Types and state

There is no named type. `ts.h:4-8` declares a **file-static anonymous struct**, `g_temp_storage`,
with the same three fields as `struct memory_pool`:

| field | C type | initial value | who touches it | synchronisation |
|---|---|---|---|---|
| `memory` | `void *` | zero-init (BSS), set by `ts_init` | main thread writes at init; read by main thread, event-loop pthread, and the forked children of `event_signal_flush` | none needed after init |
| `size` | `uint64_t` | 0 → `8 MiB` rounded up to pages | same | none |
| `used` | `volatile uint64_t` | `0` | **main run-loop thread** (startup path `yabai.c:336-342`, see §0 note 1) and the **event-loop pthread** (everywhere else) | `__sync_*` atomics for the increments (`ts.h:59,68,79,93,95`); a **plain non-atomic store** for the reset (`ts.h:102`) |

`g_temp_storage` is the only global in the file, apart from the `cpu_freq` static discussed in
§8. It is never `munmap`-ed.

### 3.3 Functions

**`bool ts_init(uint64_t size)`** — `ts.h:10-26`. Byte-for-byte the same body as
`memory_pool_init` but against the file-static. Main thread only (`yabai.c:279`). External:
`getpagesize`, `mmap`, `mprotect`.

**`static inline void ts_assert_within_bounds(uint64_t size)`** — `ts.h:28-34`. If `size >
g_temp_storage.size`, prints to `stderr` and `exit(EXIT_FAILURE)`. Called **after** the bump has
already been published, so the counter is already out of range when the process dies. Runs on
whichever thread allocated. External: `fprintf`, `exit`.

**`static inline uint64_t ts_align(uint64_t used, uint64_t align)`** — `ts.h:36-47`. Aligns the
*absolute address* `memory + used` up to `align` and returns the new offset. `assert((align &
(align-1)) == 0)` at `ts.h:38`. External: `assert`.

**`static inline void *ts_alloc_aligned(uint64_t alignment, uint64_t size)`** — `ts.h:52-64`.
CAS loop over `used`; on success bounds-checks and returns `memory + aligned`. External:
`__atomic_load_n`, `__sync_bool_compare_and_swap`.

**`#define ts_alloc_list(elem_type, elem_count)`** — `ts.h:49-50`, expands to
`ts_alloc_aligned(__alignof__(elem_type), sizeof(elem_type) * elem_count)`. Call sites:
`display.c:237`, `event_loop.c:191,275,439,502`, `space.c:35`, `window.c:99`,
`display_manager.c:401`, `view.c:824`, `window_manager.c:615,1427,1553`.

**`static inline void *ts_alloc_unaligned(uint64_t size)`** — `ts.h:66-71`. Plain
`__sync_fetch_and_add`, no alignment. Used for byte buffers: `event_loop.c:1623` (the socket
message body), `event_signal.c` (~40 sites, 128-byte env strings), `helpers.h:283`
(`ts_string_escape`), `helpers.h:364` (`ts_cfstring_copy`), `helpers.h:390` (`ts_string_copy`).

**`static inline void *ts_expand(void *ptr, uint64_t old_size, uint64_t increment)`** —
`ts.h:75-86`, wrapped in `#pragma clang diagnostic ignored "-Wunused-parameter"`
(`ts.h:73-74`, popped `ts.h:87`) because `old_size` is only read inside the `assert` at
`ts.h:78`. Grows the last allocation in place. One call site: `view.c:828`.

**`static inline void *ts_resize(void *ptr, uint64_t old_size, uint64_t new_size)`** —
`ts.h:89-98`. Shrinks or grows the last allocation; asserts identity at `ts.h:91`. One call
site: `space.c:71`.

**`static inline void ts_reset(void)`** — `ts.h:100-103`. `g_temp_storage.used = 0`. **One call
site: `event_loop.c:1671`, on the event-loop pthread**, after `event_signal_flush()`.

No callbacks registered in this file.

### 3.4 Rust translation

```rust
pub struct TempStorage { memory: *mut u8, size: u64, used: AtomicU64 }
unsafe impl Sync for TempStorage {}

pub unsafe fn ts_alloc_list<T>(count: usize) -> *mut T;
pub unsafe fn ts_alloc_unaligned(size: usize) -> *mut u8;
```

Recommendation: keep raw pointers. A lifetime-carrying `&'arena mut [T]` cannot be made sound,
because `ts_reset` invalidates every outstanding pointer from a *different* function on the same
thread, and because `window_manager_animate_window_list` hands `ts_buf` memory to code that
outlives the borrow. Phase 2 should expose `ts_*` as `unsafe fn` returning raw pointers and wrap
the results in `core::slice::from_raw_parts_mut` at each call site, inside `unsafe`.

- `__alignof__(T)` → `std::mem::align_of::<T>()`; `sizeof(T) * n` → `size_of::<T>() * n` with a
  checked multiply (the C code has none).
- `__sync_fetch_and_add` → `fetch_add(n, Ordering::SeqCst)`; `__sync_fetch_and_sub` →
  `fetch_sub`. Again: `__sync_*` are full barriers.
- `ts_reset` → `used.store(0, Ordering::Relaxed)` to mirror the plain C store, **not**
  `SeqCst` — but see the trap below; this is a race either way and should be commented on in
  the phase-1 findings, not "fixed".

**Behaviour a naive translation would silently change**

- **`ts_reset` races with main-thread allocation at startup.** `ts_reset` is only ever called by
  the event-loop pthread, but `display_manager_active_display_list` (`display_manager.c:401`),
  `display_space_list` (`display.c:237`), `window_space_list` (`window.c:99`) and
  `ts_cfstring_copy` run on the **main thread** inside `window_manager_begin` /
  `space_manager_begin` / `update_window_notifications` (`yabai.c:336-342`), i.e. after the
  event-loop pthread is live (`yabai.c:291`). The C code gets away with it because the arena is
  8 MiB and startup is short. A Rust translation that adds a debug assertion, a `Cell`, or
  `!Sync` state here will panic where C silently worked. Keep the arena `Sync` and raw.
- **Bounds checking is post-hoc.** `ts_alloc_aligned` CASes first and bounds-checks after
  (`ts.h:59-60`); `ts_alloc_unaligned` adds first and checks after (`ts.h:68-69`). A Rust
  version that checks before bumping will exit on a different allocation than C does, and will
  not reproduce the "counter already past the end" state visible in the error message.
- **`assert` disappears in release.** `make install` uses `-DNDEBUG` (`makefile:27`), so the
  three asserts (`ts.h:38,78,91`) are *not* present in shipped binaries. A Rust `assert!` is
  always on and `debug_assert!` is off in release; use `debug_assert!` to match `make install`
  and accept that `make all` (`-O0`, no `-DNDEBUG`, `makefile:4`) then differs. This is the
  faithful choice.
- **`ts_expand` and `ts_resize` are only correct when `ptr` is the most recent allocation.**
  `ts_expand` grows `used` by `increment` and returns the *same* pointer (`ts.h:79,85`);
  `view.c:824-828` relies on this to double a `uint32_t` list in place. If phase 2 reorders any
  allocation between the original `ts_alloc_list` and the `ts_expand`, memory is silently
  corrupted and the asserts are compiled out in release. Preserve statement order exactly in
  `view.c` and `space.c`.
- `fprintf(stderr, "... requested %lld, but allocated size is %lld\n", size, g_temp_storage.size)`
  at `ts.h:31` passes `uint64_t` to `%lld`. Works on this ABI; in Rust just format `u64`.
  Keep the wording byte-identical if any test greps for it.
- `exit(EXIT_FAILURE)` from a non-main thread runs `atexit` handlers and flushes stdio. Rust's
  `std::process::exit` is the equivalent.
- **The forked children of `event_signal_flush` read this arena.** `event_signal_push` stores
  `ts_alloc_unaligned` pointers into `g_signal_storage` (`event_signal.c:129-130` and ~20 more),
  and `event_signal_flush` (`event_signal.c:62-95`) forks and then dereferences them in the
  child. This works because `event_loop_run` calls `event_signal_flush()` **before**
  `ts_reset()` (`event_loop.c:1670-1671`). That ordering is load-bearing; do not reorder.
- The arena is never unmapped and `ts_init` failure is the only error path (`yabai.c:279-281`).

---

## 4. `src/misc/sbuffer.h`

### 4.1 Purpose

Two copies of the classic "stretchy buffer" (Sean Barrett / Per Vognsen style): a growable array
whose header lives immediately before the element data, so the buffer variable is a plain `T *`
and `b[i]` works. One copy is `malloc`/`realloc`-backed (`buf_*`), the other allocates from the
temp-storage arena (`ts_buf_*`).

### 4.2 Types

`struct buf_hdr` (`sbuffer.h:4-9`) and `struct ts_buf_hdr` (`sbuffer.h:34-39`) are identical:

| field | C type | notes |
|---|---|---|
| `len` | `int` | current element count |
| `cap` | `int` | capacity in elements |
| `buf` | `char[0]` | zero-length array (GNU extension); `offsetof(struct buf_hdr, buf) == 8` |

The element type is not part of the header — it is recovered at each macro call site via
`sizeof(*(b))` (`sbuffer.h:13,43`). The header owns the element storage; the elements own
whatever they contain.

### 4.3 Macros

| line | macro | notes |
|---|---|---|
| 11 / 41 | `buf__hdr(b)` / `ts_buf__hdr(b)` | `((struct buf_hdr *)((char *)(b) - offsetof(...)))` — the `(char *)` cast also **strips `const`**, which is how `buf__grow_f`'s `const void *buf` parameter reaches `realloc` |
| 12 / 42 | `buf__should_grow(b, n)` | `len + n >= cap` — note `>=`, so there is always one slack slot and `cap > len` always holds |
| 13 / 43 | `buf__fit(b, n)` | assigns back into `b`, so `b` must be an lvalue |
| 15-16 / 45-46 | `buf_len(b)` / `buf_cap(b)` | NULL-safe, return `0` |
| 17 / 47 | `buf_last(b)` | `(b)[buf_len(b)-1]`; `ts_buf_last` is **never used** |
| 18 / 48 | `buf_push(b, x)` | comma expression: fit, store at `[len]`, post-increment `len` |
| 19 / 49 | `buf_del(b, x)` | **swap-remove**: `(b)[x] = (b)[len-1], len--`. The expression value is the *pre-decrement* `len`, used as a truthy result at `event_loop.c:572` and `window_manager.c:1569` |
| 20 | `buf_free(b)` | **never called anywhere in `src/`** — there is no `ts_buf_free` |

### 4.4 Functions

**`static void *buf__grow_f(const void *buf, int new_len, int elem_size)`** — `sbuffer.h:22-32`.
`new_cap = max(1 + 2*cap, new_len)`, `realloc`s header+payload, sets `cap`, zeroes `len` only on
first allocation. No failure check on `realloc`. External: `realloc`, `offsetof`.

**`static void *ts_buf__grow_f(const void *buf, int new_len, int elem_size)`** —
`sbuffer.h:51-69`. Same capacity policy. On first allocation it calls
`ts_alloc_aligned(8, new_size)` (`sbuffer.h:61`); on growth it does **not** move the buffer —
it just bumps `g_temp_storage.used` by the **full `new_size`**, not the delta
(`sbuffer.h:58`), and keeps the existing header pointer. This is only correct because the
buffer is the most recent temp-storage allocation; there is no assert guarding it, unlike
`ts_expand`. External: `__sync_fetch_and_add`, `ts_alloc_aligned`.

Both functions are pure allocation helpers; neither runs on a distinguished thread.

### 4.5 Who owns each live buffer

`malloc`-backed (`buf_*`), all leaked for the process lifetime since `buf_free` is never called:

| buffer | element type | declared | mutated from |
|---|---|---|---|
| `g_space_manager.labels` | `struct space_label` | pushed `space_manager.c:196`, deleted `space_manager.c:175,191` | event-loop pthread (message handling) |
| `g_display_manager.labels` | `struct display_label` | pushed `display_manager.c:74`, deleted `display_manager.c:53,69` | event-loop pthread |
| `g_window_manager.rules` | `struct rule` | pushed `rule.c:178`, deleted `rule.c:186,199`, `event_loop.c:572`, `window_manager.c:1569` | event-loop pthread |
| `g_window_manager.applications_to_refresh` | `struct application *` | pushed `window_manager.c:1716`, deleted `window_manager.c:1733,1739`, `event_loop.c:266` | **main thread at startup** (`window_manager_begin` → `window_manager_add_existing_application_windows`) **and** event-loop pthread |
| `g_window_manager.scratchpad_window` | `struct scratchpad` | pushed `window_manager.c:2498`, deleted `window_manager.c:2515` | event-loop pthread |
| `g_signal_event[SIGNAL_TYPE_COUNT]` | `struct signal` | pushed `event_signal.c:355`, deleted `event_signal.c:375,391` | event-loop pthread; **read by forked children** at `event_signal.c:76,79` |

Arena-backed (`ts_buf_*`), all reclaimed wholesale by `ts_reset`:

| buffer | element type | sites |
|---|---|---|
| the `struct window_capture *window_list` threaded through `window_node_capture_windows` | `struct window_capture` | pushed `view.c:365`; length read `view.c:378`, `mouse_handler.c:178,215`, `window_manager.c:1868,1879,1920,2053` |
| `app_window_list` | `uint32_t` | `window_manager.c:1652,1657,1676,1694,1714` |

### 4.6 Rust translation

- `buf_*` → **`Vec<T>`**, straightforwardly. `buf_push` → `push`, `buf_len` → `len()`,
  `buf_last` → `last()`, `buf_free` → `drop` (currently never happens; dropping at the same
  points changes nothing observable because the process exits).
- `buf_del(b, i)` → **`Vec::swap_remove(i)`**, which is exactly the C semantics. But
  `swap_remove` returns the removed element, while the C macro's value is the pre-decrement
  length used as a boolean. Translate `if (buf_del(x, i))` (`event_loop.c:572`,
  `window_manager.c:1569`) as `if !v.is_empty() { v.swap_remove(i); true } else { false }`, or
  restructure the caller — do not assume the return value is unused.
- `ts_buf_*` → also **`Vec<T>`**. This is the one place I recommend *not* preserving the C
  allocation strategy. Reasons: the arena copy exists purely to avoid `malloc` on the hot path;
  its contents never outlive the current event; and no code takes the address of a `ts_buf`
  element across a `ts_reset`. Using `Vec` removes the unwritten invariant "the ts_buf must be
  the most recent arena allocation", which `ts_buf__grow_f` relies on with no assert. Flag this
  as the single deliberate deviation in this reader, and check it against
  `window_node_capture_windows` (`view.c:357-371`), which recurses and pushes while
  `window_manager_find_window` runs in between — that call does not allocate from the arena
  today, which is exactly why the C version works.
- The out-parameter shape `void window_node_capture_windows(struct window_node *node, struct
  window_capture **window_list)` (`view.c:357`) becomes `&mut Vec<WindowCapture>`; the
  `if (window_list)` NULL test at `view.c:378` becomes `if !list.is_empty()`.

**Behaviour a naive translation would silently change**

- `int len`/`int cap` and `int new_size` (`sbuffer.h:25,55`) overflow at 2 GiB. `Vec` uses
  `usize`. Not reachable in practice; mention only so nobody "preserves" the `i32`.
- The growth policy `max(1 + 2*cap, new_len)` differs from `Vec`'s amortised doubling. Capacity
  is not observable here, so `Vec` is fine.
- `buf__grow_f` does not check `realloc` failure and would deref NULL at `sbuffer.h:27`. `Vec`
  aborts on allocation failure — same outcome, cleaner.
- Several loops delete while iterating: `space_manager.c:171-176`, `display_manager.c:49-54`,
  `rule.c:183-187`, `window_manager.c:1565-1572`, `event_signal.c:372-376`. Check each one
  against the swap-remove semantics before rewriting the loop shape; some `break` immediately
  after the delete, others (`event_loop.c:568-574`, `window_manager.c:1565-1572`) iterate
  backwards or re-read `buf_len` each iteration.
- The `char buf[0]` flexible-array trick has no Rust equivalent and needs none.

---

## 5. `src/misc/hashtable.h`

### 5.1 Purpose

A separate-chaining hash table with a `void *` key copied into the table and a `void *` value
that the table does not own. Keys are always small integers (window id, pid, space id,
`ProcessSerialNumber`) and the hash functions are identity functions, so this is effectively an
integer map. It is the single-header kind: declarations always, implementation only when
`HASHTABLE_IMPLEMENTATION` is defined (`manifest.m:56-58`).

### 5.2 Types, typedefs, macros

| line | item | detail |
|---|---|---|
| 4 | `#define TABLE_HASH_FUNC(name)` | `unsigned long name(void *key)` — a macro that generates a function *signature*, used both to typedef and to define |
| 5 | `typedef TABLE_HASH_FUNC(table_hash_func);` | function type, used as `table_hash_func *hash` |
| 7 | `#define TABLE_COMPARE_FUNC(name)` | `int name(void *key_a, void *key_b)` — returns "equal", not an ordering |
| 8 | `typedef TABLE_COMPARE_FUNC(table_compare_func);` | |
| 10-15 | `struct bucket` | `void *key` (**owned by the table**, `malloc`-ed `hashtable.h:126`, freed `hashtable.h:63,143`); `void *value` (**not owned**); `struct bucket *next` (owned, intrusive singly-linked chain) |
| 16-24 | `struct table` | `int count`; `int capacity`; `float max_load` (always `0.75f`, `hashtable.h:50`); `table_hash_func *hash`; `table_compare_func *cmp`; `struct bucket **buckets` (**owned**, `malloc` `hashtable.h:53`, freed `hashtable.h:70`) |
| 29 | `#define table_add(table, key, value)` | `_table_add(table, key, sizeof(*key), value)` — **the key size comes from the static type of the pointer at the call site** |
| 34-41 | `#define table_for(it, table, code)` | takes the table **by value**, not by pointer; `it` is a whole declaration (`struct view *view`), expanded as `struct view *view = bucket->value;` inside the inner loop; skips buckets whose `value` is NULL (`hashtable.h:37`); `continue` inside `code` continues the bucket loop (relied on at `window_manager.c:813`) |

### 5.3 Every live instance

| table | key type (and `sizeof`) | value | capacity | init site | hash/cmp |
|---|---|---|---|---|---|
| `g_space_manager.view` | `uint64_t` (8) | `struct view *` | 23 | `space_manager.c:1213` | `hash_view`/`compare_view`, `space_manager.c:4-12` |
| `g_window_manager.application` | `pid_t` (4) | `struct application *` | 150 | `window_manager.c:2727` | `hash_wm`/`compare_wm`, `window_manager.c:9-17` |
| `g_window_manager.window` | `uint32_t` (4) | `struct window *` | 150 | `window_manager.c:2728` | same |
| `g_window_manager.managed_window` | `uint32_t` (4) | `struct view *` | 150 | `window_manager.c:2729` | same |
| `g_window_manager.window_lost_focused_event` | `uint32_t` (4) | `(void *)(intptr_t) 1` sentinel | 150 | `window_manager.c:2730` | same |
| `g_window_manager.application_lost_front_switched_event` | `pid_t` (4) | `(void *)(intptr_t) 1` sentinel | 150 | `window_manager.c:2731` | same |
| `g_window_manager.window_animations_table` | `uint32_t` (4) | `struct window_animation *` (interior pointer into a `malloc`-ed array, `window_manager.c:609`) | 150 | `window_manager.c:2732` | same |
| `g_window_manager.insert_feedback` | `uint32_t` (4) | `struct window_node *` | 150 | `window_manager.c:2733` | same |
| `g_process_manager.process` | `ProcessSerialNumber` (8) | `struct process *` | 125 | `process_manager.c:240` | `hash_psn`/`compare_psn`, `process_manager.c:4-12`; hash is `((ProcessSerialNumber *) key)->lowLongOfPSN`, compare is `psn_equals` |

`hash_wm` (`window_manager.c:11`) is `return *(uint32_t *) key;` — identity. `hash_view` is the
same on `uint64_t`. Note the mismatch for `pid_t` keys: they are stored and compared as
`uint32_t` through `hash_wm`/`compare_wm`, which is fine only because `sizeof(pid_t) == 4`.

### 5.4 Threads and synchronisation

- **`g_window_manager.window_animations_table` is the only table touched from two threads**, and
  it is the only one with a lock: `pthread_mutex_t g_window_manager.window_animations_lock`
  (`window_manager.h:84`). The **CVDisplayLink thread** holds it across
  `window_manager.c:577-589` (`table_remove` at `:584`); the **event-loop pthread** holds it
  across `window_manager.c:619-675` (`table_find` `:631`, `table_remove` `:662`, `table_add`
  `:673`).
- **`g_window_manager.window` and `g_window_manager.insert_feedback` are iterated on the main
  thread** by `update_window_notifications` (`event_loop.c:23,28`), called from `yabai.c:341`
  while the event-loop pthread is already running and mutating them. **Unsynchronised.**
  Everything else about those tables is event-loop-thread-only.
- All other tables: event-loop pthread, plus the main-thread startup window described in §0.

### 5.5 Functions

| function | line | one line | thread | allocates / frees | external symbols |
|---|---|---|---|---|---|
| `void table_init(struct table *, int capacity, table_hash_func, table_compare_func)` | 46-55 | zeroes count, sets `max_load = 0.75f`, `malloc`s + `memset`s the bucket array | main thread (all nine init sites are in `*_begin`/`*_init` reached from `yabai.c:299-338`) | allocates the bucket array; caller frees via `table_free` | `malloc`, `memset` |
| `void table_free(struct table *)` | 57-73 | frees every bucket and its key, then the array, NULLs `buckets` | never called in `src/` outside its own definition (confirmed by grep) | frees everything it allocated; leaves `count`/`capacity` stale | `free` |
| `static struct bucket **table_get_bucket(struct table *, void *key)` | 75-86 | walks the chain and returns the address of the slot holding the match, or of the trailing NULL | whichever thread calls in | none | — |
| `static void table_rehash(struct table *)` | 88-115 | doubles capacity, **re-allocates every bucket node** while reusing the existing key allocations | event-loop / main | allocates new array + nodes, frees old ones | `malloc`, `memset`, `free` |
| `void _table_add(struct table *, void *key, int key_size, void *value)` | 117-137 | inserts if absent; if present with a NULL value, fills the value in; **otherwise does nothing** | per §5.4 | `malloc`s the bucket and a `key_size` copy of the key | `malloc`, `memcpy` |
| `void table_remove(struct table *, void *key)` | 139-149 | unlinks and frees the bucket and its key | per §5.4 | frees | `free` |
| `void *table_find(struct table *, void *key)` | 151-155 | value or NULL | per §5.4 | none | — |

No callbacks registered with the OS here; the two function-pointer fields (`hash`, `cmp`) are
yabai-internal.

### 5.6 Rust translation

```rust
pub struct Table<K, V> { /* or just use HashMap */ }
```

Recommended: **`std::collections::HashMap<K, V, BuildHasherDefault<IdentityHasher>>`** with keys
as real Rust types (`u32`, `u64`, `pid_t` as `i32`, and a `#[derive(PartialEq, Eq, Hash)]`
newtype over `ProcessSerialNumber`). The C key copy (`malloc` + `memcpy` of `sizeof(*key)`)
becomes `K` by value — that removes an entire class of ownership questions.

- `table_add` → `map.entry(k).or_insert(v);` — **not** `map.insert(k, v)`. `_table_add`
  deliberately does not overwrite an existing non-NULL value (`hashtable.h:120-123`).
- `table_find` → `map.get(&k).copied()`.
- `table_remove` → `map.remove(&k)`.
- `table_for(it, t, code)` → `for value in map.values() { ... }`, with `continue` still meaning
  "next bucket". Two call sites collect into a VLA first
  (`space_manager.c:1157-1164`, `struct view *view_list[sm->view.count]`) precisely because they
  mutate the table afterwards — in Rust collect into a `Vec` the same way, which also satisfies
  the borrow checker naturally.
- Values are raw pointers to heap objects the table does not own (`struct window *`,
  `struct view *`, …). Until phase 3 decides on ownership, keep them as `*mut T` and add a
  `#[repr(transparent)]` `Send` newtype for the one table that crosses threads.
- `window_animations_table` → `Mutex<HashMap<u32, AnimationPtr>>` where
  `struct AnimationPtr(*mut WindowAnimation); unsafe impl Send for AnimationPtr {}`. Keep the
  lock scope identical to `window_manager.c:577-589` and `:619-675` — it spans SLS transaction
  calls, and shortening it changes the interleaving with the display-link thread.
- The sentinel tables (`window_lost_focused_event`,
  `application_lost_front_switched_event`) store `(void *)(intptr_t) 1` and are only ever
  tested for presence (`window_manager.c:1366,1381`) → `HashSet<u32>` / `HashSet<pid_t>`.

**Behaviour a naive translation would silently change**

- **Insert-does-not-overwrite.** `map.insert` would change `space_manager.c:108`,
  `window_manager.c:296,673,1406,1421` semantics. Use `or_insert`.
- **A NULL value is indistinguishable from absence.** `table_find` returns `bucket->value`,
  which is NULL both when the key is missing and when a bucket exists with a NULL value
  (`hashtable.h:154`), and `table_for` skips NULL-valued buckets (`hashtable.h:37`). No call
  site stores NULL today, so `Option<V>` is faithful — but do not add a code path that stores
  `None`.
- **Iteration order is bucket order and is not random.** `HashMap` iteration order is
  randomised per process. Today no iteration order reaches the socket response
  (`window_manager_query_windows_for_spaces` at `window_manager.c:38` works off space window
  lists, not the table), but three places do consume the order:
  `update_window_notifications` (`event_loop.c:23,28` → `SLSRequestNotificationsForWindows`,
  set semantics, safe), `window_manager_find_application_windows` (`window_manager.c:1424-1436`,
  feeds the unresolved-window workaround at `window_manager.c:1652-1714`), and
  `space_manager_handle_display_add` (`space_manager.c:1160-1164`, matched by UUID, safe).
  If phase 2 wants byte-identical debug logs, use an `IndexMap` or a `BTreeMap`; otherwise
  `HashMap` is acceptable — record the choice.
- `table->hash(key) % table->capacity` (`hashtable.h:78`) is `unsigned long % int`. With the
  identity hashes and small capacities this is a plain modulo; no signedness trap in practice.
- **`table_rehash` invalidates every `struct bucket *`** but not the keys or values
  (`hashtable.h:103-109`). Nothing holds a `struct bucket *` across a call today. Keep it that
  way.
- `max_load` is a `float` compared with `float load = (1.0f * count) / capacity`
  (`hashtable.h:132-133`) — `f32` in Rust, not `f64`, if you reproduce the policy at all.
- `table_init` and `_table_add` never check `malloc`. Rust aborts instead of dereferencing NULL.
- `table_free` is dead code but must still be ported (or explicitly dropped with a note);
  `HashMap`'s `Drop` covers it.

---

## 6. `src/misc/log.h`

### 6.1 Purpose

Five variadic `printf` wrappers that are the daemon's entire logging and fatal-error story.
`debug` is gated on a global verbosity flag; `error` and `require` print and then terminate the
process with different exit codes.

### 6.2 State

| symbol | line | type | definition | threads | synchronisation |
|---|---|---|---|---|---|
| `g_verbose` | declared `log.h:4`, defined `yabai.c:51`, re-declared `message.c:12` | `bool` | zero-init | **written** by the main thread at `yabai.c:248` (`--verbose`) and by the **event-loop pthread** at `message.c:1175,1177` (`yabai -m config debug_output on|off`); **read** from every thread (`log.h:9,51`, `window_manager.c:1521,1535`, `message.c:1173`) | **none** — a plain `bool`, torn reads are benign here |

No structs, enums, unions or macros in this file.

### 6.3 Functions

All five are `static inline`, all run on whatever thread called them (main run-loop thread,
event-loop pthread, message-loop pthread — `debug` appears 63 times in `event_loop.c` alone),
and none allocate.

| function | line | behaviour | external symbols |
|---|---|---|---|
| `void debug(const char *format, ...)` | 6-15 | early-returns unless `g_verbose`; `vfprintf(stdout, ...)` | `va_start`, `vfprintf`, `va_end` |
| `void warn(const char *format, ...)` | 17-24 | `vfprintf(stderr, ...)` | same |
| `void error(const char *format, ...)` | 26-35 | `vfprintf(stderr, ...)` then **`exit(EXIT_FAILURE)`** | + `exit` |
| `void require(const char *format, ...)` | 37-46 | `vfprintf(stderr, ...)` then **`exit(EXIT_SUCCESS)`** | + `exit` |
| `void debug_message(const char *prefix, char *message)` | 48-61 | gated on `g_verbose`; prints `prefix:` then walks a **NUL-separated token list**, using `fprintf`'s return value (`1 + strlen`) to step past each NUL (`log.h:55-57`); trailing `putc('\n')` + `fflush(stdout)` | `fprintf`, `putc`, `fflush` |

`debug_message` has exactly one call site: `event_loop.c:1633`, on the event-loop pthread,
printing the daemon message just read off the socket. The message layout it walks is produced by
`client_send_message` (`yabai.c:73-82`): each argv element NUL-terminated, then one extra NUL.

No callbacks registered here.

### 6.4 Rust translation

- `debug`/`warn` → `macro_rules!` wrappers over `print!`/`eprint!`, so the format string is
  checked at compile time. `debug!` keeps the `if !g_verbose { return }` guard.
- `error!`/`require!` → macros ending in `std::process::exit(1)` / `exit(0)`. Give them
  `-> !` where possible so the compiler sees the divergence (the C code relies on it: e.g.
  `yabai.c:252` calls `error(...)` and falls through to code that would otherwise use an
  uninitialised value).
- `g_verbose` → `static VERBOSE: AtomicBool`. Relaxed load/store. This is *stricter* than the C
  plain `bool` but has no behavioural cost.
- `debug_message` → take the message as `&[u8]`, `split(|&b| b == 0)`, skip the trailing empty
  element, and join with a leading space. Do **not** try to reproduce the `fprintf`-return-value
  pointer walk.

**Behaviour a naive translation would silently change**

- **stdout buffering.** C's `stdout` is line-buffered on a tty and **fully buffered when piped**
  (which is how launchd runs yabai, `scripts/`/`assets/` service plist). Rust's `Stdout` is
  always a `LineWriter`. So a Rust port flushes `debug` output on every newline where C batched
  it into 4 KiB chunks. That changes the interleaving of yabai's stdout log with stderr and with
  the config script's output in the launchd log file. It is a change for the better, but it is a
  change — record it. `debug_message` already forces `fflush(stdout)` (`log.h:60`).
- **`__FUNCTION__`.** 63 `debug` calls in `event_loop.c`, 11 in `window_manager.c`, and more
  elsewhere start with `"%s: "`, `__FUNCTION__`. Rust has no `__FUNCTION__`. Phase 2 needs one
  small helper — either a `function_name!()` macro built on
  `std::any::type_name::<fn()>()`-style trickery, or explicit string literals. Pick one and use
  it everywhere; the log lines are effectively part of the tested surface (`tests/` greps
  output).
- **`%s` with a NULL pointer** prints `(null)` on macOS libc. `window_title_ts` and friends can
  return NULL (`helpers.h:363-372` returns NULL when `CFStringGetCString` fails) and are passed
  straight to `debug` at e.g. `window_manager.c:1445`. In Rust an `Option<&str>` must be
  formatted as `"(null)"` explicitly to match.
- `exit()` from `error`/`require` may be called on the **event-loop pthread** (any `error` inside
  message handling) — it tears down the whole process, including the main run loop, without
  unwinding. `std::process::exit` matches. Do not translate these to `panic!`, which would only
  unwind the calling thread by default.
- `require` exits with **`EXIT_SUCCESS`** (`log.h:45`) — it is the "this is a normal refusal, not
  a crash" path used at `yabai.c:268,272,276` so launchd does not restart the service. Getting
  this exit code wrong changes service behaviour.

---

## 7. `src/misc/notify.h`

### 7.1 Purpose

Desktop notifications via the deprecated `NSUserNotification` API, used to surface
scripting-addition and configuration failures where stderr would not be seen. It declares a
one-method delegate class so notifications appear even while yabai is frontmost, and stamps each
notification with yabai's own icon through private KVC keys.

Objective-C, no include guard, included once at `manifest.m:50`. Wrapped in
`#pragma clang diagnostic ignored "-Wdeprecated-declarations"` (`notify.h:7-8`, popped
`notify.h:50`).

### 7.2 State

| symbol | line | type | initial | threads | synchronisation |
|---|---|---|---|---|---|
| `g_notify_init` | 4 | `static bool` | `false` | main run-loop thread only | none needed |
| `g_notify_img` | 5 | `static NSImage *` | `nil` | main run-loop thread only | none needed |

Thread determination: the only callers are `sa.m:277,283,287,292,296,354,360,376,383,396,404`
(reached from `yabai.c:216,220` in `parse_arguments`, and from `scripting_addition_load` at
`sa.m:369`) and `helpers.h:467,473,485` in `exec_config_file`, whose only call site is
`yabai.c:348` — all on the main thread before `[NSApp run]`.

### 7.3 Types

`@interface NotifyDelegate : NSObject <NSUserNotificationCenterDelegate>` (`notify.h:10-18`),
one method: `-userNotificationCenter:shouldPresentNotification:` returning `YES`
(`notify.h:14-17`), which forces presentation even when yabai is the active app.

### 7.4 Functions

**`static bool notify_init(void)`** — `notify.h:20-27`.
- `[[NSUserNotificationCenter defaultUserNotificationCenter] setDelegate:[NotifyDelegate alloc]]`
  (`notify.h:22`) — **`alloc` without `init`**, and the delegate property is unretained, so this
  object is deliberately leaked to keep it alive. Reproduce the leak, or hold the delegate in a
  `static` on the Rust side.
- `g_notify_img = [[[NSWorkspace sharedWorkspace] iconForFile:[[[NSBundle mainBundle]
  executablePath] stringByResolvingSymlinksInPath]] retain]` (`notify.h:23`) — one explicit
  `retain`, never released.
- External: `NSUserNotificationCenter`, `NSWorkspace`, `NSBundle`, `NSImage`, `objc_msgSend`.

**`static void notify(const char *subtitle, const char *format, ...)`** — `notify.h:29-48`.
Creates an `NSAutoreleasePool` (`notify.h:31`), lazily calls `notify_init` (`notify.h:33`),
builds an `NSUserNotification` with title `@"yabai"`, `subtitle` from a C string, and
`informativeText` from `[[NSString alloc] initWithFormat:... arguments:args]`
(`notify.h:37-40`), sets the private KVC keys `@"_identityImage"` and
`@"_identityImageHasBorder"` (`notify.h:41-42`), delivers it, `release`s the notification, then
`[pool drain]` (`notify.h:47`). External: everything above plus `va_start`/`va_end`.

No callbacks registered with the OS — the delegate method is called back by
`NSUserNotificationCenter` on the main thread, and its context is the singleton delegate object
which lives forever.

### 7.5 Rust translation

Crates: `objc2`, `objc2-foundation`, `objc2-app-kit`. The build already uses `-fno-objc-arc`
(`makefile:4`), so the manual retain/release maps onto `objc2`'s explicit `Retained<T>` without
ARC assumptions.

- The delegate class → `objc2::define_class!` with
  `unsafe impl NSUserNotificationCenterDelegate for NotifyDelegate` and the single method
  returning `true`. Store the instance in a `static OnceLock<Retained<NotifyDelegate>>` so it
  outlives the unretained delegate property — that is the faithful translation of the
  `alloc`-without-`init` leak, and it is also correct.
- `g_notify_img` → `OnceLock<Retained<NSImage>>`.
- The `NSAutoreleasePool` → `objc2::rc::autoreleasepool(|_| { ... })`.
- **Drop `initWithFormat:arguments:` entirely.** Format the string in Rust with `format!` and
  hand the result to `NSString::from_str`. The eleven call sites use only `%s`
  (`helpers.h:473,485`), `%X` (`sa.m:287`) and `%s` for a version string (`sa.m:283`); all are
  trivially expressible. Passing a Rust-built `va_list` to ObjC is not worth attempting.
- The private KVC keys stay as-is: `setValue:forKey:` with `"_identityImage"` and
  `"_identityImageHasBorder"` via `msg_send!`. There is no typed binding for them.

**Behaviour a naive translation would silently change**

- `notify` is only ever called from the main thread; `NSUserNotificationCenter` is not
  thread-safe. If phase 2 moves any `notify` call onto the event-loop pthread (for example while
  translating `sa.m`, which is daemon-side and *is* being translated), it will misbehave. Keep
  the call sites on the same threads.
- The delegate is set **once, lazily, on the first `notify`** (`notify.h:33`), not at startup.
  An eager `OnceLock` initialisation at process start would change when `NSApplicationLoad`
  (`yabai.c:139`) must already have run. Keep it lazy.
- `%X` at `sa.m:287` formats a `uint32_t` attribute mask in uppercase hex with no width — Rust's
  `{:X}` matches.
- `[NSString stringWithUTF8String:]` returns `nil` for invalid UTF-8; `NSString::from_str` takes
  a `&str` that is valid by construction. Where the C string comes from the OS
  (`sa.m` version strings) this is a difference in the error path only.

---

## 8. `src/misc/timer.h`

### 8.1 Purpose

An opt-in instrumentation profiler (the Casey Muratori "anchor" design): read the CPU timer,
accumulate exclusive/inclusive cycles per call site into a fixed anchor array, and print a
breakdown. It is gated behind `PROFILE`, which **no makefile target defines** — so in every
shipped and development build today this header compiles to nothing.

### 8.2 The three compilation modes

| mode | `timer.h` lines active | what the macros do |
|---|---|---|
| `PROFILE` undefined or `< 1` (**every current build**) | 156-163 | `profile_begin()`/`profile_end_and_print()` expand to `;`, `TIME_FUNCTION`/`TIME_BLOCK(label)` to nothing, `TIME_BODY(label, c)` to `c`, `PROFILER_END_TRANSLATION_UNIT` to nothing |
| `PROFILE == 1` | 5-92 + 150-155 | totals only; the per-block macros are still no-ops |
| `PROFILE >= 2` | 5-149 | full per-anchor instrumentation |

`makefile:4,21,24,27` never pass `-DPROFILE`, and no source `#define`s it. Confirmed by grep.

### 8.3 Types

| line | item | fields |
|---|---|---|
| 8-14 | `struct profile_anchor` | `uint64_t tsc_elapsed_exclusive`, `uint64_t tsc_elapsed_inclusive`, `uint64_t hit_count`, `char const *label` (points at a string literal from `__FUNCTION__` or `#label`, static lifetime) |
| 16-22 | anonymous `static struct ... g_profiler` | `uint64_t begin_tsc`, `uint64_t end_tsc`, `struct profile_anchor anchors[4096]` (~128 KiB of BSS), `uint32_t parent` |
| 95-102 | `struct time_block` (only at `PROFILE >= 2`) | `char const *label`, `uint64_t old_tsc_elapsed_inclusive`, `uint64_t begin_tsc`, `uint32_t parent_index`, `uint32_t anchor_index` — a stack-allocated RAII guard |

### 8.4 Globals and statics

| symbol | type | initial | threads | synchronisation |
|---|---|---|---|---|
| `g_profiler` | anonymous struct, `static` | zero (BSS); re-zeroed by `profile_begin` (`timer.h:68`) | event-loop pthread only in practice — `profile_begin`/`profile_end_and_print` are called at `event_loop.c:1656,1673`, and all 65 `TIME_FUNCTION` / 5 `TIME_BODY` sites are in code reached from event handlers or message handling | **none**; `g_profiler.parent` is a plain `uint32_t` mutated by `BEGIN_TIME_BLOCK`/`END_TIME_BLOCK` |
| `cpu_freq` | **function-local `static uint64_t`** inside `read_cpu_freq`, `timer.h:38` | `0` | x86_64 only; whichever thread first calls | none — benign double-calibration race |

### 8.5 Functions

| function | line | one line | thread | external symbols |
|---|---|---|---|---|
| `static inline uint64_t read_cpu_timer(void)` | 24-33 | `__rdtsc()` on x86_64; `mrs %0, cntvct_el0` inline asm on arm64 | caller's | `__rdtsc` (compiler intrinsic) |
| `static inline uint64_t read_cpu_freq(void)` | 35-64 | x86_64: busy-wait 100 ms against `read_os_timer` and derive the TSC frequency, memoised in the static; arm64: `mrs %0, cntfrq_el0` | caller's | `read_os_timer` (`helpers.h:149-154`, `mach_absolute_time` + `AbsoluteToNanoseconds`), `read_os_freq` (`helpers.h:157-160`, constant `1000000000`) |
| `static void profile_begin(void)` | 66-70 | `memset` the profiler, stamp `begin_tsc` | event-loop pthread (`event_loop.c:1656`) | `memset` |
| `static void profile_end_and_print(void)` | 72-92 | stamp `end_tsc`, print the total in ms and one line per non-empty anchor with exclusive %, and "w/children" % when inclusive differs | event-loop pthread (`event_loop.c:1673`) | `printf` |
| `static void BEGIN_TIME_BLOCK(struct time_block *, const char *label, uint32_t anchor_index)` | 104-116 | push the parent index, snapshot the anchor's inclusive total, stamp `begin_tsc` | caller's | — |
| `static void END_TIME_BLOCK(void *context)` | 118-134 | pop the parent, subtract elapsed from the parent's exclusive, add to this anchor's exclusive, set inclusive, bump hit count, store the label | caller's | — |

Macros at `PROFILE >= 2`: `TIME_FUNCTION` (136-138), `TIME_BLOCK(label)` (140-142),
`TIME_BODY(label, c)` (144-148), `PROFILER_END_TRANSLATION_UNIT` (149, a `_Static_assert` that
`__COUNTER__ < 4096`, instantiated once at `yabai.c:356`).

No callbacks registered with the OS. `END_TIME_BLOCK` is registered as a **scope-exit
destructor** via `__attribute((cleanup(END_TIME_BLOCK)))` (`timer.h:137,141`), with the
`struct time_block` on the caller's stack as its context; the context lives exactly as long as
the enclosing scope.

### 8.6 Rust translation

- Gate the whole module behind a Cargo feature, e.g. `#[cfg(feature = "profile")]` /
  `#[cfg(feature = "profile-blocks")]`, mirroring `PROFILE >= 1` and `PROFILE >= 2`. Since no
  build enables it today, a no-op module plus a feature-gated implementation is faithful and
  cheap. **Do not silently drop it** — `TIME_FUNCTION;` appears 65 times across
  `window_manager.c`, `message.c`, `event_loop.c`, `space_manager.c`, `display.c`,
  `display_manager.c`, `rule.c`, `event_signal.c`, `view.c`, `window.c`, and phase 2 has to put
  *something* at each of those lines.
- `read_cpu_timer` → `#[cfg(target_arch = "x86_64")] core::arch::x86_64::_rdtsc()` and
  `#[cfg(target_arch = "aarch64")] { let v: u64; asm!("mrs {}, cntvct_el0", out(reg) v); v }`
  using `core::arch::asm!` in an `unsafe` block. Both registers are readable from EL0 on macOS.
- `read_cpu_freq` → same split; `cntfrq_el0` via `asm!`; the x86 calibration loop translates
  directly, with the `static` becoming a `OnceLock<u64>` or an `AtomicU64`.
- `__attribute((cleanup))` → a `struct TimeBlock` with a `Drop` impl. This is the one place
  where Rust is strictly nicer and exactly equivalent.
- **`__COUNTER__` has no Rust equivalent.** The anchor index must be unique and stable per call
  site. Options, in order of preference: (a) a small proc-macro that assigns indices at
  expansion time; (b) a `static ANCHOR: OnceLock<u32>` per call site inside the macro body,
  allocating from an `AtomicU32` on first hit — this changes indices to first-execution order
  rather than source order, which only affects the print order at `timer.h:80-91`;
  (c) `line!()`-derived indices, which collide across files. Option (b) is the least machinery.
- `PROFILER_END_TRANSLATION_UNIT` (`yabai.c:356`) → drop it, or a `const _: () = assert!(...)`
  against the anchor-array length if option (a) is chosen.

**Behaviour a naive translation would silently change**

- **`#define profile_begin();` at `timer.h:157`** (note the trailing semicolon) defines a
  *function-like* macro whose replacement list is `;`, so `profile_begin();` expands to `;;`.
  Same for `profile_end_and_print()` at `timer.h:158`. Harmless, but it means these two names
  cannot be called without parentheses, and it is why the disabled build still compiles.
- **`tb_##__FUNCTION__` at `timer.h:137` pastes to the literal identifier `tb___FUNCTION__`**,
  not to the function's name — `__FUNCTION__` is a predefined *variable*, not a macro, so it is
  not expanded before `##`. Consequence: two `TIME_FUNCTION` in the same scope would collide.
  A Rust macro using a hygienic binding has no such restriction; that is fine.
- `g_profiler` is not synchronised. If any instrumented function is ever called from the
  CVDisplayLink thread or a proxy pthread, the parent-index stack corrupts. Today
  `window_manager_animate_window_list_async`'s `TIME_BODY` blocks (`window_manager.c:617-694`)
  all run on the event-loop thread, and the display-link callback has none. Preserve that.
- `read_cpu_timer` has **no `return` on architectures other than x86_64/arm64** (`timer.h:26-32`)
  — undefined behaviour, unreachable for this project's two targets.
- `printf("... %0.4fms ...", 1000.0 * (double)total / (double)freq, freq)` (`timer.h:78`) and the
  `%.2f%%` lines (`timer.h:84,87`) — reproduce the format specifiers exactly if anyone parses
  the output.
- The `100 ms` busy-wait in `read_cpu_freq` (`timer.h:40,48-51`) blocks the calling thread. It
  only runs on x86_64 and only once, at the first `profile_end_and_print`.

---

## 9. `src/misc/autorelease.h`

### 9.1 Purpose

A debugging aid that swizzles `-[NSObject autorelease]`, `-[NSAutoreleasePool drain]` and
`-[NSAutoreleasePool release]` so each call prints a full backtrace before forwarding to the
original implementation. It is used to hunt autorelease-pool imbalance.

**It is not compiled.** `manifest.m:49` has the include commented out, and the three hook calls
are inside `#if 0` at `yabai.c:158-162`.

### 9.2 State

| symbol | line | type | notes |
|---|---|---|---|
| `g_nsobject_autorelease` | 3 | `IMP` (**non-static**, so a true global if compiled) | set once by `hook_nsobject_autorelease` (`autorelease.h:73`); read from every thread that calls `autorelease` |
| `g_nsautoreleasepool_drain` | 4 | `IMP` | set at `autorelease.h:84` |
| `g_nsautoreleasepool_release` | 5 | `IMP` | set at `autorelease.h:95` |

All three would be written once on the main thread at startup and read from every thread
thereafter, unsynchronised.

### 9.3 Types

Two Objective-C **categories**, not classes:
- `@implementation NSObject(swizzle)` with `-(NSObject *)fake_autorelease` (`autorelease.h:7-27`)
- `@implementation NSAutoreleasePool(swizzle)` with `-(void)fake_drain` (`autorelease.h:30-46`)
  and `-(void)fake_release` (`autorelease.h:48-64`)

### 9.4 Functions

All three bodies are the same shape: a 40-slot `void *addr[40]` stack array,
`backtrace(addr, 40)`, `backtrace_symbols`, a print loop, `free(syms)` (`autorelease.h:18,38,58`
— the caller owns the array `backtrace_symbols` returns), then a cast-through call to the saved
`IMP` (`autorelease.h:23,45,63`).

| function | line | thread | external symbols |
|---|---|---|---|
| `-fake_autorelease` | 8-26 | any thread that autoreleases | `backtrace`, `backtrace_symbols`, `printf`, `free`, `__FUNCTION__` |
| `-fake_drain` | 30-46 | any | same |
| `-fake_release` | 48-64 | any | same |
| `static bool hook_nsobject_autorelease(void)` | 67-76 | main thread (`yabai.c:159`, inside `#if 0`) | `objc_getClass`, `class_getInstanceMethod`, `method_setImplementation`, `method_getImplementation`, `@selector` |
| `static bool hook_autoreleasepool_drain(void)` | 78-87 | main thread (`yabai.c:160`) | same |
| `static bool hook_autoreleasepool_release(void)` | 89-98 | main thread (`yabai.c:161`) | same |

The "callbacks" here are the swizzled IMPs themselves: registered by `method_setImplementation`,
fired by the Objective-C runtime on whatever thread sends the message, with `self` as the
implicit context.

### 9.5 Rust translation

Recommendation: port it as a `#[cfg(feature = "debug-autorelease")]` module that is empty by
default, and say so in the phase-2 notes rather than dropping the file without trace. It is dead
in the C build, so omitting it changes nothing observable; but the user asked for a faithful
transposition, and a stub with the same three function names keeps the map honest.

If it is implemented:
- Categories → `objc2`'s `extern_methods!` cannot add methods to an existing class; use
  `objc2::runtime::{AnyClass, Sel}` with `class_getInstanceMethod` / `method_setImplementation`
  directly through `objc2`'s runtime bindings, and write the replacement IMPs as
  `extern "C" fn(*mut AnyObject, Sel) -> *mut AnyObject`.
- `backtrace`/`backtrace_symbols` → `libc::backtrace` / `libc::backtrace_symbols` (present on
  Apple targets), remembering to `libc::free` the returned array, or use the `backtrace` crate,
  which changes the output format.
- The saved `IMP`s → `static AtomicPtr<c_void>` each.

**Behaviour a naive translation would silently change**

- `method_setImplementation` returns the *previous* IMP; the hooks store it and forward. If the
  store and the swizzle are reordered (or if the Rust version installs the hook before
  publishing the old IMP), the first forwarded call recurses infinitely.
- `printf` from a swizzled `autorelease` is called on every autoreleased object on every thread —
  that is the point, but it means the Rust version must not take any lock that `autorelease`
  could already hold.

---

## 10. `src/misc/macho_dlsym.h`

### 10.1 Purpose

Resolve a symbol inside an already-loaded Mach-O image by walking that image's load commands and
symbol table directly, instead of asking `dlsym`. This is how yabai reaches two SkyLight symbols
that `dlsym` cannot see — one C symbol and one C++-mangled *local* symbol. No include guard;
included once at `manifest.m:54`, after `helpers.h` because it uses `string_equals`.

### 10.2 Types

No types are declared here. It consumes the system Mach-O types from `<mach-o/loader.h>` and
`<mach-o/nlist.h>` (pulled in via `<mach-o/dyld.h>` and `<mach-o/swap.h>` at
`manifest.m:6-7`): `struct mach_header_64`, `struct load_command`,
`struct segment_command_64`, `struct symtab_command`, `struct nlist_64` (whose
`n_un` is a **union** whose `n_strx` member is used at `macho_dlsym.h:73`).

### 10.3 State

None — no globals, no statics, no function-local statics.

### 10.4 Functions

All four run on the **main run-loop thread, before any other thread exists**: the only call sites
are `yabai.c:148-149`, inside `configure_settings_and_acquire_lock`, which is called at
`yabai.c:287` — before `event_loop_begin` at `yabai.c:291`. None of them allocate; every pointer
they return is an interior pointer into a loaded dyld image and is valid for the process
lifetime.

| function | line | one line | external symbols |
|---|---|---|---|
| `static struct mach_header_64 *macho_find_image_header(char *target_name, uint64_t *slide)` | 1-16 | linear scan of `_dyld_image_count()` comparing `_dyld_get_image_name(i)` against `target_name`; writes the ASLR slide through the out-param | `_dyld_image_count`, `_dyld_get_image_name`, `_dyld_get_image_vmaddr_slide`, `_dyld_get_image_header`, `string_equals` (`helpers.h:254`, i.e. `strcmp`) |
| `static struct segment_command_64 *macho_find_linkedit_segment(struct mach_header_64 *)` | 18-36 | walks `ncmds` load commands looking for `LC_SEGMENT_64` with `segname == SEG_LINKEDIT` | `string_equals` |
| `static struct symtab_command *macho_find_symtab_command(struct mach_header_64 *)` | 38-53 | walks load commands looking for `LC_SYMTAB` | — |
| `void *macho_find_symbol(char *target_image, char *target_symbol)` | 55-80 | composes the three above, computes the string and symbol table addresses, then linear-scans `nsyms` `nlist_64` entries comparing names; returns `n_value + slide` | `string_equals` |

The linkedit base computation, `macho_dlsym.h:68-69`:

```c
void *symbol_str = (void *)(linkedit_segment->vmaddr - linkedit_segment->fileoff) + symtab_command->stroff + slide;
void *symbol_sym = (void *)(linkedit_segment->vmaddr - linkedit_segment->fileoff) + symtab_command->symoff + slide;
```

### 10.5 Consumers

Both results are stored into **static function pointers declared in `extern.h`**:

- `static mach_port_t (*CGSGetConnectionPortById)(int);` (`extern.h:4`), assigned `yabai.c:148`.
  NULL-checked at `window.c:956` — the NULL case falls back to the public
  `SLSGetWindowSubLevel`. Note `window.c:946` dereferences it **without** a check, but that
  function is only reachable through the checked branch.
- `static int64_t (*SLSPerformAsynchronousBridgedWindowManagementOperation)(void *);`
  (`extern.h:5`), assigned `yabai.c:149` from the mangled local symbol
  `__ZL54SLSPerformAsynchronousBridgedWindowManagementOperationP47SLSAsynchronousBridgedWindowManagementOperation`.
  NULL-checked at `space_manager.c:667,688`.

### 10.6 Rust translation

- Declare the four Mach-O structs by hand with `#[repr(C)]` (about 30 lines) rather than pulling
  in `goblin` or `object`: those crates parse *files*, and what is needed here is the exact
  in-memory layout of an already-mapped image. `libc` provides `_dyld_image_count`,
  `_dyld_get_image_name`, `_dyld_get_image_header` and `_dyld_get_image_vmaddr_slide` on Apple
  targets; `LC_SEGMENT_64` (`0x19`), `LC_SYMTAB` (`0x2`) and `SEG_LINKEDIT` (`"__LINKEDIT"`)
  must be spelled out.
- The whole file is `unsafe`. The signature should be
  `unsafe fn macho_find_symbol(image: &CStr, symbol: &CStr) -> Option<*mut c_void>`.
- The two consumers → `static CGS_GET_CONNECTION_PORT_BY_ID: OnceLock<Option<unsafe extern "C"
  fn(i32) -> mach_port_t>>` and similar, with `std::mem::transmute` at the assignment. Keep the
  `Option` so the two NULL checks (`window.c:956`, `space_manager.c:667,688`) survive as
  `if let Some(f)`.

**Behaviour a naive translation would silently change**

- **`segname` is `char[16]` and is not guaranteed NUL-terminated.**
  `string_equals(segment->segname, SEG_LINKEDIT)` (`macho_dlsym.h:27`) `strcmp`s it; a segment
  whose name fills all 16 bytes would over-read. `"__LINKEDIT"` is 10 bytes so it never bites in
  practice. In Rust, compare the first 16 bytes up to the first NUL — do **not** use
  `CStr::from_ptr` on a possibly-unterminated array.
- **Symbol names are compared with `strcmp`** (`macho_dlsym.h:74`) against a full C string in the
  string table; that table *is* NUL-terminated, so `CStr` is correct there.
- Pointer arithmetic on `void *` (`macho_dlsym.h:23,43,68,69,72`) is a GNU extension with byte
  granularity — in Rust, cast to `*const u8` first. Getting this wrong by an element size is the
  single most likely translation bug in this file.
- `(int)header->ncmds` (`macho_dlsym.h:22,42`) narrows `uint32_t` to `int`. Keep it `u32` in
  Rust; no image has more than 2^31 load commands.
- `symbol_count = symtab_command->nsyms` narrows `uint32_t` to `int` (`macho_dlsym.h:67`).
- There are **no bounds checks anywhere**: a malformed `cmdsize` of 0 makes
  `macho_find_linkedit_segment` loop forever, and a bogus `n_strx` reads out of bounds. The C
  code trusts dyld. Reproducing that trust is fine; adding checks that return `None` where C
  would have succeeded is not.
- No fat/universal handling — by the time `_dyld_get_image_header` returns, dyld has already
  chosen the slice. Correct as-is.
- `macho_find_symbol` returns `NULL` on every failure path (`macho_dlsym.h:59,62,65,79`), and
  the callers depend on `NULL` meaning "use the public API instead". `Option::None` maps
  exactly.
- The image is looked up by its **exact path string**, including
  `/System/Library/PrivateFrameworks/SkyLight.framework/Versions/A/SkyLight`
  (`yabai.c:148-149`). On systems where the dyld shared cache reports a different name this
  returns NULL and yabai falls back — same in Rust.

---

## 11. Pattern catalogue for phase 2

Consolidated view across the ten files, with the recommended Rust shape for each. Patterns
already fully covered above are cross-referenced rather than repeated.

| # | C pattern | examples in these files | Rust translation | what a naive port breaks |
|---|---|---|---|---|
| 1 | **Single-header library with an `#ifdef IMPLEMENTATION` block** | `hashtable.h:45` guarded by `manifest.m:56-58` | one `mod`; the split disappears | nothing |
| 2 | **Header-only `static inline` functions that are really program-wide** | `ts.h`, `log.h`, `helpers.h` | plain `pub fn` in a module | treating `static` as module-private state (see §3.2) |
| 3 | **Bump / arena allocation** (`ts_alloc_*`, `memory_pool_push`) | `ts.h:52-98`, `memory_pool.h:29-45` | raw `*mut u8` over an `mmap`, `AtomicU64` bump pointer; see §2.5 and §3.4 | wrap-around semantics, post-hoc bounds checks, `ts_reset` racing the main thread |
| 4 | **Ring buffer that silently overwrites when full** | `memory_pool.h:39-43` | reproduce; do not return `Err` | dropped vs corrupted events under load |
| 5 | **Stretchy buffers** (`buf_push`, header before the data) | `sbuffer.h:11-20,41-49` | `Vec<T>`; `buf_del` → `swap_remove` | the `buf_del` return value used as a boolean (`event_loop.c:572`, `window_manager.c:1569`) |
| 6 | **Arena-backed stretchy buffers** (`ts_buf_*`) | `sbuffer.h:51-69` | also `Vec<T>` — the one deliberate deviation, see §4.6 | the unwritten "must be the last arena allocation" invariant |
| 7 | **Hash table keyed by an integer id, with a copied key** | `hashtable.h:117-137` | `HashMap<u32/u64/pid_t, *mut T>` | insert-does-not-overwrite; iteration order; NULL-value ambiguity |
| 8 | **Intrusive singly-linked chains** | `struct bucket.next`, `hashtable.h:14` | `HashMap`'s own chaining | holding a `*mut bucket` across a rehash |
| 9 | **X-macro / code-generating macros** | `TABLE_HASH_FUNC` / `TABLE_COMPARE_FUNC` (`hashtable.h:4-8`), `table_for` (`hashtable.h:34-41`), `TIME_FUNCTION` (`timer.h:136-138`) | traits + generics for the first two; `for` loops for `table_for`; `macro_rules!` + `Drop` for the third | `table_for`'s `it` being a *declaration*, and `continue` targeting the inner loop |
| 10 | **Function pointers stored in structs** | `table.hash`, `table.cmp` (`hashtable.h:21-22`) | generic `BuildHasher` + `Eq`, or `fn` pointers if genericity gets in the way | — |
| 11 | **Function pointers resolved at runtime from a Mach-O image** | `macho_dlsym.h:55-80`, `extern.h:4-5` | `OnceLock<Option<unsafe extern "C" fn(..)>>` + `transmute` | dropping the `Option` and the two NULL checks |
| 12 | **RAII via `__attribute((cleanup))`** | `timer.h:137,141` | `Drop` | — |
| 13 | **Variadic `printf` wrappers** | `log.h:6-46`, `notify.h:29-48` | `macro_rules!` over `format_args!`; build the `NSString` in Rust | stdout buffering; `%s` with NULL; `__FUNCTION__` |
| 14 | **`exit()` from a worker thread as the error path** | `log.h:34,45`, `ts.h:32` | `std::process::exit`, never `panic!` | `panic!` unwinds one thread and leaves the daemon half-alive; `require` must exit `0` |
| 15 | **Fixed `char` arrays with `snprintf`** | `MAXLEN` (`macros.h:20`), `yabai.c:44-47,85-86,135-137` | `String`/`PathBuf`, or `[u8; 512]` where the array is handed to an OS struct (e.g. `sockaddr_un.sun_path`, `helpers.h:194`) | silent truncation at 512 that `String` would not reproduce; NUL termination when the buffer goes to the kernel |
| 16 | **Bit flags in an `int`** | `HANDLE_*` (`macros.h:36-40`) | `bitflags!` | equality-with-the-whole-set comparisons (`window_manager.c:374,401`) |
| 17 | **Magic integers sharing one field** | `DIR_*` + `STACK` (`macros.h:26-31`) | one enum | splitting them into two types |
| 18 | **Sparse designated-initialiser arrays indexed by a constant** | `layer_str[]` (`helpers.h:175-181`) indexed by `LAYER_*` (`macros.h:42-45`) | `match` on the enum | the NULL holes at indices 1 and 2 |
| 19 | **Tri-state `int` returns** | `REGEX_MATCH_*` (`macros.h:22-24`) | three-variant enum | collapsing to `bool` |
| 20 | **Lock-free CAS loops** (`__sync_bool_compare_and_swap`) | `memory_pool.h:36,40`, `ts.h:59` | `compare_exchange(.., SeqCst, Relaxed)` | using `Relaxed` — `__sync_*` are full barriers |
| 21 | **`volatile` used as a stand-in for atomics** | `memory_pool.h:8`, `ts.h:7` | `AtomicU64`; a plain store (`ts.h:102`, `event_signal.c:66`) becomes `store(Relaxed)` | assuming `volatile` gave ordering guarantees it never did |
| 22 | **Inline assembly** | `timer.h:30` (`mrs cntvct_el0`), `timer.h:61` (`mrs cntfrq_el0`) | `core::arch::asm!` behind `#[cfg(target_arch)]` | — |
| 23 | **Compiler intrinsics** | `__rdtsc()` (`timer.h:27`) | `core::arch::x86_64::_rdtsc()` | — |
| 24 | **`mmap` + guard page** | `memory_pool.h:21-24`, `ts.h:20-23` | `libc::mmap` + `libc::mprotect`; not `memmap2` | losing the guard page turns an immediate SIGBUS into silent corruption |
| 25 | **Objective-C message sends, categories, manual retain/release, `NSAutoreleasePool`** | `notify.h:10-48`, `autorelease.h:7-65` | `objc2` + `define_class!` + `autoreleasepool` + `Retained<T>` | the deliberate `alloc`-without-`init` delegate leak (`notify.h:22`) |
| 26 | **ObjC method swizzling** | `autorelease.h:67-98` | `objc2::runtime` + `method_setImplementation` | recursion if the old IMP is not published before the swap |
| 27 | **Private KVC keys** | `notify.h:41-42` | `msg_send![obj, setValue: v, forKey: key]` | — |
| 28 | **Backtrace capture** | `autorelease.h:11-18` | `libc::backtrace` (+ manual `free`) or the `backtrace` crate | forgetting to `free` the `backtrace_symbols` array |
| 29 | **Walking OS binary structures with raw pointer arithmetic** | `macho_dlsym.h:23,43,68-72` | cast to `*const u8`, `.add(n)`, then cast to the struct | `void *` byte arithmetic vs typed-pointer element arithmetic |
| 30 | **Unions** | `nlist_64.n_un.n_strx` (`macho_dlsym.h:73`) | `#[repr(C)] union` or just the one `u32` field | — |
| 31 | **Zero-length array as a flexible array member** | `sbuffer.h:8,38` | not needed; `Vec` owns its buffer | — |
| 32 | **Compile-time gating by a `-D` macro** | `#if PROFILE >= 1` (`timer.h:4`), `#if PROFILE >= 2` (`timer.h:94`), `HASHTABLE_IMPLEMENTATION` | Cargo features | — |
| 33 | **`assert` that vanishes under `-DNDEBUG`** | `ts.h:38,78,91` | `debug_assert!` to match `make install` (`makefile:27`) | `assert!` panicking in release where C silently continued |
| 34 | **`#pragma clang diagnostic push/ignored/pop`** | `ts.h:73-87`, `notify.h:7-50` | `#[allow(...)]`, or nothing | — |
| 35 | **Reading globals from `fork()`ed children** | `event_signal_flush` (`event_signal.c:62-95`) reads `g_signal_storage` and the temp-storage strings it points at | keep `fork` via `libc::fork`; nothing between the fork and `execvp` may allocate | `ts_reset()` must stay **after** `event_signal_flush()` (`event_loop.c:1670-1671`) |

Not present in these ten files (looked for, none found): `goto`-out cleanup, `setjmp`,
`regex.h` (used in `helpers.h`/`rule.c`, not here), SIMD intrinsics, dispatch blocks, CAS on an
`id_ptr` liveness check, CF retain/release pairs (those live in `window_manager.c`/`window.c`),
`fork`/`exec` (in `helpers.h`/`event_signal.c`), and `printf`-into-`FILE*` response formatting
(that is `message.c`'s `FILE *rsp`).

---

## 12. Comments to carry over

**There are none.** All ten assigned files were checked for `//`, `/*` and `*/`: zero matches in
every one. Phase 2 must therefore produce these modules with **no comments at all**, except that
the following non-comment directives carry intent and should be reflected in the Rust code's
structure (as attributes or `cfg`s), not as prose:

| file:line | directive | reflect as |
|---|---|---|
| `ts.h:73-74`, `ts.h:87` | `#pragma clang diagnostic push` / `ignored "-Wunused-parameter"` / `pop` around `ts_expand` | `#[allow(unused_variables)]` on the `old_size` parameter, or use it in a `debug_assert!` as C does |
| `notify.h:7-8`, `notify.h:50` | `#pragma clang diagnostic ignored "-Wdeprecated-declarations"` | `#[allow(deprecated)]` on the notification module |
| `timer.h:4`, `timer.h:94`, `timer.h:150`, `timer.h:156` | the `PROFILE` gates | two Cargo features |
| `hashtable.h:45` | `#ifdef HASHTABLE_IMPLEMENTATION` | nothing; it disappears |
| `manifest.m:49` | the commented-out `#include "misc/autorelease.h"` | a `#[cfg(feature = "debug-autorelease")]` module that is off by default |
| `yabai.c:158-162` | `#if 0` around the three hook calls | same feature gate |
