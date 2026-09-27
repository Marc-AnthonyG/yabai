# space-and-display — C map for the Rust rewrite (phase 1)

Sources covered, read in full:

- `src/space.h` (12 lines), `src/space.c` (106 lines)
- `src/display.h` (46 lines), `src/display.c` (252 lines)
- `src/display_manager.h` (93 lines), `src/display_manager.c` (506 lines)
- `src/space_manager.h` (111 lines), `src/space_manager.c` (1233 lines)

All C locations below are `path:line` into those files unless another path is written out.

---

## 0. Context the phase-2 translator needs before opening any of these files

### 0.1 Unity build and implicit globals

`src/manifest.m` includes every header and then every `.c`/`.m` in a fixed order:

```
sa.m, mission_control.c, event_loop.c, event_signal.c, workspace.m, rule.c, message.c,
display.c, space.c, view.c, window.c, process_manager.c, application.c,
display_manager.c, space_manager.c, window_manager.c, mouse_handler.c, yabai.c
```

Every global is *defined* last, in `src/yabai.c:27-52`, and *declared* by whichever `.c` file first
needs it. Because it is one translation unit, a declaration made in an earlier file stays in scope
for every later file. Two of the assigned files exploit this and declare less than they use:

| File | `extern` it declares | Globals it uses without declaring (declaration inherited from) |
|---|---|---|
| `space.c:1` | `g_connection` | `g_window_manager` (`event_loop.c:5`, included earlier) |
| `display.c:1-2` | `g_event_loop`, `g_connection` | `g_display_manager` (`event_loop.c:3`) |
| `display_manager.c:1-3` | `g_display_manager`, `g_window_manager`, `g_connection` | — |
| `space_manager.c:1-2` | `g_window_manager`, `g_connection` | `g_space_manager` (`event_loop.c:5`) |

Rust consequence: there is no per-file ownership story to recover from the `extern` lines. Treat
`g_space_manager`, `g_display_manager`, `g_window_manager`, `g_connection` as one process-wide state
blob (see §2 pattern **P1**).

### 0.2 Threads that exist in the daemon

| Thread | Created at | What runs on it |
|---|---|---|
| main run loop | `yabai.c:350` `[NSApp run]` | AXObserver callbacks (`application.c:57`), mission-control observer (`mission_control.c:87`), `CGEventTap` mouse callback (`mouse_handler.c:288`), **`display_handler` (`display.c:6`)**, NSWorkspace notifications (`workspace.m:157-192`). Each of these only *posts* to the event loop. |
| event-loop pthread | `event_loop.c:1718` | `event_loop_run`: pops the lock-free event queue, runs `EVENT_HANDLER_*`, then `event_signal_flush()` and `ts_reset()`. `DAEMON_MESSAGE` (`event_loop.c:1614`) parses and executes every `yabai -m` command here. **Essentially every function in the four assigned `.c` files runs on this thread.** |
| message-loop pthread | `message.c:3042` | `accept()` in a loop and `event_loop_post(DAEMON_MESSAGE, NULL, sockfd)` (`message.c:3009`). Never calls into these files. |
| CVDisplayLink thread + per-window proxy pthreads | `window_manager.c:700-702`, `window_manager.c:666` | window animation only; never calls into these files. |
| forked children | `event_signal.c:64`, `event_signal.c:83` | `setenv` + `execvp` only. `space_manager_mission_control_index` / `display_manager_display_id_arrangement` are called by `event_signal_push` (`event_signal.c:230,255,282,307`) **before** the fork, i.e. on the event-loop thread. |

How each per-function "Thread" line below was determined: by walking every caller. `message.c` handlers
and `event_loop.c` handlers are both reached only from `event_loop_run`, so anything they call is
event-loop thread. `mouse_handler.c` reaches `space_manager_untile_window` /
`space_manager_tile_window_on_space` (`mouse_handler.c:135,223,227`) but only from
`mouse_drop_action_*`, which is called from the `MOUSE_UP` event handler, not from the tap callback.
The only main-thread entries into these files are `display_handler` (registered
in `display_manager_begin`), and the two startup calls `display_manager_begin` (`yabai.c:303`) and
`space_manager_begin` (`yabai.c:337`).

**Documented C race, do not "fix" it silently:** `event_loop_begin` (`yabai.c:291`) starts the
event-loop thread *before* `display_manager_begin` (`yabai.c:303`) and `space_manager_begin`
(`yabai.c:337`) initialise `g_display_manager` / `g_space_manager` on the main thread. Events posted
by `process_manager_begin` (`yabai.c:299`) and the workspace observers can therefore be handled
against a zero-initialised manager. The C code gets away with it; the Rust version should preserve the
ordering exactly (§2 **P1**) rather than pretend it is synchronised.

### 0.3 Temporary storage (`ts_*`) — lifetime of every `uint64_t *`/`uint32_t *` returned here

`src/misc/ts.h` is an 8 MB `mmap`'d bump arena (`yabai.c:279`, `MEGABYTES(8)`), shared by every
thread, bumped with `__sync_bool_compare_and_swap` (`ts.h:59`) or `__sync_fetch_and_add`
(`ts.h:68`), and **reset wholesale by the event-loop thread after every event** (`event_loop.c`,
`ts_reset()` right after each handler). `ts_assert_within_bounds` (`ts.h:28-34`) calls
`exit(EXIT_FAILURE)` on overflow — a hard process exit with no unwinding.

Functions in these files that hand back `ts` memory: `space_window_list_for_connection` (`space.c:35`),
`space_window_list` (`space.c:83`), `display_space_list` (`display.c:237`),
`display_manager_active_display_list` (`display_manager.c:401`), and `ts_cfstring_copy`
(`display.c:40`). Nobody stores those pointers past the current event.

### 0.4 Build flags that change semantics

`makefile:27` (`install` target) builds with `-DNDEBUG -O3`. Every `assert()` in these files is a
**no-op in release**: `space_manager.c:451` (`assert(node)`), `display_manager.c:99` and
`display_manager.c:362` (`assert(uuid)`). Translate them as `debug_assert!`, never `assert!`/`unwrap`,
or release behaviour changes from "carry on with a null/garbage value" to "panic".

The daemon is built `-arch x86_64 -arch arm64` (`makefile:4`); `display_manager_menu_bar_rect`
(`display_manager.c:304-332`) has `#ifdef __x86_64__` / `#elif __arm64__` bodies that must both
survive as `#[cfg(target_arch = ...)]`.

---

## 1. Shared vocabulary

- `sid` — `uint64_t` SkyLight managed-space id. `0` means "none".
- `did` — `uint32_t` `CGDirectDisplayID`. `0` means "none".
- `cid` — `int` SkyLight connection id; `g_connection` is the daemon's own (`yabai.c:143`), `0` in
  `space_window_list_for_connection` means "any owner".
- "mission control index" — 1-based position of a space in the flattened
  `SLSCopyManagedDisplaySpaces` order; `0` means not found.
- "arrangement" — 1-based position of a display in `SLSCopyManagedDisplays` order (optionally
  re-sorted by `g_display_manager.order`); `0` means not found.

---

## 2. Pattern library (the Rust recipes; per-file catalogues in §3-§10 cite these by id)

### P1 — Mutable process-wide state reached without a parameter

Examples: `space_manager.c:760-774` and `space_manager.c:784` reach `g_space_manager` although the
function takes no `sm`; `display.c:58,95,129-136` reach `g_display_manager`; `space.c:45,58` reach
`g_window_manager`; `display_manager.c:409` reaches `g_window_manager.system_element`.

**Recommended translation.** Do *not* introduce a `static Mutex<...>`: the call graph is re-entrant
(`space_manager_focus_space` → `space_manager_active_space` → `window_manager_focused_window`), so a
mutex deadlocks on the same thread. Instead:

```rust
pub struct Globals {
    pub space_manager: SpaceManager,
    pub display_manager: DisplayManager,
    pub window_manager: WindowManager,
    pub connection: i32,
    // ...
}
```

owned by the event-loop thread, and give every translated function that currently reads a global an
explicit `&mut Globals` (or a narrower `&mut SpaceManager, &mut WindowManager` pair) parameter. This
is a mechanical signature change, behaviour-identical, and it is what phase 3 wants anyway. For the
two startup functions that run on the main thread (`display_manager_begin`, `space_manager_begin`),
construct the value on main and move it into the event-loop thread exactly where C hands over.

If phase 2 wants the smallest diff instead, the fallback is a `SyncUnsafeCell<Globals>` newtype with
`unsafe fn globals() -> &'static mut Globals`, documented "event-loop thread only". Do not use
`static mut` (deny-by-default in edition 2024) and do not use `thread_local!` (the main thread's
`display_handler` and the startup calls would see a different instance).

**Naive-translation hazard:** wrapping each manager in its own `RefCell` will hit real runtime
double-borrow panics — see **P7** for the concrete aliasing site.

### P2 — `goto out` / `goto err` cleanup ladders

Sites: `space.c:26,29,76,78`; `display.c:195,198,212,214,223,226,248,250`;
`display_manager.c:170,173,176,191,193,195,436,439,447,449`; `space_manager.c:509,516,539,546,575,581,604,610`.

**Rust:** RAII. A `CFRef<T>` wrapper (see **P8**) makes almost all of these disappear; the remaining
"compute a value, then return it" shape becomes an early `return` or a `let ... else`. Where the C
ladder reorders releases (`display.c:211-214`: release the array, then the uuid), the order is
irrelevant to CF and can be dropped naturally.

**Hazard:** two ladders skip an initialisation on the error path and the caller reads the
out-parameter anyway — `space.c:26` (`goto err` before `*count` is ever written) and
`display.c:218-251` (`*count` is only written inside the matching-display branch, so a display with no
entry leaves `*count` untouched). Both are safe today only because the caller checks the returned
pointer for `NULL` first. In Rust return `Option<Vec<T>>` / `Vec<T>` and the hazard is structurally
gone — but do not "helpfully" set a count to 0 in a place where C leaves it alone and a caller might
be depending on its own initialiser (`space_manager.c:754,757` initialise their counts to 0).

### P3 — Stretchy buffers (`buf_push` / `buf_del` / `buf_len`)

`src/misc/sbuffer.h:11-32`. Sites: `display_manager.c:25,37,49,53,65,69,74`;
`space_manager.c:147,159,171,175,187,191,196,784,1135`.

**Rust:** `Vec<T>`. Two details must carry over:

- `buf_del(b, x)` (`sbuffer.h:19`) is **swap-remove**: it moves the last element into the hole. Use
  `Vec::swap_remove`, not `Vec::remove`, or the observable ordering of `yabai -m query --spaces`
  label lookup changes.
- `buf_len(NULL)` is `0` (`sbuffer.h:15`), so a never-initialised `labels` pointer is a valid empty
  list. `space_manager_begin` sets `sm->labels = NULL` (`space_manager.c:1211`); `display_manager_begin`
  never touches `dm->labels` at all, relying on the global being zero-initialised. In Rust,
  `Vec::new()` in both `Default` impls.

### P4 — Hash table keyed by `uint64_t`, values are raw pointers

`src/misc/hashtable.h`. `space_manager.c:4-12` defines `hash_view` (identity hash on the sid) and
`compare_view`; `table_init(&sm->view, 23, hash_view, compare_view)` at `space_manager.c:1213`.

Memory model: `_table_add` (`hashtable.h:117-137`) `malloc`s an 8-byte **copy** of the key and stores
the `void *value` as-is; `table_remove` (`hashtable.h:139-149`) frees the key and the bucket but
**never the value**; `table_rehash` (`hashtable.h:88-115`) doubles capacity above 0.75 load and
reallocates the bucket array.

**Rust:** `HashMap<u64, Box<View>>` with a trivial hasher. Because the hash is the identity function
and the bucket index is `hash % capacity`, iteration order in C is "sid modulo capacity" order; if any
serialisation depends on that order it will change (it does not — `table_for` is only used for
broadcast updates, `space_manager.c:262,279,291,303,315,327,339,349,1160`). Consider
`std::collections::HashMap` with `BuildHasherDefault<IdentityHasher>` to keep lookups allocation-free,
or a `BTreeMap<u64, View>` if deterministic order is wanted — flag the change if you do.

**Hazard:** `table_remove` leaking the value is load-bearing in
`space_manager_swap_space_with_space_on_display` (`space_manager.c:763-774`) and
`space_manager_handle_display_add` (`space_manager.c:1181-1190`): both remove a view and immediately
re-insert the *same* `struct view *` under a different key. In Rust, `map.remove(&a)` gives you the
`Box<View>` back — re-insert that same box; do not clone.

### P5 — `table_for` macro iteration

`hashtable.h:34-41` expands to a nested loop over buckets, skipping `bucket->value == NULL`, and
declares `int i` in the enclosing scope. Sites: `space_manager.c:262,279,291,303,315,327,339,349,1160`.

**Rust:** `for view in sm.view.values_mut()`. But see **P7** — three of these sites call back into
code that can insert into the same table.

### P6 — Arena allocation (`ts_alloc_list`, `ts_resize`, `ts_cfstring_copy`)

Sites: `space.c:35` (`ts_alloc_list(uint32_t, *count)`), `space.c:71` (`ts_resize` shrinking the list
to the number actually kept), `display.c:40` (`ts_cfstring_copy`), `display.c:237`
(`ts_alloc_list(uint64_t, spaces_count)`), `display_manager.c:401`.

**Rust:** return owned `Vec<u32>` / `Vec<u64>` / `String`. `ts_resize` becomes `vec.truncate(n)`
(it only ever shrinks here: `space.c:71` with `window_count <= *count`). The arena buys nothing in
Rust; the allocation counts are tiny (a display's spaces, a space's windows) and the arena's real
purpose — no free calls in the event handler — is what `Vec`'s `Drop` gives for free.

**Hazards:**
- `ts_resize` asserts the block is the arena's top (`ts.h:91`); in release that assert is gone, so C
  silently corrupts the bump pointer if another `ts_alloc` happened in between. `space.c:37-69` calls
  `window_manager_find_window` inside the loop, which does *not* allocate — verify no equivalent call
  sneaks in when reordering.
- Temp-storage exhaustion `exit()`s (`ts.h:31-32`). A `Vec` OOM aborts too, so behaviour is close
  enough; just do not turn it into a `Result` that a caller ignores.

### P7 — Iterating a container while a callee may mutate it

`space_manager_set_layout_for_all_spaces` (`space_manager.c:259-274`) walks `sm->view` with
`table_for` and calls `window_manager_validate_and_check_for_windows_on_space`
(`window_manager.c:2625`), whose first statement is `space_manager_find_view(sm, sid)` — which
`table_add`s and can `table_rehash`, freeing the bucket array being walked.

It is safe *in practice* only because the `sid` passed is `view->sid`, i.e. a key already present, so
no insertion occurs. Rust will refuse the shape outright.

**Rust:** collect first, then act.

```rust
let sids: Vec<u64> = sm.view.values().filter(|v| !v.flags.contains(ViewFlag::LAYOUT)).map(|v| v.sid).collect();
for sid in sids { /* ... */ }
```

Same treatment for `space_manager_refresh_application_windows` (`space_manager.c:1133-1148`), which is
the swap-remove-while-iterating idiom: `window_manager_add_existing_application_windows(..., i)`
`buf_del`s at index `i` and the loop compensates with `--refresh_count; --i;`
(`space_manager.c:1143-1145`). Rust:

```rust
let mut i = 0;
while i < refresh_count { if add_existing(..., i) { refresh_count -= 1; } else { i += 1; } }
```

keeping the `window_count` snapshot taken *before* the loop (`space_manager.c:1137`) and the
`window_count != wm.window.len()` comparison after it (`space_manager.c:1147`).

### P8 — CoreFoundation retain/release pairs

Every `CFCreate*`/`*Copy*` in these files returns +1 and is released on each path:
`space.c:8-12,24-25,74-79`; `display.c:38-41,103-107,106-109,114-118,181-185,194-213,222-249`;
`display_manager.c:83,93,98-102,109-119,126-135,169-194,202-217,214 (CFRetain),223-227,361-365,379-383,456-458`;
`space_manager.c:496-517,526-547,562-582,591-611,619-629,637-649,668-674,676-678,689-695,697-699,1168-1195`.

**Rust:** use `core-foundation` (`CFString`, `CFArray`, `CFDictionary`, `CFNumber`, `CFUUID`) where the
type is public, and a hand-rolled

```rust
pub struct CFRef<T>(*const T);           // owns +1
impl<T> Drop for CFRef<T> { fn drop(&mut self) { unsafe { CFRelease(self.0 as CFTypeRef) } } }
```

for the SkyLight types that have no crate binding (`SLSWindowQueryWindows` / `...CopyWindows`
results at `space.c:31-32`). Borrowed (+0) results must **not** be wrapped: `CFArrayGetValueAtIndex`
and `CFDictionaryGetValue` (`display.c:202-206,230-242`; `space_manager.c:501-507,622-646,1172`)
return unretained references. `display_manager.c:214` explicitly `CFRetain`s a borrowed element before
returning it — that one *is* +1 to the caller.

**Hazards:**
- `display_manager_find_element_at_point` (`display_manager.c:406-426`) **leaks** `element_ref` when
  `role` comes back `NULL` (`display_manager.c:414` returns without releasing). Preserve or fix
  deliberately — an RAII translation fixes it silently, which is a behaviour change (a leak, so
  fixing is fine; just say so in the commit).
- `space.c:31-32` never null-checks `query`/`iterator`; a `NULL` would crash in
  `SLSWindowIteratorAdvance`. Keep the same non-check or make it an explicit early return, but do not
  turn it into a silent empty list without noting it.

### P9 — ObjC runtime message send through `objc_msgSend`

`space_manager.c:669-674` and `space_manager.c:690-695`:

```c
Class cls = objc_getClass("SLSBridgedMoveWindowsToManagedSpaceOperation");
SEL sel = sel_registerName("initWithWindows:spaceID:");
id operation = ((id (*)(id, SEL, id, uint64_t))objc_msgSend)([cls alloc], sel, (__bridge id)window_list_ref, sid);
SLSPerformAsynchronousBridgedWindowManagementOperation(operation);
[operation release];
```

**Rust:** `objc2` — `AnyClass::get(c"SLSBridgedMoveWindowsToManagedSpaceOperation")`, `msg_send![cls,
alloc]`, `msg_send![obj, initWithWindows: cf_array_as_id, spaceID: sid]`, then `msg_send![obj,
release]`. The build is `-fno-objc-arc` (`makefile:4`), so the manual `release` is real and must be
kept; `objc2`'s `Retained`/`Id` types handle it if you construct them correctly, otherwise call
`release` explicitly. The cast of `objc_msgSend` to a concrete fn-pointer type is mandatory on arm64
(variadic `objc_msgSend` does not work there) — `objc2` does the right thing.

### P10 — Dynamically resolved private symbol used as a feature flag

`misc/extern.h:5` declares `static int64_t (*SLSPerformAsynchronousBridgedWindowManagementOperation)(void *);`
resolved at `yabai.c:149` with `macho_find_symbol` on the SkyLight binary. It is tested for
non-`NULL` as a macOS-version feature probe at `space_manager.c:667` and `space_manager.c:688`.

**Rust:** `static SLS_PERFORM_ASYNC_BRIDGED_OP: AtomicPtr<c_void>` written once during startup on the
main thread, read on the event-loop thread; `let f: Option<extern "C" fn(*mut c_void) -> i64> =
unsafe { std::mem::transmute(ptr) }`. This is a genuine cross-thread global (main writes, event loop
reads, no barrier in C) — use `Ordering::Relaxed` loads to match, or `SeqCst` and note the
strengthening.

### P11 — Compiler barrier around a save/restore of a global

`space_manager.c:750-752` and `space_manager.c:796-797`:

```c
float window_animation_duration = g_window_manager.window_animation_duration;
g_window_manager.window_animation_duration = 0.0f;
__asm__ __volatile__ ("" ::: "memory");
   ...
__asm__ __volatile__ ("" ::: "memory");
g_window_manager.window_animation_duration = window_animation_duration;
```

The field is read on the same (event-loop) thread by `window_manager_animate_window_list`
(`window_manager.c:610,709,722`); the barrier is a *compiler* barrier, not a memory fence, and the
same idiom appears at `window_manager.c:2656-2676`.

**Rust:** the cleanest faithful form is an RAII guard whose `Drop` restores the old value —
`struct RestoreWindowAnimationDurationOnDrop { previous: f32 }`. It is also panic-safe, which the C
is not (an `exit()` mid-way leaves the duration at 0). If you want the literal barrier,
`std::sync::atomic::compiler_fence(Ordering::SeqCst)`.

### P12 — X-macro list generating an enum plus two parallel tables

`display.h:7-35` generates `enum display_property` (bit values `0x01`-`0x40`) plus
`display_property_val[]` and `display_property_str[]`, consumed by `parse_properties`
(`message.c:2425`, definition `message.c:625`) to turn `--display id,frame` into a `uint64_t` mask.
`view.h:7-40` does the same for spaces (`space_property_*`), used by `view_serialize`, which
`space_manager_query_*` calls.

**Rust:** `bitflags!` for the mask plus one `const TABLE: &[(&str, DisplayProperty)]`, or a
`#[derive(EnumIter)]`-free hand-written `fn from_name(&str) -> Option<Self>`. Keep the bit values
identical — they are not part of the wire protocol, but `flags == 0` is special-cased.

### P13 — `flags == 0` means "all properties"

`display.c:24`: `if (flags == 0x0) flags |= ~flags;` — i.e. `flags = UINT64_MAX`, so every property
block runs, including `DISPLAY_PROPERTY_HAS_FOCUS`.

**Rust:** `let flags = if flags.is_empty() { DisplayProperty::all() } else { flags };` — note
`bitflags`' `all()` is only the *defined* bits, whereas C sets all 64. That difference is invisible
here (only defined bits are tested) but would matter if someone later adds a "unknown bits are an
error" check.

### P14 — `fprintf` into a `FILE *rsp` response stream

Sites: `display.c:27-98`; `display_manager.c:13-18`; `space_manager.c:22,34,40,42,55,61,63,76,87,90,92`.
`rsp` is `fdopen(sockfd, "w")` from `event_loop.c:1632`, flushed and closed at
`event_loop.c:1636-1637`. Return values are never checked anywhere; `SIGPIPE` is ignored
(`yabai.c:152`).

**Rust:** `rsp: &mut dyn std::io::Write` (a `BufWriter<UnixStream>` at the top). Every `fprintf`
becomes `let _ = write!(rsp, ...)` — swallowing the error is required to match C, and it avoids
panicking when the client hangs up.

Formatting equivalences that must be exact (this is the CLI's wire format):

| C | Rust | Note |
|---|---|---|
| `%d` on `uint32_t did` (`display.c:30`) | `{}` on `u32` | C reinterprets as `int`; a did ≥ 2^31 would print negative in C and positive in Rust. Never happens in practice — note it and move on. |
| `%.4f` (`display.c:67`) | `{:.4}` | Matches for finite values. C prints `inf`/`nan`, Rust prints `inf`/`NaN`. |
| `%s` with a possibly-`NULL` char* | guarded by `uuid ? uuid : "<unknown>"` (`display.c:44`) | `Option<String>` + `unwrap_or("<unknown>")`. |
| `json_bool()` (`helpers.h:233`, used `display.c:95`) | `if b {"true"} else {"false"}` | |
| `fprintf(rsp, "%c", cond ? ',' : ']')` (`display_manager.c:16`, `space_manager.c:40,61,90`) | same char logic | see **P15**. |

### P15 — JSON array terminator emitted inside the loop

`display_manager.c:14-18`, `space_manager.c:35-41`, `space_manager.c:56-62`, `space_manager.c:77-91`
all print `[`, then for each element `,` or `]` depending on whether it is the last.

Two real, observable quirks that a "clean" translation would silently repair:

1. **Empty list prints an unterminated array.** `display_manager_query_displays` with `count == 0`
   emits `[` then `\n` — no `]`. Same for the space queries.
2. **A skipped element eats the terminator.** `space_manager_query_spaces_for_window`
   (`space_manager.c:36-41`) `continue`s when `space_manager_query_view` returns `NULL`; if the
   *last* element is the skipped one, `]` is never written. Same at `space_manager.c:57-62` and
   `space_manager.c:83-88`.

Preserve both. If phase 2 wants a `serde_json`-shaped writer, it must reproduce exactly these byte
sequences, which in practice means keeping the hand-rolled `write!` calls.

### P16 — Out-parameter `int *count` beside a returned pointer

`space.c:17,83`; `display.c:218`; `display_manager.c:398`.
**Rust:** return `Vec<T>` (length carries the count) or `Option<Vec<T>>` where `NULL` was meaningful.
Callers that pre-initialise their count to 0 (`space_manager.c:754,757`) then guard on the pointer
translate to `unwrap_or_default()`.

### P17 — Pointer-sized integer smuggled through a `void *`

`display.c:9,11,13,15` — `event_loop_post(..., (void *)(intptr_t) did, 0)`;
`display_manager.c:142,180,210` — the sort axis passed as `(void *)(uintptr_t) g_display_manager.order`
and read back with a cast.

**Rust:** for the event queue, a proper `enum Event { DisplayAdded(u32), ... }` payload — that is the
whole point of the rewrite. For the CF comparator context, `*mut c_void` carrying
`order as usize as *mut c_void` stays as-is inside the `unsafe extern "C"` trampoline (§P18).

### P18 — Function pointers handed to C: run-loop callback, hash functions, CF comparator

| Site | C symbol | Rust |
|---|---|---|
| `display_manager.c:505` registers `display_handler` (`display.c:6`) with `CGDisplayRegisterReconfigurationCallback` | `void (*)(CGDirectDisplayID, CGDisplayChangeSummaryFlags, void *)` | `unsafe extern "C" fn display_reconfiguration_callback(did: u32, flags: u32, context: *mut c_void)`. Must not unwind — wrap the body in `std::panic::catch_unwind` or keep it panic-free. |
| `space_manager.c:4,9` `hash_view` / `compare_view` via `TABLE_HASH_FUNC` macros | `unsigned long (*)(void *)`, `int (*)(void *, void *)` | gone entirely once the table is a `HashMap<u64, _>`. |
| `display_manager.c:180,210` `display_manager_coordinate_comparator` via `CFArraySortValues` | `CFComparisonResult (*)(const void *, const void *, void *)` | `unsafe extern "C" fn` trampoline, or — better — copy the `CFStringRef`s into a `Vec`, sort with `sort_by` in Rust, and keep `CFArraySortValues` only if you need bit-identical tie-breaking. The comparator is total and deterministic, so a Rust `sort_by` with the same key gives the same order **provided you keep the `f32` narrowing**, see **P19**. |

### P19 — Float/int narrowing and truncation that is observable

| Site | C | Consequence |
|---|---|---|
| `display_manager.c:150-151,156-157` | `float a_coord = ... a_center.y` from a `CGFloat` (f64) | Two displays whose centres differ by less than `f32` precision compare **equal** in C and get the array's original relative order. In Rust, `let a_coord: f32 = a_center.y as f32;` — do **not** compare as `f64`. |
| `display_manager.c:286` | `int distance = area_distance_in_direction(...)` where the body (`view.c:563-580`) returns `int` computed from `float` subtractions | truncation toward zero. Keep `as i32` on the `f32` result, not `round()`. |
| `display_manager.c:126,132-136,141-144` | `int effective_ext_top_padding`, `int notch_height` mixed into `CGFloat` arithmetic | implicit int→double promotion; in Rust write the `as f64` casts explicitly. |
| `space_manager.c:958` | `float sign = (new_index - cur_index) > 0 ? 1.0 : -1.0;` then `sign * 9999.0` at line 963 | `f32` literal narrowing then re-widening to `f64` for `CGEventSetDoubleValueField`. Exactly representable here, but keep the types so nothing drifts. |
| `display_manager.c:395` | `return (int)count;` from a `uint32_t` | fine for real display counts. |
| `display_manager.c:402` | `CGGetActiveDisplayList(display_count, result, (uint32_t*)count)` writing a `uint32_t` through an `int *` | type-punned out-param. In Rust: `let mut n: u32 = 0; ...; *count = n as i32;` |
| `space_manager.c:932` | `int count = abs(new_index - cur_index);` | `i32::abs`; both operands are already `int`. |

### P20 — `CFNumberGetValue(ref, CFNumberGetType(ref), &u64)`

`display.c:243`; `space_manager.c:508,538,574,603,627,647`. The *stored* number type is queried and
used for the read, so if SkyLight ever stored an `SInt32` the upper 4 bytes of the `uint64_t` would be
left at whatever they were (here: a stale `result` from the previous iteration, `space_manager.c:493`
initialises it once outside the loop).

**Rust:** `CFNumber::to_i64()` from the `core-foundation` crate is the safe equivalent and normalises
the type; that is a *behaviour difference* in the pathological case. If you want literal fidelity,
call `CFNumberGetValue(r, CFNumberGetType(r), &mut value as *mut u64 as *mut c_void)` inside `unsafe`
and initialise `value` the same way C does (once, outside the loop).

### P21 — Bit flags on the view (`view_check_flag` / `view_set_flag` / `view_clear_flag`)

`view.h:185-220`. Used here at `space_manager.c:127,141,235-238,263,280,292,304,316,328,340,350,384-387,456,485`.

**Rust:** `bitflags!` `ViewFlags` with the same values; `contains`, `insert`, `remove`. Note
`view_check_flag` yields a `uint64_t`, used as a truthy value — `contains()` is the faithful
translation only because every use is in an `if`.

### P22 — Enum-as-error-code return

`enum space_op_error` (`space_manager.h:32-45`), returned by 8 functions, consumed by `message.c`
(38 sites, e.g. `message.c:1733-1745,1784-1795,1842-1852`) which maps each variant to a specific
`daemon_fail` string.

**Rust:** `#[derive(Debug, Clone, Copy, PartialEq)] pub enum SpaceOpError { ... }` and return
`Result<(), SpaceOpError>` **only if** you also map `SPACE_OP_ERROR_SUCCESS` to `Ok(())` consistently
everywhere. Simpler and safer for phase 2: keep it as a plain enum with an explicit `Success` variant
and the same discriminants, so `message.c`'s mapping translates one-for-one. The discriminant values
(0..10) are not on the wire, but they are dense and used in comparisons only.

### P23 — Magic numbers with no named constant

- `0x79616265` (ASCII `"yabe"`) as a workspace compat id: `space_manager.c:680,682,701,703`.
- `SLSSpaceGetType` results `0` user / `2` system / `4` fullscreen: `space.c:90,95,100`.
- Window-tag bit tests: `space.c:50,52,63` (`0x2`, `0x400000000000000`, `0x1`, `0x80000000`,
  `0x1000000000000000`, `0x300000000000000`) and window levels `0 || 3 || 8` (`space.c:49,62`).
- `SLSCopyWindowsWithOptionsAndTags` options `0x7` / `0x2` (`space.c:22`).
- Raw `CGEvent` field numbers 55/110/123/124/129/132 and values 30/23/1/4 (`space_manager.c:959-969`)
  — these *do* have explanatory comments (see §11).
- SLS notification ids in `yabai.c:322-333` (not this file, but the same family).

**Rust:** `const` items with the name spelled out (`const SPACE_COMPAT_ID_YABAI: i32 = 0x79616265;`).
Carry the existing inline comments (§11) verbatim as the constants' *names* where the comment is a
name (`kCGSEventTypeField` → `CG_EVENT_FIELD_SLS_EVENT_TYPE`) — but do not invent commentary.

### P24 — VLAs on the stack

`space_manager.c:1157-1158`:

```c
struct view *view_list[sm->view.count];
CFStringRef uuid_list[sm->view.count];
```

Zero-length when the table is empty (UB in C, harmless in practice).

**Rust:** `Vec::with_capacity(n)` — or, since both arrays are written in lockstep and then nulled out
in pairs (`space_manager.c:1178-1179`), a single `Vec<Option<(u64, CFRef<CFString>)>>`.

### P25 — `#pragma clang diagnostic` suppression blocks

`display.c:4-18` (unused parameter — the macro-generated handler ignores `context`);
`display_manager.c:428-452` and `display_manager.c:461-481` (deprecated `ProcessSerialNumber` /
`CGPostMouseEvent`); `space_manager.c:925-983` (same, for `CGPostMouseEvent`).

**Rust:** `#[allow(deprecated)]` is not needed (these are raw FFI), but keep an `#[allow(unused)]`
where a macro-shaped signature forces an unused parameter, and name it `_context`.

### P26 — Patterns explicitly **absent** from these four files

No `setjmp`, no `regex.h` (that lives in `rule.c` / `event_signal.c`), no inline asm other than the
compiler barrier of **P11**, no SIMD, no `fork`/`exec`, no signal handlers, no intrusive linked lists
(the event queue's `next` pointer lives in `event_loop.h:60`), no CAS-on-`id_ptr` liveness check (that
is `process_manager` / `window_manager`), no `dispatch_async`, no `NSNotification` observers, no
`AXObserver` registration. The only OS callback registered from this set is the display
reconfiguration handler (§P18).

---

## 3. `src/space.h`

**1. Purpose.** Prototypes for the seven free functions that answer questions about a single SkyLight
space: which display owns it, which windows are on it, and what kind of space it is. No types, no
state. It is the lowest layer above the private SkyLight API.

**2. Types / constants.** None. (`space.h` declares only functions.)

**3. Globals / statics.** None.

**4. Functions.** Declarations only; see §4 for the definitions. Note the declarations are what turn
the `inline` definitions in `space.c` into external definitions (§P27 below).

**5. OS callbacks.** None.

**6. Patterns.** **P16** (`uint32_t *space_window_list(uint64_t sid, int *count, bool include_minimized)`
at `space.h:6`). Plus **P27**:

> **P27 — `inline` definition in a `.c` with a plain prototype in the `.h`.**
> `space.c:3,83,88,93,98,103` and `display.c:101,112,179` define functions with the bare `inline`
> keyword. In C11 that is not an external definition *unless* a non-`inline` declaration is also
> visible — which the headers provide (`space.h:4,6,7,8,9,10`; `display.h:38,39,42`). So these are
> ordinary external functions that the compiler may inline. In Rust: plain `pub fn`, optionally
> `#[inline]`. Nothing to preserve.

**7. Comments to carry over.** None (the file has no comments).

---

## 4. `src/space.c`

**1. Purpose.** Seven thin wrappers over SkyLight space APIs: resolve a space's display, enumerate the
windows on a set of spaces with yabai's own filtering rules, and classify a space (user / system /
fullscreen / visible). `space_window_list_for_connection` is the only non-trivial one and is the
single place where yabai decides "is this SLS window a window I should manage".

**2. Types / enums / unions / typedefs / `#define`s.** None defined here. Referenced:
`struct window` (`window.h`), `enum window_flag::WINDOW_MINIMIZE` (`window.h:112`).

**3. Globals and statics.**

| Name | Type | Declared | Defined | Threads | Synchronisation |
|---|---|---|---|---|---|
| `g_connection` | `int` | `space.c:1` | `yabai.c:50`, set `yabai.c:143` | written once on main at startup, read on event loop (and by the main-thread mouse/AX paths elsewhere) | none |
| `g_window_manager` | `struct window_manager` | inherited (`event_loop.c:5`) | `yabai.c:30` | event loop | none |

No function-local statics.

**4. Functions.**

#### `inline uint32_t space_display_id(uint64_t sid)` — `space.c:3`
- Returns the `CGDirectDisplayID` that currently owns `sid`, `0` if SkyLight has no managed display
  for it.
- Thread: event loop (every caller chain is a message/event handler; e.g. `space_manager.c:806,897,993`,
  `display_manager.c:491`, `event_loop.c:372,667`).
- Allocates: nothing heap-side; creates and releases one `CFString` (+1 from
  `SLSCopyManagedDisplayForSpace`) and one `CFUUID` (`space.c:8,11-12`).
- Externs: `SLSCopyManagedDisplayForSpace`, `CFUUIDCreateFromString`, `CGDisplayGetDisplayIDFromUUID`,
  `CFRelease`.
- Note: `CFUUIDCreateFromString` is not null-checked (`space.c:8-9`); a malformed string would pass
  `NULL` to `CGDisplayGetDisplayIDFromUUID` (which tolerates it and returns 0) and then to `CFRelease`
  (which crashes). Preserve or guard explicitly.

#### `uint32_t *space_window_list_for_connection(uint64_t *space_list, int space_count, int cid, int *count, bool include_minimized)` — `space.c:17`
- Builds the list of window ids present on the given spaces, owned by connection `cid` (`0` = any),
  applying yabai's tag/attribute/level heuristics for windows it does not already track.
- Thread: event loop. Callers: `space_window_list` (`space.c:85`) and `window_manager.c` (several).
- Allocates: `ts_alloc_list(uint32_t, *count)` at `space.c:35`, shrunk with `ts_resize` at
  `space.c:71`. Arena memory — freed implicitly by `ts_reset()` at the end of the current event; no
  caller frees it. CF objects: `space_list_ref` (+1, `space.c:24`, released `space.c:79`),
  `window_list_ref` (+1, `space.c:25`, released `space.c:77`), `query` and `iterator` (+1,
  `space.c:31-32`, released `space.c:74-75` — **only on the non-empty path**).
- Externs: `cfarray_of_cfnumbers` (`helpers.h:344`, yabai), `SLSCopyWindowsWithOptionsAndTags`,
  `CFArrayGetCount`, `SLSWindowQueryWindows`, `SLSWindowQueryResultCopyWindows`,
  `SLSWindowIteratorAdvance`, `SLSWindowIteratorGetTags/GetAttributes/GetParentID/GetWindowID/GetLevel`,
  `CFRelease`; yabai-internal `window_manager_find_window`, `window_check_flag`.
- Behaviour to preserve exactly: the two filter branches (`space.c:44-68`) differ **only** in that the
  `include_minimized == false` branch drops tracked-but-minimized windows and omits the second
  untracked-window clause (`space.c:52-54`). The magic tag/attribute constants are **P23**.
- Error paths: `goto err` (`space.c:26`) returns `NULL` **without writing `*count`**; `goto out`
  (`space.c:29`) returns `NULL` with `*count == 0`.

#### `inline uint32_t *space_window_list(uint64_t sid, int *count, bool include_minimized)` — `space.c:83`
- One-space convenience wrapper; takes the address of its own `sid` parameter.
- Thread: event loop. Allocates: whatever the callee does.
- Externs: none directly.

#### `inline bool space_is_user(uint64_t sid)` — `space.c:88`, `space_is_fullscreen` — `space.c:93`, `space_is_system` — `space.c:98`
- `SLSSpaceGetType(g_connection, sid)` compared against `0`, `4`, `2` respectively (**P23**).
- Thread: event loop. Allocates nothing. Externs: `SLSSpaceGetType`.
- `space_is_system` has **no caller** in the daemon (only `space.h:8` and its definition) — dead code;
  translate it (cheap) or drop it, but say which.
- Note the stray double space in `inline  bool space_is_user` (`space.c:88`) — cosmetic.

#### `inline bool space_is_visible(uint64_t sid)` — `space.c:103`
- `sid == display_space_id(space_display_id(sid))`.
- Thread: event loop. Two SLS round-trips per call and it is called a lot
  (`space_manager.c:138,453,482`, `window_manager.c:2644`, `view.c`, `window.c`).
- Externs: via the two callees.

**5. OS / run-loop callbacks registered.** None.

**6. Pattern catalogue for this file.**

| Pattern | Sites |
|---|---|
| **P27** `inline` + header prototype | `space.c:3,83,88,93,98,103` |
| **P1** implicit global (`g_window_manager` with no local `extern`) | `space.c:45,58` |
| **P2** goto ladder with a skipped out-param | `space.c:26,29,76,78` |
| **P6** arena alloc + shrink | `space.c:35,71` |
| **P8** CF retain/release, borrowed vs owned | `space.c:24-25,31-32,74-79` |
| **P16** pointer + `int *count` | `space.c:17,83` |
| **P23** magic tag/level/type constants | `space.c:22,49-52,62-63,90,95,100` |

Recommended Rust shape for the hot one:

```rust
pub fn window_list_for_connection(
    window_manager: &WindowManager,
    space_list: &[u64],
    cid: i32,
    include_minimized: bool,
) -> Vec<u32>
```

— `Vec<u32>` replaces the arena list and the `int *count`, the `NULL` returns become an empty `Vec`
(every caller already treats `NULL` as "nothing to do": `space_manager.c:754-758` pre-zero their
counts). The iterator loop stays `unsafe` around the `SLSWindowIterator*` calls; the filtering logic
is pure and should be a private `fn is_manageable_untracked_window(tags: u64, attributes: u64, level: i32) -> bool`
taking the same constants.

**7. Comments to carry over verbatim.** None — `src/space.c` contains no comments.

---

## 5. `src/display.h`

**1. Purpose.** Declares the display-callback signature macro, the X-macro table of serialisable
display properties (enum + two parallel lookup tables), and the free functions that query a display's
uuid, bounds, centre and spaces.

**2. Types / enums / macros.**

| Name | Definition | Notes |
|---|---|---|
| `DISPLAY_EVENT_HANDLER(name)` | `display.h:4` — expands to `void name(uint32_t did, CGDisplayChangeSummaryFlags flags, void *context)` | matches `CGDisplayReconfigurationCallBack` |
| `display_callback` | `display.h:5` — `typedef DISPLAY_EVENT_HANDLER(display_callback);` | the function-pointer type; never actually used as a variable |
| `DISPLAY_PROPERTY_LIST` | `display.h:7-14` — 7 entries `("id", DISPLAY_PROPERTY_ID, 0x01)` … `("has-focus", DISPLAY_PROPERTY_HAS_FOCUS, 0x40)` | X-macro, **P12** |
| `enum display_property` | `display.h:16-21` — `ID=0x01, UUID=0x02, INDEX=0x04, LABEL=0x08, FRAME=0x10, SPACES=0x20, HAS_FOCUS=0x40` | used as a bitmask, not as a value |

**3. Globals and statics.**

| Name | Type | Initial value | Threads | Synchronisation |
|---|---|---|---|---|
| `display_property_val[]` | `static uint64_t[7]` (`display.h:23-28`) | `{0x01,0x02,0x04,0x08,0x10,0x20,0x40}` | read-only, event loop (`message.c:2425`) | none needed |
| `display_property_str[]` | `static char *[7]` (`display.h:30-35`) | `{"id","uuid","index","label","frame","spaces","has-focus"}` | read-only, event loop (`message.c:2425`) | none needed |

Both are `static` in a unity build, so exactly one copy exists. `display_property_str` is
`char *` (not `const char *`) purely because `parse_properties` (`message.c:625`) takes `char **`.

**4. Functions.** Declarations only (`display.h:37-44`); definitions in §6.

**5. OS callbacks.** None registered here; the macro at `display.h:4` shapes the one registered in
`display_manager.c:505`.

**6. Patterns.** **P12** (`display.h:7-35`), **P13** (consumer at `display.c:24`), **P18** (`display.h:4-5`),
**P16** (`display.h:44`).

Rust:

```rust
bitflags! { pub struct DisplayProperty: u64 {
    const ID = 0x01; const UUID = 0x02; const INDEX = 0x04; const LABEL = 0x08;
    const FRAME = 0x10; const SPACES = 0x20; const HAS_FOCUS = 0x40;
} }
pub const DISPLAY_PROPERTY_BY_NAME: [(&str, DisplayProperty); 7] = [ ("id", DisplayProperty::ID), ... ];
```

Keep the array order — `parse_properties` iterates it linearly and the first match wins, and the CLI
documentation lists them in this order.

**7. Comments to carry over.** None.

---

## 6. `src/display.c`

**1. Purpose.** The display-reconfiguration callback (which does nothing but translate CG flags into
yabai events), the JSON serialiser for one display, and the primitives that map between display ids,
CF uuids, bounds (corrected for menu bar, notch, dock and external bar) and the spaces a display owns.

**2. Types.** None defined.

**3. Globals and statics.**

| Name | Type | Declared | Threads | Synchronisation |
|---|---|---|---|---|
| `g_event_loop` | `struct event_loop` | `display.c:1` (def `yabai.c:34`) | **written by the main thread** inside `display_handler`, drained by the event-loop thread | lock-free queue: `memory_pool_push` CAS (`memory_pool.h:29-45`), `__sync_bool_compare_and_swap` on `tail->next` and `sem_post` (`event_loop.c:1700-1712`) |
| `g_connection` | `int` | `display.c:2` | main (write once), event loop (read) | none |
| `g_display_manager` | `struct display_manager` | inherited (`event_loop.c:3`) | event loop; read at `display.c:58,95,129,131,132,136` | none |

No function-local statics.

**4. Functions.**

#### `static DISPLAY_EVENT_HANDLER(display_handler)` — `display.c:6`
- Maps `kCGDisplayAddFlag` → `DISPLAY_ADDED`, `kCGDisplayRemoveFlag` → `DISPLAY_REMOVED`,
  `kCGDisplayMovedFlag` → `DISPLAY_MOVED`, `kCGDisplayDesktopShapeChangedFlag` → `DISPLAY_RESIZED`
  (**first match wins**, `else if` chain, so a single reconfiguration reporting several flags produces
  exactly one event, in that priority order).
- Thread: **main run loop.** Registered from `display_manager_begin` (`display_manager.c:505`), itself
  called from `main` (`yabai.c:303`) before `[NSApp run]` (`yabai.c:350`); CoreGraphics delivers
  reconfiguration callbacks to the registering thread's run loop, and the main run loop is the only one
  running. This is the only main-thread entry point in the assigned files.
- Allocates: one `struct event` from the event loop's 512 KB pool (`event_loop.c:1699`,
  `memory_pool.h:29`) — a ring that wraps, never freed.
- Externs: `event_loop_post` (yabai).
- Context pointer: `NULL` is passed at registration (`display_manager.c:505`), so the `context`
  parameter is always `NULL` and unused (hence the `-Wunused-parameter` pragma, `display.c:4-5,18`).
  The `did` travels as `(void *)(intptr_t) did` in the *event*, not in the callback context (**P17**).

#### `void display_serialize(FILE *rsp, uint32_t did, uint64_t flags)` — `display.c:20`
- Writes the JSON object for one display, one property per `flags` bit.
- Thread: event loop (called from `display_manager_query_displays`, `display_manager.c:15`, which is
  reached only from `message.c` query handling).
- Allocates: `ts_cfstring_copy` (`display.c:40`, arena) and `display_space_list` (`display.c:75`,
  arena). CF: `display_uuid` result released at `display.c:41`.
- Externs: `fprintf`, `CGDisplayBounds`; yabai: `display_uuid`, `ts_cfstring_copy`,
  `display_manager_display_id_arrangement`, `display_manager_get_label_for_display`,
  `display_space_list`, `space_manager_mission_control_index`, `json_bool`.
- Behaviour: `flags == 0` → all properties (**P13**, `display.c:24`). `did_output` tracks whether a
  leading `,\n` is needed; `HAS_FOCUS` (`display.c:92-96`) deliberately does **not** set `did_output`
  because it is last. `spaces` is emitted as *mission-control indices*, computed as
  `first_mci + i` from the first space's index (`display.c:79-85`) — i.e. it assumes a display's
  spaces are contiguous in the global ordering. Preserve that assumption; do not call
  `space_manager_mission_control_index` per space.
- `TIME_FUNCTION` (`display.c:22`) is a no-op unless `PROFILE >= 2` (`timer.h:151`).

#### `inline CFStringRef display_uuid(uint32_t did)` — `display.c:101`
- `CGDisplayCreateUUIDFromDisplayID` → `CFUUIDCreateString`. Returns **+1** (caller releases) or `NULL`.
- Thread: event loop (and main, transitively, only during `display_manager_begin`).
- Externs: `CGDisplayCreateUUIDFromDisplayID` (private, `extern.h:40`), `CFUUIDCreateString`, `CFRelease`.

#### `inline uint32_t display_id(CFStringRef uuid)` — `display.c:112`
- Inverse of the above; `0` when the string is not a uuid.
- Externs: `CFUUIDCreateFromString`, `CGDisplayGetDisplayIDFromUUID`, `CFRelease`.

#### `CGRect display_bounds_constrained(uint32_t did, bool ignore_external_bar)` — `display.c:123`
- Display bounds minus: yabai's external-bar padding (when `mode` is `EXTERNAL_BAR_MAIN` for the main
  display or `EXTERNAL_BAR_ALL`), the menu bar (or, when auto-hidden, the notch inset), and the dock
  on the display that owns it.
- Thread: event loop (`view.c:971`, `window_manager.c:2141,2397,2420`, `event_loop.c:1257,1440`).
- Allocates: nothing.
- Externs: `CGDisplayBounds`; yabai: `display_manager_main_display_id`,
  `display_manager_menu_bar_hidden`, `workspace_display_notch_height` (ObjC, `workspace.m:125`),
  `display_manager_menu_bar_rect`, `display_manager_dock_hidden`,
  `display_manager_dock_display_id`, `display_manager_dock_rect`, `display_manager_dock_orientation`.
- Ordering of side effects matters: the notch branch subtracts only the *excess* over the external-bar
  padding (`display.c:142-145`), and `DOCK_ORIENTATION_LEFT` shifts origin.x *and* shrinks width while
  `RIGHT`/`BOTTOM` only shrink (`display.c:156-166`). A `switch` with no `default` — an unknown
  orientation changes nothing.

#### `CGPoint display_center(uint32_t did)` — `display.c:173`
- Geometric centre of the raw (unconstrained) bounds. Externs: `CGDisplayBounds`.

#### `inline uint64_t display_space_id(uint32_t did)` — `display.c:179`
- Current space of a display; `0` if the display has no uuid.
- Externs: `SLSManagedDisplayGetCurrentSpace`; yabai `display_uuid`.

#### `int display_space_count(uint32_t did)` — `display.c:190`
- Number of spaces on a display, `0` if not found. **No caller in the daemon** — dead code (only
  `display.h:43` and this definition).
- Externs: `SLSCopyManagedDisplaySpaces`, `CFArrayGetCount`, `CFArrayGetValueAtIndex`,
  `CFDictionaryGetValue`, `CFEqual`, `CFRelease`, `CFSTR`.

#### `uint64_t *display_space_list(uint32_t did, int *count)` — `display.c:218`
- The display's space ids, in SkyLight order, into arena memory; `NULL` when the display is not in
  `SLSCopyManagedDisplaySpaces`.
- Thread: event loop. Heavily used: `display.c:75`, `space_manager.c:52,79,710,729,1109,1153,1221`,
  `window_manager.c:2663`.
- Allocates: `ts_alloc_list(uint64_t, spaces_count)` (`display.c:237`) — arena; caller does not free.
- Externs: as `display_space_count`, plus `CFNumberGetValue`/`CFNumberGetType` (**P20**).
- **`*count` is only assigned inside the match (`display.c:238`)**; on the no-match path it is left
  untouched and `NULL` is returned (**P2**). The loop does not `break` after a match, but `CFEqual`
  can only match once.

**5. OS / run-loop callbacks.**

| Callback | Registration | Thread it fires on | Context pointer | Lifetime |
|---|---|---|---|---|
| `display_handler` (`display.c:6`) | `CGDisplayRegisterReconfigurationCallback(display_handler, NULL)` — `display_manager.c:505`, from `yabai.c:303` | main run loop | `NULL` | process lifetime; never unregistered |

**6. Pattern catalogue for this file.**

| Pattern | Sites |
|---|---|
| **P1** implicit `g_display_manager` | `display.c:58,95,129,131,132,136` |
| **P2** goto ladders | `display.c:195,198,212,214,223,226,248,250` |
| **P6** arena | `display.c:40,75,237` |
| **P8** CF pairs, borrowed vs owned | `display.c:38-41,103-107,114-118,181-185,197-213,225-249` |
| **P13** `flags == 0` → all | `display.c:24` |
| **P14/P15** `fprintf` into `rsp`, terminator quirks | `display.c:27-98` |
| **P17** `(void *)(intptr_t) did` | `display.c:9,11,13,15` |
| **P18** OS callback | `display.c:6` |
| **P19** int/CGFloat mixing | `display.c:126,132-136,141-144` |
| **P20** `CFNumberGetValue` with queried type | `display.c:243` |
| **P25** diagnostic pragma | `display.c:4-5,18` |

Rust sketch for the callback:

```rust
unsafe extern "C" fn display_reconfiguration_callback(did: u32, flags: CGDisplayChangeSummaryFlags, _context: *mut c_void) {
    let event = if flags & K_CG_DISPLAY_ADD_FLAG != 0 { Some(Event::DisplayAdded(did)) }
    else if flags & K_CG_DISPLAY_REMOVE_FLAG != 0 { Some(Event::DisplayRemoved(did)) }
    else if flags & K_CG_DISPLAY_MOVED_FLAG != 0 { Some(Event::DisplayMoved(did)) }
    else if flags & K_CG_DISPLAY_DESKTOP_SHAPE_CHANGED_FLAG != 0 { Some(Event::DisplayResized(did)) }
    else { None };
    if let Some(event) = event { EVENT_LOOP.post(event); }
}
```

`extern "C"` functions must not unwind: keep the body allocation-free and panic-free (posting to a
channel is fine), or wrap in `catch_unwind`.

**7. Comments to carry over verbatim.** None — `src/display.c` contains no comments.

---

## 7. `src/display_manager.h`

**1. Purpose.** Declares the dock-orientation constants, the display arrangement/external-bar enums
with their string tables, the `display_label` and `display_manager` state structs, and the whole
display-manager API.

**2. Types, enums and constants.**

| Name | Definition | Notes |
|---|---|---|
| `DOCK_ORIENTATION_BOTTOM/LEFT/RIGHT` | `2` / `3` / `4` (`display_manager.h:4-6`) | matches `CoreDockGetOrientationAndPinning`; `1` (top) is never produced by macOS |
| `enum display_arrangement_order` | `DEFAULT=0, X=1, Y=2` (`display_manager.h:8-13`) | |
| `display_arrangement_order_str[]` | `static const char *[3]` = `{"default","horizontal","vertical"}` (`display_manager.h:15-20`) | note `X` ↔ `"horizontal"`, `Y` ↔ `"vertical"` |
| `enum external_bar_mode` | `OFF=0, MAIN=1, ALL=2` (`display_manager.h:22-27`) | |
| `external_bar_mode_str[]` | `static const char *[3]` = `{"off","main","all"}` (`display_manager.h:29-34`) | |
| `struct display_label` | `uint32_t did; char *label;` (`display_manager.h:36-40`) | **`label` is owned**: `malloc`'d by `parse_label` (`message.c:596`) and `free`d by `display_manager_remove_label_for_display` (`display_manager.c:52`) / `display_manager_set_label_for_display` (`display_manager.c:68`). Nothing else frees it; labels for displays that vanish are dropped at `event_loop.c:1096`. |
| `struct display_manager` | `display_manager.h:42-54` | see below |

`struct display_manager` fields:

| Field | C type | Written by | Read by |
|---|---|---|---|
| `current_display_id` | `uint32_t` | `display_manager_begin` (main, `display_manager.c:499`), `event_loop.c` DISPLAY_CHANGED path | `display.c:95`, `event_signal.c:281`, `message.c` |
| `last_display_id` | `uint32_t` | same | `event_signal.c:282` |
| `top_padding`, `bottom_padding` | `int` | `display_manager_begin` (`display_manager.c:503-504`), `message.c` external-bar command | `display.c:132,136`, `message.c:1692` |
| `order` | `enum display_arrangement_order` | `display_manager_begin` (`display_manager.c:501`), `message.c` | `display_manager.c:178,180,208,210`, `message.c:1208` |
| `mode` | `enum external_bar_mode` | `display_manager_begin` (`display_manager.c:502`), `message.c` | `display.c:129,131`, `message.c:1692` |
| `labels` | `struct display_label *` (stretchy buffer) | `display_manager.c:53,69,74` | `display_manager.c:25,37,49,65`, `display.c:58` |

Thread story for all of them: written once on the **main thread** in `display_manager_begin`, then
read/written exclusively on the **event-loop thread**. No synchronisation of any kind. `labels` is
never initialised by `display_manager_begin` — it relies on the global being zero.

**3. Globals and statics.** The two `static const char *[]` string tables above. Read-only, event loop.

**4. Functions.** Declarations only (`display_manager.h:56-91`).

**5. OS callbacks.** None.

**6. Patterns.** **P3** (`labels`), **P12**-adjacent (enum + parallel string table, but hand-written not
X-macro — translate as `impl Display`/`FromStr` or a `const NAMES: [&str; 3]` indexed by the enum),
**P22** (`enum space_op_error` returned by `display_manager_focus_space`, `display_manager.h:90`,
defined in `space_manager.h` — a header-ordering dependency: `manifest.m:75` includes
`display_manager.h` *before* `manifest.m:76` includes `space_manager.h`, so `display_manager.h:90`
names an enum declared **later**. That is a forward reference to an enum type, which clang accepts
silently as an extension (verified with `-std=c11 -Wall -Wextra`) even though ISO C does not allow it.
In Rust, put `SpaceOpError` in a `space_manager`
module and `use` it — no ordering problem, but note the two modules are mutually dependent:
`display_manager` calls `space_manager_active_space`/`space_display_id`, and `space_manager` calls
`display_manager_focus_display`. Rust allows mutual module references inside one crate; keep them in
one crate, and in phase 3 break the cycle by moving `SpaceOpError` into a shared `ops` module.)

**7. Comments to carry over.** None.

---

## 8. `src/display_manager.c`

**1. Purpose.** The display-level policy layer: user-assigned display labels, arrangement ordering
(default SkyLight order or re-sorted by centre coordinate), the "which display is active / under the
cursor / owns the dock" queries, menu-bar and dock geometry, and the operations that move focus to a
display or a space on it. Also registers the display-reconfiguration callback.

**2. Types.** None defined here (all in the header).

**3. Globals and statics.**

| Name | Type | Declared | Threads | Synchronisation |
|---|---|---|---|---|
| `g_display_manager` | `struct display_manager` | `display_manager.c:1` | main (startup, `display_manager.c:499-504`), event loop thereafter | none |
| `g_window_manager` | `struct window_manager` | `display_manager.c:2` | event loop; read at `display_manager.c:409,465,467,468` | none |
| `g_connection` | `int` | `display_manager.c:3` | main (write once), event loop (read) | none |

No function-local statics.

**4. Functions.** (38, in source order.)

#### `bool display_manager_query_displays(FILE *rsp, uint64_t flags)` — `display_manager.c:5`
- Serialises every active display as a JSON array. Returns `false` if the display list is `NULL`.
- Thread: event loop (`message.c` query handler).
- Allocates: arena list via `display_manager_active_display_list`.
- Externs: `fprintf`; yabai `display_serialize`.
- **P15 quirk:** zero displays → prints `[` and `\n`, never `]`.

#### `struct display_label *display_manager_get_label_for_display(struct display_manager *dm, uint32_t did)` — `display_manager.c:23`
- Linear scan of `dm->labels` by did. Returns a **borrowed pointer into the stretchy buffer** —
  invalidated by any later `buf_push`/`buf_del`. Callers use it immediately (`display.c:58-59`,
  `message.c`).
- Thread: event loop. Allocates nothing. Externs: none.
- Rust: `fn label_for_display(&self, did: u32) -> Option<&DisplayLabel>` — the borrow checker enforces
  what C only hopes for.

#### `struct display_label *display_manager_get_display_for_label(struct display_manager *dm, char *label)` — `display_manager.c:35`
- Same scan, keyed by string via `string_equals` (`helpers.h:254`, null-safe `strcmp`).
- Rust: `&str` comparison. NUL handling: labels are always NUL-terminated `malloc`'d C strings; the
  incoming token is also NUL-terminated (`message.c:596-600`). `String`/`&str` is a faithful mapping
  as long as non-UTF-8 labels are handled — `parse_label` copies raw bytes from the socket, so a
  non-UTF-8 label is possible. Use `Vec<u8>`/`CString` or `String::from_utf8_lossy` and say which.

#### `bool display_manager_remove_label_for_display(struct display_manager *dm, uint32_t did)` — `display_manager.c:47`
- Frees the label string and `buf_del`s (swap-remove) the entry; `true` if one was removed.
- Allocates/frees: `free(display_label->label)` (`display_manager.c:52`).
- Rust: `if let Some(i) = ...position(...) { self.labels.swap_remove(i); true } else { false }`.

#### `void display_manager_set_label_for_display(struct display_manager *dm, uint32_t did, char *label)` — `display_manager.c:61`
- **Takes ownership of `label`.** Removes any existing label for that did, then removes any *other*
  display that already carries the same label text (`display_manager.c:65-72`), then pushes.
- Rust: `pub fn set_label(&mut self, did: u32, label: String)` — ownership becomes explicit and the
  double-free risk disappears.

#### `CFStringRef display_manager_main_display_uuid(void)` — `display_manager.c:80`
- +1 uuid of `CGMainDisplayID()`. **No caller** — dead code.

#### `uint32_t display_manager_main_display_id(void)` — `display_manager.c:86`
- `CGMainDisplayID()`. Called from `display.c:130`.

#### `CFStringRef display_manager_active_display_uuid(void)` — `display_manager.c:91`
- `SLSCopyActiveMenuBarDisplayIdentifier(g_connection)`, +1. Callers: `display_manager.c:98,361`.

#### `uint32_t display_manager_active_display_id(void)` — `display_manager.c:96`
- `assert(uuid)` (**no-op in release**, `display_manager.c:99`) then `display_id` + release.
- Thread: **main** during `display_manager_begin` (`display_manager.c:499`) **and** event loop
  (`space_manager.c:659`, `event_loop.c:1033`).
- Rust: `let uuid = active_display_uuid(); debug_assert!(uuid.is_some());` then `uuid.map(display_id).unwrap_or(0)`
  — note that in release C a `NULL` uuid would be passed to `CFUUIDCreateFromString` (returns `NULL`)
  → `CGDisplayGetDisplayIDFromUUID(NULL)` → `0`, then `CFRelease(NULL)` **crashes**. So the C release
  path crashes where the Rust `Option` path would return 0; that is a deliberate difference worth
  flagging, not silently adopting.

#### `CFStringRef display_manager_dock_display_uuid(void)` — `display_manager.c:107`
- `SLSCopyBestManagedDisplayForRect(g_connection, display_manager_dock_rect())`, +1.

#### `uint32_t display_manager_dock_display_id(void)` — `display_manager.c:113`
- Null-safe (`display_manager.c:116`). Caller: `display.c:153`.

#### `CFStringRef display_manager_point_display_uuid(CGPoint point)` — `display_manager.c:124` / `uint32_t display_manager_point_display_id(CGPoint point)` — `display_manager.c:129`
- `SLSCopyBestManagedDisplayForPoint`, +1, null-safe. Callers: `display_manager.c:236`,
  `event_loop.c:1179,1255,1285,1437`.

#### `static CFComparisonResult display_manager_coordinate_comparator(CFTypeRef a, CFTypeRef b, void *context)` — `display_manager.c:140`
- Orders two display-uuid strings by display centre: primary axis from `context`
  (`DISPLAY_ARRANGEMENT_ORDER_Y` → compare `y` first, else `x` first), secondary axis as tie-break,
  else equal.
- Thread: event loop (synchronously inside `CFArraySortValues`).
- Allocates: nothing directly; `display_id` and `display_center` each do CF create/release
  (`display_manager.c:144-148`) — i.e. **two CF allocations and two SLS round-trips per comparison**,
  O(n log n) times.
- Externs: `display_id`, `display_center` (yabai); the CF comparator contract.
- **P19:** coordinates are narrowed to `float` (`display_manager.c:150-151,156-157`).
- Rust: precompute `(uuid, CGPoint)` pairs once and `sort_by` — same total order, fewer round-trips.
  Keep the `as f32` narrowing so ties break identically.

#### `int display_manager_display_id_arrangement(uint32_t did)` — `display_manager.c:165`
- 1-based index of `did` in `SLSCopyManagedDisplays` order (re-sorted when
  `g_display_manager.order != DEFAULT`); `0` if absent.
- Thread: event loop (`display.c:51`, `event_signal.c:282,307`, `rule.c:46`, `message.c`).
- Allocates: CF array (+1), plus a mutable copy when sorting (`display_manager.c:179-181`, the original
  is released and the variable reassigned).
- Externs: `SLSCopyManagedDisplays`, `CFArrayGetCount`, `CFArrayCreateMutableCopy`, `CFArraySortValues`,
  `CFRangeMake`, `CFArrayGetValueAtIndex`, `CFEqual`, `CFRelease`; yabai `display_uuid`.
- Ladder: `empty:`/`err:`/`out:` (`display_manager.c:191-196`); the `!count` path still releases.

#### `CFStringRef display_manager_arrangement_display_uuid(int arrangement)` — `display_manager.c:199`
- Inverse of the above. Returns **+1** (`CFRetain` at `display_manager.c:214`) or `NULL`.
- **`SLSCopyManagedDisplays` is not null-checked** (`display_manager.c:202-204`) — `CFArrayGetCount(NULL)`
  crashes. The sibling function does check. Preserve the asymmetry or fix it knowingly.
- `in_range_ie(index, 0, count)` = `index >= 0 && index < count` (`macros.h:13`).

#### `uint32_t display_manager_arrangement_display_id(int arrangement)` — `display_manager.c:221`
- +1 uuid → did → release. Callers: `display_manager.c:244,252,257,263`, `message.c`.

#### `uint32_t display_manager_cursor_display_id(void)` — `display_manager.c:232`
- `SLSGetCurrentCursorLocation` then point→display. Callers: `space_manager.c:553,939`, `message.c:763`.
- Note `CGPoint cursor;` is uninitialised and relies on the SLS call filling it
  (`display_manager.c:234-235`). Rust: `let mut cursor = CGPoint::ZERO;`.

#### `uint32_t display_manager_prev_display_id(uint32_t did)` — `display_manager.c:239` / `next` — `:247` / `first` — `:255` / `last` — `:260`
- Arrangement arithmetic; `0` at the ends. `next` compares against
  `display_manager_active_display_count()` (a `CGGetActiveDisplayList` round-trip), `last` uses it as
  the index.
- Callers: `message.c` display selectors.

#### `uint32_t display_manager_find_closest_display_in_direction(uint32_t source_did, int direction)` — `display_manager.c:266`
- Nearest display in `DIR_NORTH/EAST/SOUTH/WEST` (`macros.h:26-29`: 360/90/180/270) by
  `area_is_in_direction` + `area_distance_in_direction` (`view.c:541,563`), `0` if none.
- Thread: event loop (`message.c:682,693,704,715`).
- Allocates: arena display list.
- **P19:** `int best_distance = INT_MAX` and `int distance` from `float` math — truncation.
- Note the parameter is named `source_did` in the definition but `acting_did` in the header
  (`display_manager.h:77`); irrelevant to behaviour.

#### `bool display_manager_menu_bar_hidden(void)` — `display_manager.c:297`
- `SLSGetMenuBarAutohideEnabled` into an `int`, returned as `bool` (non-zero → true).

#### `CGRect display_manager_menu_bar_rect(uint32_t did)` — `display_manager.c:304`
- x86_64: `SLSGetRevealedMenuBarBounds(&bounds, g_connection, display_space_id(did))`.
  arm64: `SLSGetDisplayMenubarHeight(did, &height)` + `CGDisplayBounds(did)` with the height replaced.
  Both then `+= 1` on the height.
- **Two comments here must survive verbatim** (see §11).
- Rust: `#[cfg(target_arch = "x86_64")]` / `#[cfg(target_arch = "aarch64")]` bodies. `bounds` starts
  `{0}` — `CGRect::ZERO`.

#### `bool display_manager_dock_hidden(void)` — `display_manager.c:334` / `int display_manager_dock_orientation(void)` — `:339` / `CGRect display_manager_dock_rect(void)` — `:347`
- `CoreDockGetAutoHideEnabled` (returns `Boolean`), `CoreDockGetOrientationAndPinning` (two `int`
  out-params, pinning discarded), `SLSGetDockRectWithReason` (rect + discarded reason).
- All three initialise their out-params before the call; keep that.

#### `bool display_manager_active_display_is_animating(void)` — `display_manager.c:355`
- Only meaningful on Big Sur / Monterey / Ventura; returns `false` otherwise with the comment at
  `display_manager.c:370`. **No caller** — dead code, but keep the comment if you translate it.
- `assert(uuid)` at `display_manager.c:362` — same NDEBUG caveat as above.

#### `bool display_manager_display_is_animating(uint32_t did)` — `display_manager.c:373`
- Same, per display, null-safe. Called from `display_manager.c:488` and
  `space_manager.c:747,748,812,862,900,906,997,1022,1025,1049,1068`.
- Comment at `display_manager.c:388` must survive.
- `workspace_is_macos_*` are zero-cost reads of file-static bools (`workspace.h:12-19`) set in
  `workspace.m:4`. Rust: a `OnceLock<MacosVersion>` or `static` set at startup.

#### `int display_manager_active_display_count(void)` — `display_manager.c:391`
- `CGGetActiveDisplayList(0, NULL, &count)` → count only.

#### `uint32_t *display_manager_active_display_list(int *count)` — `display_manager.c:398`
- Arena array of active display ids. **Always non-NULL** in practice (arena alloc never returns NULL;
  it `exit()`s instead), yet every caller null-checks it (`display_manager.c:11`,
  `space_manager.c:74,1126,1217`, `window_manager.c:2654`). Preserve the checks or drop them
  consistently.
- **P19:** `(uint32_t*)count` type-pun (`display_manager.c:402`).
- Two `CGGetActiveDisplayList` round-trips per call (count, then fill).

#### `static AXUIElementRef display_manager_find_element_at_point(CGPoint point)` — `display_manager.c:406`
- Hit-tests via the system-wide AX element, returns either the element itself (if its role is
  `kAXWindowRole`) or its `kAXWindowAttribute` — both **+1**, or `NULL`.
- Thread: event loop (only caller is `display_manager_focus_display_with_window_at_point`, itself
  called from `event_loop.c:1443`).
- Externs: `AXUIElementCopyElementAtPosition`, `AXUIElementCopyAttributeValue`, `CFEqual`, `CFRelease`,
  `kAXRoleAttribute`, `kAXWindowRole`, `kAXWindowAttribute`; `g_window_manager.system_element`.
- **Leak at `display_manager.c:414`** (returns without releasing `element_ref` when `role` is `NULL`)
  — **P8**.

#### `uint32_t display_manager_focus_display_with_window_at_point(CGPoint point)` — `display_manager.c:430`
- Focuses (and raises) the window under `point`, returns its window id or `0`.
- Thread: event loop (`event_loop.c:1443`).
- Externs: `SLSGetWindowOwner`, `SLSGetConnectionPSN`; yabai `ax_window_id` (`helpers.h:499`, wraps
  `_AXUIElementGetWindow`), `window_manager_focus_window_with_raise`.
- `int element_connection; ProcessSerialNumber element_psn;` are uninitialised and filled by the two
  SLS calls (`display_manager.c:432-433,441-442`); neither return value is checked.
- Goto ladder `err_ref:`/`out:` (`display_manager.c:436,439,447,449`) — note the success path releases
  and returns *before* the labels.
- `ProcessSerialNumber` is a deprecated Carbon struct (`{ UInt32 highLongOfPSN; UInt32 lowLongOfPSN; }`)
  passed by pointer to SLS — must be `#[repr(C)] struct ProcessSerialNumber { high: u32, low: u32 }`.

#### `void display_manager_set_active_display_id(uint32_t did)` — `display_manager.c:454`
- `SLSSetActiveMenuBarDisplayIdentifier(g_connection, uuid, uuid)` — the same uuid passed twice.
- **`display_uuid` result is not null-checked** before being passed and then `CFRelease`d
  (`display_manager.c:456-458`) — a removed display crashes here.
- Callers: `display_manager.c:469,473`, `space_manager.c:974`, `event_loop.c:1444`.

#### `void display_manager_focus_display(uint32_t did, uint64_t sid)` — `display_manager.c:463`
- If the space has a window at rank 1, focus+raise it and centre the mouse on it; otherwise warp the
  cursor to the display centre and, if the active space still is not the display's, synthesise a
  click (`CGPostMouseEvent` down/up, `display_manager.c:476-477`).
- Thread: event loop (`space_manager.c:934,1002,1030`, `message.c:1724`).
- Externs: `CGWarpMouseCursorPosition`, `CGPostMouseEvent` (deprecated, hence the pragma);
  yabai `window_manager_find_window_on_space_by_rank_filtering_window`,
  `window_manager_focus_window_with_raise`, `window_manager_center_mouse`,
  `space_manager_active_space`, `display_space_id`, `display_center`.
- Ordering of side effects is load-bearing: warp **then** set active display **then** conditionally
  click.

#### `enum space_op_error display_manager_focus_space(uint32_t did, uint64_t sid)` — `display_manager.c:483`
- Guards in order: mission control active → `IN_MISSION_CONTROL`; display animating →
  `DISPLAY_IS_ANIMATING`; `space_display_id(sid) != did` → `SAME_DISPLAY` (a misleading name — it means
  "the space is not on that display"); else `scripting_addition_focus_space(sid)` →
  `SUCCESS`/`SCRIPTING_ADDITION`.
- Thread: event loop (`message.c`).
- Externs: yabai `mission_control_is_active` (`mission_control.c:108`), `display_manager_display_is_animating`,
  `space_display_id`, `scripting_addition_focus_space` (`sa.m`, mach IPC to the injected payload).

#### `bool display_manager_begin(struct display_manager *dm)` — `display_manager.c:497`
- Initialises `current_display_id`, `last_display_id`, `order`, `mode`, `top_padding`,
  `bottom_padding` and registers `display_handler`. Returns whether registration succeeded.
- Thread: **main**, `yabai.c:303`.
- **Does not initialise `dm->labels`** — relies on the zero-initialised global (**P3**).
- Externs: `CGDisplayRegisterReconfigurationCallback`.

**5. OS / run-loop callbacks registered here.**

| Callback | Registration | Thread | Context | Lifetime |
|---|---|---|---|---|
| `display_handler` (`display.c:6`) | `display_manager.c:505` | main run loop | `NULL` | never unregistered |
| `display_manager_coordinate_comparator` (`display_manager.c:140`) | passed to `CFArraySortValues` at `display_manager.c:180` and `display_manager.c:210` | **synchronous**, caller's thread (event loop) | `(void *)(uintptr_t) g_display_manager.order` — an enum value, not a pointer; valid only for the duration of the sort | per-call |

**6. Pattern catalogue for this file.**

| Pattern | Sites |
|---|---|
| **P1** globals without parameters | `display_manager.c:178,180,208,210,409,465-468` |
| **P2** goto ladders | `display_manager.c:170,173,176,191-196,436,439,447-450` |
| **P3** stretchy buffer + swap-remove | `display_manager.c:25,37,49,53,65,69,74` |
| **P6** arena | `display_manager.c:401` |
| **P8** CF pairs / explicit `CFRetain` / leak | `display_manager.c:83,93,98-104,109-120,126-136,169-196,202-218,214,223-228,361-366,379-384,414,456-458` |
| **P14/P15** response writing | `display_manager.c:13-18` |
| **P17** enum through `void *` | `display_manager.c:142,180,210` |
| **P18** CF comparator + CG callback | `display_manager.c:140,180,210,505` |
| **P19** float narrowing, int truncation, type-punned out-param | `display_manager.c:150-160,273,286,395,402` |
| **P22** `space_op_error` | `display_manager.c:483-495` |
| **P23** dock orientation constants | `display_manager.c:155-166` (values from `display_manager.h:4-6`) |
| **P25** diagnostic pragmas | `display_manager.c:428-429,452,461-462,481` |
| arch `#ifdef` | `display_manager.c:308-323` |
| `assert` that vanishes under NDEBUG | `display_manager.c:99,362` |

**7. Comments to carry over verbatim.**

- `display_manager.c:312-316`
  ```
  //
  // NOTE(asmvik): SLSGetRevealedMenuBarBounds is broken on Apple Silicon,
  // but we expected it to return the full display bounds along with the menubar
  // height. Combine this information ourselves using two separate functions..
  //
  ```
- `display_manager.c:325-328`
  ```
  //
  // NOTE(asmvik): Height needs to be offset by 1 because that is the actual
  // position on the screen that windows can be positioned at..
  //
  ```
- `display_manager.c:370` — trailing: `// This does not return a correct result on modern macOS versions.`
- `display_manager.c:388` — the same trailing comment.

No other comments exist in this file.

---

## 9. `src/space_manager.h`

**1. Purpose.** Declares the space-label record, the `space_manager` state struct (the view hash table
plus every global default that new views inherit), the `space_op_error` result enum, and the ~60-entry
space-manager API.

**2. Types and enums.**

`struct space_label` (`space_manager.h:4-8`): `uint64_t sid; char *label;`.
`label` is **owned** — `malloc`'d in `parse_label` (`message.c:596`), `free`d at
`space_manager.c:174` and `space_manager.c:190`. `sid` is rewritten in place when spaces are swapped
(`space_manager.c:786-787`) or re-keyed on display add (`space_manager.c:1185`).

`struct space_manager` (`space_manager.h:10-30`):

| Field | C type | Initialised | Written by | Read by |
|---|---|---|---|---|
| `view` | `struct table` | `space_manager.c:1213` (`table_init(..., 23, hash_view, compare_view)`) | `space_manager.c:108,763-774,1181,1190,1226` | `space_manager.c:100,105,262,…`, `window_manager.c`, `message.c` |
| `current_space_id` | `uint64_t` | `space_manager.c:1230`, also `space_manager.c:1198` | `event_loop.c:997`, `space_manager.c:1198,1230` | `event_loop.c:1005,1057`, `event_signal.c:229` |
| `last_space_id` | `uint64_t` | `space_manager.c:1231,1199` | same | `event_signal.c:230` |
| `did_begin` | `bool` | `space_manager.c:1232` | only there | `space_manager.c:99` |
| `layout` | `enum view_type` | `VIEW_FLOAT` (`space_manager.c:1204`) | `space_manager.c:261`, `message.c` | `view.c:1001` |
| `top/bottom/left/right_padding` | `int` | **never initialised in `begin`** — zeroed global | `space_manager.c:290,302,314,326` | `view.c:1002-1005` |
| `window_gap` | `int` | **never initialised in `begin`** | `space_manager.c:278` | `view.c:1006` |
| `split_ratio` | `float` | `0.5f` (`space_manager.c:1205`) | `message.c` | `view.c` |
| `split_type` | `enum window_node_split` | `SPLIT_AUTO` (`space_manager.c:1207`) | `space_manager.c:338` | `view.c:1008` |
| `window_placement` | `enum window_node_child` | `CHILD_SECOND` (`space_manager.c:1208`) | `message.c` | `view.c:133` |
| `window_insertion_point` | `enum window_insertion_point` | `INSERT_FOCUSED` (`space_manager.c:1209`) | `message.c` | `view.c`, `window_manager.c` |
| `window_zoom_persist` | `bool` | `true` (`space_manager.c:1210`) | `message.c` | `window_manager.c` |
| `auto_balance` | `uint32_t` | `SPLIT_NONE` (`space_manager.c:1206`) | `space_manager.c:348` | `view.c:1007` |
| `labels` | `struct space_label *` | `NULL` (`space_manager.c:1211`) | `space_manager.c:175,191,196` | `space_manager.c:147,159,171,187,784,1184` |
| `skip_window_focus_animation` | `bool` | `false` (`space_manager.c:1212`) | `message.c` | `window_manager.c` |

Thread story: identical to `display_manager` — the whole struct is written once on the **main thread**
in `space_manager_begin` (`yabai.c:337`) while the event-loop thread is already running, then owned by
the event-loop thread. No synchronisation.

`enum space_op_error` (`space_manager.h:32-45`): `SUCCESS=0, MISSING_SRC=1, MISSING_DST=2,
INVALID_SRC=3, INVALID_DST=4, INVALID_TYPE=5, SAME_SPACE=6, SAME_DISPLAY=7, DISPLAY_IS_ANIMATING=8,
IN_MISSION_CONTROL=9, SCRIPTING_ADDITION=10`. Consumed only by `message.c` (**P22**).

**3. Globals and statics.** None.

**4. Functions.** Declarations only (`space_manager.h:47-109`).

> **Dead declaration:** `void space_manager_mark_view_dirty(struct space_manager *sm, uint64_t sid);`
> (`space_manager.h:55`) has **no definition anywhere** and no caller. Do not translate it.

**5. OS callbacks.** None.

**6. Patterns.** **P4** (`struct table view`), **P3** (`labels`), **P22**, **P16**
(`space_manager.h:91` `uint32_t *window_list, int window_count`).

**7. Comments to carry over.** None.

---

## 10. `src/space_manager.c`

**1. Purpose.** The space-level policy layer and the biggest file in this set: owns the `sid → struct view`
hash table, applies per-space and global tiling settings, enumerates spaces (previous/next/first/last/
mission-control index), and drives every space-mutating operation — focus, switch, swap, move to
another space or display, create, destroy — almost all of which go through the scripting addition.

**2. Types.** None defined here (all in `space_manager.h` / `view.h`).

**3. Globals and statics.**

| Name | Type | Declared | Threads | Synchronisation |
|---|---|---|---|---|
| `g_window_manager` | `struct window_manager` | `space_manager.c:1` | event loop; also the CVDisplayLink animation thread reads `window_animation_duration` indirectly via a copied context (`window_manager.c:610`) | none; **P11** compiler barriers at `space_manager.c:752,796` |
| `g_connection` | `int` | `space_manager.c:2` | main (write once), event loop (read) | none |
| `g_space_manager` | `struct space_manager` | inherited (`event_loop.c:5`) | main (startup), event loop; reached without a parameter at `space_manager.c:18,36,57,83,760-774,784,1056` | none |
| `SLSPerformAsynchronousBridgedWindowManagementOperation` | `static int64_t (*)(void *)` (`misc/extern.h:5`) | — | **written on main at `yabai.c:149`, read on the event loop at `space_manager.c:667,688`** | none (**P10**) |

Function-local statics: none. Two file-local statics: `hash_view` (`space_manager.c:4`) and
`compare_view` (`space_manager.c:9`), both macro-generated function definitions, not variables.

**4. Functions.** (67, in source order. Families with an identical shape are grouped but every
signature and line is listed.)

#### `static TABLE_HASH_FUNC(hash_view)` — `space_manager.c:4` → `unsigned long hash_view(void *key)`
Identity hash: `*(uint64_t *) key`. Event loop. No allocation.

#### `static TABLE_COMPARE_FUNC(compare_view)` — `space_manager.c:9` → `int compare_view(void *key_a, void *key_b)`
`*(uint64_t*)a == *(uint64_t*)b`. Both vanish under **P4**.

#### `bool space_manager_query_space(FILE *rsp, uint64_t sid, uint64_t flags)` — `space_manager.c:14`
Serialises one space's view; `false` if there is no view. Event loop. Calls `view_serialize`
(`view.c`). **P14**.

#### `bool space_manager_query_spaces_for_window(FILE *rsp, struct window *window, uint64_t flags)` — `space_manager.c:26`
All spaces a window is on. Event loop. Allocates: `window_space_list` (`window.c`, arena).
**P15 quirk** at `space_manager.c:38` (`continue` eats the terminator).

#### `bool space_manager_query_spaces_for_display(FILE *rsp, uint32_t did, uint64_t flags)` — `space_manager.c:47`
Same for one display; arena list from `display_space_list`. Same **P15** quirk.
Note `space_count` is read at `space_manager.c:56` although `display_space_list` may leave it
untouched — safe only because of the `NULL` check at `space_manager.c:53` (**P2**).

#### `bool space_manager_query_spaces_for_displays(FILE *rsp, uint64_t flags)` — `space_manager.c:68`
Nested display→spaces walk. Prints `,` between spaces within a display (`space_manager.c:87`) and
`,`/`]` between displays (`space_manager.c:90`) — i.e. it emits a **flat** array, not nested. Same
**P15** quirks, plus: a display whose `space_list` is `NULL` is `continue`d *before* its separator is
written (`space_manager.c:80`), so the array can end without `]`.

#### `struct view *space_manager_query_view(struct space_manager *sm, uint64_t sid)` — `space_manager.c:97`
`did_begin ? find_view(sid) : table_find(sid)`. After startup this **creates** a view on demand;
before startup it does not. Returns a borrowed pointer.

#### `struct view *space_manager_find_view(struct space_manager *sm, uint64_t sid)` — `space_manager.c:103`
Get-or-insert. Allocates: `view_create` (`view.c:986`) — `malloc` for the view and its root node, plus
a +1 `CFString` uuid (`view.c:995`); the table `malloc`s an 8-byte key copy. **Nothing frees views**
except `view_destroy` (`view.c:1033`), which does not `free(view)` itself — a known leak.
Rust: `sm.view.entry(sid).or_insert_with(|| View::new(sid))` returning `&mut View`.

#### `void space_manager_refresh_view(struct space_manager *sm, uint64_t sid)` — `space_manager.c:113`
`find_view`, skip if `VIEW_FLOAT`, then `view_update` + `view_flush`.

#### `void space_manager_mark_view_invalid(struct space_manager *sm, uint64_t sid)` — `space_manager.c:122`
`find_view`, skip if float, `view_clear_flag(view, VIEW_IS_VALID)`.

#### `void space_manager_untile_window(struct view *view, struct window *window)` — `space_manager.c:130`
Removes the window's node; flushes immediately if the space is visible, else marks the view dirty.
Callers: `event_loop.c:615,785,865`, `mouse_handler.c:135,223` (all event loop).
Externs: yabai `window_manager_adjust_layer`, `view_remove_window_node`, `space_is_visible`,
`window_node_flush`.

#### `struct space_label *space_manager_get_label_for_space(...)` — `space_manager.c:145` / `get_space_for_label` — `:157` / `remove_label_for_space` — `:169` / `set_label_for_space` — `:183`
Exact mirrors of the display-label four (`display_manager.c:23,35,47,61`); same ownership rules
(`free` at `space_manager.c:174,190`; ownership taken at `space_manager.c:196`), same **P3**
swap-remove, same borrowed-pointer return.

#### `void space_manager_set_layout_for_space(struct space_manager *sm, uint64_t sid, enum view_type layout)` — `space_manager.c:202`
Sets the view's layout, `view_clear`s it, and re-scans the space for windows unless float.
**P7**-adjacent: calls `window_manager_validate_and_check_for_windows_on_space` while holding a
`struct view *` borrowed from the table — that callee can insert into the table and invalidate the
pointer. It does not today (the sid is already present), but Rust will require re-looking-up the view
after the call, or splitting the borrow.

#### `bool space_manager_set_gap_for_space(struct space_manager *sm, uint64_t sid, int type, int gap)` — `space_manager.c:213`
`TYPE_ABS` (`0x1`) sets, `TYPE_REL` (`0x2`) adds with `add_and_clamp_to_zero` (`macros.h:11`, a
double-evaluating macro — harmless here). `false` for float views.

#### `bool space_manager_toggle_gap_for_space(...)` — `space_manager.c:230`
Toggles `VIEW_ENABLE_GAP`, updates, flushes.

#### `void space_manager_toggle_mission_control(uint64_t sid)` — `space_manager.c:247` / `toggle_show_desktop` — `:253`
Focus the space, then `CoreDockSendNotification(CFSTR("com.apple.expose.awake"), 0)` /
`CFSTR("com.apple.showdesktop.awake")`. The return value of `space_manager_focus_space` is **ignored**.
Rust: `CFSTR` becomes a `CFString::from_static_string` or a cached `static`.

#### The eight `*_for_all_spaces` setters
`set_layout_for_all_spaces` (`space_manager.c:259`), `set_window_gap_for_all_spaces` (`:276`),
`set_top_padding_for_all_spaces` (`:288`), `set_bottom_padding_for_all_spaces` (`:300`),
`set_left_padding_for_all_spaces` (`:312`), `set_right_padding_for_all_spaces` (`:324`),
`set_split_type_for_all_spaces` (`:336`), `set_auto_balance_for_all_spaces` (`:346`).

All share one shape: write the default on `sm`, then `table_for` every view and, **only if the view
does not carry the corresponding `VIEW_*` override flag**, write the field. The first six then
`view_update` + `view_flush` per view; the last two do not. `set_layout_for_all_spaces` additionally
filters on `space_is_user(view->sid)` and calls `view_clear` +
`window_manager_validate_and_check_for_windows_on_space` (**P7**, the only one that can mutate the
table mid-iteration).
Signatures: `void f(struct space_manager *sm, T value)` where `T` is `enum view_type`, `int`, `int`,
`int`, `int`, `int`, `enum window_node_split`, `uint32_t` respectively.
Thread: event loop (`message.c` config commands).

#### `bool space_manager_set_padding_for_space(struct space_manager *sm, uint64_t sid, int type, int top, int bottom, int left, int right)` — `space_manager.c:356`
ABS/REL as above, four fields at once.

#### `bool space_manager_toggle_padding_for_space(...)` — `space_manager.c:379`
Toggles `VIEW_ENABLE_PADDING`.

#### `bool space_manager_rotate_space(struct space_manager *sm, uint64_t sid, int degrees)` — `space_manager.c:396` / `mirror_space(…, enum window_node_split axis)` — `:408` / `equalize_space(…, uint32_t axis_flag)` — `:420` / `balance_space(…, uint32_t axis_flag)` — `:432`
Each requires `VIEW_BSP` (returns `false` otherwise), calls the corresponding `window_node_*`
(`view.c`), then `view_update` + `view_flush`.

#### `struct view *space_manager_tile_window_on_space_with_insertion_point(struct space_manager *sm, struct window *window, uint64_t sid, uint32_t insertion_point)` — `space_manager.c:444`
Adds the window to the space's tree; returns the view (even for float views, early at
`space_manager.c:447`). `assert(node)` at `space_manager.c:451` is a **release no-op**, so a `NULL`
node would be dereferenced by `window_node_flush`. Flush-if-visible else mark dirty
(`space_manager.c:453-457`).

#### `struct view *space_manager_tile_window_on_space(...)` — `space_manager.c:462`
Insertion point `0`.

#### `void space_manager_toggle_window_split(struct space_manager *sm, struct window *window)` — `space_manager.c:467`
Flips the parent node's split axis; if `view->auto_balance != SPLIT_NONE` rebalances the whole tree,
otherwise updates just that subtree and flushes-or-dirties.
Note it compares a `uint32_t auto_balance` against the `enum window_node_split` value `SPLIT_NONE`
(`space_manager.c:476`) — the field is typed `uint32_t` in the struct but holds split-enum values.
Rust: keep one type (`WindowNodeSplit`) and note the widening.

#### `int space_manager_mission_control_index(uint64_t sid)` — `space_manager.c:491`
1-based position of `sid` in the flattened display→spaces order; `0` if absent (`desktop_cnt = 0` at
`space_manager.c:515`). `uint64_t result` is declared once outside both loops
(`space_manager.c:493`) — relevant to **P20**. One CF array (+1) released at `space_manager.c:517`.
Thread: event loop (`display.c:79`, `event_signal.c:230,255,256`, `rule.c:47`, `space_manager.c:824,825,881,929,930`, `message.c`).

#### `uint64_t space_manager_mission_control_space(int desktop_id)` — `space_manager.c:521`
Inverse; `0` if out of range. Note the `goto out` fires **before** `++desktop_cnt`, so `result` holds
the matching space (`space_manager.c:539`).

#### `uint64_t space_manager_cursor_space(void)` — `space_manager.c:551`
`display_space_id(display_manager_cursor_display_id())`.

#### `uint64_t space_manager_prev_space(uint64_t sid)` — `space_manager.c:557`
The space immediately before `sid` in the **global flattened order**, crossing display boundaries;
returns `0` if `sid` is the very first (`p_sid` still 0) or if the previous equals `sid`
(`space_manager.c:583`). Callers compare displays themselves (`space_manager.c:818-822`).

#### `uint64_t space_manager_next_space(uint64_t sid)` — `space_manager.c:586`
The space immediately after, same flattening. `found_sid` is set at the end of each iteration and
tested at the start of the next, so the value returned is the one read *after* the match; if the match
is the last space, the loop exits with `n_sid == sid` and the guard at `space_manager.c:612` returns 0.

#### `uint64_t space_manager_first_space(void)` — `space_manager.c:615` / `last_space` — `:633`
Index `[0][0]` and `[last][last]` of the nested CF structure, with **no bounds checks** on
`CFArrayGetValueAtIndex` (`space_manager.c:622,625,641,645`).

#### `uint64_t space_manager_active_space(void)` — `space_manager.c:653`
Focused window's display → else active display → else 0; then that display's current space.
Called from a dozen places including `space_manager.c:829,831,834,836,841,843,876,878,882,884,912,975,990,1016,1198,1230`.

#### `void space_manager_move_window_list_to_space(uint64_t sid, uint32_t *window_list, int window_count)` — `space_manager.c:665`
Three strategies, in order: the bridged async operation if the private symbol resolved (**P9**,
**P10**); else `SLSMoveWindowsToManagedSpace` unless the workaround is needed; else the scripting
addition, falling back to the `SLSSpaceSetCompatID` / `SLSSetWindowListWorkspace` /
`SLSSpaceSetCompatID(…, 0)` triple with the magic id `0x79616265` (**P23**).
Externs: `cfarray_of_cfnumbers`, `objc_getClass`, `sel_registerName`, `objc_msgSend`, `-[alloc]`,
`-[release]`, `SLSPerformAsynchronousBridgedWindowManagementOperation`, `SLSMoveWindowsToManagedSpace`,
`SLSSpaceSetCompatID`, `SLSSetWindowListWorkspace`, `CFRelease`; yabai
`workspace_use_macos_space_workaround`, `scripting_addition_move_window_list_to_space`.

#### `void space_manager_move_window_to_space(uint64_t sid, struct window *window)` — `space_manager.c:686`
Identical structure for a single window; note it takes the address of `window->id`
(`space_manager.c:689,702`).

#### `static inline uint64_t space_manager_find_first_user_space_for_display(uint32_t did)` — `space_manager.c:707`
First `space_is_user` space on the display, else 0. Arena list.

#### `static inline bool space_manager_is_space_last_user_space(uint64_t sid)` — `space_manager.c:724`
True if no *other* user space exists on the same display. Returns `true` when the list is `NULL`
(`space_manager.c:730`) — i.e. "unknown" is treated as "last", which blocks destroy/move.

#### `static enum space_op_error space_manager_swap_space_with_space_on_display(uint32_t a_did, uint64_t a_sid, uint32_t b_did, uint64_t b_sid)` — `space_manager.c:745`
The cross-display swap: animation duration saved and zeroed (**P11**), both window lists captured
(including minimized), the two `struct view *` re-keyed in the table (`space_manager.c:760-774`), the
views' `sid` and `uuid` **swapped** (`space_manager.c:766-771`), the window lists moved, labels
re-pointed (`space_manager.c:784-788`), both views updated and flushed, duration restored.
- **Risk:** `table_find` results are used without a `NULL` check (`space_manager.c:760-767`); a space
  with no view crashes. In Rust, `HashMap::remove` twice returning `Option<Box<View>>` makes this
  explicit — decide whether to early-return or `expect`.
- The `uuid` swap is a plain pointer exchange with **no retain/release** — correct, since both are +1
  and stay +1.
- Callers: `space_manager.c:810,1029`.

#### `enum space_op_error space_manager_swap_space_with_space(uint64_t acting_sid, uint64_t selector_sid)` — `space_manager.c:801`
Same-display swap expressed as one or two `scripting_addition_move_space_after_space` calls; the
five-way branch (`space_manager.c:828-846`) depends on whether each space is the first on its display
and on the relative mission-control indices. Cross-display delegates to the function above
(`space_manager.c:810`).
Note `success &= ...` (`space_manager.c:834,837,841,844`) on a `bool` — bitwise AND, **no short-circuit**:
the second scripting-addition call always runs. Rust: `success &= f(...)` on `bool` behaves the same
(`BitAndAssign` for `bool` is not short-circuiting) — keep `&=`, not `&&`.

#### `enum space_op_error space_manager_move_space_to_space(uint64_t acting_sid, uint64_t selector_sid)` — `space_manager.c:851`
Three-way branch over the same "is first" predicates; both-first is silently a no-op returning
`SUCCESS` (no `else` at `space_manager.c:880-886`).

#### `enum space_op_error space_manager_move_space_to_display(struct space_manager *sm, uint64_t sid, uint32_t did)` — `space_manager.c:891`
Ordered guards (mission control, missing src, same display, src animating, last user space, dst
animating, missing dst), then the scripting addition with a `focus_space` flag, then
`space_manager_mark_view_invalid` and an optional re-focus.
**The guard order is observable** (each maps to a distinct CLI error message) — preserve it exactly.

#### `bool space_manager_focus_space_using_gesture(uint32_t new_did, uint64_t new_sid)` — `space_manager.c:927`
Synthesises `count = |new_index - cur_index|` high-velocity dock-swipe gestures via a single reused
`CGEventRef` with raw field numbers (**P23**, and the comment block at `space_manager.c:944-953` must
survive). If already on the target index it just focuses the display. Warps the cursor first when
changing display, and afterwards sets the active display and may synthesise a click.
- Allocates: one `CGEventCreate` (+1, released `space_manager.c:971`); returns `false` if it fails.
- Externs: `CGEventCreate`, `CGEventSetIntegerValueField`, `CGEventSetDoubleValueField`, `CGEventPost`,
  `CGWarpMouseCursorPosition`, `CGPostMouseEvent`, `CFRelease`, `abs`.
- **P19:** `float sign` at `space_manager.c:958`.

#### `enum space_op_error space_manager_focus_space(uint64_t sid)` — `space_manager.c:985`
Mission-control guard, same-space guard, animating guard, then the scripting addition (plus a display
focus when crossing displays) with the gesture path as fallback. **Always returns `SUCCESS`** once
past the guards, even when the gesture path fails (`space_manager.c:1005-1008`).

#### `enum space_op_error space_manager_switch_space(uint64_t sid)` — `space_manager.c:1011`
For a space on another display, performs the swap-on-display plus a display focus; otherwise
`scripting_addition_focus_space`.

#### `enum space_op_error space_manager_destroy_space(uint64_t sid)` — `space_manager.c:1037`
Guards (mission control, missing, not user → `INVALID_TYPE`, last user space → `INVALID_SRC`,
animating), destroys via the scripting addition, then revalidates the display's first user space.
Note `first_sid` is captured **before** the destroy (`space_manager.c:1047`).

#### `enum space_op_error space_manager_add_space(uint64_t sid)` — `space_manager.c:1062`
Mission control / missing / animating guards, then `scripting_addition_create_space`.

#### `void space_manager_assign_process_to_space(pid_t pid, uint64_t sid)` — `space_manager.c:1074` / `assign_process_to_all_spaces(pid_t pid)` — `:1079`
One SLS call each. **Neither has a caller** — dead code.

#### `bool space_manager_is_window_on_active_space(struct window *window)` — `space_manager.c:1084`
**No caller** — dead code.

#### `bool space_manager_is_window_on_space(uint64_t sid, struct window *window)` — `space_manager.c:1091`
Linear scan of `window_space_list`. Caller: `event_loop.c:904`.

#### `void space_manager_mark_spaces_invalid_for_display(struct space_manager *sm, uint32_t did)` — `space_manager.c:1106`
Refreshes the display's current space and invalidates the rest.

#### `void space_manager_mark_spaces_invalid(struct space_manager *sm)` — `space_manager.c:1122`
The same for every active display.

#### `bool space_manager_refresh_application_windows(struct space_manager *sm)` — `space_manager.c:1133`
Drains `g_window_manager.applications_to_refresh` with the swap-remove-while-iterating idiom (**P7**);
returns whether the tracked-window count changed. Emits a `debug()` line per application
(`space_manager.c:1140`) — `debug` is a no-op unless `-V` (`log.h:6-15`).

#### `void space_manager_handle_display_add(struct space_manager *sm, uint32_t did)` — `space_manager.c:1150`
Re-keys existing views onto the new display's space ids by matching each space's `SLSSpaceCopyName`
uuid against the views' stored uuids: snapshot the table into two VLAs (**P24**), for each new space
find the matching view, `table_remove` the old key, release the old uuid, retarget any space label,
set the new `sid`/`uuid` (+1 via `CFRetain`), `table_add` under the new key, and null the snapshot
slots so each view matches at most once. Finally refreshes `current_space_id`/`last_space_id`.
- Allocates/releases: `SLSSpaceCopyName` (+1, released `space_manager.c:1195` on every path),
  `CFRelease(view->uuid)` (`space_manager.c:1182`), `CFRetain(uuid)` (`space_manager.c:1188`).
- Caller: `event_loop.c:1087` (DISPLAY_ADDED).
- Rust: drain into `Vec<(u64, CFString)>`, then a single pass; `HashMap::remove` + `insert` moves the
  `Box<View>` without touching refcounts beyond the uuid swap.

#### `void space_manager_begin(struct space_manager *sm)` — `space_manager.c:1202`
Sets the defaults listed in §9, `table_init`s the view table with capacity 23, then creates a view for
every space of every active display, and finally records the active space and sets `did_begin`.
- Thread: **main**, `yabai.c:337`.
- **Early return at `space_manager.c:1217`** when there are no displays leaves `did_begin` false and
  `current_space_id` zero — a state the rest of the code does not expect. Preserve.
- Does not set `top/bottom/left/right_padding` or `window_gap` (zeroed global).

**5. OS / run-loop callbacks registered here.** None. The only OS-facing indirection is the ObjC
operation object handed to `SLSPerformAsynchronousBridgedWindowManagementOperation`
(`space_manager.c:672,693`), which SkyLight executes asynchronously on its own queue; yabai releases
its reference immediately afterwards (`space_manager.c:673,694`) and never observes completion. The
`CFArray` of window ids is released right after (`space_manager.c:674,695`) — SkyLight has retained
what it needs via the operation's `init`.

**6. Pattern catalogue for this file.**

| Pattern | Sites |
|---|---|
| **P1** globals reached without a parameter | `space_manager.c:18,36,57,83,209,269,660,750-751,760-774,784,797,1056,1135-1147` |
| **P2** goto ladders | `space_manager.c:509,516,539,546,575,581,604,610` |
| **P3** stretchy buffers | `space_manager.c:147,159,171,175,187,191,196,784,1135` |
| **P4** hash table, malloc'd keys, leaked values, re-keying | `space_manager.c:4-12,100,105-108,760-774,1181,1190,1213,1226` |
| **P5** `table_for` | `space_manager.c:262,279,291,303,315,327,339,349,1160` |
| **P6** arena lists | `space_manager.c:31,52,79,710,729,755,758,1094,1109,1125,1153,1216,1221` (all via callees) |
| **P7** mutate-while-iterate | `space_manager.c:259-274` (table), `space_manager.c:1133-1148` (buffer) |
| **P8** CF pairs | `space_manager.c:496-517,526-547,562-582,591-611,619-629,637-649,668-678,689-699,955-971,1168-1195` |
| **P9** ObjC msgSend + manual release | `space_manager.c:669-674,690-695` |
| **P10** dynamically resolved symbol as a feature flag | `space_manager.c:667,688` |
| **P11** compiler barrier around a global save/restore | `space_manager.c:752,796` |
| **P14/P15** response writing and terminator quirks | `space_manager.c:22,34-42,55-63,76-92` |
| **P19** float/int narrowing | `space_manager.c:932,958,963` |
| **P20** `CFNumberGetValue` with queried type | `space_manager.c:508,538,574,603,627,647` |
| **P21** view bit flags | `space_manager.c:127,141,235-238,263,280,292,304,316,328,340,350,384-387,456,485` |
| **P22** `space_op_error` returns | `space_manager.c:747-748,798,801-849,851-889,891-923,985-1009,1011-1035,1037-1060,1062-1072` |
| **P23** magic constants | `space_manager.c:680-682,701-703,959-969` |
| **P24** VLAs | `space_manager.c:1157-1158` |
| **P25** diagnostic pragma | `space_manager.c:925-926,983` |
| `assert` that vanishes under NDEBUG | `space_manager.c:451` |
| non-short-circuiting `&=` on `bool` | `space_manager.c:834,837,841,844,879` |

**7. Comments to carry over verbatim.**

- `space_manager.c:944-953`
  ```
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
- The inline field-name comments on `space_manager.c:959-969`, which are the only documentation of the
  raw `CGEvent` field numbers:
  - `:959` `/* kCGSEventTypeField */ 55, /* kCGSEventDockControl */ 30`
  - `:960` `/* kCGEventGestureHIDType */ 110, /* kIOHIDEventTypeDockSwipe */ 23`
  - `:961` `/* kCGEventGestureSwipeMotion */ 123, /* kCGGestureMotionHorizontal */ 1`
  - `:962` `/* kCGEventGestureSwipeProgress */ 124`
  - `:963` `/* kCGEventGestureSwipeVelocityX */ 129`
  - `:966` `/* kCGEventGesturePhase */ 132, /* kCGSGesturePhaseBegan */ 1`
  - `:968` `/* kCGEventGesturePhase */ 132, /* kCGSGesturePhaseEnded */ 4`

  In Rust these become named `const` items (the comment text *is* the name); that is the one place
  where the comment is allowed to disappear, because the name replaces it.

No other comments exist in this file.

---

## 11. Full list of comments to carry over (all four `.c` files, all four `.h` files)

| Location | Text |
|---|---|
| `space_manager.c:944-953` | the `NOTE(asmvik)` gesture block + `:Attribution` links, verbatim (see §10.7) |
| `space_manager.c:959,960,961,962,963,966,968` | inline `/* kCG… */` field-number names (see §10.7) |
| `display_manager.c:312-316` | `NOTE(asmvik): SLSGetRevealedMenuBarBounds is broken on Apple Silicon, …` |
| `display_manager.c:325-328` | `NOTE(asmvik): Height needs to be offset by 1 …` |
| `display_manager.c:370` | `// This does not return a correct result on modern macOS versions.` |
| `display_manager.c:388` | `// This does not return a correct result on modern macOS versions.` |

`space.h`, `space.c`, `display.h`, `display.c`, `display_manager.h`, `space_manager.h` contain **no
comments at all**. Anything that appears in the Rust version of those files is new commentary and must
not be written.

---

## 12. Dead code in this set (do not translate, or translate and say so)

| Symbol | Where | Evidence |
|---|---|---|
| `space_manager_mark_view_dirty` | declared `space_manager.h:55` | no definition, no caller anywhere in `src/` |
| `space_manager_assign_process_to_space` | `space_manager.c:1074` | referenced only by its own header |
| `space_manager_assign_process_to_all_spaces` | `space_manager.c:1079` | same |
| `space_manager_is_window_on_active_space` | `space_manager.c:1084` | same |
| `space_is_system` | `space.c:98` | same |
| `display_space_count` | `display.c:190` | same |
| `display_manager_main_display_uuid` | `display_manager.c:80` | same |
| `display_manager_active_display_is_animating` | `display_manager.c:355` | same |
| `display_callback` typedef | `display.h:5` | never used as a variable type |

---

## 13. Suggested phase-2 module shape (one C pair → one Rust module)

```
src/space.rs             // space.h + space.c
src/display.rs           // display.h + display.c   (incl. the reconfiguration callback)
src/display_manager.rs   // display_manager.h + display_manager.c
src/space_manager.rs     // space_manager.h + space_manager.c
src/sys/skylight.rs      // the `extern "C"` block mirroring misc/extern.h (shared by all four)
```

`display_manager` and `space_manager` are mutually recursive (`display_manager::focus_display` →
`space_manager::active_space`; `space_manager::focus_space` → `display_manager::focus_display`), which
is fine inside one crate. `SpaceOpError` lives in `space_manager` and is `use`d by `display_manager`,
mirroring the C header order.

Crates to pull in: `core-foundation` / `core-foundation-sys`, `core-graphics` /
`core-graphics-types` (for `CGRect`, `CGPoint`, `CGDirectDisplayID`, `CGDisplayBounds`,
`CGEvent*`, `CGWarpMouseCursorPosition`), `objc2` (for `space_manager.c:669-674,690-695`), `bitflags`,
`libc`. Everything under `SLS*` / `CoreDock*` / `_AXUIElementGetWindow` needs a hand-written
`#[link(name = "SkyLight", kind = "framework")] extern "C"` block transcribed from `misc/extern.h`.
