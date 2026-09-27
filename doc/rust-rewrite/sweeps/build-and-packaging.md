# Phase 1 sweep — build, scripting-addition embedding, packaging, tests

Cross-cutting sweep: **build-and-packaging**.

Sources read completely: `makefile` (68 lines), `assets/Info.plist` (20), `src/manifest.m` (97),
`src/sa.m` (625), `src/sa.h` (31), `src/yabai.c` (356), `src/osax/common.h` (48),
`tests/makefile` (13), `tests/src/tests.m` (54), `tests/src/area.c` (89),
`scripts/install.sh` (99), `scripts/codesign` (47), `scripts/seticon.py` (11),
`.gitignore` (5), `.github/FUNDING.yml` (4), `src/misc/service.h` (314),
`src/misc/timer.h` (165). Heads of `src/osax/payload.m` and `src/osax/loader.m` read for their
include graph; their bodies belong to other sweeps and stay in C.

Every claim marked **[verified]** was checked by running the command on this machine in a
scratch directory. Nothing under `src/`, `tests/`, `scripts/`, `assets/`, `examples/` or
`makefile` was touched; no yabai build was run.

---

## 1. What the makefile does today

### 1.1 Variables (`makefile:1-15`)

| Variable | Value | Note |
|---|---|---|
| `FRAMEWORK_PATH` | `-F/System/Library/PrivateFrameworks` | needed only for SkyLight |
| `FRAMEWORK` | `-framework Carbon -framework Cocoa -framework CoreServices -framework CoreVideo -framework SkyLight` | link order as written |
| `CLI_FLAGS` | empty | escape hatch: `make CLI_FLAGS=-DFOO` |
| `BUILD_FLAGS` | see §1.2 | overridden wholesale per target |
| `BUILD_PATH` | `./bin` | gitignored (`.gitignore:1`) |
| `OSAX_SRC` | `./src/osax/payload_bin.c ./src/osax/loader_bin.c` | generated, gitignored (`.gitignore:4-5`) |
| `YABAI_SRC` | `./src/manifest.m $(OSAX_SRC)` | three translation units total |
| `INFO_PLIST` | `./assets/Info.plist` | |
| `BINS` | `./bin/yabai` | |

The daemon is three translation units, not one: `manifest.m` (the whole unity build) plus the two
generated `*_bin.c` byte arrays. `manifest.m` includes every header (`src/manifest.m:45-78`) then
every implementation file (`src/manifest.m:80-97`), so all `static` symbols in the daemon are
visible to each other and all globals are the ones defined in `src/yabai.c:27-52`.

### 1.2 The flag matrix

The only link/compile rule is `makefile:66-68`:

```
$(BUILD_PATH)/yabai: $(YABAI_SRC)
	mkdir -p $(BUILD_PATH)
	xcrun clang $^ $(BUILD_FLAGS) $(CLI_FLAGS) $(FRAMEWORK_PATH) $(FRAMEWORK) -o $@
```

Four target-specific `BUILD_FLAGS` overrides:

| Target | Line | Flags that differ from the others |
|---|---|---|
| `all` (default) | `makefile:4`, `19` | `-g -O0` |
| `asan` | `makefile:21-22` | `-g -O0 -fsanitize=address,undefined` |
| `tsan` | `makefile:24-25` | `-g -O0 -fsanitize=thread,undefined` |
| `install` | `makefile:27-28` | `-DNDEBUG -O3` (no `-g`) |

Shared by all four: `-std=c11 -Wall -Wextra -fvisibility=hidden -mmacosx-version-min=11.0
-fno-objc-arc -arch x86_64 -arch arm64 -sectcreate __TEXT __info_plist ./assets/Info.plist`.

Three things worth spelling out, because they are easy to get wrong in the Cargo port:

1. **`make` with no argument produces a debug build** (`-g -O0`). The release build is `make
   install` — which, despite the name, installs nothing; it only rebuilds `./bin/yabai` with
   `-DNDEBUG -O3` (`makefile:27-28`). Keep that user-facing meaning.
2. **`-arch x86_64 -arch arm64` in one clang invocation** makes clang compile and link twice and
   `lipo` the result itself. Rust has no equivalent; see §6.7.
3. **`-DNDEBUG` in the release build disables every `assert()`** in the daemon, of which there
   are many (`src/misc/ts.h:38`, `src/misc/ts.h:78`, `src/misc/ts.h:91`, `src/sa.m:150`,
   `src/sa.m:255`, …). The faithful Rust counterpart of C `assert()` is `debug_assert!`, not
   `assert!`.

`all`, `asan`, `tsan` and `install` all depend on `clean-build` (`makefile:60-61`, `rm -rf
./bin`), so every build is a full rebuild of the binary — but **not** of the osax, which is
cached (§1.3).

### 1.3 The scripting-addition pipeline (`makefile:11`, `30-36`, `63-64`)

```
$(OSAX_SRC): $(OSAX_PATH)/loader.m $(OSAX_PATH)/payload.m
	xcrun clang ./src/osax/payload.m -shared -fPIC -O3 -mmacosx-version-min=11.0 \
	    -arch x86_64 -arch arm64e -o ./src/osax/payload \
	    -F/System/Library/PrivateFrameworks -framework SkyLight -framework Foundation -framework Carbon
	xcrun clang ./src/osax/loader.m -O3 -mmacosx-version-min=11.0 \
	    -arch x86_64 -arch arm64e -o ./src/osax/loader -framework Cocoa
	xxd -i -a ./src/osax/payload ./src/osax/payload_bin.c
	xxd -i -a ./src/osax/loader  ./src/osax/loader_bin.c
	rm -f ./src/osax/payload
	rm -f ./src/osax/loader
```

Facts that matter:

* **`payload` is a dylib** (`-shared -fPIC`), **`loader` is an executable**. Both are fat
  `x86_64 + arm64e`. **[verified]** `xcrun clang … -arch x86_64 -arch arm64e` still works with
  the installed Apple clang 17.0.0 for both an executable and a `-shared` dylib; `lipo -info`
  reports `x86_64 arm64e` and `file` reports `Mach-O 64-bit dynamically linked shared library
  arm64e` for the dylib slice.
* **Their framework sets differ.** payload links SkyLight (private), Foundation and Carbon;
  loader links only Cocoa and needs no `-F`.
* **`arm64e` is the whole reason this stays C.** Rust has no `arm64e-apple-darwin` target, and
  the payload is injected into Dock.app, which is arm64e on Apple Silicon. At run time this also
  requires the `-arm64e_preview_abi` boot-arg, checked at `src/sa.m:317-331`.
* **`xxd -i -a <path>` derives the C symbol name from the path**, replacing every non-identifier
  character with `_`. `./src/osax/payload` becomes `__src_osax_payload` / `__src_osax_payload_len`
  — **[verified]** by running `xxd -i -a ./src/osax/payload` on a stand-in binary; the output is
  exactly `unsigned char __src_osax_payload[] = {…}` / `unsigned int __src_osax_payload_len = N;`.
  Those are the four symbols declared at `src/sa.h:4-7`. **Any change to the makefile's path
  spelling silently breaks the link.** This whole fragility disappears in the Cargo port.
* **`-fno-objc-arc` is not passed here.** clang defaults to manual retain/release, so payload.m
  and loader.m are MRR like the daemon.

Two pre-existing makefile bugs the Cargo port should not reproduce:

* **Incomplete prerequisites** (`makefile:30`). The rule depends only on `loader.m` and
  `payload.m`. It does *not* depend on `src/osax/arm64_payload.m`, `src/osax/x64_payload.m`,
  `src/osax/common.h`, or `src/misc/hashtable.h` — all of which `payload.m` `#include`s
  (`src/osax/payload.m:27`, `:30`, `:32`, `:37`). Editing any of those four leaves a stale
  `payload_bin.c` behind until `make clean`.
* **Two targets, one recipe** (`makefile:30`). GNU make runs the recipe once per target, i.e.
  twice, and under `make -j` the two invocations race on the same intermediate files.

`make clean` (`makefile:63-64`) removes `./bin` plus the two generated `*_bin.c`.

### 1.4 `Info.plist` and why `-sectcreate` is load-bearing

`assets/Info.plist` (20 lines) declares `CFBundleExecutable=yabai` (`assets/Info.plist:7-8`),
`CFBundleIdentifier=com.asmvik.yabai` (`:9-10`), `CFBundlePackageType=APPL` (`:15-16`).

`-sectcreate __TEXT __info_plist assets/Info.plist` embeds it into the Mach-O so a *bare
executable* still has a bundle identity. That identity is what `codesign` uses as the signing
identifier, and the signing identifier is what TCC keys the Accessibility and Screen-Recording
grants on (`README.md:51-52`, `README.md:61`). If the Rust binary loses the section, every user
re-grants Accessibility.

**[verified]** end to end:

* `cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,<abs path>` produces a real
  `(__TEXT,__info_plist)` section — `otool -arch arm64 -s __TEXT __info_plist` and
  `otool -arch x86_64 …` both dump the plist from a lipo'd Rust binary.
* `codesign -f -s -` on that binary reports `Identifier=com.asmvik.yabai`, byte-identical to the
  identifier a `xcrun clang … -sectcreate __TEXT __info_plist …` build gets. Without the section
  the identifier degrades to `<filename>-<hash>`, which would break TCC.

### 1.5 `man`, `icon`, `sign`, `archive`, `publish`

| Target | Line | What it runs | Rust impact |
|---|---|---|---|
| `man` | `makefile:38-39` | `asciidoctor -b manpage doc/yabai.asciidoc -o doc/yabai.1` | none; keep verbatim |
| `icon` | `makefile:41-42` | `python3 scripts/seticon.py assets/icon/2x/icon-512px@2x.png bin/yabai` | none; keep verbatim |
| `sign` | `makefile:57-58` | `codesign -fs "yabai-cert" bin/yabai` | none; must run *after* lipo |
| `archive` | `makefile:48-55` | `man install sign icon`, then copies `bin/`, `doc/`, `examples/` into `./archive` and tars it to `bin/$(yabai --version).tar.gz`, then removes `./archive` | only the `install` half changes |
| `publish` | `makefile:44-46` | rewrites **line 60** and **line 61** of `scripts/install.sh` with the new version and tarball sha256 | none |

Details:

* `archive` names the tarball from the binary's own `--version` output: `bin/yabai --version`
  prints `yabai-v7.1.25` (`src/yabai.c:206`), so the artifact is `bin/yabai-v7.1.25.tar.gz`
  (`makefile:54`). `publish` re-derives the same name (`makefile:46`). **The built binary is the
  source of truth for the release version**, which is why the Cargo package version must equal
  `MAJOR.MINOR.PATCH` (§6.11).
* `publish` uses absolute `sed` line addresses `60s` and `61s` (`makefile:45-46`) against
  `scripts/install.sh:60` (`VERSION="7.1.25"`) and `scripts/install.sh:61`
  (`EXPECTED_HASH="76f383…"`). Brittle, but out of scope to change.
* `archive` ordering is `sign` then `icon` (`makefile:48`). `scripts/seticon.py:10` uses
  `NSWorkspace.setIcon_forFile_options_`, which writes Finder extended attributes rather than
  Mach-O contents, so it does not invalidate the signature. Keep the order.
* **[verified]** `asciidoctor` is **not installed** on this machine (`which asciidoctor` fails),
  and **PyObjC is not importable** from either `python3` on `PATH` (mise 3.11.16) or
  `/usr/bin/python3` — `import Cocoa` raises `ModuleNotFoundError`. So `make man` and `make icon`
  both fail here today, independently of the rewrite.

### 1.6 `.github` — there are no CI workflows

**There is no `.github/workflows` directory.** `.github` contains exactly one file,
`.github/FUNDING.yml` (4 lines, GitHub Sponsors + Patreon). `git log -- .github` shows two
commits, both about sponsors. **Nothing in this repository builds yabai in CI**, so the rewrite
has no CI to port. If CI is wanted, it has to be written from scratch; see §9.

### 1.7 `scripts/`

* `scripts/install.sh` (99 lines) is the end-user release installer: downloads
  `yabai-v$VERSION.tar.gz` from GitHub releases (`:67`), checks the sha256 against the constant
  at `:61`, untars, copies `archive/bin/yabai` and `archive/doc/yabai.1` into place (`:75-76`),
  and prints the sudoers line for `--load-sa` (`:89`). It never compiles anything — **unaffected
  by the rewrite** beyond the fact that `make publish` keeps rewriting lines 60-61.
* `scripts/codesign` (47 lines) is a developer helper that picks a codesigning identity
  interactively and runs `codesign --deep --force --verbose --sign <cert> <target>` (`:46`). It
  takes a path, not a build system — **unaffected**.
* `scripts/seticon.py` (11 lines) — see §1.5.

---

## 2. The test harness today

### 2.1 How it is built (`tests/makefile:8-10`)

```
clang ./src/tests.m -o ./bin/tests -DTESTS -DPROFILE=1 \
    -F/System/Library/PrivateFrameworks \
    -framework Carbon -framework Cocoa -framework CoreServices -framework CoreVideo -framework SkyLight
```

Note what is *missing* versus the daemon build: no `-arch` flags (host-only), no
`-mmacosx-version-min`, no `-sectcreate`, no `-fno-objc-arc`, no `-std=c11`, no
`$(OSAX_SRC)` — and `xcrun` is not used.

### 2.2 How it links at all

`tests/src/tests.m:1-4` defines four **fake one-byte stand-ins** for the osax symbols:

```c
unsigned char __src_osax_payload[1];
unsigned int __src_osax_payload_len;
unsigned char __src_osax_loader[1];
unsigned int __src_osax_loader_len;
```

That is the only reason the test binary links without `payload_bin.c` / `loader_bin.c`. Then
`tests/src/tests.m:6` `#include`s the entire daemon (`../../src/manifest.m`), and `-DTESTS`
suppresses the daemon's `main()` via `#ifndef TESTS` at `src/yabai.c:260-354`. So **the test
binary is the whole daemon plus a different `main()`**.

`-DPROFILE=1` turns on `src/misc/timer.h:4-92`, which is what supplies `read_cpu_timer()` and
`read_cpu_freq()` — used by the harness only for per-test timing (`tests/src/tests.m:35-47`).
`PROFILE=1` deliberately stops short of `PROFILE >= 2` (`src/misc/timer.h:94-149`), so
`TIME_FUNCTION` / `TIME_BLOCK` stay no-ops and `PROFILER_END_TRANSLATION_UNIT`
(`src/yabai.c:356`) compiles away.

### 2.3 What is actually tested

A hand-rolled macro harness: `TEST_SIG`/`TEST_FUNC`/`TEST_CHECK` (`tests/src/tests.m:8-12`), a
manual registration list `TEST_LIST` (`tests/src/tests.m:17-19`), a driver loop
(`tests/src/tests.m:28-54`) that prints ANSI-coloured results and returns `EXIT_FAILURE` if any
test failed.

**Two tests exist, both in `tests/src/area.c`:**

* `display_area_is_in_direction` (`tests/src/area.c:28-44`) — exercises `area_is_in_direction`
  (`src/view.c:541`) over a fixed three-display layout built at `tests/src/area.c:7-26`.
* `closest_display_in_direction` (`tests/src/area.c:66-89`) — exercises a local reimplementation
  of the nearest-display search (`tests/src/area.c:46-64`) that mirrors
  `src/display_manager.c:276-290`, on top of `area_distance_in_direction` (`src/view.c:563`).

Both are pure geometry over `struct area` (`src/view.h:42-48`) and `area_max_point`
(`src/view.c:126`). **Neither needs a window server, a connection, or any global state.** That is
the entire automated test suite of this project.

---

## 3. Where the Rust sources live — definitive recommendation

**Put the Rust sources in `src/*.rs`, next to the C files, with `src/main.rs` as the crate root,
`Cargo.toml` and `build.rs` at the repo root, and leave `src/osax/` alone.**

Concretely:

```
Cargo.toml                 (new)
build.rs                   (new)
rust-toolchain.toml        (new)
makefile                   (rewritten in place, §6.8)
src/main.rs                (new; was src/yabai.c)
src/window_manager.rs      (new; was src/window_manager.{c,h})
src/misc.rs + src/misc/*.rs (new; was src/misc/*.h)
src/osax/                  (UNTOUCHED — still C/ObjC, still arm64e)
src/*.c, src/*.h, src/*.m  (deleted only once `cargo build` yields a working daemon)
tests/                     (deleted at the end of phase 2; see §6.10)
```

Why this and not a separate `rust/` tree:

* **Cargo and the C unity build cannot collide.** Cargo only reads `.rs` files reachable from
  `src/main.rs`; it never looks at `src/*.c`, `src/*.h`, `src/*.m`. `src/manifest.m:80-97` names
  every C file explicitly, so a new `src/window_manager.rs` is invisible to it. `make` and
  `cargo build` can coexist for the whole of phase 2, which is exactly what a file-by-file
  transposition needs.
* **The pairing is the point of phase 2.** `src/view.c` → `src/view.rs` sitting in the same
  directory keeps "did I port this file" answerable by `ls`, keeps the diff reviewable, and makes
  the final deletion a single `git rm src/*.c src/*.h src/*.m`.
* **`src/main.rs` is Cargo's default bin path**, so `[[bin]] path = …` is unnecessary and the
  crate needs no non-default layout knobs.
* A `rust/` subtree would need `[[bin]] path = "rust/src/main.rs"` (or a workspace), would put
  the ported file two directories away from its source, and buys nothing — there is no
  name collision to avoid.

Three layout footguns, all real, all cheap to avoid:

1. **`src/misc/` already exists as a directory of headers.** Rust's 2018+ module layout wants
   `src/misc.rs` *plus* `src/misc/`, which is exactly what you get. Do **not** create
   `src/misc/mod.rs` — both forms work, but mixing them in one crate is a lint (`mod.rs` style is
   also fine, just pick one). Recommendation: `src/misc.rs` + `src/misc/<name>.rs`.
2. **Do not create `src/osax.rs`.** It would make Rust treat `src/osax/` as that module's
   submodule directory — harmless today (no `.rs` files there) but an unnecessary trap. The osax
   constants come from `OUT_DIR` instead (§6.6).
3. **`examples/` is a Cargo auto-discovery directory.** `examples/yabairc` and `examples/skhdrc`
   are shell config samples, not Rust. Cargo ignores non-`.rs` files there today, but set
   `autoexamples = false` in `[package]` so nobody's stray `examples/foo.rs` ever becomes a build
   target. `tests/src/tests.m` and `tests/src/area.c` are likewise ignored (Cargo only picks up
   `tests/*.rs` and `tests/*/main.rs`).

**Order of work inside phase 2 (this is a build constraint, not a style preference).** `cargo
build` compiles the whole crate or nothing, and every module in this daemon reaches for the
globals defined at `src/yabai.c:27-52` plus the function pointers at `src/misc/extern.h:3-4`. So
port `src/yabai.c` → `src/main.rs` **first**, as a stub whose `fn main()` does nothing but which
already declares those globals; then port bottom-up in dependency order (`misc/*`, `view`,
`window`/`display`/`space`, the managers, `event_loop`, `message`, `mouse_handler`), adding one
`mod` line to `src/main.rs` per landed file and running `cargo check` as the per-file gate. Until
the last file lands, `cargo build` will not produce a runnable daemon — that is expected, and it
is why the C build must keep working until then.

---

## 4. Toolchain state on this machine

**[verified]**

| Tool | State |
|---|---|
| `rustc` / `cargo` | 1.98.0 (stable, default `stable-aarch64-apple-darwin`) — edition 2024 requires ≥1.85, so fine |
| `aarch64-apple-darwin` target | installed |
| `x86_64-apple-darwin` target | installed; cross-linking from the arm64 host works |
| `rustfmt` 1.9.0, `clippy` | installed |
| `rust-src` | installed on **both** stable and nightly (needed for `-Zbuild-std`) |
| nightly toolchain | `1.100.0-nightly` installed (needed for `-Zsanitizer`) |
| Apple clang | 17.0.0, SDK 26.1, `-arch arm64e` accepted |
| `lipo`, `xxd`, `codesign` | `/usr/bin` |
| `asciidoctor` | **missing** — `make man` fails today |
| PyObjC (`import Cocoa`) | **missing** on both python3s — `make icon` fails today |

Nothing needs installing for the Rust build itself. `asciidoctor` (`gem install asciidoctor`) and
PyObjC (`pip install pyobjc-framework-Cocoa`) are pre-existing gaps for `man` and `icon`.

---

## 5. Things the Cargo port must not quietly change

* **Deployment target.** **[verified]** with no `MACOSX_DEPLOYMENT_TARGET`, `x86_64-apple-darwin`
  emits `LC_VERSION_MIN_MACOSX version 10.12`; the C build emits `LC_BUILD_VERSION minos 11.0`.
  Setting `MACOSX_DEPLOYMENT_TARGET=11.0` makes both Rust slices emit `LC_BUILD_VERSION minos
  11.0`, matching `-mmacosx-version-min=11.0`. This must be set, or the two builds differ.
* **Overflow semantics.** Cargo's dev profile turns on `overflow-checks`; C has none, and the
  daemon does plenty of arithmetic on window/space ids. Debug Rust will panic where debug C wraps.
  Closest C analogue is `-fsanitize=undefined` in the `asan`/`tsan` targets (`makefile:21`, `:24`),
  so keeping the default (on in dev, off in release) is the honest mapping — just know that
  `make` and `make install` will then differ in behaviour on overflow, which they do not today.
* **`assert()` vs `assert!`.** See §1.2 item 3: `-DNDEBUG` is release-only, so C `assert()` maps
  to `debug_assert!`.
* **`static mut` in edition 2024.** `static_mut_refs` is deny-by-default in edition 2024, so
  `&mut G_WINDOW_MANAGER` is a hard error; the globals from `src/yabai.c:27-52` have to be reached
  through `&raw mut` / `UnsafeCell`. That belongs to the globals sweep, but it is a direct
  consequence of the edition recommended here, so it is flagged.
* **Signing identifier.** See §1.4 — the `__info_plist` section is not cosmetic.

---

## 6. The Cargo design

### 6.1 `Cargo.toml`

```toml
[package]
name         = "yabai"
version      = "7.1.25"       # single source of truth; see §6.11
edition      = "2024"
rust-version = "1.85"
publish      = false
autoexamples = false          # examples/ holds yabairc / skhdrc, not Rust
build        = "build.rs"

[[bin]]
name = "yabai"

[profile.dev]                 # mirrors  -g -O0
opt-level = 0
debug     = true
panic     = "abort"

[profile.release]             # mirrors  -DNDEBUG -O3
opt-level        = 3
debug            = false
debug-assertions = false
overflow-checks  = false
lto              = "fat"
codegen-units    = 1
panic            = "abort"
```

`lto = "fat"` + `codegen-units = 1` is the closest Rust gets to what the unity build gives clang
today: `src/manifest.m` hands clang one translation unit, so every `static inline` helper in
`view.c`, `helpers.h`, `ts.h` and friends is inlinable everywhere. A single Rust crate already
gets cross-module inlining, but only LTO recovers inlining across the codegen-unit boundary in
release.

`strip` is left at its default (`false`). The C release build does not strip either
(`makefile:27`), and stripping would change what `codesign` seals.

**`panic = "abort"` — the justification.** Abort, in both profiles. Three independent reasons:

1. **Unwinding out of this daemon's callbacks is not a thing that can work.** Rust code will be
   entered from C in at least five places: SkyLight connection notify procs
   (`src/yabai.c:322-333`, signature at `src/misc/extern.h:1-2`), AXObserver callbacks, a
   `CGEventTap` callback, `dispatch_get_main_queue()` blocks (`src/event_loop.c:93`, `:157`,
   `:1478`, `:1516`, `:1520`) and a `CVDisplayLink` output callback. Since Rust 1.81 an
   `extern "C"` function that unwinds aborts at the boundary anyway — `panic = "unwind"` would
   buy landing pads and larger code for a behaviour you cannot use.
2. **It matches the C daemon.** There is no exception machinery in yabai; the failure idiom is
   `error(…)` → `exit` (`src/yabai.c:57`, `:89`, `:280`, …).
3. **It costs nothing in tests.** **[verified]** Cargo ignores the `panic` setting for the `test`
   and `bench` profiles: with `panic = "abort"` set in both `[profile.dev]` and
   `[profile.release]`, `cargo test` still built and ran a `#[test]` successfully. So
   `#[should_panic]` and per-test isolation keep working.

The one thing given up is `std::panic::catch_unwind`, which the C daemon has no analogue for.

### 6.2 `rust-toolchain.toml`

```toml
[toolchain]
channel    = "stable"
targets    = ["aarch64-apple-darwin", "x86_64-apple-darwin"]
components = ["rustfmt", "clippy", "rust-src"]
```

Stable, not nightly: nightly is needed only for `make asan` / `make tsan`, which pass
`cargo +nightly` explicitly (§6.9).

### 6.3 `build.rs`

`build.rs` replaces the whole `xxd` pipeline (`makefile:30-36`) and the three generated/ignored
files it produced. Sketch, with every value traced to its makefile line:

```rust
use std::{env, path::PathBuf, process::Command};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir      = PathBuf::from(env::var("OUT_DIR").unwrap());
    let osax         = manifest_dir.join("src/osax");

    // makefile:30 is missing these four; build.rs is not.
    for dependency in [
        "src/osax/payload.m", "src/osax/loader.m",
        "src/osax/arm64_payload.m", "src/osax/x64_payload.m",
        "src/osax/common.h", "src/misc/hashtable.h",
        "assets/Info.plist", "build.rs",
    ] {
        println!("cargo:rerun-if-changed={dependency}");
    }

    // makefile:31
    run(Command::new("xcrun").args(["clang"])
        .arg(osax.join("payload.m"))
        .args(["-shared", "-fPIC", "-O3", "-mmacosx-version-min=11.0",
               "-arch", "x86_64", "-arch", "arm64e"])
        .arg("-o").arg(out_dir.join("payload"))
        .args(["-F/System/Library/PrivateFrameworks",
               "-framework", "SkyLight", "-framework", "Foundation", "-framework", "Carbon"]));

    // makefile:32
    run(Command::new("xcrun").args(["clang"])
        .arg(osax.join("loader.m"))
        .args(["-O3", "-mmacosx-version-min=11.0", "-arch", "x86_64", "-arch", "arm64e"])
        .arg("-o").arg(out_dir.join("loader"))
        .args(["-framework", "Cocoa"]));

    generate_osax_constants(&osax.join("common.h"), &out_dir); // §6.6

    println!("cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET=11.0");          // §5
    println!("cargo:rustc-link-search=framework=/System/Library/PrivateFrameworks"); // makefile:1
    for framework in ["Carbon", "Cocoa", "CoreServices", "CoreVideo", "SkyLight"] {  // makefile:2
        println!("cargo:rustc-link-lib=framework={framework}");
    }
    println!(
        "cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,{}",          // makefile:4
        manifest_dir.join("assets/Info.plist").display()
    );
}
```

Verified details behind that sketch:

* **[verified]** `cargo:rustc-link-search=framework=/System/Library/PrivateFrameworks` plus
  `cargo:rustc-link-lib=framework=<name>` links all five frameworks in the makefile's order —
  `otool -L` on the result lists Carbon, Cocoa, CoreServices, CoreVideo and
  `/System/Library/PrivateFrameworks/SkyLight.framework/…`. Use these directives rather than raw
  `-F` / `-framework` link-args, because they apply to **every** target (bin *and* test binaries),
  whereas `rustc-link-arg-bins` applies only to bin targets.
* **[verified]** `cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,<abs path>` is the
  exact working syntax. `-Wl,a,b,c` splits into three linker arguments, which is what ld64's
  three-argument `-sectcreate` needs. Keep the path **absolute** (`CARGO_MANIFEST_DIR`-relative);
  there is no reason to rely on rustc's cwd. `rustc-link-arg-bins` (not plain `rustc-link-arg`) is
  right here — the section belongs on the shipped binary, not on test harnesses. Note that
  `cargo test` on a `[[bin]]` target compiles *that same bin target* with `--test`, so it does
  pick the flag up; that is harmless.
* **[verified]** `cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET=11.0` from build.rs is enough to get
  `LC_BUILD_VERSION minos 11.0` on both slices — no `.cargo/config.toml` needed. A
  `.cargo/config.toml` `[env]` table works too (also verified), but Cargo reads config from the
  **cwd** upward rather than from the manifest directory, so it silently stops applying when
  someone builds with `--manifest-path` from elsewhere. Prefer build.rs; add the config file only
  as belt-and-braces.
* **build.rs runs once per `--target`**, so the osax is compiled twice for a universal build.
  Both runs produce the identical fat `x86_64 + arm64e` artifacts (the `-arch` flags are hard-
  coded and do not depend on the host or the cargo target), so the two Rust slices embed
  byte-identical payload bytes and `lipo` is safe. The waste is a couple of seconds; `OUT_DIR` is
  the sanctioned location and sharing a cache across targets is not worth the complexity.
* **No `cc` crate.** The `cc` crate builds static archives for the current target; it cannot emit
  a fat `x86_64 + arm64e` standalone dylib or executable. Drive `xcrun clang` directly.

### 6.4 How `src/sa.rs` consumes the embedded binaries

`src/sa.h:4-7` declares the four xxd symbols; `src/sa.m:222` and `src/sa.m:226` are their only
uses, both feeding `scripting_addition_write_file(buffer, size, path, "wb")` (`src/sa.m:110-120`).
So the Rust module needs a byte slice and nothing else:

```rust
static OSAX_PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload"));
static OSAX_LOADER:  &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/loader"));
```

`__src_osax_payload_len` becomes `OSAX_PAYLOAD.len()`. `include_bytes!` yields alignment-1 data,
which is exactly right — `src/sa.m:115` only `fwrite`s it.

This deletes `src/sa.h:4-7`, the two `*_bin.c` files, their two `.gitignore` entries
(`.gitignore:4-5`) and the path-to-symbol-name coupling described in §1.3.

The rest of `src/sa.m` is an ordinary Rust port: `scripting_addition_install`
(`src/sa.m:202-237`) writes `sa_plist` (`src/sa.m:21-48`), `sa_bundle_plist` (`src/sa.m:50-76`)
and those two byte slices under `/Library/ScriptingAdditions/yabai.osax` (path layout built at
`src/sa.m:78-94`), then shells out to `chmod +x` and `codesign -f -s -` for each
(`src/sa.m:122-137`) — so the osax is **signed at install time on the user's machine**, never at
build time. Nothing in the build has to sign the embedded bytes.

### 6.5 What must *not* move into build.rs

`src/osax/common.h` is shared by code on both sides of the C/Rust line: the C payload uses
`OSAX_VERSION` at `src/osax/payload.m:945` and `:949`, and the Rust `sa` module will need it at
the five places `src/sa.m` uses it (`:39`, `:41`, `:68`, `:70`, `:183`, `:281`), plus
`SA_SOCKET_PATH_FMT` (`src/osax/common.h:4`, used at `src/sa.m:158`), `SA_SOCKET_BUFF_LEN` (`:5`),
the seven `OSAX_ATTRIB_*` bits (`:9-23`, used at `src/sa.m:282`) and all nineteen `enum sa_opcode`
values (`:25-46`, used throughout `src/sa.m:442-621`).

### 6.6 Generating the osax constants

**Recommendation: build.rs parses `src/osax/common.h` and writes `$OUT_DIR/osax_common.rs`, which
`src/sa.rs` pulls in with `include!`.**

The file is 48 lines, contains only `#define`s and one flat `enum` with explicit hex values, and
is the shared ABI between the Rust daemon and the C payload. Hand-copying it into Rust creates a
silent-drift hazard where a version bump in C leaves the Rust handshake check
(`src/sa.m:281`) comparing against a stale string — and the symptom is "the scripting addition
mysteriously stops working", not a compile error. A ~40-line regex pass in build.rs over
`#define (\w+)\s+(.+)` and the `enum sa_opcode` body removes that class of bug entirely.

If that is judged too clever, the fallback is a hand-written `src/osax_common.rs` **plus** a
build.rs assertion that re-reads `common.h` and `panic!`s on mismatch. Do not do it with no check
at all.

### 6.7 The universal binary

There is no `-arch x86_64 -arch arm64` for rustc. Two cargo invocations plus `lipo`:

```
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin
lipo -create -output bin/yabai \
    target/aarch64-apple-darwin/release/yabai \
    target/x86_64-apple-darwin/release/yabai
```

**[verified]** end to end on a probe crate: both slices build, `lipo -info` reports `x86_64
arm64`, `otool -arch <each> -s __TEXT __info_plist` dumps the plist from each slice, both carry
`LC_BUILD_VERSION minos 11.0`, and `codesign -f -s -` on the fat result succeeds with
`Format=Mach-O universal (x86_64 arm64)` and `Identifier=com.asmvik.yabai`.

Signing order is unchanged from `makefile:48`: build → lipo → `sign` → `icon`.

### 6.8 The rewritten makefile

Keep every user-facing target (`all`, `asan`, `tsan`, `install`, `sign`, `man`, `icon`, `archive`,
`publish`, `clean-build`, `clean`) and every meaning, including "`make` is debug, `make install`
is release, and `install` installs nothing". `man`, `icon`, `sign`, `archive`, `publish` are
copied verbatim from `makefile:38-58`. Only the compile rule and `clean` change:

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

Notes on the deltas:

* **`clean` no longer removes `*_bin.c`** (`makefile:64`) because they no longer exist; it removes
  `./target` instead. `.gitignore:4-5` can be deleted and `/target` added.
* **`CLI_FLAGS` (`makefile:3`) has no direct analogue.** The equivalent escape hatch is
  `RUSTFLAGS` / `cargo build --features`. Either drop the variable or document
  `make RUSTFLAGS=…`; do not invent a `-D`-style mechanism.
* **`asan`/`tsan` build host-only**, unlike `makefile:21-25` which pointlessly builds both arches
  for a sanitizer run you are going to execute locally. The sanitizer output is the point, not the
  fat binary. This is a deliberate, stated divergence.

### 6.9 Sanitizers

`-fsanitize=address,undefined` and `-fsanitize=thread,undefined` (`makefile:21`, `:24`) map to:

```
RUSTFLAGS="-Zsanitizer=address" cargo +nightly build -Zbuild-std --target aarch64-apple-darwin
RUSTFLAGS="-Zsanitizer=thread"  cargo +nightly build -Zbuild-std --target aarch64-apple-darwin
```

* `-Zsanitizer` is nightly-only; nightly `1.100.0` and `rust-src` are both installed here
  **[verified]**, so nothing needs fetching.
* `-Zbuild-std` is what instruments `std` itself; without it you sanitize only your own code and
  TSan in particular produces noise from uninstrumented std.
* Use the **dev** profile for sanitizer builds: `lto = "fat"` in release conflicts with sanitizer
  instrumentation, and you want the debug info anyway.
* The UBSan half of `-fsanitize=…,undefined` has **no Rust equivalent and needs none** — the
  undefined behaviours it catches in C (signed overflow, misaligned loads, bad shifts) are either
  defined in Rust or already trapped by `overflow-checks` in the dev profile.

### 6.10 Translating the tests

The two C tests become `#[test]` functions inside `src/view.rs`, in a `#[cfg(test)] mod tests`
block, because `area_is_in_direction` (`src/view.c:541`), `area_distance_in_direction`
(`src/view.c:563`) and `area_max_point` (`src/view.c:126`) are `static inline` — i.e.
crate-private in Rust terms, so an integration test in `tests/*.rs` could not reach them.

The mapping is mechanical:

| C | Rust |
|---|---|
| `TEST_FUNC(name, …)` (`tests/src/tests.m:11`) | `#[test] fn name()` |
| `TEST_CHECK(r, e)` (`tests/src/tests.m:12`) | `assert_eq!(r, e)` |
| `TEST_LIST` registration (`tests/src/tests.m:17-19`) | gone — libtest discovers `#[test]` |
| driver + ANSI output (`tests/src/tests.m:28-54`) | gone — libtest |
| `-DPROFILE=1` timing (`tests/src/tests.m:35-47`) | gone — libtest reports durations |
| `-DTESTS` to suppress `main()` (`src/yabai.c:260`) | gone — `cargo test` replaces `main` for a bin target |
| fake osax symbols (`tests/src/tests.m:1-4`) | gone — build.rs supplies the real bytes |
| `tests/makefile` | gone — `cargo test` |

So **`tests/` is deleted at the end of phase 2** and `make test` (new) or plain `cargo test` takes
over. Both existing tests are pure geometry, so they run without a window server — but note that
any *future* test that touches `SLSMainConnectionID` and friends will link fine (the framework
directives in §6.3 apply to test targets) and simply require a logged-in GUI session at run time.

Per the project's testing rule, these two cases are pre-existing rather than newly proposed, but
their Rust form should still be confirmed with the user before anyone writes them.

### 6.11 Version constants

`MAJOR`/`MINOR`/`PATCH` are `#define`d at `src/yabai.c:23-25` and used in exactly two places, both
of them `printf` format arguments:

* `src/yabai.c:200` — the docs URL in `--help`: `…/blob/v%d.%d.%d/doc/yabai.asciidoc`
* `src/yabai.c:206` — `--version`: `yabai-v%d.%d.%d`

Because both uses are pure string interpolation, **no integer is ever needed**, and the Cargo
package version can be the single source of truth:

```rust
const MAJOR: &str = env!("CARGO_PKG_VERSION_MAJOR");
const MINOR: &str = env!("CARGO_PKG_VERSION_MINOR");
const PATCH: &str = env!("CARGO_PKG_VERSION_PATCH");
// or, for --version directly:
const VERSION_BANNER: &str = concat!("yabai-v", env!("CARGO_PKG_VERSION"));
```

Set `version = "7.1.25"` in `Cargo.toml` to match `src/yabai.c:23-25`. This matters beyond
tidiness: `make archive` and `make publish` both name the release artifact from `bin/yabai
--version` (`makefile:45`, `:46`, `:54`), so `Cargo.toml` becomes the one place a release bump
happens.

`OSAX_VERSION` (`src/osax/common.h:7`, currently `"2.1.30"`) is **independent** of the daemon
version and must stay in `common.h`, because the C payload compares against it too
(`src/osax/payload.m:945`). See §6.6.

---

## 7. Summary of files created, changed and deleted

| Path | Phase 2 action |
|---|---|
| `Cargo.toml`, `build.rs`, `rust-toolchain.toml` | created at repo root |
| `src/main.rs` and one `src/<name>.rs` per C file | created incrementally, alongside the C |
| `src/misc.rs` + `src/misc/<name>.rs` | created |
| `src/osax/**` | **untouched**, still C/ObjC, still `arm64e` |
| `src/*.c`, `src/*.h`, `src/*.m`, `src/manifest.m` | deleted once `cargo build` yields a working daemon |
| `src/sa.h:4-7`, `src/osax/{payload,loader}_bin.c` | deleted (build.rs + `include_bytes!` replace them) |
| `tests/` | deleted; tests move into `#[cfg(test)]` blocks |
| `makefile` | rewritten to drive cargo, same targets (§6.8) |
| `.gitignore` | drop lines 4-5, add `/target` |
| `scripts/*`, `assets/*`, `doc/yabai.asciidoc`, `examples/*` | unchanged |
| `.github/` | unchanged (there is no CI) |

---

## 8. Hardest problems in this sweep

1. **The universal binary is no longer one command.** clang's `-arch x86_64 -arch arm64` becomes
   two cargo invocations plus `lipo`, which means build.rs runs twice, the osax is compiled twice,
   and `make` has to know the profile directory name (`debug` vs `release`) to find the two
   slices. It works and is verified, but it is the piece most likely to be got subtly wrong.
2. **The `__info_plist` section is a silent TCC cliff.** Get the `-sectcreate` link-arg wrong and
   the build still succeeds, the daemon still runs, and every user loses their Accessibility
   grant on upgrade because the code-signing identifier changed from `com.asmvik.yabai` to
   `yabai-<hash>`.
3. **`src/osax/common.h` straddles the C/Rust line.** It is the wire ABI between the Rust daemon
   and the C payload; duplicating it by hand is a drift hazard whose failure mode is a runtime
   handshake mismatch, not a compile error.
4. **`cargo build` is all-or-nothing during a file-by-file port.** The C unity build lets you
   compile after every edit; Rust will not link until the last module lands. The stub-`main.rs`-
   first, bottom-up order in §3 is the mitigation and it constrains the whole of phase 2.
5. **Behavioural drift the build introduces on its own**: dev-profile `overflow-checks` (panics
   where C wraps) and `assert!` vs `debug_assert!`. Neither is a build bug; both are decisions the
   build profile makes on the porter's behalf.

## 9. Open questions

* **`make asan` / `make tsan` scope.** Recommendation above is host-only single-arch, diverging
  from `makefile:21-25` which builds fat. Confirm that is acceptable.
* **`CLI_FLAGS` (`makefile:3`).** Drop it, or keep a `make RUSTFLAGS=…` passthrough?
* **Constants generation (§6.6).** build.rs parsing `src/osax/common.h`, or hand-written Rust with
  a build.rs equality check? The first is less code to maintain, the second is easier to read.
* **`clean`.** Should `make clean` run `cargo clean` (deletes the whole `./target`, several GB and
  a full rebuild) or only `rm -rf ./bin`? The C `clean` was cheap; the Rust one is not.
* **CI.** There is none today (§1.6). Is adding a GitHub Actions workflow (build both targets,
  `cargo test`, `cargo clippy`, `lipo`) in scope for this rewrite, or explicitly out?
* **`asciidoctor` and PyObjC are missing locally** (§4), so `make man` and `make icon` cannot be
  exercised on this machine. Do they need to keep working, or is `doc/yabai.1` now maintained as a
  checked-in artifact only?
* **`make publish`'s hardcoded `sed` line numbers** (`makefile:45-46` against
  `scripts/install.sh:60-61`) survive the rewrite untouched. Leave brittle, or fix while we are
  here?
