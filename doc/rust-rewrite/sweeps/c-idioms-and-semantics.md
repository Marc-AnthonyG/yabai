# Sweep: C idiom catalogue and semantic traps

Phase 1 cross-cutting sweep over the whole yabai daemon source (`src/**`, excluding `src/osax/**`
which stays in C/ObjC). This document is the **shared contract for every phase-2 translator**.
Ten translators working in parallel on ten different files must all pick the same Rust form for the
same C idiom, or the modules will not fit together.

Scope read: `src/manifest.m`, `src/yabai.c`, `src/misc/*.h` (all 13), and every `.c`/`.m` under
`src/` except `src/osax/`. `src/osax/common.h` is read because the daemon shares the SA wire
protocol with it.

Rule for the whole rewrite, restated so no translator has to look it up: **only comments that
already exist in the C source get carried into Rust.** Every "NOTE(asmvik):" block, every
`// @cleanup`, every `:AXBatching` / `:WorstApiEverMade` tag keeps its text verbatim. No new
explanatory comments are added anywhere.

---

## 0. The single most important structural fact

`src/manifest.m` is a **unity build**: it `#include`s every header (`src/manifest.m:45-78`) and then
every `.c`/`.m` file (`src/manifest.m:80-97`) into one translation unit. Consequences that bite the
file-by-file translation:

### 0.1 `static` in a `.c` file does **not** mean "module-private"

Sixteen `static` functions are defined in one `.c` and called from another. A phase-2 translator who
writes `fn` (private) instead of `pub(crate) fn` will break the build for whoever owns the caller.
Full list, verified by cross-referencing definition file against use sites:

| `static` function | defined in | called from |
| --- | --- | --- |
| `update_window_notifications` | `src/event_loop.c:16` | `src/view.c:45`, `src/view.c:111`, `src/yabai.c:341` |
| `mission_control_is_active` | `src/mission_control.c` | `src/display_manager.c`, `src/space_manager.c`, `src/event_loop.c:1015`, `:1067`, `:1119` |
| `scripting_addition_is_sip_friendly` | `src/sa.m:301` | `src/message.c:1304` |
| `area_from_cgrect` | `src/view.c:121` | `src/display_manager.c` |
| `area_max_point` | `src/view.c:126` | `src/display_manager.c` |
| `area_is_in_direction` | `src/view.c:541` | `src/display_manager.c` |
| `area_distance_in_direction` | `src/view.c:563` | `src/display_manager.c` |
| `area_make_pair` | `src/view.c:161` | `src/window_manager.c` |
| `window_node_get_gap` | `src/view.c:156` | `src/window_manager.c` |
| `window_node_get_ratio` | `src/view.c:151` | `src/window_manager.c` |
| `window_node_get_split` | `src/view.c:136` | `src/window_manager.c` |
| `window_node_is_leaf` | `src/view.c` | `src/window_manager.c` |
| `window_node_is_left_child` | `src/view.c` | `src/window.c:594`, `src/window_manager.c` |
| `window_node_is_intermediate` | `src/view.c` | `src/space_manager.c` |
| `window_node_balance` | `src/view.c` | `src/space_manager.c` |
| `window_node_equalize` | `src/view.c` | `src/space_manager.c` |

Same for `static` data: `mission_control_mode_str[]` (`src/mission_control.c:38`) is read from
`src/event_signal.c:338`, and `enum mission_control_mode` itself is **declared in a `.c` file**
(`src/mission_control.c:29-36`) while its global instance lives in `src/yabai.c:37`.

**Canonical rule:** in phase 2, `static` on a C function/global maps to `pub(crate)` by default.
Use private `fn` only when this sweep's table does not list it and a grep confirms a single
use site. Phase 3 tightens visibility; phase 2 must not break the build guessing.

### 0.2 Header-order dependencies do not survive

`src/window.h:140` declares `void window_unknown_serialize(FILE*, uint32_t, uint64_t);` which is
**never defined**. The real function is `window_nonax_serialize` (`src/window.c:121`), called from
`src/window_manager.c:48`. The dead declaration compiles only because nothing references it. Drop it
in Rust; do not create a stub.

### 0.3 Globals

`src/yabai.c:27-52` defines all 21 mutable globals; every `.c` re-declares them with `extern` at the
top of the file (e.g. `src/event_loop.c:1-14`, `src/window_manager.c:1-7`, `src/message.c:6-12`).
There is no locking on most of them; the event loop thread and the main NSApp thread both touch
them. The canonical Rust form for globals is decided by the `entry-and-event-loop` file map; this
sweep only records that **every `extern` line at the top of a `.c` file is a global access, not a
parameter**, and a translator must not "clean it up" into a parameter.

---

## 1. Idiom frequency ranking

Counted over the daemon source (excluding `src/osax`). Ranked by number of occurrences, because
that is the order in which a wrong canonical choice costs the most rework.

| # | Idiom | Occurrences | Section |
| --- | --- | --- | --- |
| 1 | `fprintf(rsp, ...)` JSON / response writing | 201 in serialisers + ~120 in `message.c` | §5.1, §7 |
| 2 | `daemon_fail(rsp, fmt, ...)` failure replies | 306 (`src/message.c`) | §5.1 |
| 3 | `token_equals(token, CONST)` command dispatch | 242 (`src/message.c`) | §5.2 |
| 4 | `*_check_flag` / `*_set_flag` / `*_clear_flag` | 189 across 8 files | §5.7 |
| 5 | `#define COMMAND_*` / `ARGUMENT_*` string constants | ~150 (`src/message.c:14-256`) | §5.3 |
| 6 | `debug(...)` logging behind `g_verbose` | 87 | §5.4 |
| 7 | `get_token(&message)` cursor advance | 80 (`src/message.c`) | §5.2 |
| 8 | `ts_*` temporary-storage allocation & strings | 115 | §5.8 |
| 9 | `buf_len` / `buf_push` / `buf_del` stretchy buffers | 63 | §5.9 |
| 10 | `table_find` / `table_add` / `table_remove` / `table_for` | 58 | §5.10 |
| 11 | `TIME_FUNCTION` / `TIME_BODY` profiler | 58 | §5.19 |
| 12 | `string_equals(a, b)` | 89 | §5.3 |
| 13 | `#pragma clang diagnostic push/ignored/pop` | 59 | §5.28 |
| 14 | pointer-as-integer event context `(void*)(intptr_t)` | 44 | §5.17 |
| 15 | `__atomic_*` / `__sync_*` / `volatile` | 41 | §5.18 |
| 16 | `snprintf` into fixed `char buf[MAXLEN]` | 81 | §5.11 |
| 17 | `goto` cleanup / early-out | 84 (10 files) | §5.16 |
| 18 | X-macro list → enum + string table + dispatch | 6 lists | §5.5 |
| 19 | enum-indexed `static const char *xxx_str[]` tables | 14 tables | §5.6 |
| 20 | sentinel `0` / `-1` / `NULL` meaning "none" | pervasive | §5.15 |
| 21 | `regcomp` / `regexec` / `regfree` | 6 compile sites, 1 exec helper | §5.13 |
| 22 | `sscanf` colon-format parsing | 10 sites | §5.12 |
| 23 | float ↔ int implicit conversion on `CGRect`/`struct area` | ~30 | §5.14 |
| 24 | varargs (`va_list`) | 6 functions | §5.25 |
| 25 | `static inline` helper in a header | 62 (`src/misc/helpers.h`, `src/misc/log.h`, `src/misc/ts.h`, `*.h`) | §5.22 |
| 26 | `assert()` and `error()`/`require()` exit paths | 13 asserts, 22 exit calls | §5.26 |
| 27 | `fork()` + `execvp()` | 3 sites | §5.27 |
| 28 | comparator callbacks (`CFArraySortValues`, table hash/cmp) | 1 + 6 | §5.24 |
| 29 | per-arch `#ifdef __x86_64__` / `__arm64__` | 4 sites | §5.21 |
| 30 | per-OS-version `workspace_is_macos_*()` | 26 call sites | §5.21 |
| 31 | SIMD (`emmintrin.h` / `arm_neon.h`) | 1 function | §5.20 |
| 32 | function-local `static` state | 3 sites | §5.23 |

---

## 2. Naming conventions — mandatory, no exceptions

The goal is that a reader can diff the Rust against the C line by line. Do **not** "improve" names in
phase 2; phase 3 owns restructuring.

| C construct | Rust form | Example |
| --- | --- | --- |
| function `snake_case` | identical `snake_case` | `window_manager_find_window` stays `window_manager_find_window` |
| `struct foo_bar` | `struct FooBar` | `struct window_node` → `struct WindowNode` |
| struct field | identical `snake_case` | `->window_count` → `.window_count` |
| `enum foo_bar` | `enum FooBar` | `enum window_node_split` → `enum WindowNodeSplit` |
| enum constant `SPLIT_Y` | CamelCase variant, prefix dropped | `SPLIT_Y` → `WindowNodeSplit::SplitY`? **No** — see below |
| bitflag `enum` constant | associated const in CamelCase | `WINDOW_FLOAT` → `WindowFlag::FLOAT` |
| global `g_foo` | identical `g_foo` (lowercase, `#[allow(non_upper_case_globals)]`) | `g_window_manager` |
| file-scope `static` const array | `static FOO_STR: [&str; N]` | `view_type_str` → `VIEW_TYPE_STR` |
| `#define COMMAND_X "..."` | `const COMMAND_X: &str = "...";` | unchanged spelling |
| macro-generated handler `EVENT_HANDLER(X)` | `fn event_handler_x(...)` | see §5.5 |

**Enum variant spelling, decided:** drop the C prefix and CamelCase the remainder, because the
prefix is already the enum name.

```rust
enum WindowNodeSplit { None, Y, X, Auto }          // SPLIT_NONE, SPLIT_Y, SPLIT_X, SPLIT_AUTO
enum ViewType { Default, Bsp, Stack, Float }        // VIEW_DEFAULT, VIEW_BSP, ...
enum FfmMode { Disabled, Autofocus, Autoraise }     // FFM_DISABLED, ...
enum PurifyMode { Disabled, Managed, Always }       // PURIFY_DISABLED, ...
enum MouseMode { None, Move, Resize, Swap, Stack }  // MOUSE_MODE_NONE, ...
```

Exception: where the C prefix is *not* the enum name, keep enough of it to stay unambiguous —
`SPACE_OP_ERROR_MISSING_SRC` → `SpaceOpError::MissingSrc`, `WINDOW_OP_ERROR_SAME_STACK` →
`WindowOpError::SameStack`, `SIGNAL_WINDOW_FOCUSED` → `SignalType::WindowFocused`,
`SA_OPCODE_WINDOW_MOVE` → `SaOpcode::WindowMove`,
`MISSION_CONTROL_MODE_SHOW_ALL_WINDOWS` → `MissionControlMode::ShowAllWindows`.

**Bitflag enums are not Rust enums.** `enum window_flag` (`src/window.h:108-118`),
`enum window_rule_flag` (`src/window.h:120-126`), `enum rule_flag` (`src/rule.h:8-20`),
`enum rule_effects_flag` (`src/rule.h:22-27`), `enum view_flag` (`src/view.h:185-199`),
`enum mouse_mod` (`src/mouse_handler.h:34-42`), `enum window_property` (`src/window.h:66-71`) and
`enum space_property` (`src/view.h:21-26`) are all OR-ed masks. They become a newtype over the same
width as the C storage field, with associated consts:

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WindowFlag(pub u8);          // struct window:uint8_t flags  (src/window.h:102)
impl WindowFlag {
    pub const SHADOW:     WindowFlag = WindowFlag(0x01);
    pub const FULLSCREEN: WindowFlag = WindowFlag(0x02);
    // ... exactly the C values, in the C order
}
```

Do **not** reach for the `bitflags` crate: it changes `Debug` output and tempts translators into
`.contains()` semantics that differ from C's `a & b` truthiness for multi-bit masks (see §5.7).

---

## 3. Error handling — keep C's shape

**Recommendation: keep `bool` and `Option`, do not introduce `Result`.** Faithfulness beats idiom
here, and the C code has no error *values* to carry — it has success/failure plus an out-parameter.

| C signature | Rust signature |
| --- | --- |
| `bool foo(...)` | `fn foo(...) -> bool` |
| `struct window *find(...)` returning `NULL` for none | `fn find(...) -> Option<&Window>` (or `*mut Window`, see below) |
| `uint32_t did` returning `0` for none | `fn ...() -> u32` returning `0` — **keep the sentinel**, §5.15 |
| `enum window_op_error` | `fn ... -> WindowOpError` — it is an enum result, not an error type |
| `void f(..., int *out_count)` | `fn f(...) -> (T, i32)` or `fn f(..., out_count: &mut i32) -> T` |

Where `Result` **is** warranted, and only there:

1. **Nowhere in the daemon's control flow.** No C function in `src/*.c` returns a rich error.
2. At the new FFI/syscall seam *only* when the Rust standard library forces it — e.g.
   `std::io::Write` on the response socket returns `io::Result`. The C code ignores `fprintf`
   failures entirely (`src/message.c` never checks a return). **Match that**: the response writer
   swallows write errors (§5.1). Do not propagate.
3. `regcomp` (`src/message.c:2641`) returns nonzero on a bad pattern and the C code only tests
   `== 0`. Keep `bool`.

**Do not add `?` operator chains.** They change control flow relative to the C, which frequently
continues after a failure and accumulates `did_parse = false` (e.g. `src/message.c:2596-2804`,
`parse_rule` keeps parsing every key after one fails and reports them all).

### 3.1 Exit-on-error

`src/misc/log.h:26-35` `error(fmt, ...)` prints to stderr and `exit(EXIT_FAILURE)`.
`src/misc/log.h:37-46` `require(fmt, ...)` prints to stderr and `exit(EXIT_SUCCESS)` — note the
**success** exit code; `src/yabai.c:268,272,276` rely on it so a launchd `KeepAlive/SuccessfulExit
= false` service does not respawn.

Canonical Rust (in the shared `log` module):

```rust
macro_rules! error { ($($arg:tt)*) => {{ eprint!($($arg)*); std::process::exit(libc::EXIT_FAILURE); }} }
macro_rules! require { ($($arg:tt)*) => {{ eprint!($($arg)*); std::process::exit(libc::EXIT_SUCCESS); }} }
```

They must be `macro_rules!`, not functions, so the call sites keep the C format string verbatim and
so the compiler sees divergence (`!`) where C sees `exit`. `error(...)` inside an `if` that has no
`else` (e.g. `src/yabai.c:56-58`) relies on never returning.

---

## 4. Numeric conversion rules

C's implicit conversions are load-bearing here. Every phase-2 translator writes the cast explicitly
and **mirrors the C conversion exactly**, including truncation-toward-zero and wrapping.

| C situation | Rust |
| --- | --- |
| `(int)float_expr` | `float_expr as i32` (Rust `as` on float→int **saturates**, C truncates-toward-zero and is UB out of range — in practice identical for the in-range values here; use `as` and note it) |
| `float` → `double` promotion in arithmetic | write the same widening: `f as f64` |
| `*value = *value * 10 + digit` on `int` (`src/message.c:353`) | `*value = value.wrapping_mul(10).wrapping_add(digit)` — C signed overflow is UB; the daemon relies on the wrap in practice |
| `*value = *value * 16 + digit` on `uint32_t` (`src/message.c:375`) | `value.wrapping_mul(16).wrapping_add(digit)` — C unsigned wrap is defined |
| `(void *)(intptr_t) did` / `(uint32_t)(intptr_t) context` | `context as usize as u32` and back, §5.17 |
| `ax_error_str[-result]` (`src/window.c:14`, `src/application.c:52`) | `AX_ERROR_STR[(-result) as usize]` — `AXError` is negative, the table is indexed by its negation (`src/application.h:26-44`) |
| `layer_str[LAYER_BELOW]` where `LAYER_BELOW` is `kCGBackstopMenuLevelKey` | designated-initialiser table, §5.6 |
| `(uint32_t)(rule->effects.flags << 16) \| (uint32_t)rule->flags` (`src/rule.c:60`) | `((effects.flags as u32) << 16) \| (rule.flags as u32)` — note `effects.flags` is `uint16_t` (`src/rule.h:41`) so the C shift promotes to `int` first; write `(effects.flags as u32) << 16` which gives the same bits |
| `flags \|= ~flags` when `flags == 0` (`src/window.c:123`, `src/window.c:411`, `src/display.c:24`, `src/view.c:864`) | `if flags == 0 { flags \|= !flags; }` — this is "no filter means all properties"; it sets **every** bit of the `u64`, not just the defined ones |

`struct area` is four `float`s (`src/view.h:42-48`) while `CGRect` is four `CGFloat` = `f64` on
64-bit. Every `area_from_cgrect` (`src/view.c:121-124`) is an `f64 → f32` narrowing, and every
`CGRect{{node->area.x, ...}}` (e.g. `src/view.c:11`) is an `f32 → f64` widening. Write both casts
explicitly. This is the single largest source of "why is my window one pixel off" if a translator
uses `f64` for `Area`.

---

## 5. Idiom-by-idiom canonical Rust forms

### 5.1 The response protocol: `FILE *rsp`, `FAILURE_MESSAGE`, and a nullable sink

**C shape.** `src/event_loop.c:1632` does `rsp = fdopen(param1, "w")`, hands it to
`handle_message(rsp, message)` (`src/message.c:2979`), then `fflush(rsp); fclose(rsp);`
(`src/event_loop.c:1636-1637`). Success output is plain `fprintf(rsp, ...)`. Failure output goes
through `daemon_fail` (`src/message.c:418-427`):

```c
static inline void daemon_fail(FILE *rsp, char *fmt, ...)
{
    if (!rsp) return;
    va_list ap;
    va_start(ap, fmt);
    fprintf(rsp, FAILURE_MESSAGE);
    vfprintf(rsp, fmt, ap);
    va_end(ap);
}
```

`FAILURE_MESSAGE` is `"\x07"` (`src/misc/macros.h:18`) — a single BEL byte prefix.

**Three facts that must be preserved byte for byte.**

1. The BEL is emitted **once per `daemon_fail` call**, not once per response. A command that fails
   three times (e.g. `parse_rule` accumulating errors, `src/message.c:2596-2804`) writes three BEL
   bytes interleaved with three messages.
2. The client (`src/yabai.c:108-120`) reads in `BUFSIZ` chunks and tests **only `rsp[0]` of each
   chunk**. If a BEL lands mid-chunk it is printed to stdout as a literal `\x07`. Do not "fix" this.
3. `rsp` is legitimately `NULL`: `src/message.c:1706` and `src/message.c:1765` call
   `parse_display_selector(NULL, ...)` / `parse_space_selector(NULL, ...)` to parse an optional
   selector without emitting errors. `daemon_fail`'s `if (!rsp) return;` is that feature.

**Canonical Rust.** One type, in the `message` module, used by every serialiser:

```rust
pub struct Response {
    file: Option<std::fs::File>,   // None == the C NULL rsp
    buf: Vec<u8>,
}

impl Response {
    pub fn from_raw_fd(fd: RawFd) -> Response { /* ... */ }
    pub fn silent() -> Response { Response { file: None, buf: Vec::new() } }
    pub fn is_silent(&self) -> bool { self.file.is_none() }
}

impl std::fmt::Write for Response { /* swallows io errors, like fprintf */ }
```

Every function that took `FILE *rsp` takes `rsp: &mut Response`. Call sites that passed `NULL` pass
a `Response::silent()`. Serialiser call sites use one macro so nobody writes `write!(...).unwrap()`:

```rust
macro_rules! rsp { ($rsp:expr, $($arg:tt)*) => { $rsp.write_fmt(format_args!($($arg)*)) } }
macro_rules! daemon_fail {
    ($rsp:expr, $($arg:tt)*) => {{
        if !$rsp.is_silent() {
            let _ = $rsp.write_str(FAILURE_MESSAGE);
            let _ = $rsp.write_fmt(format_args!($($arg)*));
        }
    }}
}
```

`FAILURE_MESSAGE` is `pub const FAILURE_MESSAGE: &str = "\x07";`.

`daemon_deprecated` (`src/message.c:429-437`) is `__unused` — currently dead. Translate it and mark
it `#[allow(dead_code)]`; do not delete it, the C author kept it deliberately.

**Buffering matters.** The C uses stdio, so the whole response is buffered and flushed once at
`src/event_loop.c:1636`. A Rust `File` is unbuffered: writing per-`fprintf` would change the packet
boundaries the client sees and therefore where the BEL check falls (point 2 above). **The `Response`
must buffer into a `Vec<u8>` and write it out on `flush`/`Drop`.** `BUFSIZ` on macOS is 1024, and
stdio's default buffer for a socket is `BUFSIZ`, so the client sees the response in 1024-byte
chunks; a single large `write` on the Rust side produces the same read chunking because the client
loop reads `sizeof(rsp)-1 == BUFSIZ-1` bytes at a time. Buffer fully, write once.

### 5.2 Token-based command parsing

**C shape.** The message is a **double-NUL-terminated** sequence of NUL-separated argv strings,
built by the client at `src/yabai.c:65-82`. `get_token` (`src/message.c:298-315`) walks a cursor:

```c
static struct token get_token(char **message)
{
    struct token token;
    token.text = *message;
    while (**message) { ++(*message); }
    token.length = *message - token.text;
    if ((*message)[0] == '\0' && (*message)[1] != '\0') {
        ++(*message);
    } else {
        // NOTE(asmvik): don't go past the null-terminator
    }
    return token;
}
```

The comment on line 312 is one of the few real comments in `message.c`; carry it.

`struct token { char *text; int length; }` (`src/message.c:254-258`) is a **borrowed slice into the
message buffer, not NUL-terminated at `length`** — except it happens to be, because the separator is
a NUL. Both facts are used: `token.length` drives `"%.*s"` printing (240+ sites), and `token.text`
is passed to `strtof` (`src/message.c:386`) and `sscanf` (`src/message.c:1672` etc.) as a C string.

`token_prefix` (`src/message.c:317-325`) and `token_equals` (`src/message.c:327-336`) compare
against a NUL-terminated `char *match`. `token_is_valid` (`src/message.c:338-341`) is
`text && length > 0` — an **empty token means "argument absent"**, which drives the "print current
value" branch of every `config` subcommand (e.g. `src/message.c:1172-1174`).

**The parsers mutate the message buffer in place.** `parse_key_value_pair` (`src/message.c:440-470`)
writes `'\0'` over the `=` at line 462. `parse_properties` (`src/message.c:625-654`) writes `'\0'`
over each `,` at line 638. A Rust translation over `&str` cannot do this.

**Canonical Rust.**

```rust
// message buffer stays a Vec<u8>, owned by the DAEMON_MESSAGE handler
pub struct Token { pub text: *mut u8, pub length: i32 }
```

Use a raw cursor type that mirrors `char **message` exactly:

```rust
pub struct MessageCursor { ptr: *mut u8 }        // == char **message
pub fn get_token(message: &mut MessageCursor) -> Token { /* byte-for-byte port */ }
```

Rationale for raw pointers rather than indices: `parse_rule` (`src/message.c:2679-2700`) takes the
`char *value` **inside** a key=value token and feeds it to `parse_display_selector(rsp, &value, ...)`
— the selector parser advances a cursor that points into the middle of another token. An index-based
cursor over a single slice would also work but requires every call site to carry the base slice;
the pointer form is a one-to-one transcription and every translator will produce the same thing.
Wrap the pointer arithmetic in `unsafe` blocks at the three primitive functions (`get_token`,
`token_prefix`, `token_equals`) and keep every caller safe.

Helpers, all `pub(crate)`, all with C semantics preserved:

```rust
pub fn token_prefix(token: Token, match_: &str) -> bool
pub fn token_equals(token: Token, match_: &str) -> bool
pub fn token_is_valid(token: Token) -> bool
pub fn token_is_positive_integer(token: Token, value: &mut i32) -> bool   // wrapping arithmetic
pub fn token_is_hexadecimal(token: Token, value: &mut u32) -> bool        // wrapping arithmetic
pub fn token_is_float(token: Token, value: &mut f32) -> bool              // strtof semantics
```

`token_to_value` (`src/message.c:397-416`) returns a tagged union. In Rust this is an enum **with
the token retained**, because every error message prints `value.token.length, value.token.text`:

```rust
pub enum TokenKind { Invalid, Unknown, Int(i32), Float(f32), U32(u32), Str(*mut u8) }
pub struct TokenValue { pub token: Token, pub kind: TokenKind }
```

**Order of attempts is semantic** (`src/message.c:400-410`): positive-integer, then hexadecimal,
then float, then string. Therefore `-1` is a `Float`, `007` is an `Int`, `0x0` is a `U32`,
`1e5` / `inf` / `nan` / `0x1p3` are all `Float` because `strtof` accepts them. Do **not** substitute
Rust's `str::parse::<f32>()` — it rejects hex floats and accepts a different set of forms. Call
`libc::strtof` on the NUL-terminated token text and replicate the `if (!end || *end)` check
(`src/message.c:386-388`).

The dispatch itself (`src/message.c:1169` `for (; token_is_valid(command); command = get_token(&message))`
then a 30-arm `if/else if token_equals(...)` chain, repeated per domain) stays a literal
`if/else if` chain in Rust. **Do not convert to `match`** on a parsed enum: several arms use
`token_prefix` rather than `token_equals` (`src/message.c:1035` `ARGUMENT_COMMON_SEL_STACK_PREFIX`),
and the arm order is observable when prefixes overlap.

### 5.3 String constants

`src/message.c:14-256` is 150 `#define`s: 7 `DOMAIN_*`, then `COMMAND_*`, `SELECTOR_*` and
`ARGUMENT_*` per domain, plus `ARGUMENT_COMMON_*`. They are pure ASCII literals and
**part of the CLI's public surface**.

```rust
pub const DOMAIN_CONFIG: &str = "config";
pub const COMMAND_CONFIG_DEBUG_OUTPUT: &str = "debug_output";
// ... same names, same order, same values
```

Keep them in the `message` module as module-level `const`, keep the `/* ---- DOMAIN X ---- */`
banner comments (they exist in the C, so they carry over).

Four of them are **`sscanf` format strings**, not literals, and must not be turned into `&str`
constants used with Rust formatting — see §5.12:
`ARGUMENT_CONFIG_EXTERNAL_BAR "%5[^:]:%d:%d"` (`src/message.c:88`),
`ARGUMENT_SPACE_PADDING "%255[^:]:%d:%d:%d:%d"` (`src/message.c:120`),
`ARGUMENT_SPACE_GAP "%255[^:]:%d"` (`src/message.c:121`),
`ARGUMENT_WINDOW_GRID "%d:%d:%d:%d:%d:%d"` (`src/message.c:164`),
`ARGUMENT_WINDOW_MOVE/RESIZE "%255[^:]:%f:%f"` (`src/message.c:165-166`),
`ARGUMENT_WINDOW_RATIO "%255[^:]:%f"` (`src/message.c:167`),
`ARGUMENT_RULE_VALUE_GRID "%d:%d:%d:%d:%d:%d"` (`src/message.c:217`).
One is a **`char`**, not a string: `ARGUMENT_RULE_VALUE_SPACE '^'` (`src/message.c:216`) → `const ARGUMENT_RULE_VALUE_SPACE: u8 = b'^';`.

`string_equals` (`src/misc/helpers.h:254-257`) is `a && b && strcmp(a,b) == 0` — **NULL-safe**.
It is used 89 times, often with a possibly-NULL side (`src/rule.c:148`, `src/event_signal.c:389`).

```rust
pub fn string_equals(a: Option<&CStr>, b: Option<&CStr>) -> bool { matches!((a,b),(Some(a),Some(b)) if a == b) }
```

Where both sides are known-non-NULL Rust `&str`, plain `==` is fine, but the NULL-accepting form
must exist for the label/regex comparisons.

### 5.4 Logging

`src/misc/log.h` defines four varargs functions and one helper:

| C | behaviour | Rust |
| --- | --- | --- |
| `debug(fmt, ...)` (`:6-15`) | no-op unless `g_verbose`; `vfprintf(stdout, ...)` | `macro_rules! debug` |
| `warn(fmt, ...)` (`:17-24`) | always; stderr | `macro_rules! warn_` (name clash with std's `warn` attr is a non-issue but `warn!` collides with `log` crate conventions — use `warn` and do not depend on the `log` crate) |
| `error(fmt, ...)` (`:26-35`) | stderr then `exit(EXIT_FAILURE)` | `macro_rules! error`, diverges |
| `require(fmt, ...)` (`:37-46`) | stderr then `exit(EXIT_SUCCESS)` | `macro_rules! require`, diverges |
| `debug_message(prefix, message)` (`:48-61`) | prints a double-NUL-terminated message as space-separated tokens | plain `fn` |

`debug` is called 87 times, always with `__FUNCTION__` as the first argument:
`debug("%s: %s (%d)\n", __FUNCTION__, process->name, process->pid)` (`src/event_loop.c:171`).
Rust has no `__FUNCTION__`. **Canonical substitute:** a `function_name!()` helper macro built from
`std::any::type_name` on a local closure, invoked at the call site so the format string stays
verbatim:

```rust
macro_rules! debug {
    ($($arg:tt)*) => {{ if crate::g_verbose() { print!($($arg)*); } }}
}
```

and at each call site `debug!("{}: {} ({})\n", function_name!(), process.name, process.pid)`.

Note the format specifiers change (`%s`→`{}`, `%d`→`{}`, `%.2f`→`{:.2}`) — that is unavoidable, but
**debug output is not part of the byte-identical contract** (it goes to stdout/the launchd log, not
the socket). Query and error output through `rsp` **is**, and there the format strings must be
translated with matching precision (§7).

`debug_message` (`src/misc/log.h:48-61`) walks the double-NUL message using `message += fprintf(...)`
— it advances by the number of bytes printed, which is `1 + strlen(message)` because of the leading
space. Port it literally; it is the `DAEMON_MESSAGE` trace at `src/event_loop.c:1633`.

### 5.5 X-macro lists

Six in the daemon. Each generates two or three things from one list.

| List | defined | generates |
| --- | --- | --- |
| `EVENT_TYPE_LIST` (40 entries) | `src/event_loop.h:6-46` | `enum event_type` (`:48-53`), handler names `EVENT_HANDLER_##value` via `EVENT_HANDLER(t)` (`src/event_loop.h:4`), and the dispatch `switch` (`src/event_loop.c:1664-1668`) |
| `ANIMATION_EASING_TYPE_LIST` (21) | `src/misc/helpers.h:4-25` | `enum animation_easing_type` + `EASING_TYPE_COUNT` (`:27-33`), `animation_easing_type_str[]` (`:35-40`), and the easing dispatch `switch` (`src/window_manager.c:549-553`) |
| `WINDOW_PROPERTY_LIST` (33) | `src/window.h:31-64` | `enum window_property` bit values (`:66-71`), `window_property_val[]` (`:73-78`), `window_property_str[]` (`:80-85`) |
| `SPACE_PROPERTY_LIST` (12) | `src/view.h:7-19` | same three (`:21-40`) |
| `DISPLAY_PROPERTY_LIST` (7) | `src/display.h:7-14` | same three (`:16-35`) |
| `SUPPORTED_MACOS_VERSION_LIST` (6) | `src/workspace.h:4-10` | a `static bool _workspace_is_macos_version_##name` + `static inline bool workspace_is_macos_##name(void)` per entry (`:12-19`), and the assignment block (`src/workspace.m:3-6`) |

`enum signal_type` (`src/event_signal.h:4-45`) is **not** an X-macro but is the same pattern by hand,
with a parallel designated-initialiser table `signal_type_str[]` (`:47-88`).

**Canonical Rust: a single `macro_rules!` per list, mirroring the C.** Do not hand-expand — the
whole point is that adding an event type touches one place, and phase 3 will rely on that.

```rust
macro_rules! event_type_list {
    ($entry:ident) => {
        $entry!(APPLICATION_LAUNCHED);
        $entry!(APPLICATION_TERMINATED);
        // ... all 40, in the C order
    };
}
```

with the enum, the handler-name mapping and the dispatch each defined by a small `$entry` macro,
exactly as the C does with `#define EVENT_TYPE_ENTRY(value) ...` / `#undef`.

`enum event_type` has **no explicit discriminants** in C, so the ordinal is positional — and nothing
serialises it, so Rust `#[derive]`d discriminants are fine. `enum signal_type` also has none, but
`signal_type_str` is indexed by it (`src/event_signal.c:77`, `:338`, `:427`) and
`signal_type_from_string` iterates `SIGNAL_APPLICATION_LAUNCHED..SIGNAL_TYPE_COUNT`
(`src/event_signal.c:345`) — so the enum must be `#[repr(u32)]` with `SIGNAL_TYPE_UNKNOWN = 0` and a
`SIGNAL_TYPE_COUNT` sentinel, and `g_signal_event[SIGNAL_TYPE_COUNT]` (`src/yabai.c:27`) is an array
sized by it.

The property lists **do** have explicit hex values (`0x000000001` … `0x100000000`,
`src/window.h:32-64`) and those values reach the outside world only indirectly (they select which
JSON keys appear). Keep the exact values; `WINDOW_PROPERTY_IS_GRABBED` is `0x100000000` — **33 bits,
so the mask type is `u64`**, matching `uint64_t flags` in the signatures. `SPACE_PROPERTY_*`
(`0x001`..`0x800`) and `DISPLAY_PROPERTY_*` (`0x01`..`0x40`) fit in smaller types but are declared
`uint64_t` in `space_property_val[]` / `display_property_val[]`; keep them `u64` so
`parse_properties` (§5.2) has one signature.

`SUPPORTED_MACOS_VERSION_LIST` becomes six `static AtomicBool`s (they are written once in
`workspace_event_handler_begin`, `src/workspace.m:1-14`, and read from many threads) plus six
`pub fn workspace_is_macos_*()`. See §5.21.

### 5.6 Enum-indexed string tables

Fourteen tables. Two shapes:

**Positional** — `static const char *x[] = { "a", "b", "c" };`

- `bool_str` (`src/misc/helpers.h:173`) `{"off","on"}`, indexed by a `bool` (`src/message.c:1173`)
- `window_insertion_point_str` (`src/view.h:101-106`)
- `window_node_child_str` (`src/view.h:115-120`) `{"none","second_child","first_child"}`
- `window_node_split_str` (`src/view.h:130-136`)
- `auto_balance_str` (`src/view.h:138-143`)
- `view_type_str` (`src/view.h:177-183`)
- `purify_mode_str` (`src/window_manager.h:33-38`) — note the values are `{"on","float","off"}` while
  the enum reads `PURIFY_DISABLED, PURIFY_MANAGED, PURIFY_ALWAYS`: **the string is inverted relative
  to the enum name.** `config window_shadow off` sets `PURIFY_ALWAYS` (`src/message.c:1335`) and
  reading it back prints `"off"`. Do not "correct" this.
- `ffm_mode_str` (`src/window_manager.h:47-52`)
- `window_origin_mode_str` (`src/window_manager.h:61-66`)
- `display_arrangement_order_str` (`src/display_manager.h:15-20`)
- `external_bar_mode_str` (`src/display_manager.h:29-34`)

**Designated-initialiser** — `static const char *x[] = { [ENUM_A] = "a", ... };` with gaps filled by
`NULL`:

- `layer_str` (`src/misc/helpers.h:175-181`) indexed by `LAYER_AUTO=0`, `LAYER_BELOW =
  kCGBackstopMenuLevelKey`, `LAYER_NORMAL = kCGNormalWindowLevelKey`, `LAYER_ABOVE =
  kCGFloatingWindowLevelKey` (`src/misc/macros.h:42-45`). Those Core Graphics keys are **not
  contiguous small integers**, so this array is sparse and its length is `max(key)+1`. Resolve the
  three constants at build time and reproduce the same sparse table, or use a `match` — but the
  values must be identical.
- `signal_type_str` (`src/event_signal.h:47-88`)
- `mission_control_mode_str` (`src/mission_control.c:38-44`)
- `mouse_mod_str` (`src/mouse_handler.h:83-91`) — indexed by the **bit value**, not the ordinal:
  `MOUSE_MOD_NONE = 0x01` → index 1, `ALT = 0x02` → 2, `SHIFT = 0x04` → 4, `CMD = 0x08` → 8,
  `CTRL = 0x10` → 16, `FN = 0x20` → 32. So the array is 33 entries with `NULL` holes at
  0, 3, 5, 6, 7, 9-15 and 17-31. `src/message.c:1621` indexes it with
  `g_mouse_state.modifier`, which is only ever assigned a single one of those five bits
  (`src/message.c:1622-1631`, default `MOUSE_MOD_FN` at `src/mouse_handler.c:268`), so no hole is
  reachable today. Keep the sparse layout anyway — `[Option<&str>; 33]` or a `match` on the bit
  value — because a dense `[&str; 6]` would silently change what `config mouse_modifier` prints
  if a future value is added.
- `mouse_mode_str` (`src/mouse_handler.h:93-100`) — dense, indexed by `enum mouse_mode` 0..4.
- `ax_error_str` (`src/application.h:26-44`) indexed by `-AXError`
- `ax_application_notification_str` / `ax_application_notification` (`src/application.h:46-66`)
- `ax_window_notification_str` / `ax_window_notification` (`src/window.h:17-29`)

**Canonical Rust.** Positional tables become `static FOO_STR: [&str; N] = ["a","b","c"];` with an
`impl Enum { pub fn as_str(self) -> &'static str { FOO_STR[self as usize] } }`. Sparse tables become
`static FOO_STR: [Option<&str>; N]` with the same holes; keep the index arithmetic identical rather
than reordering to make it dense.

**Do not** replace the tables with `Display` impls: `animation_easing_type_str` is *also* iterated
for parsing (`src/message.c:1320-1325`), and `signal_type_str` is iterated by
`signal_type_from_string` (`src/event_signal.c:343-350`). The table must remain indexable and
iterable.

### 5.7 Bit flags and option masks

Three families, 189 call sites.

**Per-struct flag accessors** generated by hand as `static inline`:

```c
static inline bool window_check_flag(struct window *w, enum window_flag x) { return w->flags & x; }
static inline void window_clear_flag(struct window *w, enum window_flag x) { w->flags &= ~x; }
static inline void window_set_flag  (struct window *w, enum window_flag x) { w->flags |=  x; }
```
(`src/window.h:128-134`, and the same trio for `window_rule_flag` at `:132-134`,
`rule_flag`/`rule_effects_flag` at `src/rule.h:59-65`.)

`view` uses **macros** instead (`src/view.h:218-220`):
```c
#define view_check_flag(v, x) ((v)->flags  &  (x))
```
Note `view_check_flag` returns the **masked value**, not a `bool`. Every use is in a boolean context
so it is equivalent — except `src/view.c:864`-style uses where a `u64` is expected. Return `bool`
from the Rust `view_check_flag` and audit: a grep of all 18 `view_check_flag` uses shows all are in
`if`/`?:` position, so `bool` is safe.

**Canonical Rust:**

```rust
impl Window {
    pub fn check_flag(&self, x: WindowFlag) -> bool { self.flags & x.0 != 0 }
    pub fn clear_flag(&mut self, x: WindowFlag) { self.flags &= !x.0; }
    pub fn set_flag(&mut self, x: WindowFlag) { self.flags |= x.0; }
}
```

Keep the free-function spelling too (`window_check_flag(w, x)`) if a translator finds the method form
awkward at a borrow-checker-hostile call site; both must exist so all ten files compile. Prefer the
free function for one-to-one transcription:
`pub fn window_check_flag(w: &Window, x: WindowFlag) -> bool`.

**Query property masks** (`uint64_t flags` parameters) are §5.5's `WINDOW_PROPERTY_*` /
`SPACE_PROPERTY_*` / `DISPLAY_PROPERTY_*` values, tested with `if (flags & WINDOW_PROPERTY_ID)`.
Canonical: plain `u64` with `if flags & WINDOW_PROPERTY_ID != 0`.

**Multi-bit masks** where `& ` is not a single-bit test, and `.contains()` would be wrong:
- `(window->notification & AX_WINDOW_ALL) == AX_WINDOW_ALL` (`src/window.c:18`)
- `(application->notification & AX_APPLICATION_ALL) == AX_APPLICATION_ALL` (`src/application.c:60`)
- `(attrib & OSAX_ATTRIB_ALL) == OSAX_ATTRIB_ALL` (`src/sa.m:282`)
- `parse_resize_handle` returns `HANDLE_TOP | HANDLE_LEFT` (`src/message.c:493`)
- `mouse_mod_from_cgflags` accumulates (`src/mouse_handler.c:5-17`)

Write these as explicit `(a & MASK) == MASK`.

### 5.8 `ts_*` temporary storage and the `ts` string helpers

`src/misc/ts.h` is a 8 MB bump allocator (`src/yabai.c:279` `ts_init(MEGABYTES(8))`) reset once per
event (`src/event_loop.c:1671` `ts_reset()`). Allocation is a lock-free CAS bump
(`src/misc/ts.h:52-63`) with a guard page (`:23`) and a fatal overrun check (`:28-34`, which prints
to stderr and `exit(EXIT_FAILURE)`).

Users: `ts_alloc_unaligned`, `ts_alloc_aligned`, `ts_alloc_list(type, n)` (`:49-50`), `ts_expand`,
`ts_resize` (`:89-98`, used by `src/space.c:71`), plus the string helpers in
`src/misc/helpers.h`: `ts_string_copy` (`:387-395`), `ts_cfstring_copy` (`:361-371`),
`ts_string_escape` (`:259-319`), and the `ts_buf_*` stretchy buffer in `src/misc/sbuffer.h:34-69`.

115 call sites. Everything returned by a `ts_*` function is a **pointer into the arena with a
lifetime ending at the next `ts_reset()`**, and several are stored into `struct event_signal`
(`src/event_signal.c:129-338`) which is then read after a `fork()` in `event_signal_flush`
(`src/event_signal.c:60-97`) — **before** the `ts_reset()` at `src/event_loop.c:1671`, so it is
sound, but only just.

**This is owned by the `misc-containers-and-allocators` file map, not by this sweep.** What this
sweep fixes for all translators:

1. The Rust signature of every `ts_*`-returning function is `-> &'ts T` where `'ts` is a lifetime
   tied to a per-event arena token, **or** a raw `*mut u8`. The arena decision is the container
   sweep's; until it lands, translators write the C-shaped signature returning the pointer type and
   mark the function `unsafe` if it hands out a raw pointer.
2. **Never** substitute `String`/`Vec` for a `ts_*` result at a call site. `window_title_ts`
   (`src/window.c:719-722`), `window_role_ts`, `window_subrole_ts`, `window_property_title_ts`
   (`src/window.c:708-717`) are called inside the serialisers and inside `rule_apply`
   (`src/rule.c:163`) precisely because they must not allocate on the heap; switching them to
   `String` changes the arena's high-water mark and hides the `ts` reset discipline.
3. `ts_string_escape` (`src/misc/helpers.h:259-319`) **returns `NULL` when nothing needed escaping**
   — every call site is `escaped ? escaped : original` (`src/window.c:179`, `:189`, `:487`,
   `src/rule.c:42-45`, `src/event_signal.c:424-428`). Rust: `-> Option<&'ts str>` and the call site
   is `escaped.unwrap_or(original)`. Preserving the `None` case matters because the original may
   itself be `NULL` in `rule_serialize` (`src/rule.c:42`: `escaped_app ? escaped_app : app ? app : ""`).

### 5.9 Stretchy buffers (`buf_*`)

`src/misc/sbuffer.h:4-32` — a header-before-data growable array, `realloc`-based, with
`buf_len`/`buf_cap`/`buf_push`/`buf_del`/`buf_free`/`buf_last`. **`buf_del` swaps the last element
into the hole** (`:19`), so it does not preserve order.

Used for: `g_window_manager.rules` (`src/window_manager.h:85`),
`g_window_manager.applications_to_refresh` (`:86`), `g_window_manager.scratchpad_window` (`:102`),
`g_signal_event[type]` (`src/yabai.c:27`), `g_space_manager.labels` (`src/space_manager.h:28`),
`g_display_manager.labels` (`src/display_manager.h:53`). 63 call sites.

**Canonical Rust: `Vec<T>`**, with `buf_del` translated to `swap_remove`. Two behaviours to
preserve:

- `buf_len(NULL)` is `0` (`src/misc/sbuffer.h:15`), so an uninitialised buffer iterates zero times.
  `Vec::new()` matches.
- `buf_del` returns a truthy value only when the buffer is non-NULL; `src/event_loop.c:173`
  depends on it: `if (buf_del(g_window_manager.rules, i)) { --i; --rule_len; }`. With `Vec`,
  `swap_remove` always succeeds on a valid index, so the guard becomes unconditional — write it as
  `{ vec.swap_remove(i); i -= 1; rule_len -= 1; }` **inside** the same `if` shape so the loop
  arithmetic is unchanged, and note the `i` is an `i32` going to `-1` on the first element
  (`src/event_loop.c:174`) which then `++i`s back to 0. Use `i32`, not `usize`, for that loop
  counter.

The parallel `ts_buf_*` (`src/misc/sbuffer.h:34-69`) allocates from the `ts` arena and **never
frees**; `ts_buf__grow_f` even leaks the old block by bumping `used` (`:58`). Not `Vec`. Keep it as
an arena-backed type from the container sweep.

### 5.10 Hash table (`table_*`)

`src/misc/hashtable.h` — chained buckets, `float max_load = 0.75f` (`:50`), doubling rehash (`:89`),
`table_hash_func`/`table_compare_func` function pointers (`:4-8`). `table_add` is a macro that
captures `sizeof(*key)` (`:29`) and **copies the key** (`:126-128`). `table_for(it, table, code)`
(`:34-41`) is a two-level iteration macro that skips `NULL` values.

Seven tables in `struct window_manager` (`src/window_manager.h:77-83`), one in
`struct space_manager` (`src/space_manager.h:12`), one in `struct process_manager`
(`src/process_manager.h:19`). Hash/compare implementations:
`hash_wm`/`compare_wm` on `uint32_t` (`src/window_manager.c:9-17`),
`hash_view`/`compare_view` on `uint64_t` (`src/space_manager.c:4-12`),
`hash_psn`/`compare_psn` on `ProcessSerialNumber` using `psn_equals` → `SameProcess`
(`src/process_manager.c:4-12`, `src/misc/helpers.h:534-539`).

**Canonical Rust: `HashMap<K, V>`** with `K` = `u32` / `u64` / a `PsnKey` newtype implementing
`Hash`+`Eq` via `SameProcess`. Two behaviours:

- `table_add` **does not overwrite an existing non-NULL value** (`src/misc/hashtable.h:120-123`).
  `HashMap::insert` does. Translate `table_add` as
  `map.entry(k).or_insert(v)` — and where the C relies on replacing a `NULL` value, as
  `if map.get(&k).map_or(true, |v| v.is_null()) { map.insert(k, v); }`. Check each of the 58 sites.
- `table_for` iterates buckets in hash order, which is **not** insertion order and not
  `HashMap`'s order either. Nothing in the daemon serialises `table_for` output directly — the
  query paths go through `space_window_list` (`src/space.c:17-81`), which gets its order from
  SkyLight, not from the table. So `HashMap` iteration order is safe. `rule_reapply_all`
  (`src/rule.c:113-129`) and `rule_apply` (`src/rule.c:159-173`) use `table_for` and their effects
  are order-independent. **Verified: no observable ordering dependency on `table_for`.**

### 5.11 Fixed `char` buffers and `snprintf`

`MAXLEN` is `512` (`src/misc/macros.h:20`). 81 `snprintf` calls, almost all into a
`char buf[MAXLEN]`. Notable sizes that are **not** `MAXLEN`:

- `char g_config_file[4096]` (`src/yabai.c:46`) and `char exe_path[4096]` (`src/misc/service.h:113`)
- `char rsp[BUFSIZ]` (`src/yabai.c:106`, `src/sa.m:243`)
- `char mode[6]` (`src/message.c:1670`) paired with `"%5[^:]"` — five chars plus NUL
- `char type[MAXLEN]` (`src/message.c:1970`, `:1981`, `:2230`, `:2258`) paired with `"%255[^:]"` —
  **the format caps at 255 but the buffer is 512**, so there is slack; keep the 255 cap
- `char bootargs[2048]` (`src/sa.m:320`)
- `char num_str[255]` (`src/misc/helpers.h:323`)
- `uint32_t window_list[1024]` (`src/event_loop.c:19`) — **no bounds check** in the
  `table_for` fill at `src/event_loop.c:23-25`; more than 1024 windows overflows the stack buffer.
  Reproduce the buffer size; a Rust `[u32; 1024]` with the same unchecked `window_list[window_count]
  = ...` would panic instead of corrupting. **Prefer the panic** — it is a latent bug, not a
  behaviour to preserve. Flag it (§8).
- `uint32_t proxy_wid[512]` / `real_wid[512]` in the jankyborders message (`src/window_manager.c:441-442`)
  — same shape, `data.count` is bounded by `animation_count` with no check.
- `char bytes[SA_SOCKET_BUFF_LEN]` where `SA_SOCKET_BUFF_LEN` is `0x1000`
  (`src/osax/common.h:5`, used at `src/sa.m:244`, `:418`)

**Canonical Rust.** For a buffer that is only ever `snprintf`'d once and then used as a C string:
`let mut buf = [0u8; MAXLEN];` plus a small helper that writes a `format!` result and truncates at
`MAXLEN-1` with a NUL — because `snprintf` **truncates** rather than failing, and
`populate_plist_path` (`src/misc/service.h:80-99`) sizes its `malloc` from `strlen(fmt)-2 +
strlen(home) + 1` and relies on the truncation not happening.

Where the buffer never leaves Rust, use `String`. Where it is passed to a C API (`socket_connect`'s
`sun_path`, `src/misc/helpers.h:194`), keep the byte array.

`struct sockaddr_un.sun_path` is 104 bytes on macOS and `snprintf(socket_address.sun_path,
sizeof(socket_address.sun_path), "%s", socket_path)` (`src/misc/helpers.h:194`,
`src/message.c:3019`) silently truncates. Keep the truncation.

### 5.12 `sscanf` colon-format parsing

Ten sites (§5.3 lists the formats). The daemon uses `sscanf` **as a validator**: it tests the
return value against the field count and falls back to "print current value" or `daemon_fail` on
mismatch (e.g. `src/message.c:1672-1693`, `src/message.c:1972-1978`).

**Canonical Rust: hand-written splitters, one per format, in the `message` module**, not a `sscanf`
crate and not `sscanf` via FFI. Semantics to replicate exactly:

- `"%255[^:]:%d:%d:%d:%d"` — the first field is *greedy up to the first `:`*, capped at 255 chars,
  and **must be non-empty** (`%[^:]` fails on a zero-length match). `%d` skips leading whitespace,
  accepts an optional sign, and stops at the first non-digit — so `"abs:1:2:3:4x"` **succeeds** with
  the trailing `x` ignored, because `sscanf` returns 5.
- `"%d:%d:%d:%d:%d:%d"` into `unsigned` (`src/message.c:2220`, `src/message.c:2708`) — `%d` into an
  `unsigned*` is technically undefined but in practice reads a signed decimal into the unsigned
  slot; `"-1:2:..."` yields `0xFFFFFFFF`. Replicate with `i32` parse then `as u32`.
- `"%f"` (`src/message.c:2718`) — `strtof` semantics again, and the result is then range-checked
  with `in_range_ii(..., 0.0f, 1.0f)`.
- `"%5[^:]:%d:%d"` (`src/message.c:1672`) — **five characters max**, so `"main:0:0"` parses `mode`
  as `"main"`, but a six-letter mode is silently truncated to five and then fails the
  `string_equals` checks at `:1673-1685`.
- Partial-success is treated as failure everywhere (`== 3`, `== 5`, `== 6`), so the Rust splitter
  returns `Option<(..)>`.

Write one function per format with a name that says the format:
`fn parse_external_bar_argument(s: &str) -> Option<(String, i32, i32)>` etc. All ten live together
in `message`, so the ten translators do not each invent one.

### 5.13 `regex.h`

Six `regcomp(&r, value, REG_EXTENDED)` sites (`src/message.c:2641`, `:2651`, `:2661`, `:2671`,
`:2895`, `:2903`), four `regfree` sites in `rule_destroy` (`src/rule.c:209-212`) and two in
`event_signal_destroy` (`src/event_signal.c:360-361`), one `regexec` in `regex_match`
(`src/misc/helpers.h:573-579`).

```c
static inline int regex_match(bool valid, regex_t *regex, const char *match)
{
    if (!valid) return REGEX_MATCH_UD;
    int result = regexec(regex, match, 0, NULL, 0);
    return result == 0 ? REGEX_MATCH_YES : REGEX_MATCH_NO;
}
```

`REGEX_MATCH_UD = 0`, `_YES = 1`, `_NO = 2` (`src/misc/macros.h:22-24`). The **three-valued** result
is the whole point: a rule with no `app=` filter yields `UD`, which matches neither the "expect yes"
nor the "expect no" comparison, so the filter is skipped. See `window_manager_rule_matches_window`
(`src/window_manager.c`) and `event_signal_filter` (`src/event_signal.c:8-58`):

```c
int regex_match_app = rule_check_flag(rule, RULE_APP_EXCLUDE) ? REGEX_MATCH_YES : REGEX_MATCH_NO;
if (regex_match(rule_check_flag(rule, RULE_APP_VALID), &rule->app_regex, window->application->name) == regex_match_app) return false;
```

**Canonical Rust.** POSIX ERE is **not** the `regex` crate's dialect (backreferences, leftmost-longest
vs leftmost-first, POSIX character classes, and the handling of `\` differ). Rules and signals are
user-supplied patterns from `.yabairc` files in the wild. **Bind to the system `regcomp`/`regexec`/
`regfree` through `libc`** rather than using the `regex` crate; anything else silently changes which
windows a user's rules match.

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RegexMatch { Ud = 0, Yes = 1, No = 2 }

pub struct PosixRegex { raw: libc::regex_t }       // Drop calls regfree
pub fn regex_match(valid: bool, regex: &PosixRegex, match_: &CStr) -> RegexMatch
```

Keep `regex_match` returning the three-valued enum and keep the comparison-against-expected shape at
the call sites. `RegexMatch` must be comparable with `==` and the "expected" value computed the same
way (`if exclude { Yes } else { No }`).

`regex_t` is **not** movable safely in glibc; on macOS's BSD regex it stores internal pointers too.
Box it (`Box<libc::regex_t>`) and never move the struct after `regcomp`. `struct rule` and
`struct signal` embed `regex_t` by value (`src/rule.h:51-54`, `src/event_signal.h:112-113`) and are
stored in a `buf_*` array that **reallocs** (`src/rule.c:178` `buf_push`) — the C code gets away with
this because BSD `regex_t` is position-independent. In Rust, box it to be safe; the observable
behaviour is identical.

### 5.14 Float math on `CGRect` / `struct area`

`struct area { float x, y, w, h; }` (`src/view.h:42-48`) vs `CGRect` (`f64` fields). Conversions at
`area_from_cgrect` (`src/view.c:121-124`) and the inverse at every `CGRect{{a.x,a.y},{a.w,a.h}}`.

The sharp edges:

**`area_make_pair` (`src/view.c:161-184`)** truncates to `int` and then re-widens:

```c
float left_width  = (parent_area->w - gap) * ratio;
float right_width = (parent_area->w - gap) * (1 - ratio);
left_area->w   = (int)left_width;
right_area->w  = (int)right_width;
right_area->x += (int)(left_width + 0.5f) + gap;
```

`gap` is `int`, promoted to `float`; `ratio` is `float`; `(1 - ratio)` promotes `1` to `float`. The
`(int)` truncates toward zero, the `+ 0.5f` before the second truncation is a round-half-up. All
three casts are semantic — a translator who writes `.round()` or keeps `f32` changes window
geometry. Rust: `left_area.w = left_width as i32 as f32;` and
`right_area.x += ((left_width + 0.5f32) as i32 + gap) as f32;`.

**`area_distance_in_direction` (`src/view.c:563-582`)** returns `int` from `CGFloat` subtraction:
`return r2_max.y > r1->y ? r2_max.y - r1->y : r1->y - r2_max.y;` — an implicit `f64 → i32`
truncation, and the sentinel is `INT_MAX` (`:581`). Rust: `(r2_max.y - r1.y) as i32`, sentinel
`i32::MAX`.

**`area_max_point` (`src/view.c:126-129`)** is `{ x + w - 1, y + h - 1 }` in `f32`, returned as a
`CGPoint` (`f64`). The `-1` is an inclusive-edge adjustment and comparisons in
`area_is_in_direction` (`src/view.c:541-562`) are strict/non-strict in a specific pattern. Transcribe
the comparison operators exactly; they are not symmetric.

**`lerp(a, t, b)`** is `(((1.0-t)*a) + (t*b))` (`src/misc/macros.h:16`) — `1.0` is a **double**, so
the whole expression evaluates in `f64` even though `t` and the operands are `f32`, then narrows on
assignment to `proxy.tx` (`f32`, `src/view.h:61`). `src/window_manager.c:560-563`. Rust must do the
arithmetic in `f64` and narrow: `(((1.0f64 - t as f64) * a as f64) + (t as f64 * b as f64)) as f32`.

**Animation `t`** (`src/window_manager.c:544-546`) is `double`, clamped with
`if (t <= 0.0) t = 0.0f; if (t >= 1.0) t = 1.0f;` and then `mt = easing(t)` where every easing
function takes `float` (`src/misc/helpers.h:42-145`) — a `double → float` narrowing at the call.
`mt` is **uninitialised if the switch matches nothing** (`src/window_manager.c:548-553`); the
`ANIMATION_EASING_TYPE_LIST` covers every value of `animation_easing` so it cannot happen, but Rust
must give `mt` a definite value. Use an exhaustive `match` on the enum.

**`in_range_ii/ie/ei/ee`** (`src/misc/macros.h:12-15`) are inclusive/exclusive range macros used on
floats: `in_range_ii(value.float_value, 0.0f, 1.0f)` for menubar opacity (`src/message.c:1348`) but
`in_range_ei(...)` — **exclusive lower bound** — for active/normal window opacity
(`src/message.c:1357`, `:1366`). A translator who writes one helper for all four breaks
`window_opacity 0.0`. Provide four distinct functions with the C names.

**`add_and_clamp_to_zero`, `min`, `max`** (`src/misc/macros.h:9-11`) are macros with
double-evaluation hazards in C; as Rust generic `fn`s they are strictly safer and behaviourally
identical for the arguments used. Name them `min`/`max`/`add_and_clamp_to_zero` and shadow the
std ones locally.

**`AX_ABS` / `AX_DIFF`** (`src/view.h:4-5`): `AX_DIFF(a,b)` is `|a-b| >= 1.5f`. Used 12 times in
`src/event_loop.c` (`:310-314`, `:409-417`) as the "did the window really move" debounce. The
`1.5f` threshold is observable behaviour. Keep as `fn ax_diff(a: f32, b: f32) -> bool`, and note the
arguments are sometimes `CGFloat` (`new_origin.x`) against `float` (`node->area.x`) — the C promotes
to `double` for the subtraction and compares against `1.5f` promoted to `double`. Write
`(a as f64 - b as f64).abs() >= 1.5f32 as f64`.

`clampf_range` (`src/misc/helpers.h:581-586`), `cgrect_clamp_x_radius` / `_y_radius`
(`:542-556`), `cgrect_contains_point` (`:558-562`), `triangle_contains_point` (`:564-571`) —
straight ports, `f32` throughout except `triangle_contains_point` which takes `CGPoint` (`f64`) and
computes in `float`: `float l1 = (p.x - t[0].x) * ...` narrows each product. Transcribe with
explicit `as f32`.

### 5.15 Integer types and sentinel values

| C type | meaning | Rust | sentinel |
| --- | --- | --- | --- |
| `uint32_t wid` | window id | `u32` | `0` == none (`src/event_loop.c:646`, `src/misc/helpers.h:499-504`) |
| `uint32_t did` | display id | `u32` | `0` == none (`src/display_manager.c:165-195`, every `display_manager_*_display_id`) |
| `uint64_t sid` | space id | `u64` | `0` == none (`src/space.c:105`, `src/window.c:83`) |
| `int` mission-control index | 1-based space index | `i32` | `0` == not found (`src/space_manager.c`, `src/view.c:885`) |
| `int` display arrangement | 1-based | `i32` | `0` == not found (`src/display_manager.c:165-195`) |
| `pid_t` | process id | `i32` | `0` == none (`src/process_manager.c:26-29`) |
| `struct window *` | | `Option<&Window>` / `*mut Window` | `NULL` == none |
| `int active` in `struct signal` | tri-state | `i32` | `SIGNAL_PROP_UD=0`, `_YES=1`, `_NO=2` (`src/event_signal.h:90-92`) |
| `int manage/sticky/mff/fullscreen` in `struct rule_effects` | tri-state | `i32` | `RULE_PROP_UD=0`, `_ON=1`, `_OFF=2` (`src/rule.h:4-6`) |
| `int` regex result | tri-state | `RegexMatch` | §5.13 |

**Do not turn `0`-means-none into `Option<NonZeroU32>`.** The ids are passed to SkyLight, compared
against each other, stored in structs and printed. `json_optional_bool(int value)`
(`src/misc/helpers.h:225-231`) literally serialises the tri-state as `"null"`/`"true"`/`"false"` —
the sentinel reaches the JSON.

`json_optional_bool` is used for `rule.effects.manage/sticky/mff/fullscreen` (`src/rule.c:50-54`) and
`signal.active` (`src/event_signal.c:426`).

**`struct window::id_ptr`** (`src/window.h:92`) is `uint32_t *volatile` used as an
invalidation token: `__sync_bool_compare_and_swap(&window->id_ptr, &window->id, NULL)` marks a
window dead (`src/application.c:36`, `src/event_loop.c:280`), and
`__sync_bool_compare_and_swap(&window->id_ptr, &window->id, &window->id)` tests liveness without
changing it (`src/event_loop.c:647` and 7 more). This is a self-referential pointer inside the
struct — it cannot be a Rust reference or a `Box`. It becomes an `AtomicPtr<u32>` initialised to
`&self.id`, which means `Window` must be pinned/boxed and the pointer set after construction. See
§8 for why this is the hardest single construct in the codebase.

### 5.16 `goto` cleanup

84 `goto`s across 10 files. Three shapes:

**Cascading CF-release cleanup** — the dominant one. `src/window.c:67-84`:

```c
    CFArrayRef window_list_ref = cfarray_of_cfnumbers(...);
    CFArrayRef space_list_ref = SLSCopySpacesForWindows(...);
    if (!space_list_ref) goto err;
    int count = CFArrayGetCount(space_list_ref);
    if (!count) goto free;
    /* ... */
free:
    CFRelease(space_list_ref);
err:
    CFRelease(window_list_ref);
    return sid ? sid : window_display_space(wid);
```

Also `src/space.c:26-80`, `src/display_manager.c:165-197`, `src/window.c:86-108`.

**Canonical Rust:** RAII wrappers over CF types (`CFRetained<T>` with a `Drop` that calls
`CFRelease`), which makes the `goto` labels disappear and produces **exactly** the same release
order because Rust drops in reverse declaration order and the C labels are in reverse acquisition
order. Verify per function that the C order is reverse-acquisition — in all four cases above it is.
Where it is not, use explicit `drop(x)` calls rather than restructuring.

**Single-exit accumulator** — `src/sa.m:202-237` (`scripting_addition_install` with a
`cleanup:` label that removes the partially-installed bundle) and `src/sa.m:96-108`
(`scripting_addition_create_directory` with `goto err` from seven `mkdir` checks). Canonical Rust:
an inner `fn` returning `bool`, with the cleanup after the call.

**Loop/early-out `goto`** — `src/event_loop.c:1661` `if (!next) goto empty;` jumping out of two
nested loops, and `src/event_loop.c:343` `out:` reached by `goto out` from an early failure and by
fallthrough. Canonical Rust: labelled `break 'label` for the first; for the second, an inner
function or a `loop { ... break; }` block so the shared tail runs either way. Do **not** duplicate
the tail code.

`src/event_loop.c:1119-1121` and `src/window_manager.c:575` use `goto out` purely as an early
return with a shared `return` statement — plain `return` in Rust.

### 5.17 Pointer-as-integer event context

`event_loop_post(&g_event_loop, TYPE, context, param1)` (`src/event_loop.h:74`) takes a `void *`.
44 sites smuggle a scalar through it:

```c
event_loop_post(&g_event_loop, DISPLAY_ADDED, (void *)(intptr_t) did, 0);   // src/display.c:9
uint32_t window_id = (uint32_t)(intptr_t) context;                          // src/event_loop.c:640
uint64_t sid = (uint64_t)(uintptr_t) context;                               // src/event_signal.c:229
enum mission_control_mode mode = (enum mission_control_mode)(uintptr_t) context;  // src/event_signal.c:332
struct application *application = window_manager_find_application(&g_window_manager, (pid_t)(intptr_t) context);  // src/event_loop.c:428
```

and the rest pass a genuine pointer (`struct process *`, `struct window *`, a retained `AXUIElementRef`
or `CGEventRef`).

**Canonical Rust.** The faithful minimum: keep `context: *mut c_void` in `struct Event` and cast at
both ends (`did as usize as *mut c_void` / `context as usize as u32`). That is what phase 2 writes.

The tempting alternative — an `enum EventPayload` — changes `struct event`'s size and the lock-free
queue's memory-pool arithmetic (`src/event_loop.c:1689` `memory_pool_push(&pool, sizeof(struct event))`)
and would require all 40 handlers to change signature at once. **Phase 2 keeps `*mut c_void`.**
Phase 3 may introduce the enum.

`(void *) CFRetain(element)` (`src/application.c:9`) and `(void *) CFRetain(event)`
(`src/mouse_handler.c:34`, `:44`) pass **an owned +1 reference** through the queue; the handler
releases it (`src/event_loop.c:155` `CFRelease(context)`). Note which handlers release and which do
not — `WINDOW_CREATED` releases on every early-out path (`src/event_loop.c:155-164`) but **not** on
success, because `window_manager_create_and_add_window` takes ownership.

### 5.18 Atomics, memory barriers and `volatile`

41 sites. Four distinct uses:

1. **The lock-free event queue** (`src/event_loop.c:1655-1700`): `__atomic_load_n(..., __ATOMIC_RELAXED)`,
   `__atomic_store_n(..., __ATOMIC_RELEASE)`, `__sync_bool_compare_and_swap`, and a compiler barrier
   `__asm__ __volatile__ ("" ::: "memory")` at `:1694`. Rust: `AtomicPtr`/`AtomicU32` with
   `Ordering::Relaxed`/`Release`/`AcqRel`, and `std::sync::atomic::compiler_fence(Ordering::SeqCst)`
   for the bare barrier.
2. **The `ts`/`memory_pool` bump allocators** (`src/misc/ts.h:55-63`, `src/misc/memory_pool.h:31-44`):
   `__atomic_load_n` + `__sync_bool_compare_and_swap` CAS loop, and `__sync_fetch_and_add`
   (`src/misc/ts.h:68`, `src/event_signal.c:107`). Rust: `AtomicU64::compare_exchange_weak` in a loop,
   `fetch_add`.
3. **Cross-thread flags**: `volatile bool __pending_window_focus`, `__pending_gesture`,
   `volatile uint64_t __last_gesture_time`, `__last_cmd_tab_time` (`src/event_loop.c:11-14`), and
   `volatile bool skip` in `struct window_animation` (`src/view.h:75`), `volatile uint8_t modifier`
   in `struct mouse_state` (`src/mouse_handler.h:71`), `bool volatile terminated` in
   `struct process` (`src/process_manager.h:14`), `uint32_t *volatile id_ptr`
   (`src/window.h:92`). Rust: `AtomicBool` / `AtomicU64` / `AtomicU8` / `AtomicPtr`.
   **`volatile` alone is not a Rust `AtomicX`'s equivalent**, but every one of these is read/written
   with `__atomic_*` or `__sync_*` in practice, so the atomic type is the right translation.
4. **Non-atomic barriers protecting a plain global**: `src/space_manager.c:752` and `:796` bracket a
   save/restore of `g_window_manager.window_animation_duration` around a space swap. Rust:
   `compiler_fence(Ordering::SeqCst)` in both places; the global stays a plain `f32`.
   Same at `src/window_manager.c:648` and `src/process_manager.c:192`.

`pthread_mutex_t window_animations_lock` (`src/window_manager.h:84`) guards
`window_animations_table` — the only real lock in the daemon. Rust: `Mutex<...>`, or a raw
`libc::pthread_mutex_t` if the table must stay accessible from the `CVDisplayLink` callback
(`src/window_manager.c:575-589`) without restructuring. Phase 2: `Mutex`.

`sem_open("yabai_event_loop_semaphore", O_CREAT, 0600, 0)` + immediate `sem_unlink`
(`src/event_loop.c:1709-1710`) — a **named** POSIX semaphore unlinked right after creation because
macOS has no working `sem_init`. Rust: keep the `libc::sem_open`/`sem_unlink`/`sem_wait`/`sem_post`
calls; `std::sync::Condvar` changes the wakeup semantics relative to `sem_post` from a signal-ish
context.

### 5.19 Profiler macros

`src/misc/timer.h`. All of `TIME_FUNCTION`, `TIME_BLOCK(label)`, `TIME_BODY(label, code)`,
`PROFILER_END_TRANSLATION_UNIT`, `profile_begin()`, `profile_end_and_print()` compile to **nothing**
unless `-DPROFILE=1` / `=2` (`src/misc/timer.h:150-163`). The makefile never sets `PROFILE`
(`makefile:4`, `:20`, `:23`, `:26`), so in every shipping build they are no-ops.

58 `TIME_FUNCTION` sites (34 in `src/window_manager.c` alone), 5 `TIME_BODY` sites
(`src/window_manager.c:614-690`), one `PROFILER_END_TRANSLATION_UNIT` (`src/yabai.c:356`).

`TIME_FUNCTION` relies on `__attribute((cleanup(...)))` (`src/misc/timer.h:137`) —
a scope-exit hook, i.e. Rust `Drop`. `PROFILER_END_TRANSLATION_UNIT` is a `_Static_assert` on
`__COUNTER__` (`:149`) that has no Rust analogue and no purpose once the unity build is gone.

**Canonical Rust:** a `time_function!()` macro that expands to nothing under the default feature and
to a `Drop` guard under a `profile` cargo feature. Keep the macro at every one of the 58 sites so the
Rust and C read the same; drop `PROFILER_END_TRANSLATION_UNIT` entirely.

```rust
#[cfg(not(feature = "profile"))]
macro_rules! time_function { () => {} }
```

### 5.20 SIMD

Exactly one function: `cgimage_restore_alpha` (`src/misc/helpers.h:588-675`), with an
`#ifdef __x86_64__` SSE2 path (`:600-604`, `:614-639`) and an `#elif __arm64__` NEON path
(`:605-609`, `:640-665`). It un-premultiplies alpha across a captured window image, four pixels at a
time, used only from `window_manager_build_window_proxy_thread_proc` (`src/window_manager.c:523`).

The loop is `for (int i = 0; i < height*width; i += 4)` with **no tail handling** — if
`height*width` is not a multiple of 4 it reads and writes up to 3 pixels past the buffer. The
`calloc(height*pitch, 1)` at `:593` usually leaves slack, but this is a real overrun. Flag it (§8).

**Canonical Rust:** `std::arch::x86_64::*` / `std::arch::aarch64::*` intrinsics behind
`#[cfg(target_arch = ...)]`, in one `unsafe fn`, transcribed instruction for instruction. Do **not**
rewrite as scalar Rust and trust auto-vectorisation: the two paths already differ (`_mm_cvtps_epi32`
rounds-to-nearest-even, `vcvtnq_s32_f32` rounds-to-nearest-even too, but `_mm_cvtps_epi32` respects
MXCSR) and a scalar rewrite would produce a third result. Byte-identical window animation frames are
not part of the external contract, but reproducing the arch split keeps the two builds consistent
with the C ones.

Note `vshlq_u32(source, vdupq_n_s32(-8))` (`src/misc/helpers.h:643`) — a **negative** shift count is
how NEON expresses a right shift. Preserve that; `vshrq_n_u32` is not a drop-in.

### 5.21 Per-arch and per-OS-version branches

**Per-arch `#ifdef`** — four sites:
- `src/manifest.m:10-14` selects `<emmintrin.h>` vs `<arm_neon.h>` → Rust `#[cfg(target_arch)]` on the
  intrinsics import.
- `src/misc/helpers.h:600-666` — §5.20.
- `src/misc/timer.h:26-32` and `:37-63` — `__rdtsc()` vs `mrs cntvct_el0` inline asm, profiler-only.
- `src/sa.m:317-331` `scripting_addition_is_arm64e_enabled()` — reads `kern.bootargs` via
  `sysctlbyname` and looks for `-arm64e_preview_abi`. Guarded by `#ifdef __arm64__` and called at
  `src/sa.m:393-400` inside the same guard. Rust: `#[cfg(target_arch = "aarch64")]` on both the
  function and the call.

**Per-OS-version** — `workspace_is_macos_{tahoe,sequoia,sonoma,ventura,monterey,bigsur}()`,
generated at `src/workspace.h:12-19`, set once at `src/workspace.m:3-6` from
`[[NSProcessInfo processInfo] operatingSystemVersion].majorVersion`. **Exact equality on the major
version**, so a future macOS 27 makes all six false. 26 call sites, the important ones being:

- `src/yabai.c:311-334` — which SkyLight connection notification ids to register
  (1204 on old, 1327/1328 on Ventura+, 804 on Sequoia+, always 808 and 1202)
- `src/event_loop.c:21`, `:245`, `:339`, `:598`, `:631` and `src/yabai.c:340` — Sequoia/Tahoe
  subscribe to *all* windows for notifications
- `src/view.c:44`, `:110` — the inverse condition for the feedback-window path
- `src/event_loop.c:1549-1556` — re-observe mission control after a Dock restart

There is also `workspace_use_macos_space_workaround()` (`src/workspace.m:16-26`) which checks
**minor** versions: `12.7+`, `13.6+`, `14.5+`, or `>= 15`.

**Canonical Rust:** six `static AtomicBool` plus six `pub fn workspace_is_macos_*() -> bool`, set in
`workspace_event_handler_begin`. Generate them from one `macro_rules!` mirroring
`SUPPORTED_MACOS_VERSION_LIST` (§5.5). These are **runtime** checks, not `cfg!` — a translator who
reaches for `#[cfg]` here produces a binary that only works on the build machine's macOS.

### 5.22 `static inline` in headers

62 of them, concentrated in `src/misc/helpers.h` (44), `src/misc/log.h` (5), `src/misc/ts.h` (6),
plus the flag accessors (§5.7) and `src/view.c`'s file-local helpers (§0.1).

Canonical Rust: plain `pub(crate) fn` with `#[inline]` only where the C author's intent was clearly
performance (`window_check_flag`, `area_from_cgrect`, `ts_alloc_*`). Everywhere else, drop the
`inline` — Rust's inliner does not need the hint and the attribute is noise.

`inline` **without** `static` appears at `src/display.c:101`, `src/space.c:3`, `:83`, `:88`, `:93`,
`:98`, `:103` — C99 `extern inline`, which in a unity build is just a definition. Plain `pub fn`.

### 5.23 Function-local `static` state

Three sites, all must become module-level state in Rust:

- `src/event_loop.c:1561-1562`: `static enum ffm_mode ffm_value;` and `static int is_menu_open = 0;`
  — a save/restore of focus-follows-mouse across menu open/close, with a **counter** that can go
  negative and is clamped (`src/event_loop.c:1580-1585`). File-scope in C, so module-level `static
  mut`/`Cell` in Rust, touched only from the event-loop thread.
- `src/window.c:173`: `static char process_name[PROC_PIDPATHINFO_MAXSIZE];` **inside**
  `window_nonax_serialize` — a function-local static buffer reused across calls. It is written by
  `proc_name(pid, ...)` and immediately read, all on the event-loop thread. Rust: a module-level
  buffer or a stack array; a stack `[u8; PROC_PIDPATHINFO_MAXSIZE]` (4096) is fine and safer.
- `src/misc/timer.h:38`: `static uint64_t cpu_freq;` memoisation inside `read_cpu_freq()` —
  profiler-only (§5.19).

### 5.24 Comparators

**`CFArraySortValues` callback** — `display_manager_coordinate_comparator`
(`src/display_manager.c:140-163`), used at `src/display_manager.c:180` and `:210` to sort managed
display UUIDs by screen coordinate. Returns `CFComparisonResult`; compares primary axis then
secondary, both as `float` narrowed from `CGPoint` (`f64`). Rust: an `extern "C" fn` passed to
`CFArraySortValues`, or sort the UUIDs in Rust with the same two-key comparison and then look up the
index. **Prefer the Rust sort** — `CFArraySortValues`'s algorithm is unspecified, but the
comparison is a total order here (ties return `kCFCompareEqualTo` and the index lookup at
`src/display_manager.c:184-189` uses `CFEqual`, so a stable sort reproduces the C for distinct
coordinates and any order for exact ties, which cannot happen for distinct displays).

**Table hash/compare callbacks** — `TABLE_HASH_FUNC`/`TABLE_COMPARE_FUNC` (§5.10). These become
`Hash`/`Eq` impls, not function pointers.

There is **no `qsort` in the daemon.** (`src/window_manager.c:2334`, `:2363` `assert(node)` are
unrelated.)

### 5.25 Varargs

Six: `debug`, `warn`, `error`, `require` (`src/misc/log.h`), `notify`
(`src/misc/notify.h:29-48`), `daemon_fail` / `daemon_deprecated` (`src/message.c:418`, `:429`).

All become `macro_rules!` (§5.4, §5.1). `notify` is special: it builds an `NSString` with
`initWithFormat:arguments:` from a C `va_list` (`src/misc/notify.h:40`) — the format string is
interpreted by **Foundation**, not by stdio. Every call site uses only `%s`, `%d` and `%X`
(`src/sa.m:277-296`, `src/misc/helpers.h:467-485`), so formatting in Rust and passing the finished
string to `[NSString stringWithUTF8String:]` is equivalent.

### 5.26 `assert`, and where the process exits

**13 `assert()`s.** With `-DNDEBUG` in the `install` target (`makefile:26`) they vanish; the default
`all` target (`makefile:4`) keeps them. They are all "this cannot happen" invariants:
`src/display_manager.c:99`, `:362`, `src/space_manager.c:451`, `src/view.c:642-643`,
`src/window_manager.c:2334`, `:2363`, `src/sa.m:150`, `:255`, `src/misc/ts.h:38`, `:78`, `:91`.

Canonical Rust: `debug_assert!` — same on/off behaviour as C's `assert` vs `NDEBUG`, since cargo
release builds disable `debug_assert!`.

**22 exit points.** Inventory, because an exit changes observable behaviour:

| site | code | reason |
| --- | --- | --- |
| `src/yabai.c:201` | `EXIT_SUCCESS` | `--help` |
| `src/yabai.c:207` | `EXIT_SUCCESS` | `--version` |
| `src/yabai.c:212` | `client_send_message` result | `-m` |
| `src/yabai.c:216-240` | subcommand result | `--load-sa`, `--uninstall-sa`, `--{install,uninstall,start,restart,stop}-service` |
| `src/yabai.c:252`, `:255` | `error()` → `EXIT_FAILURE` | bad option |
| `src/yabai.c:268`, `:272`, `:276` | `require()` → **`EXIT_SUCCESS`** | running as root / no AX privilege / separate-spaces disabled |
| `src/yabai.c:280-292`, `:308`, `:345` | `error()` → `EXIT_FAILURE` | init failures |
| `src/misc/ts.h:32` | `EXIT_FAILURE` | temp storage exhausted |
| `src/misc/helpers.h:482` | `execvp` result | config-file child process |
| `src/event_signal.c:92`, `:96` | `execvp` result / `EXIT_SUCCESS` | signal child processes |
| `src/misc/service.h:86`, `:92`, `:105`, `:110`, `:116`, `:122`, `:176`, `:187`, `:201`, `:254`, `:268` | `error()` → `EXIT_FAILURE` | service management |

The `require()` → `EXIT_SUCCESS` cases are the subtle ones. A launchd job with
`KeepAlive.SuccessfulExit = false` (`src/misc/service.h:26-32`) must **not** respawn when yabai
refuses to start for a configuration reason. Getting this wrong produces an infinite respawn loop.

### 5.27 `fork()` + `execvp()`

Three places, all shell-out:

1. `exec_config_file` (`src/misc/helpers.h:463-487`): `fork()`, and in the child
   `execvp("/usr/bin/env", ["sh","-c",config_file])` if the file is executable, else
   `["sh", config_file]`. The parent does **not** wait — `signal(SIGCHLD, SIG_IGN)`
   (`src/yabai.c:151`) reaps.
2. `event_signal_flush` (`src/event_signal.c:60-97`): a **double fork**. The outer `fork()` returns
   in the parent which resets `g_signal_storage.used = 0` and returns; the child then forks once per
   matching subscriber, `setenv`s up to four `YABAI_*` variables and
   `execvp("/usr/bin/env", ["sh","-c",signal->command])`, and finally `exit(EXIT_SUCCESS)`.
3. `safe_exec` (`src/misc/service.h:53-78`): `posix_spawn` + `waitpid` with `EINTR` retry and
   `usleep(1000)`, optionally redirecting stdout/stderr to `/dev/null`. Returns `WEXITSTATUS`, or
   `1` if signalled/stopped.

**Canonical Rust.** `fork()` in a process with threads and an Objective-C runtime is only safe
because the child immediately `exec`s. Use `libc::fork` directly, not `std::process::Command`:
`Command` would `posix_spawn` and lose the "parent returns immediately without waiting" and the
double-fork structure that `event_signal_flush` depends on. For `safe_exec`, `libc::posix_spawn`
with `posix_spawn_file_actions_*` transcribed.

The child of `event_signal_flush` reads `g_signal_event[]` and the `ts` arena **after** the fork —
copy-on-write memory, no synchronisation needed, but it means the Rust globals must be plain memory,
not anything with a background thread or a lazily-initialised lock that could be held at fork time.
**Do not put `g_signal_event` behind a `OnceLock`/`LazyLock`/`Mutex`.**

### 5.28 `#pragma clang diagnostic`

59 push/ignored/pop groups. Three warnings suppressed:
`-Wunused-parameter` (callbacks with a fixed signature), `-Wdeprecated-declarations`
(Carbon `ProcessSerialNumber`, `NSUserNotification`), `-Wmissing-field-initializers`
(`src/window_manager.c:435`), `-Wswitch` (`src/mouse_handler.c:20`).

Rust equivalents: `_` parameter names or `let _ = x;`, `#[allow(deprecated)]`,
`..Default::default()`, and an exhaustive `match` with a `_ => {}` arm. Where the C suppressed a
warning, the Rust should express the same intent explicitly rather than `#[allow(...)]`-ing broadly.

---

## 6. Byte-identical external behaviour — full inventory

Everything below is observable by something outside the process. A phase-2 translator who changes
any of it breaks a user's config, a status bar, or a shell script.

### 6.1 Client/daemon socket wire format

- **Socket path**: `"/tmp/yabai_%s.socket"` with `$USER` (`src/yabai.c:2`, built at `src/yabai.c:136`
  and `:86`). Mode `0600` via `chmod` after `bind` (`src/message.c:3028`). `unlink` before `bind`
  (`src/message.c:3019`). `SOMAXCONN` backlog (`src/message.c:3034`). `FD_CLOEXEC` set on the
  listening fd (`src/message.c:3037`).
- **Request framing**: a 4-byte host-endian `int` byte count, then that many bytes of
  NUL-separated arguments terminated by an **extra** NUL (double-NUL). Built at
  `src/yabai.c:65-82`; the count **excludes** the 4-byte header. Read at `src/event_loop.c:1622-1631`
  with a read loop.
- **Response framing**: raw bytes until the daemon closes (`src/event_loop.c:1636-1637`
  `fflush` + `fclose`). No length prefix.
- **Failure marker**: byte `0x07` (BEL) immediately before each failure message
  (`src/misc/macros.h:18`, `src/message.c:423`). Client checks only the first byte of each read
  (`src/yabai.c:111`) and strips exactly one byte (`src/yabai.c:114` `rsp + 1`).
- **Client exit code**: `EXIT_FAILURE` if any chunk started with BEL, else `EXIT_SUCCESS`
  (`src/yabai.c:103`, `:112`, `:123`).
- **Client shutdown**: `shutdown(sockfd, SHUT_WR)` after send (`src/yabai.c:100`) — the daemon's read
  loop needs the half-close.

### 6.2 Query JSON

See §7 for the exact format strings. Contract in brief:
- Objects are `"{\n"` … `"\n}"` with `\t`-indented keys and **no space after the colon**.
- Keys appear in `WINDOW_PROPERTY_LIST` (`src/window.h:31-64`) / `SPACE_PROPERTY_LIST`
  (`src/view.h:7-19`) / `DISPLAY_PROPERTY_LIST` (`src/display.h:7-14`) order,
  filtered by the mask; separators are `",\n"` emitted **before** each key after the first
  (`did_output` pattern).
- Floats are `%.4f`; frames are a nested object with keys `x`,`y`,`w`,`h`.
- Arrays of ids use `", "` (comma **space**) between elements; arrays of objects use `","`
  (comma, no space).
- Top-level array terminators differ per query — see §7.4 for the quirks that must be preserved.

### 6.3 Signal environment variables

Set by `event_signal_push` (`src/event_signal.c:99-341`) and exported with `setenv(name, value, 1)`
in the forked child (`src/event_signal.c:86-89`). At most four per signal. Complete list with format:

| variable | format | signals | line |
| --- | --- | --- | --- |
| `YABAI_PROCESS_ID` | `%d` | application_launched / activated / deactivated / visible / terminated / hidden / front_switched | `:132`, `:143`, `:169`, `:180` |
| `YABAI_RECENT_PROCESS_ID` | `%d` | application_front_switched | `:171` |
| `YABAI_WINDOW_ID` | `%d` | window_created / focused / deminimized / destroyed / moved / resized / minimized / title_changed | `:194`, `:206`, `:221` |
| `YABAI_SPACE_ID` | `%lld` | space_created / space_destroyed / space_changed | `:237`, `:248`, `:268` |
| `YABAI_RECENT_SPACE_ID` | `%lld` | space_changed | `:270` |
| `YABAI_SPACE_INDEX` | `%d` | space_created / space_changed | `:239`, `:273` |
| `YABAI_RECENT_SPACE_INDEX` | `%d` | space_changed | `:275` |
| `YABAI_DISPLAY_ID` | `%d` | display_added / moved / resized / removed / changed | `:289`, `:300`, `:320` |
| `YABAI_RECENT_DISPLAY_ID` | `%d` | display_changed | `:322` |
| `YABAI_DISPLAY_INDEX` | `%d` | display_added / moved / resized / changed | `:291`, `:325` |
| `YABAI_RECENT_DISPLAY_INDEX` | `%d` | display_changed | `:327` |
| `YABAI_MISSION_CONTROL_MODE` | `%s` from `mission_control_mode_str` | mission_control_enter / exit | `:337` |

Each name and value is written into a 128-byte `ts` allocation (`arg_size = 128`,
`src/event_signal.c:104`) with `snprintf` — so a value longer than 127 chars truncates.

The command is run as `execvp("/usr/bin/env", ["sh", "-c", signal->command])`
(`src/event_signal.c:91`).

### 6.4 File paths in `/tmp`

| path | format | defined |
| --- | --- | --- |
| daemon socket | `/tmp/yabai_%s.socket` | `src/yabai.c:2` |
| lock file | `/tmp/yabai_%s.lock` | `src/yabai.c:3` |
| scripting-addition socket | `/tmp/yabai-sa_%s.socket` | `src/yabai.c:1`, `src/osax/common.h:4` |
| launchd stdout log | `/tmp/yabai_%s.out.log` | `src/misc/service.h:34` |
| launchd stderr log | `/tmp/yabai_%s.err.log` | `src/misc/service.h:36` |

`%s` is `$USER` in all five (or `getpwuid(SUDO_UID)->pw_name` for the SA socket when running as root,
`src/sa.m:145-160`).

**Lock file semantics** (`src/yabai.c:164-177`): `open(O_CREAT|O_WRONLY|O_CLOEXEC, 0600)` then
`fcntl(F_SETLK)` with `F_WRLCK`, whole file, `l_pid = getpid()`. The fd is **never closed** — the
lock is released by process exit. A Rust translation must keep the fd alive for the process
lifetime; letting a `File` drop releases the lock and allows a second instance.

### 6.5 Config file discovery

`get_config_file("yabairc", buf, size)` (`src/misc/helpers.h:445-461`), in order:
1. `$XDG_CONFIG_HOME/yabai/yabairc` (only if `XDG_CONFIG_HOME` is set **and non-empty**)
2. `$HOME/.config/yabai/yabairc`
3. `$HOME/.yabairc`

`exec_config_file` (`src/misc/helpers.h:463-487`) then runs it with `sh -c <path>` if executable, or
`sh <path>` if not, and emits a user-facing notification on failure
(`notify("configuration", ...)`).
`-c/--config <path>` overrides (`src/yabai.c:249-253`, into `g_config_file[4096]`).

### 6.6 launchd plist

`_NAME_YABAI_PLIST` is `"com.asmvik.yabai"` (`src/misc/service.h:5`);
path `%s/Library/LaunchAgents/com.asmvik.yabai.plist` with `NSHomeDirectoryForUser(NULL)`
(`src/misc/service.h:6`, `:80-99`). The XML body is `_YABAI_PLIST` (`src/misc/service.h:8-42`) with
four substitutions in order: executable path (`_NSGetExecutablePath`), `$PATH`, `$USER`, `$USER`.

**The literal bytes matter**, including the tab+spaces indentation anomaly on lines 29-31
(`" \t     <false/>\n"` — a space, a tab, five spaces). Copy the string verbatim into a Rust
`const`; do not reformat.

Sizing: `populate_plist` computes `strlen(_YABAI_PLIST)-8 + strlen(exe_path) + strlen(path_env) +
2*strlen(user) + 1` (`src/misc/service.h:119`) — the `-8` accounts for the four `%s` (2 chars each).
`*length = size-1` (`:127`) and the file is written with `fwrite(buf, length, 1, handle)`
(`src/misc/service.h:164`), i.e. **without a trailing NUL and without a trailing newline**.

`launchctl` invocations (`src/misc/service.h:215-311`), exact argv:
- `print gui/<uid>/com.asmvik.yabai` (output suppressed) — bootstrapped probe
- `enable gui/<uid>/com.asmvik.yabai`
- `bootstrap gui/<uid> <plist path>`
- `kickstart gui/<uid>/com.asmvik.yabai`
- `kickstart -k gui/<uid>/com.asmvik.yabai` (restart)
- `kill SIGTERM gui/<uid>/com.asmvik.yabai`
- `bootout gui/<uid> <plist path>`
- `disable gui/<uid>/com.asmvik.yabai`

`/bin/launchctl` is the hardcoded path (`src/misc/service.h:4`). `<uid>` is `getuid()` formatted
`%d`.

### 6.7 Scripting-addition socket protocol

The daemon is a **client** of the payload injected into Dock.app. Shared definitions in
`src/osax/common.h`, which stays C — the Rust side must mirror it by hand.

- Socket: `/tmp/yabai-sa_%s.socket`, `SA_SOCKET_BUFF_LEN = 0x1000` (`src/osax/common.h:4-5`).
- **Frame**: `int16_t length` (little-endian, host order) at offset 0, then a 1-byte opcode at
  offset 2, then packed arguments. Built by the macro trio at `src/sa.m:418-420`:
  ```c
  #define sa_payload_init() char bytes[SA_SOCKET_BUFF_LEN]; int16_t length = 1+sizeof(length)
  #define pack(v) memcpy(bytes+length, &v, sizeof(v)); length += sizeof(v)
  #define sa_payload_send(op) *(int16_t*)bytes = length-sizeof(length), bytes[sizeof(length)] = op, scripting_addition_send_bytes(bytes, length)
  ```
  So `length` starts at 3 (2 bytes of header + 1 opcode), arguments are appended raw with **native
  alignment ignored** (`memcpy`, so packed), and the header field written back is
  `length - 2` = opcode byte + payload bytes. The number of bytes actually sent is `length`.
- **Opcodes** (`src/osax/common.h:25-46`), values `0x01`–`0x13`:
  `HANDSHAKE 0x01`, `SPACE_FOCUS 0x02`, `SPACE_CREATE 0x03`, `SPACE_DESTROY 0x04`,
  `SPACE_MOVE 0x05`, `WINDOW_MOVE 0x06`, `WINDOW_OPACITY 0x07`, `WINDOW_OPACITY_FADE 0x08`,
  `WINDOW_LAYER 0x09`, `WINDOW_STICKY 0x0A`, `WINDOW_SHADOW 0x0B`, `WINDOW_FOCUS 0x0C`,
  `WINDOW_SCALE 0x0D`, `WINDOW_SWAP_PROXY_IN 0x0E`, `WINDOW_SWAP_PROXY_OUT 0x0F`,
  `WINDOW_ORDER 0x10`, `WINDOW_ORDER_IN 0x11`, `WINDOW_LIST_TO_SPACE 0x12`,
  `WINDOW_TO_SPACE 0x13`.
- **Argument layouts**, per `src/sa.m:442-621` — these are the exact field types and order:

  | opcode | fields |
  | --- | --- |
  | `SPACE_FOCUS/CREATE/DESTROY` | `u64 sid` |
  | `SPACE_MOVE` | `u64 src_sid, u64 dst_sid, u64 src_prev_sid, bool focus` (`:463-471`); the "after space" variant sends `dummy_sid = 0` for `src_prev_sid` (`:473-482`) |
  | `WINDOW_MOVE` | `u32 wid, i32 x, i32 y` |
  | `WINDOW_OPACITY` / `_FADE` | `u32 wid, f32 opacity, f32 duration` — opcode chosen by `duration > 0.0f` (`:499`) |
  | `WINDOW_LAYER` | `u32 wid, i32 layer` |
  | `WINDOW_STICKY` / `_SHADOW` | `u32 wid, bool value` (**1 byte**) |
  | `WINDOW_FOCUS` | `u32 wid` |
  | `WINDOW_SCALE` | `u32 wid, f32 x, f32 y, f32 w, f32 h` |
  | `WINDOW_SWAP_PROXY_IN` / `_OUT` | `i32 count`, then per entry either `u32 0` (skipped, **one** field) or `u32 wid, u32 proxy_id` (**two** fields) — a variable-width encoding (`:544-574`) |
  | `WINDOW_ORDER` | `u32 a_wid, i32 order, u32 b_wid` |
  | `WINDOW_ORDER_IN` | `i32 count`, then per entry `u32 0` if already ordered in, else `u32 wid` (`:586-602`) |
  | `WINDOW_LIST_TO_SPACE` | `u64 sid, i32 count`, then `u32 wid` × count |
  | `WINDOW_TO_SPACE` | `u64 sid, u32 wid` |

  `bool` is C `_Bool` = **1 byte**. There is **no padding** between fields. In Rust, build the frame
  with explicit `to_ne_bytes()` appends, never `#[repr(C)]` struct transmutes — the C is packed and a
  `#[repr(C)]` struct would insert alignment padding.
- **Handshake reply** (`src/sa.m:239-268`): the payload sends a NUL-terminated version string
  followed by a 4-byte `uint32_t attrib`. Compared against `OSAX_VERSION "2.1.30"`
  (`src/osax/common.h:7`) and `OSAX_ATTRIB_ALL = 0x7F` (`:17-23`).
- Every other opcode's reply is a single ignored byte (`recv(sockfd, &dummy, 1, 0)`, `src/sa.m:431`).

### 6.8 Scripting-addition bundle on disk

`/Library/ScriptingAdditions/yabai.osax` with `Contents/{Info.plist,MacOS/loader,Resources/payload.bundle/Contents/{Info.plist,MacOS/payload}}`
(`src/sa.m:78-94`). Two verbatim plists at `src/sa.m:21-48` and `:50-76` with
`CFBundleIdentifier` `com.asmvik.yabai-osax` / `com.asmvik.yabai-sa`, `CFBundleVersion`
`OSAX_VERSION`. Directories `0755` under `umask(S_IWGRP|S_IWOTH)` (`src/sa.m:204`).
Post-install runs `chmod +x` and `codesign -f -s -` on both binaries via `system()`
(`src/sa.m:122-137`), then terminates Dock.app (`src/sa.m:139-143`).
Loader invoked with `popen("/Library/ScriptingAdditions/yabai.osax/Contents/MacOS/loader", "r")`
(`src/sa.m:335`).

`__src_osax_payload` / `__src_osax_loader` and their `_len` (`src/sa.h:4-7`) are produced by
`xxd -i -a` in the makefile (`makefile:31-32`). In the Rust build these become
`include_bytes!` of artifacts the build script produces with `xcrun clang` — **the symbol names no
longer matter**, but the byte content and the `chmod`/`codesign` post-processing do.

### 6.9 CLI surface

`src/yabai.c:5-21` defines the flags; `src/yabai.c:183-258` parses them. The `--help` text
(`src/yabai.c:185-200`) and the `--version` string `"yabai-v%d.%d.%d\n"` (`src/yabai.c:206`) with
`MAJOR 7 / MINOR 1 / PATCH 25` (`src/yabai.c:23-25`) are byte-for-byte contracts:
`makefile:44` parses `yabai --version` output with `cut -d "v" -f 2`, and
`scripts/install.sh` is rewritten from it.

Flags are checked **only against `argv[1]`** for the subcommand forms (`src/yabai.c:183-241`), and
only the `-V/--verbose` and `-c/--config` forms loop over all of `argv` (`:243-257`). So
`yabai -V --version` prints nothing about the version and errors on `-V`? No — `argv[1]` is `-V`,
which is not one of the early-return options, so the loop runs and sets `g_verbose`, then errors on
`--version` being "not a valid option". Preserve the ordering.

### 6.10 SkyLight connection notification ids

Registered in `src/yabai.c:311-334` and dispatched in `src/mission_control.c:7-26`:
`1204` (mission control enter, pre-Monterey), `1327`/`1328` (space created/destroyed, Ventura+),
`808` (window ordered), `1202` (cmd-tab), `804` (window destroyed, Sequoia+). Also `1325`/`1326`
sent **outward** to jankyborders via mach (`src/window_manager.c:579`, `:653`, `:654`, `:689`).
These numbers are an external ABI with both SkyLight and jankyborders.

---

## 7. Exact JSON serialisation formats

Everything here is a literal transcription of the C format strings. `%d` → `{}`, `%lld` → `{}`,
`%s` → `{}`, `%.4f` → `{:.4}`, `%c` → `{}`. **Rust's `{:.4}` and C's `%.4f` agree** for finite
values (both round-half-to-even at the printed digit and both print a fixed number of decimals);
they differ for infinities (`inf` vs `inf`) and NaN (`nan` vs `NaN`) — neither occurs in these
fields.

### 7.1 Window object — `window_serialize` (`src/window.c:409-712`)

Opening `"{\n"`. Each present property emits `",\n"` first if anything was emitted before it
(`did_output`), then its own fragment. Closing `"\n}"`.

```
\t"id":%d                                window->id                          (u32)
\t"pid":%d                               window->application->pid
\t"app":"%s"                             escaped_app ?: app
\t"title":"%s"                           escaped_title ?: title
\t"scratchpad":"%s"                      window->scratchpad ?: ""
\t"frame":{\n\t\t"x":%.4f,\n\t\t"y":%.4f,\n\t\t"w":%.4f,\n\t\t"h":%.4f\n\t}
\t"role":"%s"                            window_role_ts(window)
\t"subrole":"%s"                         window_subrole_ts(window)
\t"root-window":%s                       json_bool(window->is_root)
\t"display":%d                           display_manager_display_id_arrangement(space_display_id(sid))
\t"space":%d                             space_manager_mission_control_index(sid)
\t"level":%d
\t"sub-level":%d
\t"layer":"%s"                           window_layer(level)
\t"sub-layer":"%s"                       window_layer(sub_level)
\t"opacity":%.4f                         window_opacity(window->id)
\t"split-type":"%s"                      window_node_split_str[node && node->parent ? node->parent->split : 0]
\t"split-child":"%s"                     window_node_child_str[node ? (left ? CHILD_FIRST : CHILD_SECOND) : CHILD_NONE]
\t"stack-index":%d                       node && node->window_count > 1 ? index_of(node, id)+1 : 0
\t"can-move":%s
\t"can-resize":%s
\t"has-focus":%s                         window->id == g_window_manager.focused_window_id
\t"has-shadow":%s
\t"has-parent-zoom":%s                   node && node->zoom && node->zoom == node->parent
\t"has-fullscreen-zoom":%s               node && node->zoom && node->zoom == view->root
\t"has-ax-reference":%s                  json_bool(true)
\t"is-native-fullscreen":%s
\t"is-visible":%s                        ordered_in && !is_minimized && !app->is_hidden && (is_sticky || space_is_visible(sid))
\t"is-minimized":%s
\t"is-hidden":%s
\t"is-floating":%s
\t"is-sticky":%s
\t"is-grabbed":%s                        window == g_mouse_state.window      -- NOTE: does NOT set did_output
```

`json_bool` prints `"true"`/`"false"` unquoted (`src/misc/helpers.h:233-236`).
`window_layer` returns `"unknown"` for a level matching none of the three
(`src/window.c:113-119`).

**`window_nonax_serialize`** (`src/window.c:121-406`) emits the **same keys in the same order** but
with placeholders for everything it cannot know: `"scratchpad":""`, `"role":""`, `"subrole":""`,
`"split-type"` = `window_node_split_str[0]` = `"none"`, `"split-child"` = `"none"`,
`"stack-index":0`, `can-move`/`can-resize`/`has-focus`/`has-parent-zoom`/`has-fullscreen-zoom`/
`has-ax-reference`/`is-visible`/`is-minimized`/`is-hidden`/`is-floating`/`is-grabbed` all `false`.
`"app"` comes from `proc_name()` into a function-local `static char[PROC_PIDPATHINFO_MAXSIZE]`
(`src/window.c:173-174`), `"title"` from `window_property_title_ts` (`src/window.c:186`),
`"frame"` from `SLSGetWindowBounds` (`src/window.c:202-206`), `"root-window"` from
`window_parent(wid) == 0` (`src/window.c:227`).

### 7.2 Space object — `view_serialize` (`src/view.c:860-964`)

```
\t"id":%lld                              view->sid
\t"uuid":"%s"                            ts_cfstring_copy(view->uuid) ?: "<unknown>"
\t"index":%d                             space_manager_mission_control_index(view->sid)
\t"label":"%s"                           space_label ? space_label->label : ""
\t"type":"%s"                            view_type_str[view->layout]
\t"display":%d
\t"windows":[%d, %d, ... %d]             "%d, " for all but last, "%d" for last   (src/view.c:917-925)
\t"first-window":%d                      first_leaf ? first_leaf->window_order[0] : 0
\t"last-window":%d
\t"has-focus":%s
\t"is-visible":%s
\t"is-native-fullscreen":%s              -- does NOT set did_output
```

Note the `"windows"` array separator is **`", "` with a space**.

### 7.3 Display object — `display_serialize` (`src/display.c:20-99`)

```
\t"id":%d                                did
\t"uuid":"%s"                            ts_cfstring_copy(display_uuid(did)) ?: "<unknown>"
\t"index":%d
\t"label":"%s"
\t"frame":{\n\t\t"x":%.4f,\n\t\t"y":%.4f,\n\t\t"w":%.4f,\n\t\t"h":%.4f\n\t}     CGDisplayBounds(did)
\t"spaces":[%d, %d, ...]                 first_mci + i, "%d, " / "%d"        (src/display.c:77-88)
\t"has-focus":%s                         -- does NOT set did_output
```

`"spaces"` is computed as `first_mci + i` — the mission-control index of the **first** space on the
display plus the loop counter, which assumes contiguous indices.

### 7.4 Top-level array wrappers, with their quirks

| function | line | shape |
| --- | --- | --- |
| `display_manager_query_displays` | `src/display_manager.c:5-21` | `"["` + per item `serialize` + `"%c"` (`,` or `]`) + `"\n"` — **emits nothing (no `]`) when `count == 0`**, producing `"[\n"` |
| `space_manager_query_spaces_for_window` | `src/space_manager.c:26-44` | same `%c` pattern; **`continue` on a null view skips the separator**, so a skipped last element leaves the array unterminated |
| `space_manager_query_spaces_for_display` | `src/space_manager.c:46-65` | same |
| `space_manager_query_spaces_for_displays` | `src/space_manager.c:67-93` | nested: inner loop uses `","`, outer uses `%c` |
| `space_manager_query_space` | `src/space_manager.c:14-24` | single object + `"\n"` |
| `window_manager_query_windows_for_spaces` | `src/window_manager.c:38-52` | `"["` + `","` between + **unconditional `"]\n"`** — so an empty result is `"[]\n"` |
| `window_manager_query_window_rules` | `src/window_manager.c:26-35` | `"["` … `"]\n"` |
| `event_signal_list` | `src/event_signal.c:431-454` | `"["` + per-type comma bookkeeping via `event_did_output` + `"]\n"` |
| single window query | `src/message.c:2583-2584` | object + `"\n"` |

The asymmetry between the `%c`-terminated arrays (display/space) and the `"]\n"`-terminated ones
(window/rule/signal) is real and must be preserved. So must the empty-array behaviour, which differs
between them.

### 7.5 Rule object — `rule_serialize` (`src/rule.c:4-61`)

A single `fprintf` with the whole object; unlike the others it is **not** property-masked, so all
19 keys always appear, and the separators are baked into the format string:

```
{\n
\t"index":%d,\n
\t"label":"%s",\n
\t"app":"%s",\n
\t"title":"%s",\n
\t"role":"%s",\n
\t"subrole":"%s",\n
\t"display":%d,\n
\t"space":%d,\n
\t"follow_space":%s,\n
\t"opacity":%.4f,\n
\t"manage":%s,\n              json_optional_bool -> null|true|false
\t"sticky":%s,\n
\t"mouse_follows_focus":%s,\n
\t"sub-layer":"%s",\n         RULE_LAYER ? layer_str[effects.layer] : ""
\t"native-fullscreen":%s,\n
\t"grid":"%d:%d:%d:%d:%d:%d",\n
\t"scratchpad":"%s",\n
\t"one-shot":%s,\n
\t"flags":"0x%08x"\n
}
```

`"flags"` is `(uint32_t)(effects.flags << 16) | (uint32_t)rule->flags` (`src/rule.c:60`), zero-padded
to 8 hex digits — Rust `{:08x}`.

### 7.6 Signal object — `event_signal_serialize` (`src/event_signal.c:400-429`)

```
{\n
\t"index":%d,\n
\t"label":"%s",\n
\t"app":"%s",\n
\t"title":"%s",\n
\t"active":%s,\n              json_optional_bool(signal->active)
\t"event":"%s",\n             signal_type_str[type]
\t"action":"%s"\n
}
```

### 7.7 Scalar config responses

All in `src/message.c`. The format is chosen per setting and is observable:

| setting | format | line |
| --- | --- | --- |
| booleans (`debug_output`, `mouse_follows_focus`, `window_zoom_persist`, `skip_window_focus_animation`, `window_opacity`) | `"%s\n"` from `bool_str` | `:1173`, `:1184`, `:1258`, `:1269`, `:1280` |
| `window_opacity_duration`, `window_animation_duration` | `"%f\n"` (**six decimals**, not `%.4f`) | `:1291`, `:1300` |
| `menubar_opacity`, `active_window_opacity`, `normal_window_opacity`, `split_ratio` | `"%.4f\n"` | `:1346`, `:1355`, `:1364`, `:1545` |
| `insert_feedback_color` | `"0x%x\n"` (**no zero padding**) | `:1373` |
| paddings, `window_gap` | `"%d\n"` | `:1384`-`:1487` |
| enum settings | `"%s\n"` from the matching `*_str` table | throughout |
| `external_bar` | `"%s:%d:%d\n"` | `:1691` |

`%f` vs `%.4f` is not a typo to fix — `yabai -m config window_animation_duration` returns
`0.000000` today and scripts parse it.

### 7.8 String escaping

`ts_string_escape` (`src/misc/helpers.h:259-319`) escapes `"` `\` `\b` `\f` `\n` `\r` `\t` to their
two-character forms and **every other byte in `0x00..=0x1f`** to `\u%04x` (lowercase hex,
`src/misc/helpers.h:311`). Bytes `>= 0x80` pass through unescaped — the output is UTF-8 as
received from `CFStringGetCString(..., kCFStringEncodingUTF8)`.

The size computation counts 1 extra byte per simple escape and 5 extra per `\uXXXX`
(`:272`, `:274`). Note `*cursor >= 0x00` on a **signed** `char` means bytes `0x80..0xff` are negative
and fail the test, which is why they pass through. A Rust port over `u8` must write the condition as
`c <= 0x1f` only (the `>= 0x00` is vacuous for `u8`), which gives the same result.

Applied to: window `app` and `title` (`src/window.c:177-190`, `:474-489`), rule `app`/`title`/`role`/
`subrole` (`src/rule.c:13-16`), signal `app`/`title`/`action` (`src/event_signal.c:408-410`).
**Not** applied to: `label`, `scratchpad`, `uuid`, `role`/`subrole` in `window_serialize`
(`src/window.c:507-519` prints `window_role_ts` unescaped). That inconsistency is current behaviour.

---

## 8. Semantic traps, ranked by how much damage a wrong translation does

1. **`window->id_ptr` self-referential atomic pointer** (`src/window.h:92`). The idiom
   `__sync_bool_compare_and_swap(&window->id_ptr, &window->id, NULL)` is a CAS whose *expected* value
   is the address of a field in the same struct. It is used from the AX callback thread
   (`src/application.c:36`) to invalidate a window whose destroy event is already queued, and tested
   from the event loop (`src/event_loop.c:647`, `:681`, `:731`, `:833`, `:877`, `:930`, `:960`,
   `:1159`, `:1240`). In Rust the `Window` must be heap-allocated and never moved after `id_ptr` is
   initialised. A `Vec<Window>` that reallocates, or a `Window` returned by value, silently breaks
   the liveness check and reintroduces the use-after-free this guards against.
2. **`rsp` may be `NULL`** (`src/message.c:1706`, `:1765`). A `Response` type that cannot be silent
   turns optional-selector probing into spurious error output on stdout for the user.
3. **`static` is not private** (§0.1). Sixteen functions.
4. **In-place mutation of the message buffer** by `parse_key_value_pair` (`src/message.c:462`) and
   `parse_properties` (`src/message.c:638`). A `&str`-based parser cannot express this and will
   produce different `token.text` contents for subsequent tokens.
5. **`flags |= ~flags` when zero** (`src/window.c:123`, `:411`, `src/display.c:24`, `src/view.c:864`).
   Sets all 64 bits, including undefined ones. A translator who writes
   `if flags == 0 { flags = ALL_PROPERTIES }` changes nothing observable today but breaks the moment
   a property bit is added.
6. **Three-valued regex/tri-state comparisons** (§5.13, §5.15). Collapsing `REGEX_MATCH_UD` into
   "no match" inverts every rule that omits a filter.
7. **`purify_mode_str` is inverted** relative to the enum names (`src/window_manager.h:33-38`).
8. **`(int)` truncation in `area_make_pair`** (`src/view.c:170-182`) and the `+0.5f` rounding.
9. **`area_distance_in_direction` truncates `f64` to `i32`** (`src/view.c:563-582`) and uses
   `INT_MAX` as "no candidate".
10. **`window_list[1024]` has no bounds check** (`src/event_loop.c:19-31`). A Rust array indexes and
    panics where C corrupts the stack. Prefer the panic; do not silently grow the array, because the
    1024 cap also caps what is sent to `SLSRequestNotificationsForWindows`.
11. **`cgimage_restore_alpha` reads past the buffer** when `width*height % 4 != 0`
    (`src/misc/helpers.h:613`). Same call: reproduce the loop, and note the overrun.
12. **`mouse_mod_str` is indexed by bit value, not ordinal** (`src/mouse_handler.h:83-91`,
    read at `src/message.c:1621`). A dense 6-entry table gives the wrong string.
13. **`event_signal_flush` forks a process that reads globals** (`src/event_signal.c:60-97`). Any
    Rust global behind a lazily-initialised lock is a deadlock waiting in the child.
14. **The lock file's fd is never closed** (`src/yabai.c:164-177`). Dropping a Rust `File` releases
    the `fcntl` lock.
15. **`require()` exits with `EXIT_SUCCESS`** (`src/misc/log.h:37-46`). Getting it wrong makes
    launchd respawn yabai forever.
16. **`table_add` does not overwrite** (`src/misc/hashtable.h:120-123`) but `HashMap::insert` does.
17. **`buf_del` is `swap_remove`**, not `remove` (`src/misc/sbuffer.h:19`), and `src/event_loop.c:173`
    compensates with `--i`.
18. **`mt` is uninitialised if the easing switch falls through** (`src/window_manager.c:548-553`).
    Rust forces an exhaustive `match`; keep it exhaustive.
19. **`token_is_float` accepts `inf`, `nan` and hex floats** because it is `strtof`
    (`src/message.c:386`). `str::parse::<f32>()` does not accept hex floats and accepts a different
    grammar.
20. **Signed-`char` comparison in `ts_string_escape`** (`src/misc/helpers.h:273`, `:308`) is what
    lets UTF-8 continuation bytes through. Over `u8` the condition must be written as `c <= 0x1f`
    alone.
21. **`window_unknown_serialize` is declared but never defined** (`src/window.h:140`). Do not create
    a stub for it.
22. **`enum mission_control_mode` is declared in a `.c` file** (`src/mission_control.c:29-36`) while
    its global lives in `src/yabai.c:37`. Whichever Rust module owns the enum, the other must import
    it, not redefine it.
23. **`%f` vs `%.4f`** for the two duration settings (§7.7).
24. **Empty-array JSON differs** between the `%c`-terminated and `"]\n"`-terminated queries (§7.4).

---

## 9. What this sweep implies for module layout

Phase 2 keeps one Rust module per C file pair. But six things are shared by all ten translators and
must land **before** any of them starts, or ten incompatible copies appear:

| shared piece | source | who owns it |
| --- | --- | --- |
| `Response` + `rsp!` + `daemon_fail!` | `src/misc/macros.h:18`, `src/message.c:418-437` | `message` |
| `debug!` / `warn` / `error!` / `require!` / `debug_message` | `src/misc/log.h` | `log` |
| `time_function!` (no-op) | `src/misc/timer.h` | `timer` |
| `string_equals`, `json_bool`, `json_optional_bool`, `ts_string_escape`, `ts_string_copy`, `ts_cfstring_copy`, `min`/`max`/`in_range_*`/`lerp`/`clampf_range` | `src/misc/helpers.h`, `src/misc/macros.h` | `helpers` |
| `Token` / `TokenValue` / `get_token` / `token_*` | `src/message.c:254-416` | `message` |
| `PosixRegex` + `RegexMatch` + `regex_match` | `src/misc/helpers.h:573-579`, `src/misc/macros.h:22-24` | `helpers` |
| `MAXLEN`, `FAILURE_MESSAGE`, `DIR_*`, `STACK`, `TYPE_*`, `HANDLE_*`, `LAYER_*` | `src/misc/macros.h` | `macros` |
| `Vec`-backed `buf_*` idiom, `HashMap`-backed `table_*` idiom, the `ts` arena | `src/misc/sbuffer.h`, `src/misc/hashtable.h`, `src/misc/ts.h` | `misc-containers-and-allocators` sweep |
| `SaOpcode` + the packed frame builder | `src/osax/common.h`, `src/sa.m:418-420` | `sa` |
| `workspace_is_macos_*` | `src/workspace.h:4-19` | `workspace` |
