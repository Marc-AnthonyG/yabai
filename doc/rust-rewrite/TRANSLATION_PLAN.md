# Translation plan — phase 2 execution, phase 3 sketch

This document elaborates decisions 2, 6, 9, 10 and 13 of `DECISIONS.md` into something a
translator follows mechanically. `DECISIONS.md` is binding; where an inventory in `files/` or
`sweeps/` recommended something else, this plan carries the decision, not the recommendation.

Nothing here is a menu. Every path, every name, every ordering below is the choice. A phase 2
agent that finds itself picking between two options has found a gap in this document and must
say so rather than pick.

Conventions used throughout:

* C locations are `path:line`.
* Rust sketches obey decisions 37-39: full names, no abbreviated bindings, and no comments
  except comments that exist in the C source.
* "C pair" means a `.c`/`.m` and its `.h`, as `src/manifest.m:45-97` includes them.

---

## 1. The target tree at the end of phase 2

Phase 2 ends when `bin/yabai` is a Rust universal binary built by `make`, the C daemon sources
are gone, and `src/osax/` is untouched C. The tree at that moment:

```
Cargo.toml
rust-toolchain.toml
build.rs
makefile
assets/Info.plist
src/main.rs
src/globals.rs
src/state.rs
src/event_loop.rs
src/event_signal.rs
src/mission_control.rs
src/message.rs
src/rule.rs
src/view.rs
src/window.rs
src/application.rs
src/process_manager.rs
src/display.rs
src/display_manager.rs
src/space.rs
src/space_manager.rs
src/window_manager.rs
src/mouse_handler.rs
src/workspace.rs
src/sa.rs
src/misc.rs
src/misc/macros.rs
src/misc/log.rs
src/misc/notify.rs
src/misc/timer.rs
src/misc/table.rs
src/misc/regex.rs
src/misc/response.rs
src/misc/helpers.rs
src/misc/service.rs
src/misc/hashtable.h
src/ffi.rs
src/ffi/skylight.rs
src/ffi/skylight_dynamic.rs
src/ffi/core_graphics.rs
src/ffi/color_sync.rs
src/ffi/accessibility.rs
src/ffi/carbon_process.rs
src/ffi/carbon_events.rs
src/ffi/carbon_core.rs
src/ffi/core_video.rs
src/ffi/core_foundation.rs
src/ffi/appkit.rs
src/ffi/foundation.rs
src/ffi/mach_port.rs
src/ffi/macho.rs
src/ffi/libsystem.rs
src/ffi/dispatch.rs
src/osax/common.h
src/osax/loader.m
src/osax/payload.m
src/osax/arm64_payload.m
src/osax/x64_payload.m
```

Four layout rules that are not negotiable:

1. **No `src/misc/mod.rs` and no `src/ffi/mod.rs`.** The 2018 form is used: `src/misc.rs` plus
   the directory `src/misc/`, `src/ffi.rs` plus the directory `src/ffi/`. One form in the
   crate, and this is it.
2. **No `src/osax.rs` is ever created.** Creating it would make `src/osax/` that module's
   directory. The osax constants come from `OUT_DIR/osax_common.rs` (decision 9).
3. **`autoexamples = false`.** `examples/yabairc` and `examples/skhdrc` are shell config
   samples; the flag stops a future `examples/*.rs` from becoming a build target.
4. **`src/misc/hashtable.h` stays on disk as C.** It is the only file outside `src/osax/` that
   the scripting addition includes: `src/osax/payload.m:36-38` is
   `#define HASHTABLE_IMPLEMENTATION` / `#include "../misc/hashtable.h"` /
   `#undef HASHTABLE_IMPLEMENTATION`, outside any `#if`, and `payload.m` makes five `table_*`
   calls against it (`:660`, `:704`, `:725`, `:742`, `:1096`). Decision 1 forbids editing
   `src/osax/`, so the include cannot be redirected and the header cannot move. `src/misc/table.rs`
   is a second, independent consumer of the same algorithm, never a replacement for the header.
   `W3-5` therefore exempts it from the deletion, and `build.rs` lists it in `rerun-if-changed`
   (§1.3 item 6).

During phase 2 the Rust files sit next to the C files (decision 6). Cargo reads only what is
reachable from `src/main.rs`; `src/manifest.m:80-97` names every C file explicitly. The two
builds do not see each other, so `make` keeps working until work unit `W3-5` deletes the C.

### 1.1 `Cargo.toml`, full text

```toml
[package]
name         = "yabai"
version      = "7.1.25"
edition      = "2024"
rust-version = "1.85"
publish      = false
autoexamples = false
autobenches  = false
build        = "build.rs"

[[bin]]
name = "yabai"
path = "src/main.rs"

[dependencies]
objc2                    = "0.6.4"
objc2-foundation         = "0.3.2"
objc2-app-kit            = "0.3.2"
objc2-core-foundation    = "0.3.2"
objc2-core-graphics      = "0.3.2"
objc2-application-services = "0.3.2"
objc2-core-video         = "0.3.2"
block2                   = "0.6.2"
dispatch2                = "0.3.1"
libc                     = "0.2.189"

[profile.dev]
opt-level       = 0
debug           = true
overflow-checks = false
panic           = "unwind"

[profile.release]
opt-level        = 3
debug            = false
debug-assertions = false
overflow-checks  = false
lto              = "fat"
codegen-units    = 1
panic            = "unwind"
```

Notes a translator needs:

* `version = "7.1.25"` is the single source of truth for `MAJOR`/`MINOR`/`PATCH`
  (`src/yabai.c:23-25`). Both C uses are pure string interpolation
  (`src/yabai.c:200`, `src/yabai.c:206`), so `env!("CARGO_PKG_VERSION_MAJOR")` and friends
  replace the integers. `make archive` and `make publish` read `bin/yabai --version`
  (`makefile:45-46`, `makefile:54`), so a release bump happens here and nowhere else.
* `panic = "unwind"` in both profiles is decision 7. It is not a preference: under
  `panic = "abort"` the process dies on the `removeObserver` exception yabai deliberately
  swallows. The abort behaviour comes from the hook in `main`, §1.4.
* `overflow-checks = false` in both profiles is decision 8. Arithmetic that relied on wrapping
  writes `wrapping_add` / `wrapping_sub` / `wrapping_mul` explicitly anyway.
* `debug-assertions` follows the profile default: on in dev, off in release. That is the exact
  mapping of `-DNDEBUG` being release-only (`makefile:27` versus `makefile:4`), which is what
  makes decision 33's `debug_assert!` correct.
* `lto = "fat"` + `codegen-units = 1` recovers in release what the unity build gives clang
  today: `src/manifest.m` hands clang one translation unit, so every `static inline` in
  `view.c`, `helpers.h` and friends inlines everywhere.
* `strip` stays at its default `false`. The C release build does not strip (`makefile:27`), and
  stripping would change what `codesign` seals.
* The dependency list is decision 11, closed. No `regex`, `serde`, `bitflags`, `crossbeam`,
  `indexmap`, `bumpalo`, `bindgen`, `mach2`, `objc2-color-sync`, no `cc` build-dependency, and
  no servo `core-foundation` / `core-graphics`.

### 1.2 `rust-toolchain.toml`, full text

```toml
[toolchain]
channel    = "stable"
targets    = ["aarch64-apple-darwin", "x86_64-apple-darwin"]
components = ["rustfmt", "clippy", "rust-src"]
```

Stable, not nightly. Nightly is needed only by `make asan` / `make tsan`, which invoke
`cargo +nightly` explicitly (§1.8).

### 1.3 `build.rs` responsibilities

`build.rs` replaces the whole `xxd` pipeline (`makefile:30-36`) and the two generated
`*_bin.c` files. It has exactly six responsibilities and no others.

1. **Compile `src/osax/payload.m`** with `xcrun clang`, transcribing `makefile:31`:
   `-shared -fPIC -O3 -mmacosx-version-min=11.0 -arch x86_64 -arch arm64e`,
   `-F/System/Library/PrivateFrameworks`, `-framework SkyLight -framework Foundation
   -framework Carbon`, output `OUT_DIR/payload`.
2. **Compile `src/osax/loader.m`**, transcribing `makefile:32`:
   `-O3 -mmacosx-version-min=11.0 -arch x86_64 -arch arm64e -framework Cocoa`, output
   `OUT_DIR/loader`. A non-zero exit from either `clang` fails the build with the captured
   stderr.
3. **Generate `OUT_DIR/osax_common.rs` from `src/osax/common.h`** (decision 9). It parses the
   `#define` lines and the single flat `enum sa_opcode` with explicit hex values, and emits
   `OSAX_VERSION`, `SA_SOCKET_PATH_FMT`, `SA_SOCKET_BUFF_LEN`, the seven `OSAX_ATTRIB_*` bits
   (`src/osax/common.h:9-23`) and the nineteen `SaOpcode` variants (`src/osax/common.h:25-46`).
   If the header cannot be parsed the build **fails**; it never emits a partial file and never
   falls back to a hand-written copy. `common.h` is the wire ABI between the Rust daemon and
   the C payload, which compares `OSAX_VERSION` at `src/osax/payload.m:945` and `:949`; a
   silent drift there presents as "the scripting addition stopped working", not as a compile
   error.
4. **Emit the link configuration**, all from the makefile:
   * `cargo:rustc-link-search=framework=/System/Library/PrivateFrameworks` (`makefile:1`)
   * `cargo:rustc-link-lib=framework=` for `Carbon`, `Cocoa`, `CoreServices`, `CoreVideo`,
     `SkyLight`, in that order (`makefile:2`)
   * `cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,<absolute path to
     assets/Info.plist>` (`makefile:4`). The path is built from `CARGO_MANIFEST_DIR` and is
     absolute. `-Wl,a,b,c` splits into the three linker arguments ld64's `-sectcreate` needs.
     This is the highest-consequence line in the file: get it wrong and the build still
     succeeds, the daemon still runs, and every user loses their Accessibility grant because
     the signing identifier changed from `com.asmvik.yabai` to `yabai-<hash>`.
5. **Emit `cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET=11.0`** (decision 9). Without it the
   `x86_64-apple-darwin` slice emits `LC_VERSION_MIN_MACOSX version 10.12` where the C build
   emits `LC_BUILD_VERSION minos 11.0`, and the two slices disagree.
6. **Emit a complete `cargo:rerun-if-changed` list**: `build.rs`, `src/osax/payload.m`,
   `src/osax/loader.m`, `src/osax/arm64_payload.m`, `src/osax/x64_payload.m`,
   `src/osax/common.h`, `src/misc/hashtable.h`, `assets/Info.plist`. The last five are the ones
   `makefile:30` forgets; `payload.m` `#include`s the two arch payloads (`:30`, `:32`) and
   `src/misc/hashtable.h` (`:37`).

Explicitly not `build.rs`'s job: signing anything (the osax is signed at install time on the
user's machine, `src/sa.m:122-137`), running `lipo` (that is the makefile, §1.8), and using the
`cc` crate (it cannot emit a fat `x86_64 + arm64e` standalone dylib).

`build.rs` runs once per `--target`, so the osax compiles twice for a universal build. The
`-arch` flags are hard-coded and host-independent, so both runs produce byte-identical
artifacts and `lipo` on the two Rust slices is safe.

### 1.4 `src/main.rs`

`src/main.rs` is the crate root and holds the bodies of `src/yabai.c`'s four functions
(`client_send_message` `:54`, `configure_settings_and_acquire_lock` `:128`, `parse_arguments`
`:181`, `main` `:261`), plus the module tree and the panic hook.

The first statement of `main`, before argument parsing, installs the hook required by
decision 7:

```rust
fn main() {
    std::panic::set_hook(Box::new(|panic_information| {
        eprintln!("{panic_information}");
        std::process::abort();
    }));
```

The hook is what makes a panic take the whole daemon down. `panic = "unwind"` exists so that
`objc2::exception::catch` can catch the ObjC exception yabai deliberately swallows
(decision 7); it is not there to let a thread die quietly. No `catch_unwind` appears anywhere
in the crate except inside `objc2::exception::catch`.

Two further things live beside `main` because decisions 12 and 18 require containers that every
module reads, and those containers cannot sit inside a module that also holds behaviour:

* **`src/globals.rs`** — declarations only, no function bodies. It holds the process-wide
  values of decision 18 as `OnceLock` statics (`g_connection`, `g_pid`, `g_bs_port`,
  `g_layer_normal_window_level`, `g_layer_below_window_level`, `g_layer_above_window_level`,
  `g_cv_host_clock_frequency`, `g_sa_socket_file`, `g_socket_file`, `g_config_file`,
  `g_lock_file`, and the two runtime-resolved SkyLight pointers, which live in
  `src/ffi/skylight_dynamic.rs` and are re-exported from here), `g_verbose` as an `AtomicBool`,
  and the four `__`-prefixed event-loop flags as atomics. Source: `src/yabai.c:27-52` plus
  `src/misc/log.h:4`.
* **`src/state.rs`** — the `EventLoopOwnedState` struct of decision 12 and nothing else, with
  exactly the fields and the field order `patterns/state-and-ownership.md` §1.1 declares:

  ```rust
  pub struct EventLoopOwnedState {
      pub process_manager: ProcessManager,
      pub display_manager: DisplayManager,
      pub space_manager: SpaceManager,
      pub window_manager: WindowManager,
      pub mouse_drag_state: MouseDragState,
      pub mission_control_mode: MissionControlMode,
      pub signal_event: [Vec<Signal>; SIGNAL_TYPE_COUNT],
      pub signal_storage: Vec<PendingSignal>,
      pub focus_follows_mouse_suspended_value: FfmMode,
      pub is_menu_open: i32,
  }
  ```

  That order is the **canonical manager order** and is what §3.2's appended-parameter rule
  refers to. Eight fields come from `src/yabai.c:27-37`; the last two come from the two
  file-scope statics `static enum ffm_mode ffm_value;` and `static int is_menu_open = 0;` at
  `src/event_loop.c:1561-1562`, which the unity build makes program-wide singletons owned by
  the event-loop thread. `mouse_drag_state` is the drag half of `struct mouse_state`
  (decision 23); the tap half is a static of atomics in `src/globals.rs`.

  `g_event_bytes` (`src/yabai.c:42`) is **not** a field. It becomes a zeroed `[u8; 0x100]`
  local in each of the two functions that use it, `window_manager_make_key_window`
  (`src/window_manager.c:1280-1290`) and `window_manager_focus_window_without_raise`
  (`:1298-1317`); both `memset` bytes `0..0xf8` before every use and never read `0xf8..0x100`.
  One `DEVIATIONS.md` line. `g_workspace_context` (`src/yabai.c:35`) is not a field either — it is read
  on the main thread at `src/process_manager.c:191`, `:262` and `src/window_manager.c:2753`,
  so it is `static WORKSPACE_CONTEXT: OnceLock<WorkspaceContext>`.

  The struct carries the `unsafe impl Send` justified in `THREADS.md`. It holds no methods in
  phase 2; the handlers stay free functions taking `&mut` parameters per decision 13.

`src/main.rs` declares the whole tree, in `src/manifest.m:45-97` order so the two files can be
read side by side. This is the tree **at the end of wave 1**, not at the end of `W0-3`: a `mod`
line whose file does not exist is a compile error, and every wave gate is a `cargo check`
(§3). Each `mod` line is therefore added by the unit that creates its file, inserted at its
position in this list, so the final order is the one below:

```rust
mod ffi;
mod misc;
mod globals;
mod state;
mod view;
mod sa;
mod event_loop;
mod event_signal;
mod workspace;
mod rule;
mod message;
mod display;
mod space;
mod window;
mod process_manager;
mod application;
mod display_manager;
mod space_manager;
mod window_manager;
mod mouse_handler;
mod mission_control;
```

Who writes which line:

* `W0-3` writes `mod ffi;`, `mod misc;`, `mod globals;`, `mod state;` and nothing else, and
  creates `src/ffi.rs` and `src/misc.rs` empty alongside `src/globals.rs` and `src/state.rs`.
  An empty `src/ffi.rs` and an empty `src/misc.rs` compile; the `pub mod` lines arrive with
  their files.
* `W0-13` adds `mod sa;` when it creates `src/sa.rs`.
* `W1-main` adds the remaining sixteen lines — `view`, `event_loop`, `event_signal`,
  `workspace`, `rule`, `message`, `display`, `space`, `window`, `process_manager`,
  `application`, `display_manager`, `space_manager`, `window_manager`, `mouse_handler`,
  `mission_control` — each at its position above. `W1-main` depends on every other wave 1 unit
  (§3.3), so all sixteen files exist when it runs, and the wave 1 gate is the first
  `cargo check` that sees them.

`src/misc.rs` and `src/ffi.rs` contain only `pub mod` lines and nothing else. They too are
grown one line at a time by the unit that creates the module:

| File | Line | Added by |
| --- | --- | --- |
| `src/ffi.rs` | `pub mod skylight;`, `pub mod skylight_dynamic;` | `W0-4` |
| `src/ffi.rs` | the other fifteen `pub mod` lines, in the §1.5 table order | `W0-5` |
| `src/misc.rs` | `pub mod macros;`, `pub mod log;`, `pub mod timer;` | `W0-6` |
| `src/misc.rs` | `pub mod table;` | `W0-7` |
| `src/misc.rs` | `pub mod regex;` | `W0-8` |
| `src/misc.rs` | `pub mod response;` | `W0-9` |
| `src/misc.rs` | `pub mod notify;` | `W0-10` |
| `src/misc.rs` | `pub mod helpers;` | `W0-11` |
| `src/misc.rs` | `pub mod service;` | `W0-12` |

Wave 0 is sequential (§3.1), so no two units ever edit `src/ffi.rs` or `src/misc.rs` at the
same time. Each of those units carries `src/ffi.rs` or `src/misc.rs` in its own Rust-path
column, which is what §4 block 2 authorises it to touch.

### 1.5 `src/ffi/*`

One module per linked framework or per OS surface. Nothing in `src/ffi/` holds state except the
two runtime-resolved SkyLight pointers in `skylight_dynamic.rs`. No other module in the crate
imports an `objc2-*` crate directly; everything goes through `src/ffi/`.

| Module | Holds | From |
| --- | --- | --- |
| `skylight.rs` | `#[link(name = "SkyLight", kind = "framework")]` with the 76 live SkyLight declarations, in `extern.h` order; the `ConnectionCallback` alias; `kCPSAllWindows`/`kCPSUserGenerated`/`kCPSNoWindows` | `src/misc/extern.h:1-97` minus `:4-5`, `:6`, `:7-8`, `:38-39`, `:40`, `:52-53`, `:64`, and minus the seven dead declarations of `patterns/ffi-objc-and-os.md` §1.4 (`:25`, `:28`, `:29`, `:32`, `:65`, `:87`, `:91`); `src/window_manager.h:4-6` |
| `skylight_dynamic.rs` | the two `macho_find_symbol`-resolved pointers behind `OnceLock<Option<...>>` accessors, plus their symbol and image literals | `src/misc/extern.h:4-5`, filled at `src/yabai.c:148-149` |
| `core_graphics.rs` | re-exports from `objc2-core-graphics`; hand-written `#[link(name = "CoreGraphics", kind = "framework")]` for `CGPostMouseEvent`, `CGRegionCreateEmptyRegion`, `CGSNewRegionWithRect`; the private `kCGS*` numeric event fields | `src/misc/extern.h:38-39`; `src/space_manager.c:959-969`; `src/mouse_handler.c:69`; `src/mouse_handler.h:11,19` |
| `color_sync.rs` | hand-declared `#[link(name = "ColorSync", kind = "framework")]` `CGDisplayCreateUUIDFromDisplayID` and `CGDisplayGetDisplayIDFromUUID` | decision 11; `src/misc/extern.h:40`; `src/display.c:103,117` |
| `accessibility.rs` | AX re-exports from `objc2-application-services`; the ~35 `kAX*` CFString constants; `_AXUIElementGetWindow` and `_AXUIElementCreateWithRemoteToken`; the `ObserverCallback` alias; `ax_privilege`, `ax_window_id`, `ax_window_pid`, `ax_enhanced_userinterface`, the `AX_ENHANCED_UI_WORKAROUND` wrapper; the `ax_error_str` table | `src/misc/extern.h:7-8`; `src/misc/helpers.h:171,489,499,506,511,524-530`; `src/application.h:4,26-44` |
| `carbon_process.rs` | `#[link(name = "ApplicationServices", kind = "framework")]`: `ProcessSerialNumber`, `ProcessInfoRec`, `kNoProcess`; re-exported `GetProcessPID`, `GetNextProcess`, `CopyProcessName`, `SameProcess`, `IsProcessVisible`; hand-declared `GetProcessInformation`; `psn_equals`; the three `CoreDock*`, which HIServices exports and SkyLight does not | `src/misc/helpers.h:534`; `src/misc/extern.h:52-53,64` |
| `carbon_events.rs` | `#[link(name = "Carbon", kind = "framework")]` Event Manager: the six types, `noErr`, the six functions, and the six four-char-code constants | `src/process_manager.c` call sites |
| `carbon_core.rs` | `AbsoluteTime`/`Nanoseconds` as `#[repr(C)]`, `AbsoluteToNanoseconds` under `#[link(name = "CoreServices", kind = "framework")]`, `read_os_timer`, `read_os_freq` | `src/misc/helpers.h:149-160` |
| `core_video.rs` | re-exports `CVDisplayLink` and its four lifecycle functions, `CVTimeStamp`, `CVReturn`, `CVOptionFlags`, `kCVReturnSuccess`, `CVGetHostClockFrequency`, `CVDisplayLinkOutputCallback` | `objc2-core-video` |
| `core_foundation.rs` | every CF type and function used, plus the pure-CF helpers `cfstring_copy`, `CFSTRINGNUM32`, `CFNUM32`, `cfarray_of_cfnumbers`, `sls_window_disable_shadow` | `src/misc/helpers.h:321,328,333,344,361,373` |
| `appkit.rs` | thin re-exports: `NSApplication`, `NSApplicationLoad`, `NSApp`, `NSWorkspace` and its five notification names, `NSRunningApplication`, `NSScreen`, `NSImage`, `NSApplicationActivationPolicy` | `objc2-app-kit` |
| `foundation.rs` | thin re-exports: `NSString`, `NSArray`, `NSDictionary`, `NSNumber`, `NSProcessInfo`, `NSBundle`, `NSNotificationCenter`, `NSDistributedNotificationCenter`, `NSException`, `NSUserNotification` + Center + Delegate, `NSHomeDirectoryForUser`, `NSKeyValueObservingOptions`, `NSKeyValueChangeNewKey`, `NSOperatingSystemVersion` | `objc2-foundation` |
| `mach_port.rs` | hand-declared Mach types and `mach_msg` (decision 11 rejects `mach2`); `mach_send` with its `#[repr(C, packed(4))]` OOL message struct and its `const` size assertion (decision 35); `mig_get_special_reply_port`; `NDR_record`; `bootstrap_look_up`; `task_get_special_port` | `src/misc/helpers.h:204-223`; `src/misc/extern.h:6` |
| `macho.rs` | `symtab_command`, `nlist_64`, `LC_SYMTAB`, `SEG_LINKEDIT`, the three private finders and `macho_find_symbol`, one-for-one | `src/misc/macho_dlsym.h:1-80` |
| `libsystem.rs` | `csr_get_active_config`, `CSR_ALLOW_UNRESTRICTED_FS`, `CSR_ALLOW_TASK_FOR_PID`, `_NSGetExecutablePath`, `proc_name`, `PROC_PIDPATHINFO_MAXSIZE`, the process-traced check, `getpagesize` | `src/sa.m:3-5`; `src/misc/helpers.h` |
| `dispatch.rs` | one function, `post_to_main_queue_after`, wrapping `DispatchQueue::main()` and `block2::RcBlock` | `src/event_loop.c:93,157,1478,1516,1520` |

The one MIG struct that does **not** live here is `SLSGetWindowSubLevel__Internal`
(`src/window.c:930-952`): it has a single call site and stays in `src/window.rs`. It uses the
`#[repr(C, packed(4))]` pattern and the `NDR_record` import from `mach_port.rs`.

### 1.6 `src/misc/*`

| Module | C source | Content |
| --- | --- | --- |
| `macros.rs` | `src/misc/macros.h:1-47` | `MAXLEN`, `FAILURE_MESSAGE`, `DIR_NORTH`/`EAST`/`SOUTH`/`WEST`, `STACK`, `TYPE_ABS`/`TYPE_REL`, the five `HANDLE_*` bits as a newtype with associated constants (decision 31), the four `LAYER_*`, and `max`/`add_and_clamp_to_zero`/`in_range_ii`/`in_range_ie`/`in_range_ei`/`lerp` as generic `fn` or `macro_rules!` matching the C expansion. `array_count` disappears into `.len() as i32`. `REGEX_MATCH_UD`/`YES`/`NO` move to `regex.rs` as the three-valued enum. **Five items of this header are not translated** (decision 5, one `DEVIATIONS.md` line each): `GIGABYTES` (`:6`), `min` (`:9`) and `in_range_ee` (`:15`) have no use anywhere outside `macros.h` — the three `min` hits in `src/misc/helpers.h:581-583` are a parameter of `clampf_range`, never followed by `(`, so the macro never expands; and `KILOBYTES` (`:4`) and `MEGABYTES` (`:5`) lose every call site when decisions 17 and 19 replace the `ts` arena and the memory pool, leaving `src/yabai.c:279` (`ts_init`), `src/yabai.c:283` and `src/event_loop.c:1707` (`memory_pool_init`) with nothing to size. |
| `log.rs` | `src/misc/log.h:1-63` | `debug!`, `warn!`, `error!`, `require!`, `debug_message!` as `macro_rules!` taking the C format text (decision 33). `error!` writes to stderr and exits `EXIT_FAILURE`; `require!` writes to stderr and exits `EXIT_SUCCESS`. Neither calls `notify`. Never `panic!`. |
| `notify.rs` | `src/misc/notify.h:1-50` | the `NSUserNotification` path, `define_class!` for the delegate, the deliberate `alloc`-without-`init` kept as-is. |
| `timer.rs` | `src/misc/timer.h` clock readers only | `read_os_timer`-adjacent clock readers that live code calls, the `__rdtsc` / `mrs cntvct_el0` / `mrs cntfrq_el0` paths behind `#[cfg(target_arch)]`. The whole `PROFILE` machinery (`TIME_FUNCTION`, `PROFILER_END_TRANSLATION_UNIT`, the anchors) is dead code and is not translated (decision 5); the removal is one line in `DEVIATIONS.md`. |
| `table.rs` | `src/misc/hashtable.h:1-156` | the safe generic `Table` of decision 16: the C hash functions, the C bucket iteration order, the "add does not overwrite" rule, `table_for` as a normal iterator. Never `std::collections::HashMap`. |
| `regex.rs` | `src/misc/helpers.h:573-579`, `src/misc/macros.h:22-24` | `PosixRegex` wrapping a boxed `libc::regex_t` with `Drop` calling `regfree` (decision 26), and `RegexMatch` with the three variants `Undefined`, `Yes`, `No`. A `const` assertion that `size_of::<libc::regex_t>() == 32`. |
| `response.rs` | `src/misc/macros.h:18`, `src/message.c:418-427`, `src/event_loop.c:1632-1636` | the one `Response` type of decision 28: owns the socket descriptor, owns the `FAILURE_MESSAGE` prefix byte (`"\x07"`), and owns the "no response wanted" case that C spells `rsp == NULL`. Writes bytes; `Result` appears only here, at `io::Write` (decision 32). `daemon_fail` becomes a method. `daemon_deprecated` (`src/message.c:429-437`) is dead code and is not translated (decision 5). |
| `helpers.rs` | `src/misc/helpers.h:1-677` minus what moved to `ffi/` | the twenty-one easing functions (`:42-147`) and `animation_easing_type_str` (`:35`); `bool_str` (`:173`) and `layer_str` (`:175-181`); sockets (`socket_open` `:183`, `socket_connect` `:189`, `socket_close` `:198`); JSON (`json_optional_bool` `:225`, `json_bool` `:233`); `rgba_color_from_hex` (`:238`); `is_root` (`:249`); strings (`string_equals` `:254`, `ts_string_escape` `:259`, `ts_string_copy` `:387`, `string_copy` `:397`); filesystem (`directory_exists` `:408`, `file_exists` `:419`, `file_can_execute` `:434`, `get_config_file` `:445`, `exec_config_file` `:463`); geometry (`cgrect_clamp_x_radius` `:542`, `cgrect_clamp_y_radius` `:550`, `cgrect_contains_point` `:558`, `triangle_contains_point` `:564`, `clampf_range` `:581`); `cgimage_restore_alpha` (`:588`, decision 36). Functions keep their C names. |
| `service.rs` | `src/misc/service.h:1-314` | `safe_exec`, `populate_plist_path`, `populate_plist`, `ensure_directory_exists`, `service_install*`, `service_uninstall`, `service_start`, `service_restart`, `service_stop`, and the plist template constants verbatim. Keeps its libc calls (decision 34). |

`src/misc/extern.h` has no `src/misc/` module: its contents become `src/ffi/skylight.rs` and
`src/ffi/skylight_dynamic.rs`. `src/misc/macho_dlsym.h` becomes `src/ffi/macho.rs`.
`src/misc/autorelease.h`, `src/misc/memory_pool.h`, `src/misc/sbuffer.h` and `src/misc/ts.h`
produce no module at all; §2.2 says where each one goes.

### 1.7 One Rust module per C pair

| C pair | Rust module |
| --- | --- |
| `src/view.h` + `src/view.c` | `src/view.rs` |
| `src/window.h` + `src/window.c` | `src/window.rs` |
| `src/application.h` + `src/application.c` | `src/application.rs` |
| `src/process_manager.h` + `src/process_manager.c` | `src/process_manager.rs` |
| `src/display.h` + `src/display.c` | `src/display.rs` |
| `src/display_manager.h` + `src/display_manager.c` | `src/display_manager.rs` |
| `src/space.h` + `src/space.c` | `src/space.rs` |
| `src/space_manager.h` + `src/space_manager.c` | `src/space_manager.rs` |
| `src/window_manager.h` + `src/window_manager.c` | `src/window_manager.rs` |
| `src/event_loop.h` + `src/event_loop.c` | `src/event_loop.rs` |
| `src/event_signal.h` + `src/event_signal.c` | `src/event_signal.rs` |
| `src/rule.h` + `src/rule.c` | `src/rule.rs` |
| `src/message.h` + `src/message.c` | `src/message.rs` |
| `src/mouse_handler.h` + `src/mouse_handler.c` | `src/mouse_handler.rs` |
| `src/workspace.h` + `src/workspace.m` | `src/workspace.rs` |
| `src/sa.h` + `src/sa.m` | `src/sa.rs` |
| `src/mission_control.c` (no header) | `src/mission_control.rs` |
| `src/yabai.c` (no header) | `src/main.rs` + `src/globals.rs` + `src/state.rs` |

`src/yabai.c` is the only three-way split, and only because decisions 12 and 18 require two
declaration-only containers that every other module reads. All four `src/yabai.c` function
bodies stay in `src/main.rs`; `globals.rs` and `state.rs` hold no behaviour.

### 1.8 The makefile

Every user-facing target survives with the same meaning, including "`make` is debug,
`make install` is release, and `install` installs nothing". `man`, `icon`, `sign`, `archive`
and `publish` are copied verbatim from `makefile:38-58`. Only the compile rule and `clean`
change (decision 10):

```make
BUILD_PATH  = ./bin
DOC_PATH    = ./doc
SCRIPT_PATH = ./scripts
ASSET_PATH  = ./assets
SMP_PATH    = ./examples
ARCH_PATH   = ./archive
ARM_TARGET  = aarch64-apple-darwin
X64_TARGET  = x86_64-apple-darwin

CARGO       = cargo
CARGO_FLAGS =
PROFILE_DIR = debug

.PHONY: all asan tsan install man icon archive publish sign clean-build clean

all: clean-build $(BUILD_PATH)/yabai

install: CARGO_FLAGS = --release
install: PROFILE_DIR = release
install: clean-build $(BUILD_PATH)/yabai

asan: CARGO       = RUSTFLAGS="-Zsanitizer=address" cargo +nightly
asan: CARGO_FLAGS = -Zbuild-std
asan: clean-build $(BUILD_PATH)/yabai-host

tsan: CARGO       = RUSTFLAGS="-Zsanitizer=thread" cargo +nightly
tsan: CARGO_FLAGS = -Zbuild-std
tsan: clean-build $(BUILD_PATH)/yabai-host

$(BUILD_PATH)/yabai:
	mkdir -p $(BUILD_PATH)
	$(CARGO) build $(CARGO_FLAGS) --target $(ARM_TARGET)
	$(CARGO) build $(CARGO_FLAGS) --target $(X64_TARGET)
	lipo -create -output $@ \
	    ./target/$(ARM_TARGET)/$(PROFILE_DIR)/yabai \
	    ./target/$(X64_TARGET)/$(PROFILE_DIR)/yabai

$(BUILD_PATH)/yabai-host:
	mkdir -p $(BUILD_PATH)
	$(CARGO) build $(CARGO_FLAGS)
	cp ./target/$(PROFILE_DIR)/yabai $(BUILD_PATH)/yabai

clean-build:
	rm -rf $(BUILD_PATH)

clean: clean-build
	cargo clean
```

Deliberate divergences, each recorded in `DEVIATIONS.md`:

* `asan` and `tsan` build **host-only**, where `makefile:21-25` pointlessly built both arches
  for a sanitizer run executed locally. They are nightly targets because `-Zsanitizer` and
  `-Zbuild-std` are nightly-only. They use the dev profile: `lto = "fat"` conflicts with
  sanitizer instrumentation.
* The UBSan half of `-fsanitize=...,undefined` has no Rust target and needs none.
* `clean` removes `./target` instead of the two `*_bin.c` files (`makefile:64`), which no
  longer exist.
* `CLI_FLAGS` (`makefile:3`) is dropped. The escape hatch is `make RUSTFLAGS=...`; no `-D`-style
  mechanism is invented.
* `.gitignore` loses `/src/osax/loader_bin.c` and `/src/osax/payload_bin.c` and gains `/target`.
  `/tests/bin` goes when `tests/` goes.

Signing order is unchanged from `makefile:48`: build, then `lipo`, then `sign`, then `icon`.

---
## 2. Ground-truth inventory

Every `.c`, `.h` and `.m` under `src/` outside `src/osax/`, plus `tests/`. Line counts are
`wc -l` at the commit this plan was written against. Nothing is unassigned: a file either names
a Rust module or names the decision that deletes it.

### 2.1 `src/` top level — 35 files, 16201 lines

| C file | Lines | Rust module |
| --- | ---: | --- |
| `src/application.c` | 144 | `src/application.rs` |
| `src/application.h` | 92 | `src/application.rs`; `:4` (`ObserverCallback`) and `:26-44` (`ax_error_str`) to `src/ffi/accessibility.rs` |
| `src/display.c` | 252 | `src/display.rs` |
| `src/display.h` | 46 | `src/display.rs` |
| `src/display_manager.c` | 506 | `src/display_manager.rs` |
| `src/display_manager.h` | 93 | `src/display_manager.rs` |
| `src/event_loop.c` | 1721 | `src/event_loop.rs`; `:1561-1562` (`ffm_value`, `is_menu_open`) to `src/state.rs` as two `EventLoopOwnedState` fields (§1.4) |
| `src/event_loop.h` | 76 | `src/event_loop.rs`; the `EVENT_TYPE_LIST` X-macro (`:6-45`) becomes one `macro_rules!` generating the `Event` enum and its string table (decision 31) |
| `src/event_signal.c` | 454 | `src/event_signal.rs` |
| `src/event_signal.h` | 129 | `src/event_signal.rs` |
| `src/manifest.m` | 97 | none — replaced by `Cargo.toml`, `build.rs` and the `mod` tree in `src/main.rs` |
| `src/message.c` | 3045 | `src/message.rs`; `:418-427` (`daemon_fail`) to `src/misc/response.rs` |
| `src/message.h` | 7 | `src/message.rs` |
| `src/mission_control.c` | 111 | `src/mission_control.rs`; `enum mission_control_mode` (`:29-36`) and `mission_control_mode_str[]` (`:38-44`) stay here as `pub(crate)`, read from `src/event_signal.rs` |
| `src/mouse_handler.c` | 303 | `src/mouse_handler.rs` |
| `src/mouse_handler.h` | 114 | `src/mouse_handler.rs`; the event-tap half of `struct mouse_state` becomes the static of atomics in `src/globals.rs` (decision 23) |
| `src/process_manager.c` | 265 | `src/process_manager.rs` |
| `src/process_manager.h` | 35 | `src/process_manager.rs` |
| `src/rule.c` | 221 | `src/rule.rs` |
| `src/rule.h` | 78 | `src/rule.rs` |
| `src/sa.h` | 31 | `src/sa.rs`; `:4-7` (the four `xxd` symbols) deleted — `include_bytes!` from `OUT_DIR` replaces them |
| `src/sa.m` | 625 | `src/sa.rs` |
| `src/space.c` | 106 | `src/space.rs` |
| `src/space.h` | 12 | `src/space.rs` |
| `src/space_manager.c` | 1233 | `src/space_manager.rs` |
| `src/space_manager.h` | 111 | `src/space_manager.rs` |
| `src/view.c` | 1042 | `src/view.rs` |
| `src/view.h` | 253 | `src/view.rs` |
| `src/window.c` | 1144 | `src/window.rs` |
| `src/window.h` | 176 | `src/window.rs`; `:140` (`window_unknown_serialize`, declared and never defined) is dead and produces no Rust |
| `src/window_manager.c` | 2765 | `src/window_manager.rs` |
| `src/window_manager.h` | 216 | `src/window_manager.rs`; `:4-6` (`kCPS*`) to `src/ffi/skylight.rs` |
| `src/workspace.h` | 39 | `src/workspace.rs` |
| `src/workspace.m` | 303 | `src/workspace.rs` |
| `src/yabai.c` | 356 | `src/main.rs` (all four function bodies); `:27-52` split between `src/globals.rs` (decision 18) and `src/state.rs` (decision 12), with `:35` and `:42` going to neither (§1.4) |

### 2.2 `src/misc/` — 13 files, 1970 lines

| C file | Lines | Rust module |
| --- | ---: | --- |
| `src/misc/autorelease.h` | 98 | none — dead code, never included (`src/manifest.m:49` is commented out). Decision 5; one line in `DEVIATIONS.md` |
| `src/misc/extern.h` | 97 | `src/ffi/skylight.rs` (`:1-2` and `:9-97` minus the lines below); `src/ffi/skylight_dynamic.rs` (`:4-5`); `src/ffi/mach_port.rs` (`:6`, libSystem/MIG, not SkyLight); `src/ffi/accessibility.rs` (`:7-8`); `src/ffi/core_graphics.rs` (`:38-39`); `src/ffi/color_sync.rs` (`:40`, decision 11 — ColorSync exports `CGDisplayCreateUUIDFromDisplayID`, CoreGraphics does not); `src/ffi/carbon_process.rs` (`:52-53`, `:64`, exported by HIServices). Seven of the remaining SkyLight declarations — `:25`, `:28`, `:29`, `:32`, `:65`, `:87`, `:91` — have no call site anywhere outside `extern.h` and are dropped under decision 5, leaving 76 |
| `src/misc/hashtable.h` | 156 | `src/misc/table.rs` (decision 16). **The C header is not deleted.** `src/osax/payload.m:36-38` includes it unconditionally and `src/osax/` may not be edited (decision 1), so it survives `W3-5` and stays in `build.rs`'s `rerun-if-changed` list. It is the only file in this table with a second, non-Rust consumer |
| `src/misc/helpers.h` | 677 | `src/misc/helpers.rs`; `:149-160` to `src/ffi/carbon_core.rs`; `:204-223` to `src/ffi/mach_port.rs`; `:321,328,333,344,361,373` to `src/ffi/core_foundation.rs`; `:171,489,499,506,511,524-530` to `src/ffi/accessibility.rs`; `:534` to `src/ffi/carbon_process.rs`; `:573-579` to `src/misc/regex.rs` |
| `src/misc/log.h` | 63 | `src/misc/log.rs`; `:4` (`g_verbose`) to `src/globals.rs` as an `AtomicBool` (decision 18) |
| `src/misc/macho_dlsym.h` | 80 | `src/ffi/macho.rs` |
| `src/misc/macros.h` | 47 | `src/misc/macros.rs`; `:18` (`FAILURE_MESSAGE`) also used by `src/misc/response.rs`; `:22-24` (`REGEX_MATCH_*`) to `src/misc/regex.rs` |
| `src/misc/memory_pool.h` | 47 | none — replaced. The event pool becomes the `std::sync::mpsc` channel (decision 19); `g_signal_storage` becomes an owned queue in `src/event_signal.rs` (decision 17). One line in `DEVIATIONS.md` |
| `src/misc/notify.h` | 50 | `src/misc/notify.rs` |
| `src/misc/sbuffer.h` | 71 | none — replaced by `Vec`, `buf_del` by `swap_remove` (decision 17). One line in `DEVIATIONS.md` |
| `src/misc/service.h` | 314 | `src/misc/service.rs` |
| `src/misc/timer.h` | 165 | `src/misc/timer.rs`, clock readers only. `TIME_FUNCTION`, `PROFILER_END_TRANSLATION_UNIT` and the anchors are dead (decision 5) |
| `src/misc/ts.h` | 105 | none — replaced by `Vec`/`String`; the three sites that concatenate consecutive arena allocations extend one `Vec` (decision 17). One line in `DEVIATIONS.md` |

### 2.3 `src/osax/` — untouched

`src/osax/common.h` (48), `src/osax/loader.m` (285), `src/osax/payload.m` (1121),
`src/osax/arm64_payload.m` (244), `src/osax/x64_payload.m` (296) stay C/Objective-C and are
never edited (decision 1). `build.rs` compiles two of them and parses `common.h`.

`src/misc/hashtable.h` (156) is part of this set in everything but location: `payload.m:36-38`
includes it, `payload.m` calls `table_init`, `table_find`, `table_add` and `table_remove` against
it, and decision 1 makes the include unchangeable. It is C for as long as `src/osax/` is C.

`src/osax/payload_bin.c` and `src/osax/loader_bin.c` are generated by `xxd` (`makefile:33-34`)
and git-ignored (`.gitignore:4-5`). They disappear with the `xxd` rule in work unit `W3-5`.

### 2.4 `tests/` — 3 files, 156 lines

| File | Lines | Disposition |
| --- | ---: | --- |
| `tests/src/area.c` | 89 | the two test cases transpose into `#[cfg(test)] mod tests` inside `src/view.rs` (decisions 40 and 2). They must live in the module because `area_is_in_direction` (`src/view.c:541`), `area_distance_in_direction` (`src/view.c:563`) and `area_max_point` (`src/view.c:126`) are crate-private |
| `tests/src/tests.m` | 54 | deleted. `TEST_FUNC` becomes `#[test] fn`, `TEST_CHECK` becomes `assert_eq!`, the `TEST_LIST` registration and the ANSI driver are libtest's job, the `-DPROFILE=1` timing is libtest's job, and the four fake osax symbols (`:1-4`) are unnecessary once `build.rs` supplies the real bytes |
| `tests/makefile` | 13 | deleted; `cargo test` replaces it. `-DTESTS` (`src/yabai.c:260`) disappears — `cargo test` on a bin target replaces `main` |

Decision 40 is closed: these two cases are transposed and no other test is written.

### 2.5 Everything else in the repository

`scripts/`, `assets/`, `doc/yabai.asciidoc`, `examples/` and `.github/` are unchanged.
`makefile` is rewritten in place (§1.8). `.gitignore` is edited by `W3-5`.

---
## 3. The waves

Six waves. Wave 0, wave 0b and wave 3 are sequential; wave 4 is fully parallel. Waves 1 and 2
are parallel in the middle and sequential at the edges: wave 1 is `W1-1`, then seventeen
per-module units in parallel, then `W1-main`; wave 2 is seventeen per-module units in parallel,
then `W2-main`. A wave starts only when the previous wave's gate is green.

The gate command, used by every wave, is:

```
cargo check --target aarch64-apple-darwin && cargo check --target x86_64-apple-darwin
```

"Clean" means zero errors and zero warnings. `#[allow(dead_code)]` at the crate root is
permitted from wave 0 until the end of wave 2 and is removed by `W3-2`; nothing else is
silenced.

### 3.1 Wave 0 — foundation

Sequential, one unit at a time, because each unit's gate is a `cargo check` on a crate that
must compile at every step. Fourteen small units. No unit in wave 0 translates a manager, a
view or a window; wave 0 exists so that wave 1 has somewhere to put its signatures.

Because the gate is a `cargo check`, a module is declared in the same unit that creates its
file and never earlier: `W0-3` writes only `mod ffi; mod misc; mod globals; mod state;`, and
every unit from `W0-4` to `W0-13` adds its own `pub mod` line to `src/ffi.rs` or `src/misc.rs`
(or, for `W0-13`, its `mod sa;` line to `src/main.rs`) at the position §1.4 fixes. A unit
whose `cargo check` is clean only because its new module is unreachable from the crate root has
not finished. The remaining sixteen `mod` lines belong to `W1-main` (§3.3).

| Id | C sources | Rust path | Depends on | Size | Done when |
| --- | --- | --- | --- | --- | --- |
| `W0-1` | `makefile:1-27`, `src/yabai.c:23-25` | `Cargo.toml`, `rust-toolchain.toml` | — | 45 lines | Both files match §1.1 and §1.2 byte for byte. `cargo metadata` resolves all ten dependencies at the pinned versions |
| `W0-2` | `makefile:30-36`, `src/osax/common.h`, `assets/Info.plist` | `build.rs` | `W0-1` | 130 lines | All six responsibilities of §1.3 implemented. `cargo build` produces `OUT_DIR/payload`, `OUT_DIR/loader` and `OUT_DIR/osax_common.rs`; `lipo -info` on both binaries reports `x86_64 arm64e`; a deliberately corrupted `common.h` fails the build |
| `W0-3` | `src/yabai.c:27-52`, `src/misc/log.h:4`, `src/event_loop.c:1561-1562` | `src/main.rs`, `src/globals.rs`, `src/state.rs`, `src/ffi.rs`, `src/misc.rs` | `W0-2` | 356 C lines in; ~120 Rust lines out | `main` installs the panic hook of §1.4 as its first statement and does nothing else yet; `src/main.rs` declares exactly `mod ffi; mod misc; mod globals; mod state;` and no other `mod` line, and `src/ffi.rs` and `src/misc.rs` exist and are empty, so `cargo check` is clean (§1.4); `globals.rs` declares every decision-18 static; `state.rs` declares `EventLoopOwnedState` with its manager fields as opaque placeholder types and its `unsafe impl Send` |
| `W0-4` | `src/misc/extern.h:1-5`, `:9-37`, `:41-51`, `:54-63`, `:65-97`, `src/window_manager.h:4-6` | `src/ffi/skylight.rs`, `src/ffi/skylight_dynamic.rs`, `src/ffi.rs` | `W0-3` | 97 C lines in; ~190 Rust lines out | All 76 live SkyLight declarations present in `extern.h` order; the seven declarations listed in `patterns/ffi-objc-and-os.md` §1.4 — `SLSNewWindow` (`:25`), `SLSSetWindowTags` (`:28`), `SLSClearWindowTags` (`:29`), `SLSSetWindowBackgroundBlurRadiusStyle` (`:32`), `SLSMoveWindow` (`:65`), `SLSSetWindowTransform` (`:87`), `SLSTransactionOrderWindow` (`:91`) — are absent and each is one line in `DEVIATIONS.md`; the `ConnectionCallback` alias present; the two dynamic pointers behind `OnceLock<Option<...>>` accessors that keep the NULL test callers rely on |
| `W0-5` | `src/misc/macho_dlsym.h`, `src/misc/helpers.h:149-160,204-223,321-373,489-534`, `src/application.h:4,26-44`, `src/sa.m:3-5`, `src/misc/extern.h:6,7-8,38-39,40,52-53,64` | the other fifteen `src/ffi/*.rs`, `src/ffi.rs` | `W0-4` | ~700 Rust lines out | Every module of §1.5 exists with its listed contents; no `objc2-*` crate is imported outside `src/ffi/`; the two `const` size assertions of decision 35 compile; the five `extern.h` declarations `W0-4` left behind are here and nowhere else — `:6` in `mach_port.rs`, `:7-8` in `accessibility.rs`, `:38-39` in `core_graphics.rs`, `:40` in `color_sync.rs`, `:52-53` and `:64` in `carbon_process.rs` |
| `W0-6` | `src/misc/macros.h`, `src/misc/log.h`, `src/misc/timer.h` | `src/misc/macros.rs`, `src/misc/log.rs`, `src/misc/timer.rs`, `src/misc.rs` | `W0-5` | 275 C lines in; ~200 Rust lines out | `debug!`, `warn!`, `error!`, `require!`, `debug_message!` take the C format text and the function name as a literal (decision 33); `error!` writes to stderr and exits `EXIT_FAILURE`, `require!` writes to stderr and exits `EXIT_SUCCESS`; the `PROFILE` machinery is absent and recorded in `DEVIATIONS.md`; `KILOBYTES` (`src/misc/macros.h:4`), `MEGABYTES` (`:5`), `GIGABYTES` (`:6`), `min` (`:9`) and `in_range_ee` (`:15`) are absent and each is one line in `DEVIATIONS.md` |
| `W0-7` | `src/misc/hashtable.h` | `src/misc/table.rs`, `src/misc.rs` | `W0-6` | 156 C lines in; ~180 Rust lines out | `Table` is generic and safe; the C hash functions, the C bucket iteration order and the add-does-not-overwrite rule are reproduced; `std::collections::HashMap` appears nowhere in the file |
| `W0-8` | `src/misc/helpers.h:573-579`, `src/misc/macros.h:22-24` | `src/misc/regex.rs`, `src/misc.rs` | `W0-7` | ~60 Rust lines out | `PosixRegex` boxes the `regex_t` and frees it in `Drop`; `RegexMatch` has exactly three variants; the `size_of::<libc::regex_t>() == 32` assertion compiles |
| `W0-9` | `src/misc/macros.h:18`, `src/message.c:418-427`, `src/event_loop.c:1632-1636` | `src/misc/response.rs`, `src/misc.rs` | `W0-8` | ~90 Rust lines out | `Response` owns the descriptor, the `"\x07"` failure prefix and the no-response case; it writes bytes; `Result` appears only at `io::Write`; `daemon_deprecated` is absent and recorded in `DEVIATIONS.md` |
| `W0-10` | `src/misc/notify.h` | `src/misc/notify.rs`, `src/misc.rs` | `W0-9` | 50 C lines in; ~80 Rust lines out | The delegate is built with `define_class!`; the deliberate `alloc`-without-`init` of `src/misc/notify.h:22` is reproduced, not "fixed" |
| `W0-11` | `src/misc/helpers.h` (the remainder) | `src/misc/helpers.rs`, `src/misc.rs` | `W0-10` | 677 C lines in; ~700 Rust lines out | Every helper that did not move to `src/ffi/` is present under its C name; `cgimage_restore_alpha` uses `std::arch` SSE2 and NEON intrinsics instruction for instruction with the 1-to-3-pixel tail on a padded scratch copy (decision 36) |
| `W0-12` | `src/misc/service.h` | `src/misc/service.rs`, `src/misc.rs` | `W0-11` | 314 C lines in; ~360 Rust lines out | The plist template text is byte-identical to `src/misc/service.h`; `system`, `popen`, `posix_spawn` with a NULL environment, `_NSGetExecutablePath` and the never-closed lock-file descriptor are all kept (decision 34) |
| `W0-13` | `src/sa.h:4-7`, `src/osax/common.h` | `src/sa.rs` (statics only), `src/main.rs` (the `mod sa;` line only) | `W0-12` | ~20 Rust lines out | `include_bytes!(concat!(env!("OUT_DIR"), "/payload"))` and the loader equivalent compile; `include!(concat!(env!("OUT_DIR"), "/osax_common.rs"))` brings in `OSAX_VERSION`, `SA_SOCKET_PATH_FMT`, `SA_SOCKET_BUFF_LEN`, the seven attribute bits and the nineteen opcodes; `mod sa;` is in `src/main.rs` at its §1.4 position |
| `W0-14` | `makefile` | `makefile` | `W0-13` | 55 lines | The makefile matches §1.8; `make` and `make install` both produce a universal `bin/yabai` from the current (empty) crate; `make man`, `make icon`, `make sign`, `make archive`, `make publish`, `make clean-build`, `make clean` behave as before |

**Wave 0 gate.** `cargo check` clean for both targets, `make` and `make install` both produce a
universal `bin/yabai`, `lipo -info bin/yabai` reports `x86_64 arm64`, and
`otool -arch x86_64 -s __TEXT __info_plist bin/yabai` and the `arm64` equivalent both dump the
plist. The C daemon still builds from `src/manifest.m`; nothing under `src/*.c` has been
touched.

### 3.2 Wave 0b — the state-access closure

Decision 13 derives every Rust signature from the set of managers a function touches
transitively. That set is computed once, before any signature is written, so that callers and
callees cannot disagree. Wave 0b is one unit and it blocks all of wave 1.

Output: `doc/rust-rewrite/STATE_ACCESS.md`, a generated table with one row per C function.

Starting point, already on disk: `doc/rust-rewrite/state-access.tsv` (645 functions, generated
by `state-access-closure.py`, columns described in `state-access.md`) holds the textual closure
of steps 2 to 5 over every `g_*` global. `W0b-1` cross-checks it against the preprocessed
unity build, narrows it to the event-loop-owned set of step 3, and adds the thread context and
the final Rust parameter list.

**The mechanical method.** Six steps, in order.

1. **Preprocess the unity build.** Run
   `xcrun clang -E -std=c11 -mmacosx-version-min=11.0 -fno-objc-arc -arch arm64
   -F/System/Library/PrivateFrameworks src/manifest.m` and keep the output with its `#line`
   markers. Every later step runs on this file, not on the individual `.c` files. This is what
   makes the method reliable: the `EVENT_HANDLER` macro (`src/event_loop.h:4`), `table_for`
   (`src/misc/hashtable.h:34-41`) and `AX_ENHANCED_UI_WORKAROUND`
   (`src/misc/helpers.h:524-530`) all hide call sites that a textual grep misses, and the unity
   build means the sixteen `static` functions that are called across file boundaries resolve
   without special handling.
2. **Extract every function definition** from the preprocessed text: name, the `#line`-derived
   original `file:line`, and the body's start and end offsets by brace matching. Write
   `functions.tsv`. Cross-check the count against the C: 40 `EVENT_HANDLER` bodies in
   `src/event_loop.c`, plus the free functions in every other file.
3. **Record direct global references.** For each body, scan for the eight event-loop-owned
   globals of `src/yabai.c:27-37` — `g_signal_event`, `g_process_manager`, `g_display_manager`,
   `g_window_manager`, `g_space_manager`, `g_signal_storage`, `g_mouse_state`,
   `g_mission_control_mode` — and for `g_event_loop` and the two file-scope statics
   `ffm_value` and `is_menu_open` (`src/event_loop.c:1561-1562`), which are event-loop-owned
   too. Nothing else is scanned for. `g_workspace_context` (`src/yabai.c:35`) and
   `g_event_bytes` (`:42`) are **not** on the list: `patterns/state-and-ownership.md` §1.2
   makes the first a `static WORKSPACE_CONTEXT: OnceLock<WorkspaceContext>`, because it is read
   on the main thread at `src/process_manager.c:191`, `:262` and `src/window_manager.c:2753`,
   and dissolves the second into a local `[u8; 0x100]` in each of its two users. Neither ever
   enters a signature, and neither do the decision-18 statics.
4. **Record declared manager parameters.** A C signature that already takes
   `struct window_manager *wm`, `struct space_manager *sm`, `struct display_manager *dm`,
   `struct process_manager *pm`, `struct view *view` or `struct event_loop *event_loop`
   contributes that manager **and its position**. Decision 13 keeps the C parameter order, so
   the position is part of the row.
5. **Record direct callees** and iterate to a fixed point. For every identifier in a body that
   is followed by `(` and appears in `functions.tsv`, add an edge. Then repeat

   ```
   touches(f) = direct_globals(f) ∪ declared_params(f) ∪ ⋃ over callees c of touches(c)
   ```

   until no set changes. The sets are finite and the operator is monotone, so recursion and
   mutual recursion converge; `display_manager_focus_display` and `space_manager_focus_space`
   are mutually recursive and must both end with the union.
6. **Seed the roots.** Functions the OS calls have no textual caller and must be listed
   explicitly: the 40 `EVENT_HANDLER` bodies, the AX observer callback
   (`src/application.h:4`), the `CGEventTap` callback (`src/mouse_handler.c:21`), the
   `CVDisplayLink` output callback (`src/window_manager.c`), the SkyLight connection notify
   proc (`src/yabai.c:322-333`), the Carbon event handler (`src/process_manager.c`), the
   `NSWorkspace` and KVO selectors (`src/workspace.m`), and the five
   `dispatch_get_main_queue()` blocks (`src/event_loop.c:93,157,1478,1516,1520`). A root's row
   records the context it runs on as well, because a main-thread root may not take
   `&mut EventLoopOwnedState` at all (decision 20).

**Row format.** One row per function:

`C name | file:line | thread context | managers touched (transitive) | final Rust parameter list`

The final parameter list is the C parameter list with the C-declared managers in their C
positions, and any manager reached only through a global appended after them, in the canonical
manager order — the `EventLoopOwnedState` field order of §1.4. That appended order is fixed
there so two translators cannot produce different orders for the same set.

| Id | Wave | C sources | Rust path | Depends on | Size | Done when |
| --- | --- | --- | --- | --- | --- | --- |
| `W0b-1` | 0b | all of `src/` outside `src/osax/` | `doc/rust-rewrite/STATE_ACCESS.md` | wave 0 gate | ~700 rows | Every function in `functions.tsv` has a row; the fixed point is reported as reached; the 40 event handlers and the eight other root kinds are present and marked; twenty rows picked at random are verified by hand against the C and recorded as verified in the document |

**Wave 0b gate.** `STATE_ACCESS.md` exists, covers every extracted function, and the hand
verification of twenty rows is recorded in it.

---
### 3.3 Wave 1 — skeleton, one unit per header

Nineteen units, in three stages, all starting from the wave 0b gate. `W1-1` runs first and
alone, because every other unit spells its names out of the glossary it writes. The seventeen
per-module units then run in parallel. `W1-main` runs last and alone, because it adds the
sixteen `mod` lines of §1.4 and replaces the placeholder field types of `EventLoopOwnedState`
with the real manager types, both of which need every other module's file on disk.

Each unit writes **every** type, constant, static and function *signature* of its module in
final form, with `todo!()` for every body, so the whole crate compiles before a single body
exists. After wave 1 no signature changes without a recorded deviation, which is what makes
wave 2 parallelisable.

What a wave 1 unit writes, in this order:

1. Every `struct`, `enum` and `union` from the header, with decision 31's explicit
   discriminants where the value is observable or used as an index, and flag sets as newtypes
   with associated constants. X-macro lists become one `macro_rules!` each, generating the enum
   and its string table.
2. Every `#define` constant, as `const` with the C name.
3. Every static table and string array.
4. Every function signature, taken verbatim from its `STATE_ACCESS.md` row, with the body
   `todo!()`. Visibility is `pub(crate)` by default: in the unity build a C `static` function is
   not module-private, and sixteen of them are called across file boundaries. Private `fn` is
   used only where `STATE_ACCESS.md` shows exactly one caller in the same module.
5. Nothing else. No bodies, no helper functions the C does not have, no `impl` blocks the C
   does not imply.

Naming follows decision 37 with no exceptions: `struct window_manager` becomes `WindowManager`,
enum constants become CamelCase variants without the prefix, and no field, parameter or local
is abbreviated — `window_manager` not `wm`, `window_id` not `wid`, `space_id` not `sid`,
`display_id` not `did`, `process_id` not `pid`, `process_serial_number` not `psn`. The glossary
that fixes every spelling is `doc/rust-rewrite/GLOSSARY.md`, written by `W1-1` before any other
wave 1 unit starts.

| Id | C sources | Rust path | Depends on | Size | Done when |
| --- | --- | --- | --- | --- | --- |
| `W1-1` | decision 37, `STATE_ACCESS.md` | `doc/rust-rewrite/GLOSSARY.md` | `W0b-1` | ~120 entries | One row per recurring C spelling and its single Rust spelling, covering every abbreviation the C uses and every type rename |
| `W1-view` | `src/view.h` (253) | `src/view.rs` | `W1-1` | 27 fn, 13 types | `Area`, `WindowCapture`, `WindowProxy`, `WindowAnimation`, `WindowAnimationContext`, `BalanceNode`, `WindowInsertionPoint`, `WindowNodeChild`, `WindowNodeSplit`, `FeedbackWindow`, `WindowNode`, `ViewType`, `ViewFlags`, `View`, the `SPACE_PROPERTIES` table and the five string tables exist; the arena of decision 15 is declared (`nodes: Vec<WindowNode>`, `NodeId` handles for `parent`/`left`/`right`/`zoom`, root pinned at `NodeId` 0); the four functions `view.h` forgets to declare are present |
| `W1-window` | `src/window.h` (176) | `src/window.rs` | `W1-1` | 45 fn | Every signature present; the liveness cell of decision 21 declared on `Window`; `src/window.h:140` absent |
| `W1-application` | `src/application.h` (92) | `src/application.rs` | `W1-1` | 9 fn | Every signature present; `ObserverCallback` and `ax_error_str` referenced from `src/ffi/accessibility.rs`, not redefined |
| `W1-process_manager` | `src/process_manager.h` (35) | `src/process_manager.rs` | `W1-1` | 4 fn | `Arc<Process>` with atomic `terminated` and `ns_application`, and the process table behind a `Mutex`, both declared (decision 22) |
| `W1-display` | `src/display.h` (46) | `src/display.rs` | `W1-1` | 8 fn | Every signature present, including the reconfiguration callback |
| `W1-display_manager` | `src/display_manager.h` (93) | `src/display_manager.rs` | `W1-1` | 36 fn | Every signature present |
| `W1-space` | `src/space.h` (12) | `src/space.rs` | `W1-1` | 7 fn | Every signature present |
| `W1-space_manager` | `src/space_manager.h` (111) | `src/space_manager.rs` | `W1-1` | 63 fn | Every signature present; `SpaceOpError` declared here and used from `src/display_manager.rs`, mirroring the C header order |
| `W1-window_manager` | `src/window_manager.h` (216) | `src/window_manager.rs` | `W1-1` | 110 fn | Every signature present; `Arc<AnimationContext>` with `skip: AtomicBool` and `tx`/`ty`/`tw`/`th` as `AtomicU32` holding `f32` bits, and `window_animations_table` as `Arc<Mutex<_>>`, all declared (decision 24); `purify_mode_str[]` keeps the order `src/window_manager.h:32-38` actually has, which is not the order of `enum purify_mode` |
| `W1-event_loop` | `src/event_loop.h` (76) | `src/event_loop.rs` | `W1-1` | 40 handlers + 2 fn | The `EVENT_TYPE_LIST` X-macro becomes one `macro_rules!` generating the `Event` enum and its string table; the `Event` variants own their payloads and the `Drop` of decision 19 replaces the per-type `CFRelease`/`free`/`close`; the `std::sync::mpsc` sender and receiver are declared; the ring buffer and the named semaphore are absent |
| `W1-event_signal` | `src/event_signal.h` (129) | `src/event_signal.rs` | `W1-1` | 8 fn | Every signature present; queued signals own their strings (decision 17) |
| `W1-rule` | `src/rule.h` (78) | `src/rule.rs` | `W1-1` | 16 fn | Every signature present; rule regexes are `PosixRegex` from `src/misc/regex.rs` |
| `W1-message` | `src/message.h` (7) + the constants and static function definitions in `src/message.c` | `src/message.rs` | `W1-1` | 31 fn, ~95 constants | The seven `DOMAIN_*` and every `COMMAND_*`/`ARGUMENT_*` constant present with its exact C string; `Token` and `TokenValue` as `(start, length)` ranges over the mutable message buffer (decision 27); every static function signature from `STATE_ACCESS.md` |
| `W1-mouse_handler` | `src/mouse_handler.h` (114) | `src/mouse_handler.rs` | `W1-1` | 10 fn | The split of decision 23 is visible: the event-tap half is a static of atomics in `src/globals.rs`, the drag half is a field of `EventLoopOwnedState` |
| `W1-workspace` | `src/workspace.h` (39) | `src/workspace.rs` | `W1-1` | 11 fn | The five `workspace_is_macos_*` predicates and the observer class declared with `define_class!` |
| `W1-sa` | `src/sa.h` (31, minus `:4-7`) | `src/sa.rs` | `W1-1`, `W0-13` | 20 fn | Every signature present; the fixed 4096-byte frame buffer with its `i16` length header and the bounds check that fails the call (decision 34) declared |
| `W1-mission_control` | `src/mission_control.c` (whole file) | `src/mission_control.rs` | `W1-1` | 5 fn | `MissionControlMode` and its string table (`:29-36`, `:38-44`) are `pub(crate)` and `src/event_signal.rs` can name them; all five function signatures are present — `connection_handler` (`:7`), `mission_control_notification_handler` (`:59`), `mission_control_observe` (`:73`), `mission_control_unobserve` (`:93`), `mission_control_is_active` (`:108`); `g_mission_control_observer` (`:46-50`) is declared as `static MISSION_CONTROL_OBSERVER: Mutex<Option<MissionControlObserver>>` and the four `kAXExpose*` constants (`:52-55`) as `OnceLock<CFStringOwned>` each, per `patterns/state-and-ownership.md` §1.3 |
| `W1-main` | `src/yabai.c:1-52`, `src/event_loop.c:1561-1562` | `src/main.rs`, `src/globals.rs`, `src/state.rs` | `W1-1`, all other wave 1 units | 4 fn | `EventLoopOwnedState`'s placeholder field types are replaced by the real manager types; the four `src/yabai.c` function signatures exist with `todo!()` bodies; the sixteen remaining `mod` lines of §1.4 — `view`, `event_loop`, `event_signal`, `workspace`, `rule`, `message`, `display`, `space`, `window`, `process_manager`, `application`, `display_manager`, `space_manager`, `window_manager`, `mouse_handler`, `mission_control` — are in `src/main.rs`, each at its §1.4 position, and `cargo check` sees the whole crate for the first time |

**Wave 1 gate.** `cargo check` clean for both targets with every body `todo!()`. `cargo build`
is not attempted — a `todo!()` crate links but does nothing, and running it is pointless.
`GLOSSARY.md` and `STATE_ACCESS.md` are consistent: every signature in the crate matches its
`STATE_ACCESS.md` row.

### 3.4 Wave 2 — bodies, parallel by C file

Eighteen units: the seventeen per-module units run in parallel, and `W2-main` runs last and
alone, because `main` calls into every one of them. **Each translator owns exactly one Rust
module and edits no other file.** A translator who needs a signature changed does not change
it: they write the change into `doc/rust-rewrite/SIGNATURE_CHANGES.md` (C location, current signature, proposed
signature, why) and the integrator applies it in wave 3. Silently widening a parameter, adding
a manager, or flipping a return type is the one thing that breaks wave 2's parallelism.

Each unit also appends to `doc/rust-rewrite/DEVIATIONS.md` per decisions 4 and 5: one line per
undefined behaviour made safe and one line per piece of dead code not translated.

| Id | C source | Rust path | Depends on | Size | Done when |
| --- | --- | --- | --- | --- | --- |
| `W2-view` | `src/view.c` (1042) | `src/view.rs` | wave 1 gate | 52 fn | Every body written; the BSP tree is the index arena of decision 15; the two `tests/src/area.c` cases are present as `#[cfg(test)]` (decision 40) |
| `W2-window` | `src/window.c` (1144) | `src/window.rs` | wave 1 gate | 44 fn | Every body written; `SLSGetWindowSubLevel__Internal` (`:930-952`) is `#[repr(C, packed(4))]` with its size assertion; `window_destroy` (`:1138`) sets `id = 0` before releasing anything |
| `W2-application` | `src/application.c` (144) | `src/application.rs` | wave 1 gate | 9 fn | Every body written; the AX observer source uses the run-loop mode the C uses |
| `W2-process_manager` | `src/process_manager.c` (265) | `src/process_manager.rs` | wave 1 gate | 8 fn | Every body written; the process-table `Mutex` is never held across an ObjC or AX call, because KVO with `NSKeyValueObservingOptionInitial` re-enters synchronously (decision 22) |
| `W2-display` | `src/display.c` (252) | `src/display.rs` | wave 1 gate | 8 fn | Every body written |
| `W2-display_manager` | `src/display_manager.c` (506) | `src/display_manager.rs` | wave 1 gate | 38 fn | Every body written |
| `W2-space` | `src/space.c` (106) | `src/space.rs` | wave 1 gate | 7 fn | Every body written |
| `W2-space_manager` | `src/space_manager.c` (1233) | `src/space_manager.rs` | wave 1 gate | 65 fn | Every body written; the `kCGS*` gesture constants come from `src/ffi/core_graphics.rs` |
| `W2-window_manager` | `src/window_manager.c` (2765) | `src/window_manager.rs` | wave 1 gate | 121 fn | Every body written. Internally sequenced in four checkpoints, each ending in a `cargo check`: (a) queries and lookups, (b) rules and eligibility, (c) window operations and placement arithmetic, (d) the animation path. The thirteen hazards listed in `files/window-manager.md` §5 are each either reproduced or recorded in `DEVIATIONS.md` |
| `W2-event_loop` | `src/event_loop.c` (1721) | `src/event_loop.rs` | wave 1 gate | 40 handlers + 5 fn | Every body written. Internally sequenced in three checkpoints: (a) the loop and the channel, (b) the application/window/space/display handlers, (c) the mouse, mission-control and daemon-message handlers. The autorelease pool is drained when the queue runs empty, as in C |
| `W2-event_signal` | `src/event_signal.c` (454) | `src/event_signal.rs` | wave 1 gate | 10 fn | Every body written; the fork path computes argv, environment and the regex verdict in the parent, and the child calls only async-signal-safe functions and leaves through `_exit` (decision 25); the double fork is kept |
| `W2-rule` | `src/rule.c` (221) | `src/rule.rs` | wave 1 gate | 10 fn | Every body written; the three-valued regex result stays three-valued |
| `W2-message` | `src/message.c` (3045) | `src/message.rs` | wave 1 gate | 31 fn | Every body written. Internally sequenced in four checkpoints, each ending in a `cargo check`: (a) the tokeniser and `Response` wiring, (b) the `config` domain, (c) the `display`, `space` and `window` domains, (d) the `query`, `rule` and `signal` domains. Query JSON is byte for byte identical including the asymmetric `[`/`]` quirks; floats are cast to `f64` before formatting; `%d` of a `u32` prints `as i32`; `%lld` of a `u64` prints `as i64`; a NULL `%s` prints `(null)` (decision 29) |
| `W2-mouse_handler` | `src/mouse_handler.c` (303) | `src/mouse_handler.rs` | wave 1 gate | 11 fn | Every body written; the event tap callback reads only the atomic half of the mouse state |
| `W2-workspace` | `src/workspace.m` (303) | `src/workspace.rs` | wave 1 gate | 11 fn | Every body written; the KVO observer is registered and removed exactly where the C does, and the `removeObserver` exception is caught with `objc2::exception::catch` |
| `W2-sa` | `src/sa.m` (625) | `src/sa.rs` | wave 1 gate | 37 fn | Every body written; the frame layout is byte-identical to `src/osax/common.h`'s opcodes; `system`, `popen` and the install shell-outs are kept (decision 34) |
| `W2-mission_control` | `src/mission_control.c` (111) | `src/mission_control.rs` | wave 1 gate | 5 fn | Every body written; the two callbacks follow `patterns/ffi-objc-and-os.md` §19.2 and §19.4 |
| `W2-main` | `src/yabai.c` (356) | `src/main.rs` | wave 1 gate, every other wave 2 unit | 4 fn | `main` reproduces `src/yabai.c:261-354` in order, with the event-loop thread spawned after `window_manager_begin` and `update_window_notifications` rather than at the C position, and the channel live from `event_loop_begin` onward so start-up events queue and are handled in order (decision 12). `client_send_message`, `configure_settings_and_acquire_lock` and `parse_arguments` reproduce their C exactly, including exit codes |

**Per-unit gate.** `cargo check --target aarch64-apple-darwin` and
`cargo check --target x86_64-apple-darwin`, both clean, with `todo!()` remaining only in
modules other translators own.

**Wave 2 gate.** No `todo!()` anywhere in `src/`. `cargo check` clean for both targets.
`DEVIATIONS.md` has an entry for every undefined behaviour made safe and every piece of dead
code dropped. `SIGNATURE_CHANGES.md` is the complete list of what wave 3 must reconcile.

### 3.5 Wave 3 — integration

Sequential, five units, one agent.

| Id | C sources | Rust path | Depends on | Size | Done when |
| --- | --- | --- | --- | --- | --- |
| `W3-1` | `SIGNATURE_CHANGES.md` | every module | wave 2 gate | as recorded | Every recorded signature change is applied at the definition and at every call site; `STATE_ACCESS.md` is updated to match; cross-module type mismatches are resolved in favour of the C, never in favour of whichever module is easier to edit |
| `W3-2` | — | every module | `W3-1` | — | The crate-root `#[allow(dead_code)]` is removed and each remaining dead item is either deleted with a `DEVIATIONS.md` line or given a caller; `cargo clippy --all-targets` passes with the correctness lint group denied; style lints that would force a rename are **not** applied, because decision 37 fixes the names |
| `W3-3` | — | `bin/yabai` | `W3-2` | — | `cargo build --release --target aarch64-apple-darwin` and `--target x86_64-apple-darwin` both succeed; `lipo -create` produces `bin/yabai`; `lipo -info` reports `x86_64 arm64`; both slices carry `LC_BUILD_VERSION minos 11.0` and the `__TEXT,__info_plist` section; `codesign -f -s -` reports `Identifier=com.asmvik.yabai` |
| `W3-4` | `makefile` | `makefile` | `W3-3` | — | `make`, `make install`, `make sign`, `make clean-build`, `make clean` all run to completion; `make asan` and `make tsan` build host-only on nightly; `make man`, `make icon`, `make archive` and `make publish` are unchanged from `makefile:38-58` and fail only where they already failed |
| `W3-5` | `src/*.c`, `src/*.h`, `src/*.m`, `src/manifest.m`, `tests/`, `makefile:30-36`, `.gitignore` | — | `W3-4` | 18015 C lines deleted | `src/manifest.m` and every `.c`/`.h`/`.m` under `src/` outside `src/osax/` are deleted **except `src/misc/hashtable.h`**, which stays because `src/osax/payload.m:36-38` includes it and decision 1 forbids editing `src/osax/` (§1 layout rule 4); deleting it would stop `build.rs` compiling the payload and the crate would not build at all; the `xxd` rule and `OSAX_SRC` are gone from the makefile; `tests/` is deleted; `.gitignore` loses `/src/osax/loader_bin.c`, `/src/osax/payload_bin.c` and `/tests/bin` and gains `/target`; `make` still produces a universal `bin/yabai` |

**Wave 3 gate.** `make install` produces a signed universal `bin/yabai` from a tree whose only
C is `src/osax/`. `cargo test` runs the two transposed `area` cases green.

### 3.6 Wave 4 — fidelity review

Nineteen units, all parallel, all read-only against the C and write-only into their own review
document. A wave 4 reviewer is adversarial: the job is to find where the Rust and the C differ,
not to agree that they match.

Eighteen **per-module** reviewers, one per Rust module of §1.7, each comparing its module
against its C pair function by function, in C source order, checking:

* **Control flow.** Every early return, every `break`/`continue` target, every loop bound,
  every short-circuit. The `table_for` macro's `it` being a declaration and `continue`
  targeting the inner loop is the classic miss.
* **Numeric casts.** Decision 30: every implicit C conversion is an explicit `as` at the same
  point in the expression. `f32`/`f64`/`int` mixing, truncation and rounding happen exactly
  where they did. `struct area` is still `f32`. Wrapping arithmetic is spelled `wrapping_*`.
* **Format strings.** Decision 29: every `printf`/`fprintf` format reproduced character for
  character, `%d` of a `u32` as `as i32`, `%lld` of a `u64` as `as i64`, floats widened to
  `f64`, NULL `%s` as `(null)`, and the asymmetric `[`/`]` quirks in query JSON preserved.
* **Comments policy.** Decision 38: the module contains the comments the C has, verbatim, at
  the matching place, and no others. Any doc comment, `SAFETY` comment or comment describing
  the translation is a finding.
* **Naming glossary.** Decision 37 and `GLOSSARY.md`: no abbreviated field, parameter or local;
  every type and function spelled the way the glossary says.
* **Sentinels and nullability.** Decision 32: `bool` returns still `bool`, nullable pointers
  `Option`, `INT_MAX` and index 0 still observable.

One **unsound-`unsafe`** reviewer over the whole crate, checking decision 39: `unsafe` appears
only at FFI calls, refcon and context casts, `Send`/`Sync` impls, packed Mach structs, the
Mach-O symbol walker and the AX pid offset read. For each block: does the pointer provenance
hold, is the lifetime of what the refcon points at longer than the callback's use of it
(decision 20: the `Arc` is released on the main queue after the notification is removed), is
every packed struct read through an unaligned access, and does any `&mut` alias another `&mut`
across an FFI call that can re-enter.

| Id | C sources | Rust path reviewed | Depends on | Size | Done when |
| --- | --- | --- | --- | --- | --- |
| `W4-view` … `W4-main` (18 units) | the module's C pair | the module | wave 3 gate | one review doc each | Every C function has a verdict: matches, differs (with `path:line` on both sides), or deliberately deviates with a `DEVIATIONS.md` line backing it. A "differs" verdict without a fix or a deviation line is a blocking finding |
| `W4-unsafe` | — | every `unsafe` block in `src/` | wave 3 gate | one review doc | Every `unsafe` block is listed with its justification and mapped to one of decision 39's six permitted categories; anything outside those six is a blocking finding |

**Wave 4 gate.** No blocking finding open. `THREADS.md` carries the soundness arguments for the
`unsafe impl Send` on `EventLoopOwnedState` and for every atomic ordering the port chose,
because decision 38 forbids those arguments living in code comments.

---
## 4. The contract every phase 2 agent prompt carries

Every prompt handed to a phase 2 agent, in every wave, carries these seven blocks. They are not
optional and they are not summarised; a prompt that drops one produces work that has to be
redone.

**1. Read these, in this order, before writing anything.**

* `doc/rust-rewrite/DECISIONS.md` — in full. It is binding and settles every disagreement
  between the inventories. If an inventory recommends something a decision rejects, the
  decision wins and the rejected option is not mentioned.
* `doc/rust-rewrite/TRANSLATION_PLAN.md` §1, §2, and the unit's own row in §3.
* `doc/rust-rewrite/STATE_ACCESS.md` — the rows for the functions in this unit. Signatures come
  from here, not from the C header, because the C header omits the managers reached through
  globals.
* `doc/rust-rewrite/GLOSSARY.md` — every spelling.
* `doc/rust-rewrite/THREADS.md` — if the unit touches a thread boundary, an atomic, a callback
  or an `unsafe impl`.
* The named sections of the unit's `files/` inventory and of the relevant `sweeps/` document.
  Never the whole document: run `grep -n "^## "` to find the section, then `sed -n 'A,Bp'`.
* The C source. **The C source is ground truth.** Where an inventory is vague, or two
  inventories disagree, open the C and go by what it does.

**2. What this unit may touch.** Exactly the Rust paths in its row, plus append-only entries in
`DEVIATIONS.md` and `SIGNATURE_CHANGES.md`. Nothing else. Specifically: no other agent's Rust
module, no `src/osax/`, no `DECISIONS.md`, no other document in `doc/rust-rewrite/`, no
`Cargo.toml` (only `W0-1` edits it), no `build.rs` (only `W0-2`), no `makefile` (only `W0-14`
and `W3-4`), and no `src/*.c`, `src/*.h` or `src/*.m` until `W3-5`.

**3. Never run a state-changing git command.** `git status`, `git diff`, `git log` and
`git show` are fine. `git add`, `git commit`, `git checkout`, `git restore`, `git stash`,
`git reset`, `git rebase`, `git merge`, `git push` and `git clean` are not, in any form,
including inside a script. The orchestrator commits.

**4. Never build or launch the daemon.** `cargo check` is the gate for waves 0, 1 and 2.
`cargo build` and `make` appear only in `W3-3` and `W3-4`. No agent starts `bin/yabai`, sends
it a message, or drives the UI; that is the user's to run.

**5. No new dependency, ever.** Decision 11 closes the list. An agent that believes it needs
one writes the case into `SIGNATURE_CHANGES.md` and works around it; it does not add a line to
`Cargo.toml`.

**6. How to record a deviation.** Two files, both append-only, both one line per entry.

* `doc/rust-rewrite/DEVIATIONS.md`, for decisions 4 and 5:
  `` `src/event_loop.c:19-31` | `update_window_notifications` filled a `uint32_t window_list[1024]` from a `table_for` with no bound | Rust stops filling at 1024 ``
  and for dead code:
  `` `src/misc/timer.h:136-138` | `TIME_FUNCTION`, dead under the shipped build | not translated ``
* `doc/rust-rewrite/SIGNATURE_CHANGES.md`, for anything a wave 2 unit cannot implement against
  its wave 1 signature: the C location, the current Rust signature, the proposed one, and why.
  The unit does **not** apply the change; `W3-1` does.

**7. Report format.** The agent returns: what it wrote, the gate command it ran and its output,
the `DEVIATIONS.md` lines it appended, the `SIGNATURE_CHANGES.md` entries it opened, and any
place where an inventory and the C source disagreed and it followed the C.

Two rules that bear repeating inside every prompt because they are the ones agents break:

* **Comments.** Decision 38. Carry over only comments that exist in the C source, verbatim, at
  the matching place. No doc comments, no `SAFETY` comments, no comments describing the
  translation. A soundness argument goes in `THREADS.md`.
* **Names.** Decision 37. Keep the C function and type names so the Rust can be read against
  the C, and never abbreviate a field, parameter or local. Phase 3 renames; phase 2 does not.

---

## 5. Phase 3 sketch

Phase 2 leaves a crate that is correct and readable against the C, with two modules over two
thousand lines and several that mix three concepts. Phase 3 splits and renames. It changes no
behaviour: every move is a move, and `cargo check` for both targets runs between every one.

### 5.1 The target module tree

```
src/main.rs
src/cli/                      argument parsing, the client, --version and --help text
src/startup/                  panic hook, lock file, settings, config exec, the start-up order
src/state/                    EventLoopOwnedState, the process-wide statics
src/event/
  queue.rs                    the channel, the Event enum, its Drop
  run_loop.rs                 the loop, autorelease draining
  handlers/application.rs
  handlers/window.rs
  handlers/space.rs
  handlers/display.rs
  handlers/mouse.rs
  handlers/mission_control.rs
  handlers/menu.rs
  handlers/system.rs
  handlers/daemon_message.rs
src/notifications/
  window.rs                   AX and SkyLight notifications for windows
  space.rs                    space created/destroyed/changed
  display.rs                  display added/removed/moved/resized/changed
  application.rs              AX observer lifecycle, Carbon process events
  mouse.rs                    the CGEventTap
  mission_control.rs          the mission-control observers
  workspace.rs                NSWorkspace, NSDistributedNotificationCenter, KVO
src/message/
  token.rs                    the cursor, Token, TokenValue
  dispatch.rs                 the domain switch
  domain/config.rs
  domain/display.rs
  domain/space.rs
  domain/window.rs
  domain/query.rs
  domain/rule.rs
  domain/signal.rs
src/query/                    the read-side lookups the query domain calls
src/serialise/
  window.rs  space.rs  display.rs  rule.rs  signal.rs
src/layout/
  area.rs                     struct area and its arithmetic
  tree.rs                     the BSP arena: nodes, split, insert, remove, balance, equalize
  settings.rs                 view type, padding, gap, flags, SPACE_PROPERTIES
  insertion.rs                the insertion point and the feedback window
src/window/
  model.rs                    the Window struct, its flags, its liveness cell
  operations.rs               move, resize, set frame, layer, opacity, order
  animation.rs                AnimationContext, the CVDisplayLink, the proxies
  rules.rs                    rule matching and effects
  capture.rs                  WindowCapture and the image path
src/application/
src/process/
src/display/
src/space/
src/mouse/
  tap.rs                      the atomic half the tap reads
  drag.rs                     the drag half the event loop owns
  drop.rs                     drop-action determination
src/signal/
  definition.rs               the signal types and their parsing
  queue.rs                    the owned queue
  exec.rs                     the fork/exec path
src/scripting_addition/
  frame.rs                    the packed frame and its opcodes
  client.rs                   the socket client that talks to the payload
  installer.rs                install, uninstall, load, the version handshake
src/service/
src/support/                  table.rs regex.rs response.rs log.rs notify.rs timer.rs
                              strings.rs geometry.rs sockets.rs filesystem.rs json.rs image.rs
src/ffi/                      unchanged from phase 2
src/osax/                     still C
```

### 5.2 One `AGENTS.md` per module folder

Every folder above gets an `AGENTS.md`: `src/cli/`, `src/startup/`, `src/state/`, `src/event/`,
`src/event/handlers/`, `src/notifications/`, `src/message/`, `src/message/domain/`,
`src/query/`, `src/serialise/`, `src/layout/`, `src/window/`, `src/application/`,
`src/process/`, `src/display/`, `src/space/`, `src/mouse/`, `src/signal/`,
`src/scripting_addition/`, `src/service/`, `src/support/`, `src/ffi/`.

Each one says two things and only two things:

1. **What the module holds** — the concept, in a sentence or two. "The BSP tree: the node
   arena, splitting, insertion, removal and the two rebalancing walks." Not a list of files,
   not a list of functions, not a call graph. Those go stale the first time someone moves a
   function, and a stale map is worse than none.
2. **Notes about the module** — the things that are true of the whole module and are not
   visible from any one function. Which thread its code runs on. Which invariants hold at its
   boundary. Which lock may never be held across which call. Which numbers are observable
   externally and so may not be "cleaned up". Where its wire format is pinned.

Concretely: `src/notifications/AGENTS.md` records that every callback here runs on the main
thread and may not dereference event-loop-owned memory (decision 20), and that a refcon carries
either an integer id or a raw `Arc` pointer released on the main queue after the notification
is removed. `src/window/AGENTS.md` records that `animation.rs` shares an `Arc<AnimationContext>`
with a CVDisplayLink callback and that the `tx`/`ty`/`tw`/`th` atomics are `f32` bits.
`src/message/domain/AGENTS.md` records that the response bytes are part of the public wire
format and that the query JSON's asymmetric `[`/`]` quirks are deliberate.
`src/layout/AGENTS.md` records that the root is always `NodeId` 0 because the C reset the root
in place.

No `AGENTS.md` repeats a decision from `DECISIONS.md`; it cites the number.

### 5.3 The order to split in

Fifteen steps. Each step is one move or one batch of moves of the same kind, ends with
`cargo check` for both targets, and is committed on its own. Pure leaves move first; the two
giants move only once their helpers are out; the rename pass is last so that until then every
Rust name is still greppable against the C.

1. `src/misc/helpers.rs` splits by concept into `src/support/{strings,geometry,sockets,
   filesystem,process,json,image}.rs`. Only `use` lines change anywhere else.
2. `src/misc/{table,regex,response,log,notify,timer}.rs` move to `src/support/`. `src/misc.rs`
   and `src/misc/` disappear.
3. `src/view.rs` splits into `src/layout/{area,tree,settings,insertion}.rs`.
4. `src/window.rs` splits into `src/window/{model,capture}.rs`.
5. `src/sa.rs` splits into `src/scripting_addition/{frame,client,installer}.rs`.
6. `src/mouse_handler.rs` splits into `src/mouse/{tap,drag,drop}.rs`.
7. `src/event_signal.rs` splits into `src/signal/{definition,queue,exec}.rs`.
8. Serialisation is lifted out of `window_manager`, `space_manager`, `display_manager`, `rule`
   and `signal` into `src/serialise/*`. Nothing else moves in this step.
9. `src/window_manager.rs` splits into `src/window/{operations,animation,rules}.rs` and what
   remains of the registry joins `src/window/model.rs`.
10. `src/space_manager.rs` and `src/display_manager.rs` split the same way, into
    `src/space/` and `src/display/`, with their query-side lookups going to `src/query/`.
11. `src/message.rs` splits into `src/message/token.rs`, `src/message/dispatch.rs` and
    `src/message/domain/*.rs`, one file per `DOMAIN_*` constant (`src/message.c:14-20`).
12. `src/event_loop.rs` splits into `src/event/queue.rs`, `src/event/run_loop.rs` and
    `src/event/handlers/*.rs`, grouped by event subject.
13. The OS notification registration and removal is lifted out of `event`, `window`, `space`,
    `display`, `application`, `mouse`, `mission_control` and `workspace` into
    `src/notifications/*`. This step comes after 12 because until the handlers are separated
    the registration sites and the handling sites sit in the same function.
14. `src/main.rs` splits into `src/cli/` and `src/startup/`; `src/globals.rs` and
    `src/state.rs` become `src/state/`.
15. The rename pass. Every C name phase 2 preserved becomes the name the Rust wants, one LSP
    rename at a time, `cargo check` after each batch of ten. `GLOSSARY.md` gains a second
    column: the phase 3 spelling. This is last because every earlier step depended on being
    able to grep a C name and find its Rust counterpart.

Each `AGENTS.md` is written in the step that creates its folder, not afterwards.
