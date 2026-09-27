# Phase 1 map — `src/misc/helpers.h`, `src/misc/service.h`, `src/misc/extern.h`

Reader: `misc-helpers-service-extern`. All three files were read in full. Line citations are
`path:line` against the current working tree (branch `main`, `dd84572`).

---

## 0. Context shared by the three files

### 0.1 How they enter the build

`src/manifest.m` is the single translation unit. Inclusion order matters because these headers
have *no* includes of their own and rely on everything before them:

| order | manifest.m | provides to our files |
|---|---|---|
| 1 | system headers, `manifest.m:1-43` | `mach_msg`, `sockaddr_un`, `regex_t`, `posix_spawn`, `__m128`/`float32x4_t`, Carbon/Cocoa/CoreVideo |
| 2 | `misc/extern.h`, `manifest.m:45` | all private SkyLight/HIServices prototypes used by `helpers.h` |
| 3 | `misc/macros.h`, `manifest.m:46` | `array_count`, `MAXLEN`, `REGEX_MATCH_*`, `LAYER_*` |
| 4 | `misc/ts.h`, `manifest.m:48` | `ts_alloc_unaligned` |
| 5 | `misc/notify.h`, `manifest.m:50` | `notify()` |
| 6 | `misc/log.h`, `manifest.m:51` | `debug`/`warn`/`error`/`require` |
| 7 | **`misc/helpers.h`**, `manifest.m:52` | — |
| 8 | `misc/service.h`, `manifest.m:59` | needs `cfstring_copy`, `directory_exists`, `file_exists` from `helpers.h` |

`extern.h` has **no include guard** (compare `helpers.h:1-2`, `service.h:1-2`); it is included
exactly once and would break on a second include only for the two `static` function-pointer
definitions (`extern.h:4-5`), which would silently become per-TU copies.

Everything in `helpers.h` and `service.h` is `static` / `static inline` at file scope: internal
linkage, one copy, no header/implementation split. In Rust each file becomes one module with
`pub fn` items; nothing about the C linkage needs preserving.

### 0.2 Thread inventory (as used by this document)

| name | created at | what runs on it |
|---|---|---|
| **main run loop** | process entry, `[NSApp run]` at `src/yabai.c:350` | CLI paths in `parse_arguments` (`yabai.c:181-258`, before any thread exists), AX observer callbacks (`application.c:57`, `mission_control.c:87` both `CFRunLoopAddSource(CFRunLoopGetMain(), …)`), CGEventTap (`mouse_handler.c:288`), Carbon process handler (`process_manager.c:251`), SLS connection notify proc (`yabai.c:322-334`), `dispatch_get_main_queue()` blocks (`event_loop.c:93,157,1478,1516,1520`) |
| **event-loop pthread** | `pthread_create` at `event_loop.c:1718` | every `EVENT_HANDLER_*`, hence all message/query handling (`DAEMON_MESSAGE` → `handle_message`, `event_loop.c:1634`), `event_signal_flush()` and `ts_reset()` (`event_loop.c:1670-1671`) |
| **message-loop pthread** | `pthread_create` at `message.c:3042` | only `accept()` + `event_loop_post` (`message.c:3005-3010`); touches none of our helpers |
| **CVDisplayLink thread** | `CVDisplayLinkStart` at `window_manager.c:702`, callback `window_manager_animate_window_list_thread_proc` (`window_manager.c:537`) | per-frame animation step: easing call at `window_manager.c:551`, `mach_send` via `window_manager_notify_jankyborders` at `window_manager.c:579` |
| **proxy-build pthreads** (transient, joined at `window_manager.c:680`) | `pthread_create` at `window_manager.c:666` | `window_manager_build_window_proxy_thread_proc` (`window_manager.c:507`): `cgimage_restore_alpha` (`:525`), `sls_window_disable_shadow` (`:473`), `window_level`/`window_sub_level` (`:513-514`) |
| **forked child** | `fork()` at `helpers.h:477` | only `execvp` (`helpers.h:482`) |

How thread attribution was determined for every claim below: by walking each call site upward to
the nearest registration/`pthread_create`/`main` shown in this table. Where the chain is
ambiguous, it is called out.

### 0.3 What these three files need that lives elsewhere (phase-2 module edges)

`helpers.h` ← `ts_alloc_unaligned` (`ts.h:66`), `warn`/`error` (`log.h:17,26`), `notify`
(`notify.h:29`), `array_count` (`macros.h:8`), `LAYER_*` (`macros.h:42-45`), `REGEX_MATCH_*`
(`macros.h:22-24`), `SLSWindowSetShadowProperties` / `_AXUIElementGetWindow` (`extern.h:85,8`).
`service.h` ← `cfstring_copy`, `directory_exists`, `file_exists` (`helpers.h:373,408,419`),
`error`/`warn` (`log.h`), `MAXLEN` (`macros.h:20`).

---

## 1. `src/misc/helpers.h` (677 lines)

### 1.1 Purpose

Header-only grab bag of leaf utilities for the daemon: the 21 animation easing curves and their
X-macro name table, a monotonic nanosecond clock, colour unpacking, UNIX-socket open/connect/close,
a hand-rolled out-of-line mach message send, JSON string/bool emission into the temp arena,
CFString ↔ C string copying (arena and `malloc` flavours), filesystem predicates plus config-file
discovery and `fork`+`exec` of the user's `yabairc`, four Accessibility helpers (one of which reads
a private struct field by hardcoded offset), geometry predicates, POSIX regex matching and a
SIMD un-premultiply pass over a captured window image.

It has **no state of its own beyond three read-only string tables**, and registers nothing with
the OS.

### 1.2 Types, enums, X-macros, constants

#### `ANIMATION_EASING_TYPE_LIST` — `helpers.h:4-25`
X-macro list of 21 identifiers (`ease_in_sine` … `ease_in_out_circ`). Expanded three times in the
program:
- `helpers.h:29-31` → enum constants `ease_in_sine_type` = 0 … `ease_in_out_circ_type` = 20;
- `helpers.h:37-39` → the name table (below);
- `window_manager.c:551-553` → `case value##_type: mt = value(t); break;` inside the CVDisplayLink
  callback, i.e. the list also generates the dispatch.

#### `enum animation_easing_type` — `helpers.h:27-33`
21 variants plus `EASING_TYPE_COUNT` = 21 (used as a loop bound at `message.c:1321`). The value is
*not* stored as this enum anywhere: `window_manager.h:100` declares `int window_animation_easing`,
written from the message handler (`message.c:1323`, event-loop thread) and snapshotted into the
animation context at `window_manager.c:611` (event-loop thread) before the CVDisplayLink thread
reads `context->animation_easing`. So the enum never crosses a thread boundary as a global.

**Rust:**
```rust
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AnimationEasingType { EaseInSine = 0, /* … 21 variants … */ }
impl AnimationEasingType {
    pub const ALL: [AnimationEasingType; 21] = [ … ];
    pub fn name(self) -> &'static str { … }
    pub fn apply(self, t: f32) -> f32 { match self { … } }
}
```
One `match` replaces both the string table and the generated switch. **Behaviour to watch:** the C
switch at `window_manager.c:550-554` has **no `default`**, so an out-of-range
`window_animation_easing` leaves `float mt` uninitialised and the frame transform is garbage. The
value is only ever set from the validated loop at `message.c:1321-1327`, so it cannot happen today;
a Rust enum makes it unrepresentable, which is a strict improvement and not an observable change.

#### `static char *animation_easing_type_str[]` — `helpers.h:35-40`
Designated-initialiser table, 21 entries, indexed by the enum. Note it is `char *`, not
`const char *` (the other two tables are const); the pointees are string literals, so any write
would be UB. Read at `message.c:1318` (print current) and `message.c:1322` (parse) — event-loop
thread only. Memory: string literals in `__TEXT`, never freed.

#### `struct rgba_color` — `helpers.h:162-169`
```c
struct rgba_color { uint32_t p; float r; float g; float b; float a; };   // 20 bytes, align 4
```
(size verified by compiling a probe). No pointer fields. One instance exists:
`g_window_manager.insert_feedback_color` (`window_manager.h:101`), initialised at
`window_manager.c:2725`, rewritten from the message handler at `message.c:1375` (event-loop
thread), read at `view.c:29-37` when drawing the insert-feedback window (event-loop thread).
Single-threaded in practice; `p` keeps the original packed hex so the setting can be echoed back.

**Rust:** plain `#[derive(Clone, Copy)] pub struct RgbaColor { pub p: u32, pub r: f32, pub g: f32,
pub b: f32, pub a: f32 }`. No `repr(C)` needed — it is never handed to the OS (the float fields are
passed one by one to `CGContextSetRGBFillColor` at `view.c:28-37`).

#### `static const CFStringRef kAXEnhancedUserInterface` — `helpers.h:171`
`CFSTR("AXEnhancedUserInterface")` — a compile-time constant `__CFConstantString`, never
retained/released. Used at `helpers.h:516` and inside the `AX_ENHANCED_UI_WORKAROUND` macro
(`helpers.h:527,529`). Touched from the event-loop thread only (all three macro uses:
`window_manager.c:361,405,741`).

**Rust:** there is no stable way to emit a `__CFConstantString` from Rust. Use a leaked,
lazily-created immortal string:
```rust
static AX_ENHANCED_USER_INTERFACE: OnceLock<SendCFStringRef> = OnceLock::new();
// CFStringCreateWithCStringNoCopy(null, b"AXEnhancedUserInterface\0", kCFStringEncodingASCII, kCFAllocatorNull)
```
The AX API only hashes/compares the contents, so this is behaviour-identical. **Do not** create it
per call — `AXUIElementCopyAttributeValue` is called on every window move (`window_manager.c:361`).
`CFStringRef` is `*const c_void`, not `Send`; wrap it in a newtype that asserts `Send`/`Sync`
(it is an immortal immutable CF object, so that is sound) or store it as `AtomicPtr`.

#### `static const char *bool_str[]` — `helpers.h:173`
`{ "off", "on" }`, indexed with a `bool` at `message.c:1173,1184,1258,1269,1280` (event-loop
thread). **Rust:** `fn bool_str(v: bool) -> &'static str`.

#### `static const char *layer_str[]` — `helpers.h:175-181`
Designated initialisers at indices `LAYER_AUTO`=0, `LAYER_BELOW`=`kCGBackstopMenuLevelKey`=3,
`LAYER_NORMAL`=`kCGNormalWindowLevelKey`=4, `LAYER_ABOVE`=`kCGFloatingWindowLevelKey`=5 (values
verified by compiling a probe). **The array therefore has length 6 with `NULL` holes at indices 1
and 2.** Read at `rule.c:53` (`layer_str[rule->effects.layer]`) and `window.c:115-117`; the holes
are never indexed today.

**Rust:** do *not* translate this as `[&str; 6]` with two empty strings — a lookup that would have
returned `NULL` in C must not silently return `""`. Either `fn layer_str(layer: i32) ->
Option<&'static str>` with a `match` on 0/3/4/5, or an enum `Layer { Auto, Below, Normal, Above }`
with an `i32` conversion. `LAYER_*` values come from `CGWindowLevelKey` and are hardcoded by
`macros.h:42-45`; keep them as `const` (not `CGWindowLevelForKey` results — those are the *levels*,
computed separately into `g_layer_*_window_level` at `yabai.c:145-147`).

#### `AX_ENHANCED_UI_WORKAROUND(r, c)` — `helpers.h:524-530`
Statement macro: read the attribute, clear it, run the caller's block `c`, restore it. Three call
sites, all event-loop thread: `window_manager.c:361,405,741`.

**Rust:** a closure-taking function plus a Drop guard so the restore also runs on unwind:
```rust
pub fn with_enhanced_user_interface_disabled<R>(app: AXUIElementRef, body: impl FnOnce() -> R) -> R
```
with an internal `RestoreEnhancedUserInterfaceOnDrop`. Note the C macro's block `c` at
`window_manager.c:361-…` calls back into window-manager code, so the closure will capture
`&mut`-ish state — phase 2 should pass what it needs explicitly rather than fight the borrow
checker with a closure over globals.

### 1.3 Globals / statics in this file

| symbol | line | type | init | threads | synchronisation today |
|---|---|---|---|---|---|
| `animation_easing_type_str` | 35 | `char *[21]` | string literals | event-loop only | none needed (read-only) |
| `kAXEnhancedUserInterface` | 171 | `const CFStringRef` | `CFSTR(...)` | event-loop only | none (immortal CF constant) |
| `bool_str` | 173 | `const char *[2]` | literals | event-loop only | none |
| `layer_str` | 175 | `const char *[6]` (holes at 1,2) | literals | event-loop only | none |

No function-local statics in this file. (`timer.h:38` has one, `cpu_freq`, but that is another
reader's file.)

### 1.4 Functions

Unless stated otherwise: no allocation, no ownership transfer, no external symbols beyond libm/libc.

#### Easing curves — `helpers.h:42-145` (21 functions)
`static inline float ease_*(float t)`. Pure math on `float`; call `cosf/sinf/powf/sqrtf` from libm.
**Thread:** CVDisplayLink thread only (`window_manager.c:551`).

Rust: `fn ease_in_sine(t: f32) -> f32 { 1.0 - (t * std::f32::consts::PI / 2.0).cos() }` etc.
Behaviour notes:
- `M_PI` is a `double`; `(t * M_PI) / 2.0f` promotes `t` to double, divides in double, then
  `cosf` truncates the *argument* back to float (`helpers.h:44,49,54`). To be bit-faithful use
  `((t as f64 * std::f64::consts::PI) / 2.0) as f32` then `.cos()` on `f32`. In practice the
  difference is ≤1 ulp and invisible after the `lerp` at `window_manager.c:560-563`; phase 2 should
  pick one rule and apply it to all 21 uniformly.
- `powf(x, 3)` at `helpers.h:79,94,109` passes an `int` literal promoted to float — Rust
  `x.powf(3.0)` (not `powi`, which has a different rounding path).
- The caller passes a `double t` into a `float` parameter (`window_manager.c:551`), i.e. an
  implicit narrowing; Rust needs an explicit `t as f32` there (phase 2 of `window_manager.c`).

#### `read_os_timer(void) -> uint64_t` — `helpers.h:149-154`
`mach_absolute_time()` then `AbsoluteToNanoseconds` (deprecated Carbon), with two rounds of
type-punning through `AbsoluteTime`/`Nanoseconds` (both `UnsignedWide`). Returns **nanoseconds**,
not mach ticks — this matters on Apple Silicon where the timebase is 125/3. Wrapped in
`#pragma clang diagnostic ignored "-Wdeprecated-declarations"` (`helpers.h:147-155`).
**Threads:** main run loop (`mission_control.c:24` in the SLS notify proc, `mouse_handler.c:79` in
the event tap) and event-loop thread (`event_loop.c:363,1265,1353`); also `timer.h:43,49` when
`PROFILE >= 1` (off by default).

**Rust:** `unsafe { clock_gettime_nsec_np(CLOCK_UPTIME_RAW) }` gives the same quantity without the
deprecated Carbon call and without the punning; alternatively `mach_absolute_time()` scaled by a
`mach_timebase_info` cached in a `OnceLock`. Do **not** return raw `mach_absolute_time()` — the
deltas at `event_loop.c:363,1353` are divided by `read_os_freq()` = 1e9 and compared against
millisecond thresholds, so raw ticks would change behaviour on arm64.

#### `read_os_freq(void) -> uint64_t` — `helpers.h:157-160`
Returns the constant `1000000000`. **Rust:** `pub const OS_TIMER_FREQUENCY: u64 = 1_000_000_000;`

#### `socket_open(int *sockfd) -> bool` — `helpers.h:183-187`
`socket(AF_UNIX, SOCK_STREAM, 0)`; writes the fd through the out-param, returns `fd != -1`.
**Threads:** main (`yabai.c:88` CLI client, `sa.m:246,428` from the `--load-sa` CLI path),
event-loop (`sa.m:428` via `scripting_addition_send_bytes` from window/space commands),
CVDisplayLink (`sa.m:428` via `scripting_addition_swap_window_proxy_out`,
`window_manager.c:580`).

#### `socket_connect(int sockfd, char *socket_path) -> bool` — `helpers.h:189-196`
Fills a `struct sockaddr_un` with `sun_family = AF_UNIX` and `snprintf` into `sun_path`, then
`connect(..., sizeof(socket_address))`.
**Two details a naive Rust translation changes:**
1. `socket_address` is **not** zero-initialised: `sun_len` (byte 0 on Darwin, verified: `sizeof` =
   106, `offsetof(sun_family)` = 1) and the tail of `sun_path` after the NUL are uninitialised
   stack bytes handed to the kernel. Darwin uses the `socklen_t` argument, so it is benign; Rust's
   `UnixStream::connect` zeroes everything, which is fine and strictly safer.
2. The length passed is `sizeof(struct sockaddr_un)` = 106, not `SUN_LEN`. Keep passing the full
   size if hand-rolling; `UnixStream::connect` does the equivalent.

**Rust:** `std::os::unix::net::UnixStream::connect(path)` replaces `socket_open` +
`socket_connect` + `socket_close` as one owned handle. That is the recommended shape — it removes
the fd-leak paths at `sa.m:246-266` — but note `socket_open`/`socket_connect` are also used where
the fd is handed to `event_loop_post` as an `int` (`message.c:3005-3009` → `event_loop.c:1613`),
so the RAII type must be able to give up ownership (`into_raw_fd`) and be rebuilt
(`from_raw_fd`) on the event-loop thread.

#### `socket_close(int sockfd)` — `helpers.h:198-202`
`shutdown(fd, SHUT_RDWR)` then `close(fd)`. **Threads:** main (`yabai.c:122`), event-loop
(`event_loop.c:1643` — the error path of `DAEMON_MESSAGE`), plus the `sa.m` sites above.
**Rust:** `shutdown(Shutdown::Both)` then drop. `close` errors are ignored in C; ignore them too.

#### `mach_send(mach_port_t port, void *data, uint32_t size)` — `helpers.h:204-223`
Builds a complex mach message carrying one out-of-line descriptor pointing at `data`
(`MACH_MSG_VIRTUAL_COPY`, `deallocate = false`) and sends it fire-and-forget (`MACH_SEND_MSG`, no
receive, no timeout). The return value of `mach_msg` is discarded.
**Threads:** event-loop (`window_manager.c:653,654,689` via `window_manager_notify_jankyborders`)
and CVDisplayLink (`window_manager.c:579`).
**External symbols:** `mach_msg` (libsystem_kernel).

**The load-bearing layout fact:** `<mach/message.h>` wraps the descriptor typedefs in
`#pragma pack(push, 4)` (SDK `mach/message.h:291` … `:604`). I verified by compiling a probe:
`sizeof(mach_msg_header_t)` = 24, `sizeof(mach_msg_ool_descriptor_t)` = 16 with **alignment 4**,
so the anonymous struct at `helpers.h:206-210` is **44 bytes** with the descriptor at offset 28 —
exactly where the kernel expects descriptors to start. A Rust `#[repr(C)]` struct holding a
naturally-aligned (8) descriptor would be 48 bytes with the descriptor at 32 and the message would
be misparsed. Translate as:
```rust
#[repr(C, packed(4))]
struct OolMessage { header: mach_msg_header_t, descriptor_count: u32, descriptor: MachMsgOolDescriptor }
```
and assert `size_of::<OolMessage>() == 44` in a `const` assertion. `msgh_bits` is
`0x80000013` (verified) = `MACH_MSG_TYPE_COPY_SEND` (19) in the remote field | `MACH_MSGH_BITS_COMPLEX`;
either recompute it with the same macro or hardcode `0x8000_0013` with the C expression kept
alongside. `msgh_size = sizeof(msg)` = 44 and the `send_size` argument is also 44.
Taking `&data` of a caller stack struct (`window_manager.c:457` passes a ~4 KB struct) is fine —
the copy happens inside `mach_msg`.

#### `json_optional_bool(int value) -> char *` — `helpers.h:225-231`
Tri-state: `0` → `"null"`, `1` → `"true"`, anything else → `"false"`. Returns a string literal;
nothing to free. **Threads:** event-loop (`rule.c:50-54`, `event_signal.c:426`).
**Rust:** the callers store an `int` tri-state (`rule->effects.manage` etc.), so the honest
translation is `fn json_optional_bool(value: i32) -> &'static str` now, and an `Option<bool>` when
phase 3 reshapes `rule_effects`. Keep the “anything not 0 or 1 is false” rule.

#### `json_bool(bool value) -> char *` — `helpers.h:233-236`
`"true"`/`"false"` literal. Heavily used from the query serialisers (`window.c`, `view.c`,
`display.c`, `rule.c`) — event-loop thread.

#### `rgba_color_from_hex(uint32_t color) -> struct rgba_color` — `helpers.h:238-247`
Unpacks `0xAARRGGBB` into four `float` channels in 0..1 and keeps the packed value in `p`.
**Threads:** event-loop (`message.c:1375`) and startup/main (`window_manager.c:2725`, called from
`window_manager_init` at `yabai.c:336`).
**Rust:** straight port; `((color >> 16) & 0xff) as f32 / 255.0`. Watch the shift widths — all
`u32`, no sign extension.

#### `is_root(void) -> bool` — `helpers.h:249-252`
`getuid() == 0 || geteuid() == 0`. **Threads:** main only (`yabai.c:267`, `sa.m:358,374`).
**Rust:** `unsafe { libc::getuid() == 0 || libc::geteuid() == 0 }`.

#### `string_equals(const char *a, const char *b) -> bool` — `helpers.h:254-257`
NULL-safe `strcmp == 0`; **returns false if either side is NULL**, including NULL == NULL.
**Threads:** main (`yabai.c:183-250`, argv parsing) and event-loop (`space_manager.c:161,189`,
`rule.c:148,197`, `event_signal.c:346,389`, `window_manager.c:180-205,2434`,
`process_manager.c:54` — that last one on the **main** thread, see §0.2, Carbon handler).
**Rust:** the C-string form stays at the FFI boundary only. Where both sides are already Rust
strings (`window_manager.c:180` compares against `"AXWindow"`), use `Option<&str>` equality and
keep the NULL-means-not-equal rule: `a.zip(b).is_some_and(|(a, b)| a == b)`.

#### `ts_string_escape(char *s) -> char *` — `helpers.h:259-319`
JSON-escapes `"` `\` `\b` `\f` `\n` `\r` `\t` and C0 controls (as `\u00xx`), into the temp arena.
**Returns `NULL` when nothing needed escaping** — every caller relies on this
(`rule.c:13-16` then `escaped_app ? escaped_app : app` style; `event_signal.c:408-410`;
`window.c:177,187,477,487`).
**Threads:** event-loop only (all call sites are query/serialise paths).
**Allocation:** `ts_alloc_unaligned` (`helpers.h:283`) — bump arena, freed wholesale by
`ts_reset()` on the event-loop thread at `event_loop.c:1671`. Never `free`d individually.
Correctness notes worth keeping in mind while porting:
- `char` is signed on both targets, so the `*cursor >= 0x00 && *cursor <= 0x1f` test at
  `helpers.h:273,308` deliberately excludes bytes ≥ 0x80 — UTF-8 passes through untouched.
- The `sprintf(dst, "%04x", …)` at `helpers.h:311` writes 5 bytes (incl. NUL) but advances 4; the
  stray NUL is overwritten by the next iteration, or lands exactly on `result[size_in_bytes]` for a
  trailing control char. In bounds, but a Rust port must use `write!`/`format_args` into a buffer
  and not replicate the overlap.
**Rust:** `fn ts_string_escape(s: &str) -> Option<String>` (or `Cow<'_, str>` returning `Borrowed`
instead of `None`). Returning an owned `String` instead of arena memory is safe here: no caller
keeps the pointer past the current `fprintf`.

#### `CFSTRINGNUM32(int32_t num) -> CFStringRef` — `helpers.h:321-326`
`snprintf` into a 255-byte stack buffer, then `CFStringCreateWithCString(NULL, …,
kCFStringEncodingMacRoman)`. **Caller owns the returned CFString (Create rule) — and there are
currently no callers anywhere in the tree.** Phase 2 may drop it; flag it rather than inventing a
user.

#### `CFNUM32(int32_t num) -> CFNumberRef` — `helpers.h:328-331`
`CFNumberCreate(NULL, kCFNumberSInt32Type, &num)`. Caller owns (released at `helpers.h:340`).
Only internal caller: `sls_window_disable_shadow` (`helpers.h:335`).

#### `sls_window_disable_shadow(uint32_t id)` — `helpers.h:333-342`
Builds `{ "com.apple.WindowShadowDensity": 0 }` and calls the private
`SLSWindowSetShadowProperties`. Releases both the number and the dictionary; the `CFSTR` key is
immortal. **Threads:** proxy-build pthreads (`window_manager.c:473` via
`window_manager_create_window_proxy` ← `window_manager_build_window_proxy_thread_proc`), event-loop
(`window_manager.c:652` same function on the re-use path, and `view.c:21` for the insert-feedback
window). So this one genuinely runs on ≥2 threads concurrently — CF object creation is
thread-safe, so no change is needed.
**External symbols:** `CFNumberCreate`, `CFDictionaryCreate`, `CFRelease`,
`SLSWindowSetShadowProperties` (SkyLight).

#### `cfarray_of_cfnumbers(void *values, size_t size, int count, CFNumberType type) -> CFArrayRef` — `helpers.h:344-359`
Creates `count` CFNumbers by reading `size` bytes at `values + size*i`, packs them into a
CFArray, releases the numbers (the array retains them). **Caller owns the array**; every caller
`CFRelease`s it (e.g. `window.c:906-921` via the `err1`/`err2` goto ladder).
**Threads:** event-loop (`space_manager.c:668-697`, `space.c:24`, `window.c:71,92,847,876,966`)
**and** main (`process_manager.c:112`) **and** proxy-build pthreads (`window.c:904` reached from
`window_level` at `window_manager.c:513`).
**Pattern risk:** `CFNumberRef temp[count]` (`helpers.h:346`) is a stack VLA; `count` is a window
count that can be large (`space_manager.c:668` passes the whole window list).
**Rust:** generic helper with a `SmallVec<[CFNumberRef; 16]>` or a plain `Vec` (the allocation is
noise next to the CF calls):
```rust
unsafe fn cfarray_of_cfnumbers<T: Copy>(values: &[T], ty: CFNumberType) -> CFArrayRef
```
Taking a typed slice removes the `size`/`count` pair and the raw pointer walk. Callers pass
`&[u32]` / `&[u64]` / `std::slice::from_ref(&wid)`.

#### `ts_cfstring_copy(CFStringRef string) -> char *` — `helpers.h:361-371`
Sizes with `CFStringGetMaximumSizeForEncoding(len, kCFStringEncodingUTF8)`, allocates
`num_bytes + 1` in the **arena**, `CFStringGetCString`. Returns NULL on conversion failure (the
arena bytes are not rewound — a small arena leak until `ts_reset`).
**Threads:** event-loop (`display.c:40`, `view.c:877`, `window.c:719,726,1006,1027`).
**Rust:** `fn cfstring_to_string(s: CFStringRef) -> Option<String>`. Note the C result is
*over-allocated* (max size for encoding, not actual); nothing depends on the excess.

#### `cfstring_copy(CFStringRef string) -> char *` — `helpers.h:373-385`
Same but with `malloc`; frees on conversion failure and returns NULL. **Caller owns and must
`free`.** Callers: `process_manager.c:44` (main thread; freed at `process_manager.c:50,56` and
stored into `process->name`, freed in `process_destroy`) and `service.h:83` (main thread, CLI;
**never freed** — the process exits immediately after).
**Rust:** the same `Option<String>`; the ownership question disappears.

#### `ts_string_copy(char *s) -> char *` / `string_copy(char *s) -> char *` — `helpers.h:387-406`
`strlen` + copy into arena / `malloc`. `string_copy` returns NULL on OOM; `ts_string_copy` cannot
fail (the arena aborts the process instead, `ts.h:28-34`).
- `ts_string_copy` threads: event-loop (`event_signal.c:146,209`, `window.c:717,726,1004,1025`).
  Note `event_signal.c:209` mixes an arena string with a string literal `"<unknown>"` in the same
  field — so the field is *not* uniformly owned. Phase 2 must model that as `Cow<'static, str>` or
  always allocate.
- `string_copy` threads: event-loop (`rule.c:95`, `window_manager.c:160`, `message.c:2618-2923`).
  Owned by the rule/signal structs; freed in `rule_destroy` / `event_signal_destroy`.
**Rust:** `String` / `str::to_owned`. `length` is `int` in C (`helpers.h:389,399`) — a >2 GB string
would overflow; irrelevant in practice, and `usize` in Rust is strictly better.

#### `directory_exists(char *filename) -> bool` — `helpers.h:408-417`
`stat` + `S_ISDIR`. **Threads:** main (`service.h:144` only).

#### `file_exists(char *filename) -> bool` — `helpers.h:419-432`
`stat`, then **false if `S_IFDIR`**, true otherwise (so a socket/fifo/symlink-to-file counts as
existing). **Threads:** main (`service.h:175,186,196,253,267`, `helpers.h:450,457,460,471`).
**Rust:** `fs::metadata(p).is_ok_and(|m| !m.is_dir())` — note `metadata` follows symlinks like
`stat` does.

#### `file_can_execute(char *filename) -> bool` — `helpers.h:434-443`
`stat` + `st_mode & S_IXUSR` (owner execute bit only — *not* an `access(X_OK)` check, so it can
disagree with what `exec` would do). **Threads:** main (`helpers.h:479`).
**Rust:** `metadata.permissions().mode() & 0o100 != 0`. Do not "improve" this to `access(X_OK)`:
it decides between `sh -c <path>` and `sh <path>` below.

#### `get_config_file(char *restrict filename, char *restrict buffer, int buffer_size) -> bool` — `helpers.h:445-461`
Probes, in order: `$XDG_CONFIG_HOME/yabai/<filename>` (only if the env var is set *and* non-empty),
`$HOME/.config/yabai/<filename>`, `$HOME/.<filename>`. Writes the winning path into `buffer` and
returns whether it exists; **`buffer` is left holding the last candidate even on failure**.
Returns false if `$HOME` is unset. **Threads:** main (`helpers.h:465`).
**Rust:** return `Option<PathBuf>`; the "buffer holds the last candidate" behaviour is only
observable through `exec_config_file`, which returns early on false, so it is safe to drop.
`restrict` has no Rust counterpart and no observable effect here.

#### `exec_config_file(char *config_file, int config_file_size)` — `helpers.h:463-487`
1. If `config_file` is empty, resolve it via `get_config_file("yabairc", …)` **writing back into
   the caller's buffer** (the global `g_config_file[4096]`, `yabai.c:46`); on failure `warn` +
   `notify` and return.
2. If the file does not exist, `warn` + `notify` and return.
3. `fork()`; in the child `execvp("/usr/bin/env", …)` with either
   `{"/usr/bin/env","sh","-c",config_file,NULL}` (executable bit set) or
   `{"/usr/bin/env","sh",config_file,NULL}` — both are **compound literals**, valid only inside the
   enclosing block, which is fine because `execvp` never returns on success.
   `exit(execvp(...))` → exit status 255 on exec failure.
4. On `fork() == -1`, `warn` + `notify`.
The parent never waits: `SIGCHLD` is `SIG_IGN` (`yabai.c:151`), so no zombie.
**Thread:** main, once, at `yabai.c:348` — *after* the event-loop, message-loop and workspace
threads already exist. `fork()` from a multi-threaded process is only safe because the child does
nothing but `execvp`; keep it that way.
**External symbols:** `getenv`, `fork`, `execvp`, `exit`, plus `warn`/`notify`.

**Rust:** `std::process::Command::new("/usr/bin/env").args([...]).spawn()` (which uses
`posix_spawn` on macOS). Three behavioural differences to decide on explicitly:
- **SIGPIPE.** The C child inherits `SIG_IGN` for SIGPIPE (`yabai.c:152`) across `exec`. Rust's
  `Command` resets SIGPIPE to `SIG_DFL` in the child. A `yabairc` containing `… | head -1` behaves
  differently. To preserve the C behaviour, set it back with `unsafe { pre_exec(|| { signal(SIGPIPE, SIG_IGN); Ok(()) }) }`
  — or decide that SIG_DFL is the wanted behaviour and say so.
- **SIGCHLD.** Same story (`SIG_IGN` is inherited across exec); `sh` normally resets it, but a
  script that relies on `wait` could notice.
- **Reaping.** With `SIGCHLD = SIG_IGN` the child is auto-reaped, so the returned `Child` must be
  dropped without `wait()`; `Child::wait` would return `ECHILD`. Use `let _ = cmd.spawn();`.
Do not use `Command::new("sh")` — the C code goes through `/usr/bin/env` deliberately.

#### `ax_privilege(void) -> bool` — `helpers.h:489-497`
`AXIsProcessTrustedWithOptions({kAXTrustedCheckOptionPrompt: true})` — **this prompts the user**.
Uses `kCFCopyStringDictionaryKeyCallBacks` for keys (not `kCFTypeDictionaryKeyCallBacks`).
**Thread:** main, once (`yabai.c:271`). **External:** `CFDictionaryCreate`, `CFRelease`,
`AXIsProcessTrustedWithOptions` (HIServices).

#### `ax_window_id(AXUIElementRef ref) -> uint32_t` — `helpers.h:499-504`
Private `_AXUIElementGetWindow`; returns 0 when the call fails because `wid` is pre-zeroed.
**Threads:** main (`application.c:12-20` — the AX observer callback; `event_loop.c:553`;
`display_manager.c:438`) and event-loop (`application.c:85,97`, `window_manager.c:1558,1621,1687`).
**Rust:** `unsafe extern "C" { fn _AXUIElementGetWindow(r: AXUIElementRef, wid: *mut u32) -> AXError; }`
and keep the "0 on failure" contract; do not turn it into `Result` at this layer or every caller
changes shape.

#### `ax_window_pid(AXUIElementRef ref) -> pid_t` — `helpers.h:506-509`
```c
return *(pid_t *)((void *) ref + 0x10);
```
Reads the pid out of the opaque `AXUIElement` struct at a **hardcoded offset 0x10** (and uses the
GNU `void *` arithmetic extension). **Thread:** main only — its one caller is `event_loop.c:559`.
**Rust:** irreducibly `unsafe`:
```rust
pub unsafe fn ax_window_pid(r: AXUIElementRef) -> libc::pid_t {
    unsafe { *((r as *const u8).add(0x10) as *const libc::pid_t) }
}
```
Keep the literal `0x10` and the function name; this is the single most ABI-fragile line in the
three files.

#### `ax_enhanced_userinterface(AXUIElementRef ref) -> bool` — `helpers.h:511-522`
`AXUIElementCopyAttributeValue(ref, kAXEnhancedUserInterface, &value)`; on success
`CFBooleanGetValue(value)` then `CFRelease(value)`. **Caller owns `value` (Copy rule)** and does
release it. If the attribute is not a CFBoolean, `CFBooleanGetValue` is given a foreign type —
preserved as-is. **Thread:** event-loop (via the macro, `window_manager.c:361,405,741`).
**External:** `AXUIElementCopyAttributeValue`, `CFBooleanGetValue`, `CFRelease`.

#### `psn_equals(ProcessSerialNumber *a, ProcessSerialNumber *b) -> bool` — `helpers.h:534-539`
Deprecated Carbon `SameProcess` (HIServices), inside a deprecation pragma pair
(`helpers.h:532-540`). Returns `result == 1`. **Threads:** main (`process_manager.c:11`, the
hashtable compare func used from the Carbon process handler) and event-loop
(`window_manager.c:1297`, `application.c:107`).
**Rust:** either bind `SameProcess`, or compare the two `u32` fields directly — but that is a
*behaviour change* if Carbon ever treats `{0,kCurrentProcess}` specially, so prefer the FFI call
and keep the `== 1` comparison (`Boolean` is `u8`; do not map it to Rust `bool` at the FFI
boundary, see §3.3).

#### `cgrect_clamp_x_radius` / `cgrect_clamp_y_radius(CGRect frame, float radius) -> float` — `helpers.h:542-556`
Halve the radius if it exceeds the rect extent. `CGRectGetWidth` returns `CGFloat` (f64) so the
comparison and the division happen in double and the result is truncated to `float` on return.
**Thread:** event-loop (`view.c:89`). **Rust:** keep `f32` in/out with `f64` inside.

#### `cgrect_contains_point(CGRect r, CGPoint p) -> bool` — `helpers.h:558-562`
Inclusive on all four edges (unlike `CGRectContainsPoint`, which is exclusive on max edges) — that
difference is deliberate; do not swap in the CG function. **Thread:** main
(`event_loop.c:1441`, inside a `goto out` ladder).

#### `triangle_contains_point(CGPoint t[3], CGPoint p) -> bool` — `helpers.h:564-571`
Three cross products, all-positive or all-negative. The array parameter decays to a pointer, so
`t` is really `CGPoint *` and the `[3]` is documentation only. The products are computed in `f64`
(CGFloat) and each assigned to a `float` (`helpers.h:566-568`) before the sign test — an
intentional-looking but lossy truncation; keep it (`let l1 = (…) as f32;`) or the sign of a
near-degenerate triangle can flip. **Thread:** main (`mouse_handler.c:120-126`, inside the event
tap). **Rust:** take `&[CGPoint; 3]`.

#### `regex_match(bool valid, regex_t *regex, const char *match) -> int` — `helpers.h:573-579`
Returns `REGEX_MATCH_UD` (0) when the regex was never compiled, else `REGEX_MATCH_YES` (1) /
`REGEX_MATCH_NO` (2) from `regexec(regex, match, 0, NULL, 0)`. **Thread:** event-loop
(`event_signal.c:18-50…`, `rule.c` matching path, `window_manager.c:180-205`).
**Rust — this is a real decision point.** The regexes are compiled with `regcomp(…, REG_EXTENDED)`
only (`message.c:2641,2651,2661,2671,2895,2903`; `REG_EXTENDED` = 1, no `REG_ICASE`, no
`REG_NOSUB`) and freed with `regfree` (`rule.c:209-212`, `event_signal.c:360-361`). The `regex`
crate is **not** POSIX ERE: leftmost-first vs POSIX leftmost-longest, different treatment of
`\`-escapes and of an empty alternation, and it rejects some patterns POSIX accepts. User
`yabairc` files in the wild contain arbitrary EREs, so switching engines is an observable
behaviour change. **Recommendation: FFI to the system engine.** `regex_t` on macOS is
`{ int re_magic; size_t re_nsub; const char *re_endp; struct re_guts *re_g; }`
(SDK `_regex.h:113-118`) → `#[repr(C)] struct regex_t { re_magic: c_int, re_nsub: usize,
re_endp: *const c_char, re_g: *mut c_void }` (32 bytes), with `regcomp`/`regexec`/`regfree`
declared by hand if the `libc` crate does not expose them for `*-apple-darwin`. Wrap it in a
`PosixRegex` type with `Drop` calling `regfree`, which also fixes the `valid` bool being carried
separately (it becomes `Option<PosixRegex>`). The `match` argument must be NUL-terminated —
callers pass arena strings from `ts_string_copy`, so phase 2 needs a `CString`/`&CStr` at this
boundary.

#### `clampf_range(float value, float min, float max) -> float` — `helpers.h:581-586`
NaN-propagating-by-omission clamp (`if (v<min) … if (v>max) … return v`): a NaN falls through and
is returned unchanged. Rust's `f32::clamp` **panics** on a NaN bound and returns NaN for a NaN
value; `.max().min()` instead returns a bound. **Translate the three `if`s literally** rather than
calling `clamp`. **Thread:** event-loop (`window_manager.c:312,315,390,395`).

#### `cgimage_restore_alpha(CGImageRef image) -> CGImageRef` — `helpers.h:588-675`
Draws the captured image into a fresh 32-bit premultiplied-last bitmap context, then un-premultiplies
in place with SSE2 (`helpers.h:600-604,614-639`) or NEON (`helpers.h:605-609,640-665`), and returns
`CGBitmapContextCreateImage`. **Caller owns the returned CGImage** — released at
`window_manager.c:492` in `window_manager_destroy_window_proxy`. The `calloc`'d scratch buffer is
freed at `helpers.h:673`; the colour space and context are released.
**Thread:** proxy-build pthreads (`window_manager.c:525`), falling back to the event-loop thread
when `pthread_create` fails (`window_manager.c:669`). Several of these run concurrently.
**External:** `CGImageGetWidth/Height`, `CGColorSpaceCreateDeviceRGB`, `CGBitmapContextCreate`,
`CGColorSpaceRelease`, `CGContextDrawImage`, `CGBitmapContextCreateImage`, `CGContextRelease`,
`calloc`, `free`, plus intrinsics.

**Rust:** `core::arch::x86_64` / `core::arch::aarch64` intrinsics are all available and map 1:1
(`_mm_loadu_si128` → `_mm_loadu_si128`, `vld1q_s32` → `vld1q_s32`, …) inside
`#[cfg(target_arch = …)] unsafe` blocks, with `#[target_feature(enable = "sse2")]` (baseline on
x86_64, so it can be omitted) and nothing needed for NEON on aarch64. Keep both paths; a scalar
fallback would change output because `_mm_cvtps_epi32` / `vcvtnq_s32_f32` round to nearest while a
scalar `as u8` in Rust truncates (and saturates).
**Two defects to carry knowingly, not to "fix" silently:**
1. `for (int i = 0; i < height*width; i += 4)` (`helpers.h:613`) processes 4 pixels per iteration
   with no tail handling: when `width*height % 4 != 0` the final iteration reads *and writes* up to
   12 bytes past the `calloc(height*pitch, 1)` buffer. Any window whose pixel count is not a
   multiple of 4 hits this. A Rust port using `chunks_exact_mut(4)` silently drops the last 1-3
   pixels instead (different output, no overflow); using `chunks_mut(4)` with a scalar tail changes
   the last pixels' rounding. Phase 2 should reproduce the SIMD loop over an over-allocated buffer
   (e.g. allocate `round_up(w*h, 4) * 4` bytes) and note the deviation, or keep a tail loop — pick
   one and record it.
2. `r/a` with `a == 0` yields inf/NaN; the lane is discarded by the `a > 0` mask
   (`helpers.h:620,646`), so the garbage never lands. With intrinsics this is identical in Rust.
Also: `height*width` is `int` (`helpers.h:613`); a >2 Gpixel image would overflow. Use `usize`.
`pitch = width * 4` likewise.

### 1.5 Callbacks registered with the OS from this file

**None.** `helpers.h` registers nothing. It is *called from* callbacks owned by other files:
`application.c:12-20` (AXObserver, main thread), `event_loop.c:553,559,1441` (main thread),
`mouse_handler.c:120-126` (CGEventTap, main thread), `mission_control.c:24` (SLS notify proc, main
thread), `window_manager.c:537` (CVDisplayLink, its own thread). Phase 2 must therefore assume
every function here can be entered from several threads; none of them touch mutable global state,
which is why that is safe today, and the Rust port must keep them stateless (no `static mut`
caches, no lazily-initialised buffers except immortal ones like the `CFSTR` replacement).

### 1.6 C-pattern catalogue → Rust (`helpers.h`)

| pattern | example | Rust translation | silent-change risk |
|---|---|---|---|
| X-macro list generating enum + table + switch | `helpers.h:4-25,29-31,37-39` + `window_manager.c:551-553` | one `enum` + `impl` with `name()`/`apply()` | C switch has no `default` → uninitialised `mt`; Rust match is total |
| designated-initialiser table with holes | `layer_str`, `helpers.h:175-181` | `match` or `[Option<&str>; 6]` | `""` instead of `NULL` for indices 1,2 |
| `static const char *[]` literal tables | `helpers.h:35,173,175` | `const [&'static str; N]` | none |
| `CFSTR` compile-time constant | `helpers.h:171` | `OnceLock<CFStringRef>` built once with `CFStringCreateWithCStringNoCopy` + `kCFAllocatorNull` | creating it per call would be a hot-path regression |
| CF Create/Copy → caller releases | `helpers.h:328-331,335-341,344-359,361-371,511-522` | RAII wrapper (`core-foundation`'s `CFType`, or a local `CfOwned<T>` with `Drop`) | forgetting `CFRelease` leaks; double-wrapping a Get-rule object over-releases. `CFSTR` keys must **not** be wrapped |
| malloc'd string returned to caller | `helpers.h:373-385,397-406` | `String` | C returns NULL on OOM; Rust aborts — acceptable, note it |
| bump-arena string (`ts_alloc_unaligned`) | `helpers.h:283,364,390` | `String`, or a `bumpalo`-backed `&'arena str` if profiling demands | arena memory dies at `ts_reset()` (`event_loop.c:1671`); Rust lifetimes cannot express that, so owned data is the safe default |
| sentinel return (`NULL` = "nothing to do") | `helpers.h:280` | `Option<String>` / `Cow` | turning it into `Some("")` breaks `rule.c:13-16` |
| out-param + bool return | `helpers.h:183-187` (`socket_open`) | return `Option`/`Result`, or an owned `UnixStream` | — |
| uninitialised struct handed to the kernel | `helpers.h:191-195` (`sockaddr_un`, `sun_len` never set) | zero-init; keep `sizeof` as the length | none observed |
| hand-built mach message, `#pragma pack(4)` layout | `helpers.h:206-222` | `#[repr(C, packed(4))]` + `const_assert!(size_of == 44)` | **natural alignment gives 48 bytes and a misparsed message** |
| type-punning via pointer cast | `helpers.h:152-153` (`AbsoluteTime`/`Nanoseconds`) | `clock_gettime_nsec_np` or `to_ne_bytes`/`transmute` | returning mach ticks instead of ns changes all the ms thresholds |
| private-struct field read at a fixed offset | `helpers.h:508` | `unsafe` pointer `add(0x10)` cast | any change here is a crash, not a warning |
| statement macro wrapping a block | `helpers.h:524-530` | closure + `Drop` guard | closure must restore on unwind, which the macro does not |
| stack VLA sized by runtime count | `helpers.h:346` | `SmallVec`/`Vec` | VLA can blow the stack for large window lists |
| `void *` + element size + count | `helpers.h:344-359` | generic over `T: Copy`, take `&[T]` | — |
| arch-conditional SIMD intrinsics | `helpers.h:600-666` | `core::arch::{x86_64,aarch64}` under `cfg` | scalar fallback changes rounding; see §1.4 |
| signed `char` range test as a UTF-8 guard | `helpers.h:273,308` | operate on `u8` and test `< 0x20` | `u8` makes 0x80-0xff pass the `<= 0x1f` test only if you forget the sign — do not port the `>= 0x00` half literally |
| `int` lengths/areas | `helpers.h:282,389,399,613` | `usize` | overflow only at absurd sizes |
| `float` result of `double` math | `helpers.h:545,553,566-568` | compute `f64`, `as f32` | changing the truncation point flips signs near degeneracy |
| deprecated-API pragma pairs | `helpers.h:147-155,532-540` | nothing (Rust has no equivalent warning) | — |
| `fork` + `execvp` fire-and-forget | `helpers.h:477-486` | `Command::spawn`, drop the `Child` | SIGPIPE/SIGCHLD disposition (see §1.4) |
| POSIX `regex.h` | `helpers.h:573-579` | FFI to `regcomp`/`regexec`/`regfree` | the `regex` crate is not POSIX ERE |
| `exit()` from a library-ish helper | via `error()` (`log.h:26-35`), reached from `helpers.h` only through `warn`/`notify` | `-> !` wrapper around `process::exit` | `helpers.h` itself never exits; `service.h` does |

### 1.7 Comments to carry over

**None.** `helpers.h` contains zero comments (verified: no `//` and no `/*` in the file). Only
`#pragma clang diagnostic` pairs at `helpers.h:147-155` and `:532-540`, which have no Rust
counterpart and should simply disappear.

---

## 2. `src/misc/service.h` (314 lines)

### 2.1 Purpose

Implements the five `--*-service` CLI verbs by writing a launchd `LaunchAgent` plist into
`~/Library/LaunchAgents/com.asmvik.yabai.plist` and driving `/bin/launchctl` through `posix_spawn`.
Everything here runs **once, on the main thread, before any other thread exists**, and each entry
point's return value is passed straight to `exit()` (`yabai.c:224-241`). Errors go through
`error()` (`log.h:26`) which prints to stderr and `exit(EXIT_FAILURE)`.

### 2.2 Constants and macros

| name | line | value / shape |
|---|---|---|
| `_PATH_LAUNCHCTL` | 4 | `"/bin/launchctl"` |
| `_NAME_YABAI_PLIST` | 5 | `"com.asmvik.yabai"` |
| `_PATH_YABAI_PLIST` | 6 | `"%s/Library/LaunchAgents/com.asmvik.yabai.plist"` — one `%s` (home) |
| `_YABAI_PLIST` | 8-42 | the full plist template, **four** `%s`: executable path, `PATH`, user, user |

The plist sets `Label`, `ProgramArguments`, `EnvironmentVariables.PATH`, `RunAtLoad`,
`KeepAlive = {SuccessfulExit: false, Crashed: true}`, `StandardOutPath = /tmp/yabai_<user>.out.log`,
`StandardErrorPath = /tmp/yabai_<user>.err.log`, `ProcessType = Interactive`, `Nice = -20`.
Lines 29-31 contain literal tab characters inside the XML indentation — carry the template
**byte for byte** (a raw Rust string literal `r#"…"#` with `{}` placeholders, or `format!` with
positional args `{0} {1} {2} {2}`).

No structs, enums, unions, typedefs, bit flags or tagged unions in this file. No globals, no
statics, no function-local statics.

### 2.3 Functions

All of them: **main thread, single-threaded, CLI-only.** Determined from the only call chain,
`main` → `parse_arguments` (`yabai.c:264`) → `service_*` (`yabai.c:223-241`) → `exit`.

#### `safe_exec(char *const argv[], bool suppress_output) -> int` — `service.h:53-78`
`posix_spawn_file_actions_init`, optionally redirecting stdout+stderr to `/dev/null`
(`O_WRONLY|O_APPEND`, two separate `addopen` calls), then `posix_spawn(&pid, argv[0], &actions,
NULL, argv, NULL)` — note **`posix_spawn`, not `posix_spawnp`** (no PATH search; `argv[0]` is
always the absolute `/bin/launchctl`) and **`envp == NULL`**. Returns 1 if `posix_spawn` failed,
then `waitpid` in an `EINTR` loop with `usleep(1000)`, returning 1 for signalled/stopped and
`WEXITSTATUS(status)` otherwise.
**Leaks:** `posix_spawn_file_actions_destroy` is never called (process exits immediately).
**`status` is reused** for both the spawn result and the wait status (`service.h:64,67`).
**External:** `posix_spawn_file_actions_init/addopen`, `posix_spawn`, `waitpid`, `usleep`.
**Rust:**
```rust
fn safe_exec(argv: &[&str], suppress_output: bool) -> i32
```
over `std::process::Command` with `.stdout(Stdio::null()).stderr(Stdio::null())` and `.status()`
(which retries `EINTR` itself). Map the result: `status.code().unwrap_or(1)` gives exactly the C
mapping (signalled → `code()` is `None` → 1). Two things to decide: `Stdio::null()` opens
`/dev/null` `O_RDWR` rather than `O_WRONLY|O_APPEND` (no observable difference for launchctl), and
`Command` **inherits the environment** while C passes `envp == NULL` — see §5, open question 1.

#### `populate_plist_path(void) -> char *` — `service.h:80-99`
`NSHomeDirectoryForUser(NULL)` (ObjC, bridged to `CFStringRef` under `-fno-objc-arc` so `__bridge`
is a no-op) → `cfstring_copy` → `malloc(strlen(_PATH_YABAI_PLIST)-2 + strlen(home) + 1)` (the `-2`
drops the `%s`) → `memset` 0 → `snprintf`. `error()`s out if the home dir or the allocation fails.
**Returned buffer is never freed by any caller** (`service.h:173,184,195,252,266`) — intentional,
the process exits. `home` itself is also leaked.
**Rust:** `String` via `format!`; both leaks vanish with no behaviour change.
`NSHomeDirectoryForUser(nil)` is documented to behave like `NSHomeDirectory()`; keep the same call
through `objc2-foundation` rather than substituting `$HOME` (they differ under `sudo`/launchd) —
see §5, open question 2.

#### `populate_plist(int *length) -> char *` — `service.h:101-130`
Reads `$USER` and `$PATH` (each `error()`s if unset), `_NSGetExecutablePath` into a 4096-byte stack
buffer (`error()`s on failure), sizes with `strlen(_YABAI_PLIST)-8 + strlen(exe) + strlen(path) +
2*strlen(user) + 1` (the `-8` drops four `%s`), `malloc` + `memset` + `snprintf`, sets
`*length = size-1`. Result never freed.
**External:** `getenv`, `_NSGetExecutablePath` (`<mach-o/dyld.h>`), `malloc`, `snprintf`.
**Rust:** `format!` + `s.len()`. **Use `_NSGetExecutablePath` directly, not
`std::env::current_exe()`** — `current_exe()` canonicalises through `realpath`, so a yabai launched
via a symlink (Homebrew's `/opt/homebrew/bin/yabai`) would get a *different* `ProgramArguments`
string baked into the plist than the C version does.

#### `ensure_directory_exists(char *yabai_plist_path)` — `service.h:132-153`
Temporarily truncates the path at the last `/` (writing into the caller's buffer), `mkdir(…, 0755)`
if missing, then restores the slash. Ignores `mkdir` errors. Only creates **one** level.
**Rust:** `Path::parent()` + `fs::create_dir` (not `create_dir_all`, which would create more than
the C does — although in practice `~/Library` always exists). No in-place mutation needed; the two
comments explaining the mutation (§2.5) then have nothing to attach to — carry them anyway if the
literal translation keeps the trick, drop them if `Path::parent()` is used. Phase 2 should prefer
`Path::parent()` and drop both comments, since a comment that describes code that no longer exists
is worse than no comment.

#### `service_install_internal(char *yabai_plist_path) -> int` — `service.h:155-169`
`populate_plist` → `ensure_directory_exists` → `fopen(path, "w")` (returns 1 on failure) →
`fwrite(plist, length, 1, handle)` → result is `bytes == 1 ? 0 : 1` → `fclose`.
Note `fwrite` with `size = length, nitems = 1`, so a short write returns 0 items → result 1.
**Rust:** `fs::write(path, contents)` — but that maps a *partial* write to success where C maps it
to 1. `File::create` + `write_all` + map any error to 1 is the faithful version.

#### `service_install(void) -> int` — `service.h:171-180`
`error()`s if the plist already exists, else `service_install_internal`.

#### `service_uninstall(void) -> int` — `service.h:182-191`
`error()`s if the plist is missing, else `unlink(path) == 0 ? 0 : 1`.

#### `service_start(void) -> int` — `service.h:193-248`
Installs the plist first if missing (warn + install; `error()` if the install fails). Builds
`service_target = "gui/<uid>/com.asmvik.yabai"` and `domain_target = "gui/<uid>"` into
`char[MAXLEN]` (512) with `getuid()`. Probes `launchctl print <service_target>` with output
suppressed; if that fails (not bootstrapped) → `launchctl enable <service_target>` then
`launchctl bootstrap <domain_target> <plist_path>` (returning the latter's status); if it succeeds
→ `launchctl kickstart <service_target>`.
Note the inner `args` arrays shadow the outer one (`service.h:215,227,235,245`) — a plain
`&[&str]` per call in Rust, no shadowing needed.
**`getuid()` returns `uid_t` (u32) and is printed with `%d`** — harmless, but in Rust use
`{}` on the `u32`.

#### `service_restart(void) -> int` — `service.h:250-262`
`error()`s if the plist is missing; `launchctl kickstart -k <service_target>`.

#### `service_stop(void) -> int` — `service.h:264-312`
`error()`s if the plist is missing. Same bootstrapped probe; if not bootstrapped →
`launchctl kill SIGTERM <service_target>`; if bootstrapped →
`launchctl bootout <domain_target> <plist_path>` (status ignored) then
`launchctl disable <service_target>` (status returned).

### 2.4 Callbacks registered with the OS

**None.** No observers, taps, handlers, signals or blocks. The only child processes are the
`launchctl` invocations, waited for synchronously.

### 2.5 C-pattern catalogue → Rust (`service.h`)

| pattern | example | Rust translation | silent-change risk |
|---|---|---|---|
| multi-line string-literal concatenation as a template | `service.h:8-42` | raw string + `format!` positional args | the template contains literal tabs (`:29-31`) — copy bytes exactly |
| manual `snprintf` sizing (`strlen(fmt) - 2*n_placeholders + …`) | `service.h:89,119` | `format!` | off-by-one in the C sizing would truncate; `format!` cannot |
| `malloc` + `memset` + `snprintf`, never freed | `service.h:90-96,120-126` | `String` | none (process exits either way) |
| out-param length beside the buffer | `service.h:101,127` | return `String`, use `.len()` | `*length = size-1` is the strlen, not the allocation |
| in-place path truncation at the last `/` | `service.h:141-152` | `Path::parent()` | mutating a borrowed `&str` is not expressible; do not fight it |
| fixed `char[MAXLEN]` for formatted targets | `service.h:205,208,257,271,274` | `String`/`format!` | `MAXLEN` is 512 and the targets are short; no truncation today |
| `const char *const argv[]` NULL-terminated, cast away const at the call | `service.h:215-216` etc. | `&[&str]` + `Command::args` | the trailing `NULL` is the C array terminator, not an argument |
| block-scoped shadowed `args` arrays | `service.h:227,235` | separate bindings | — |
| `posix_spawn` + `waitpid(EINTR)` loop | `service.h:53-78` | `Command::status()` | envp NULL vs inherited (§5) |
| exit-status triage (`WIFSIGNALED`/`WIFSTOPPED`/`WEXITSTATUS`) | `service.h:71-77` | `status.code().unwrap_or(1)` | identical mapping |
| `error()` = print + `exit(EXIT_FAILURE)` mid-function | `service.h:86,92,105,110,116,122,176,187,201,254,268` | `fn error(args) -> !` wrapping `process::exit(1)` | C's `error` is **not** `noreturn`-annotated, so the code after it is reachable to the compiler; in Rust `!` makes the control flow explicit and the `return` after it disappears |
| return code straight into `exit()` | `yabai.c:224-241` | `process::exit(service_install())` | `service_*` returning `-1` (`sa.m:365`-style) would become exit status 255 — not the case here |
| `FILE*` + `fwrite` | `service.h:161-166` | `File::create` + `write_all` | `fs::write` hides partial writes |

### 2.6 Existing comments — carry these over verbatim

All ten blocks are `NOTE(asmvik):` prose that explains launchd semantics, i.e. exactly the kind of
comment that earns its place. Line ranges and text:

| lines | text |
|---|---|
| 44-51 | `A launchd service has the following states:` / `1. Installed / Uninstalled` / `2. Active (Enable / Disable)` / `3. Bootstrapped (Load / Unload)` / `4. Running (Start / Stop)` |
| 134-139 | `Temporarily remove filename. We know the filepath will contain a slash, as it is controlled by us, so don't bother checking the result..` |
| 148-150 | `Restore original filename.` |
| 211-213 | `Check if service is bootstrapped` |
| 220-225 | `Service is not bootstrapped and could be disabled. There is no way to query if the service is disabled, and we cannot bootstrap a disabled service. Try to enable the service. This will be a no-op if the service is already enabled.` |
| 230-233 | `Bootstrap service into the target domain. This will also start the program **iff* RunAtLoad is set to true.` |
| 239-243 | `The service has already been bootstrapped. Tell the bootstrapped service to launch immediately; it is an error to bootstrap a service that has already been bootstrapped.` |
| 277-279 | `Check if service is bootstrapped` |
| 286-290 | `Service is not bootstrapped, but the program could still be running an instance that was started **while the service was bootstrapped**, so we tell it to stop said service.` |
| 296-304 | `Service is bootstrapped; we stop a potentially running instance of the program and unload the service, making it not trigger automatically in the future.` / `This is NOT the same as disabling the service, which will prevent it from being boostrapped in the future (without explicitly re-enabling it first).` |

Keep the `NOTE(asmvik):` prefix and the original wording including the typos (`boostrapped`,
`**iff*`). Blocks 134-139 and 148-150 only survive if the literal in-place-truncation translation
survives (§2.3).

---

## 3. `src/misc/extern.h` (97 lines)

### 3.1 Purpose

The private-API surface: one callback typedef, two `static` function pointers resolved at runtime
by Mach-O symbol-table scanning, and 92 `extern` prototypes for undocumented SkyLight /
CoreGraphics / ColorSync / HIServices / libsystem_kernel entry points that the daemon links
against directly. It declares no storage other than the two pointers and contains no logic.
It has **no include guard**.

### 3.2 Declarations

#### `CONNECTION_CALLBACK(name)` — `extern.h:1`
```c
#define CONNECTION_CALLBACK(name) void name(uint32_t type, void *data, size_t data_length, void *context, int cid)
```
#### `typedef connection_callback` — `extern.h:2`
A **function type** (not a pointer-to-function) typedef; `SLSRegisterConnectionNotifyProc`
(`extern.h:12`) takes `connection_callback *`.
- Sole implementation: `mission_control.c:7` (`static CONNECTION_CALLBACK(connection_handler)`).
- Registration sites: `yabai.c:322,323,326,329,330,333` — six registrations of the *same* function
  for event types 1327, 1328, 1204, 808, 1202, 804, depending on macOS version.
- **Thread it fires on:** the main run loop. Determined from: registration happens on the main
  thread before `[NSApp run]` (`yabai.c:350`), SkyLight delivers connection notifications through a
  CFRunLoop source on the registering thread, and the handler itself does nothing but
  `event_loop_post` (queueing to the event-loop thread) and one `__ATOMIC_RELEASE` store
  (`mission_control.c:24`) that is consumed with an acquire load on the event-loop thread
  (`event_loop.c:363`) — an atomic handoff that only makes sense across threads.
- **Context pointer:** `NULL` at all six registrations, so its lifetime is moot; `data` is owned by
  SkyLight and valid only for the duration of the call (`mission_control.c:9-22` `memcpy`s out of
  it immediately), `data_length` is unused by the handler.
**Rust:**
```rust
pub type ConnectionCallback = unsafe extern "C" fn(u32, *mut c_void, usize, *mut c_void, c_int);
```
and `SLSRegisterConnectionNotifyProc(cid: c_int, handler: ConnectionCallback, event: u32, context: *mut c_void) -> CGError`.
Because the registration outlives everything, the handler must be a plain `unsafe extern "C" fn`
with no captured state — matching the C exactly. Mark it `#[unsafe(no_mangle)]`-free; the address
is taken directly.

#### `static mach_port_t (*CGSGetConnectionPortById)(int)` — `extern.h:4`
#### `static int64_t (*SLSPerformAsynchronousBridgedWindowManagementOperation)(void *)` — `extern.h:5`
| | |
|---|---|
| initial value | `NULL` (BSS) |
| written | main thread, once, at `yabai.c:148-149` via `macho_find_symbol(…SkyLight…)` — these two are *not* exported (my `dlsym(RTLD_DEFAULT, "CGSGetConnectionPortById")` probe returns not-found), hence the Mach-O symbol-table scan; the second is a C++-mangled internal symbol `__ZL54SLSPerformAsynchronous…` |
| read | event-loop thread (`space_manager.c:667,672,688,693`), proxy-build pthreads (`window.c:946,956` via `window_sub_level` ← `window_manager.c:514`), event-loop thread (`window.c:956` on the ordinary path) |
| synchronisation | **none** — publication relies on the write happening at `yabai.c:148` before any thread is created (`event_loop_begin` is `yabai.c:291`). Both are also null-checked before use (`space_manager.c:667`, `window.c:956`) |

**Rust:** `static CGS_GET_CONNECTION_PORT_BY_ID: OnceLock<Option<unsafe extern "C" fn(c_int) -> mach_port_t>>`
(or an `AtomicPtr<c_void>` with `Relaxed` load and a transmute at the call site). Do **not** use
`static mut` — reading it from three threads would be UB even though the C is benign in practice.
The `Option<fn>` shape preserves the null check for free.

#### The 92 `extern` prototypes — `extern.h:6-97`
I resolved every one with `dlsym(RTLD_DEFAULT, …)` + `dladdr` against the same framework set the
makefile links (`-framework Carbon -framework Cocoa -framework CoreServices -framework CoreVideo
-framework SkyLight`, `-F/System/Library/PrivateFrameworks`). **All 92 resolve**, distributed as:

| image | count | symbols |
|---|---|---|
| `SkyLight` (private framework) | 84 | every `SLS*`/`SLPS*`/`SLWindowContextCreate` in the file |
| `HIServices` (inside ApplicationServices) | 5 | `_AXUIElementCreateWithRemoteToken`, `_AXUIElementGetWindow`, `CoreDockGetAutoHideEnabled`, `CoreDockGetOrientationAndPinning`, `CoreDockSendNotification` |
| `CoreGraphics` | 2 | `CGRegionCreateEmptyRegion`, `CGSNewRegionWithRect` |
| `ColorSync` | 1 | `CGDisplayCreateUUIDFromDisplayID` |
| `libsystem_kernel.dylib` | 1 | `mig_get_special_reply_port` |

(`SameProcess` and `AXIsProcessTrustedWithOptions`, used by `helpers.h`, also live in HIServices
but are declared by the public SDK headers, not here.)

**Rust FFI blocks** — one per image, with the link attributes; `build.rs` adds
`cargo:rustc-link-search=framework=/System/Library/PrivateFrameworks`:
```rust
#[link(name = "SkyLight", kind = "framework")]      unsafe extern "C" { … 84 fns … }
#[link(name = "ApplicationServices", kind = "framework")] unsafe extern "C" { … 5 fns … }
#[link(name = "CoreGraphics", kind = "framework")]  unsafe extern "C" { … 2 fns … }
#[link(name = "ColorSync", kind = "framework")]     unsafe extern "C" { CGDisplayCreateUUIDFromDisplayID }
unsafe extern "C" { fn mig_get_special_reply_port() -> mach_port_t; }   // libSystem, always linked
```
Linking `ApplicationServices` (or `Carbon`, which re-exports it) is enough for the HIServices five;
the C build gets them via `-framework Carbon`/`-framework CoreServices`.

### 3.3 Type mapping for the prototypes

| C | Rust | note |
|---|---|---|
| `CGError`, `OSStatus`, `AXError` | `i32` | all are `int32_t`; 0 = success (`kCGErrorSuccess`/`noErr`/`kAXErrorSuccess`) |
| `Boolean` (`CoreDockGetAutoHideEnabled`, `extern.h:52`) | `u8`, compare `!= 0` | Rust `bool` must be exactly 0 or 1; a foreign `Boolean` need not be |
| `bool` (`SLSManagedDisplayIsAnimating`, `extern.h:46`; `SLSSetWindowOpacity`, `:31`) | `bool` | C99 `_Bool` is 0/1 by ABI, so `bool` is sound here |
| `CFTypeRef`, `CFStringRef`, `CFArrayRef`, `CFUUIDRef`, `CFDataRef`, `CFDictionaryRef`, `CGContextRef`, `AXUIElementRef` | `*const c_void` / newtypes | ownership follows the CF naming rule: `SLSCopy*` / `*Create*` return +1 (caller releases), everything else returns +0 |
| `mach_port_t` | `u32` | |
| `pid_t` | `i32` | |
| `uint64_t *tags` (`extern.h:26,28,29,58`) | `*mut u64` | `tag_size` at `:26` is **64** (bits, not bytes) at `window_manager.c:472` — keep the literal |
| `ProcessSerialNumber` **by value** (`SLSSpaceSetFrontPSN`, `extern.h:55`) | `#[repr(C)] struct ProcessSerialNumber { high_long_of_psn: u32, low_long_of_psn: u32 }` passed by value | an 8-byte aggregate: one integer register on both x86-64 SysV and AAPCS64; `repr(C)` by value is correct |
| `ProcessSerialNumber *` (`:76,78,80,81,82`) | `*mut ProcessSerialNumber` | |
| `CGAffineTransform` **by value** (`:87,90`) | `#[repr(C)] struct CGAffineTransform { a,b,c,d,tx,ty: f64 }` | 48 bytes → MEMORY class on x86-64, indirect on AAPCS64; `repr(C)` by value matches clang |
| `CGRect`/`CGPoint`/`CGSize` by value and by pointer | `#[repr(C)]` structs of `f64` | `CGFloat` is `f64` on both 64-bit targets only |
| `uint8_t *bytes` (`SLPSPostEventRecordTo`, `:82`) | `*mut u8` | the caller passes the 0x100-byte `g_event_bytes` buffer (`yabai.c:141`) |
| `CFNumberType`, `CFIndex` | `i32` (`CFNumberType` is an enum) / `isize` | |
| `double unused1, unused2` (`:47`) | `f64` | keep the parameter names |
| `int zero, int one, int zero_again` (`:83`) | `c_int` | the names document the required constants; keep them |

Argument-order oddity worth carrying: `SLSGetRevealedMenuBarBounds(CGRect *rect, int cid, uint64_t sid)`
(`extern.h:49`) takes the out-param *first*, unlike every neighbouring function.

### 3.4 Callbacks registered with the OS

Only one is *declared* here: `SLSRegisterConnectionNotifyProc` (`extern.h:12`); see §3.2 for its
registration sites, thread and context lifetime. Nothing is registered by this file itself.

### 3.5 C-pattern catalogue → Rust (`extern.h`)

| pattern | example | Rust translation | silent-change risk |
|---|---|---|---|
| macro that generates a function signature | `extern.h:1-2` | `type ConnectionCallback = unsafe extern "C" fn(…)` | a `fn` type vs `Option<fn>` — the C parameter is a plain pointer and is never NULL |
| function-type typedef used as `T *` | `extern.h:12` | `ConnectionCallback` (already a pointer in Rust) | — |
| runtime-resolved private symbol in a `static` pointer | `extern.h:4-5` + `yabai.c:148-149` | `OnceLock<Option<fn>>` set before threads start | `static mut` read from 3 threads = UB |
| header with no include guard | whole file | modules have no such problem | — |
| `extern` declarations of undocumented ABI | `extern.h:6-97` | `unsafe extern "C"` blocks per framework | wrong integer width or a missing `repr(C)` silently corrupts arguments |
| struct passed by value across FFI | `:55,87,90` | `#[repr(C)]` + by value | `repr(Rust)` would reorder fields |
| out-params everywhere (`CGError` + `T *out`) | `:10,13,14,16,25,…` | keep raw `*mut T` at the boundary; wrap into `Result<T, CGError>` one layer up | wrapping at the boundary makes the 92 declarations diverge from the C header and hides which call is which |

---

## 4. Recommended phase-2 shape for these three files

Phase 2 rule is "one `src/foo.c` + `src/foo.h` → one Rust module", and these are headers with no
`.c`, so:

- `src/misc/helpers.h` → `src/misc/helpers.rs` — one module, functions keeping their C names
  (`ts_string_escape`, `cfstring_copy`, `ax_window_pid`, …) so phase 3 can split them by concept
  (easing / strings / sockets / fs / ax / geometry / image) without a second round of renaming.
- `src/misc/service.h` → `src/misc/service.rs` — `safe_exec`, `populate_plist_path`,
  `populate_plist`, `ensure_directory_exists`, `service_install*`, `service_uninstall`,
  `service_start`, `service_restart`, `service_stop`, plus the plist template constants.
- `src/misc/extern.h` → `src/misc/extern_sys.rs` (`extern` is a Rust keyword; `r#extern` is legal
  but ugly) — the `unsafe extern "C"` blocks grouped by image, the `ConnectionCallback` type, the
  two `OnceLock` function pointers with their `macho_find_symbol` initialisers called from the
  `yabai.rs` startup path.

Crates these three files justify: `libc` (sockets, stat, fork/exec, regex FFI, uid), `mach2` (mach
message types and `mach_msg`) or hand-rolled `repr(C, packed(4))` types, `core-foundation-sys`
(CF types and `CFRelease`; the higher-level `core-foundation` wrappers are optional),
`objc2` + `objc2-foundation` (only for `NSHomeDirectoryForUser` in `service.h`). **No `regex`
crate** (§1.4). `bumpalo` only if the arena is kept as an arena.

Two `const` assertions are worth writing in phase 2 because they encode facts verified here:
`size_of::<OolMessage>() == 44` and `size_of::<RegexT>() == 32`.

---

## 5. Open questions for whoever owns the decision

1. **`posix_spawn(…, envp = NULL)` at `service.h:64`.** Darwin's behaviour for a NULL `envp` is not
   documented (empty environment vs inherited `environ`). `launchctl` mostly does not care, but
   `Command` in Rust inherits by default, so if Darwin passes an empty environment the Rust port
   changes what `launchctl` sees. Worth a one-line experiment (`posix_spawn` `/usr/bin/env` with
   NULL envp) before phase 2 writes `service.rs`.
2. **`NSHomeDirectoryForUser(NULL)` at `service.h:82`.** Do we keep an ObjC dependency in the
   service path just for this, or accept `getpwuid(getuid())->pw_dir`? They agree in every normal
   session; they can differ under `sudo` or a sandboxed launch. Keeping the ObjC call is the
   faithful choice, and `service.h` is the only place in these three files that needs ObjC at all.
3. **`cgimage_restore_alpha`'s 1-3 pixel overrun** (`helpers.h:613`, §1.4). Reproduce the overrun
   on an over-allocated buffer (bit-identical output, no UB), or handle the tail (different output
   for the last pixels of odd-sized windows)? This is a behaviour decision, not a translation one.
4. **Arena vs owned strings.** `ts_string_escape` / `ts_cfstring_copy` / `ts_string_copy` return
   memory that dies at `ts_reset()` on the event-loop thread. Owned `String` is the safe default
   and I recommend it, but it changes the allocation profile of the query path (`message.c` builds
   large JSON responses). If the arena is kept, the whole `ts.h` module needs a lifetime story
   first, and these three functions must follow it.
5. **`CFSTRINGNUM32` (`helpers.h:321`) has no callers.** Drop it in phase 2, or keep it as dead
   code for fidelity?
6. **Easing float/double promotion (§1.4).** Pick one rule for all 21 curves and record it, so the
   animation output is reproducible rather than accidentally per-function.
