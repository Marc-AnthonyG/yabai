# FFI, Objective-C and the OS interfaces

Phase 1 mapping. No Rust is written into the repository here. This document elaborates
`DECISIONS.md` 7, 9, 11, 34 and 35 into a form a phase 2 translator follows mechanically:
the crate set, the `src/ffi/` module layout, every hand-written declaration with its full Rust
signature, the ABI traps, the Objective-C reconstruction, the callbacks, the
scripting-addition wire client and the osax build pipeline.

It never presents a choice. Where an inventory offered two options, one is picked here and the
other is not mentioned again.

Rust sketches obey `DECISIONS.md` 37-39: C names are kept, nothing is abbreviated, and the only
comments that appear are comments that exist in the C source.

---

## 0. What this document settles

| Question | Answer, in one line |
| --- | --- |
| Which crates, which versions, which features | §1 |
| Which framework exports which symbol | §2.2, and each module section |
| Where every `extern.h` declaration goes | §3 |
| How the two runtime-resolved pointers work | §4, §5 |
| Where the `kAX*` strings come from | §8 |
| How `ProcessSerialNumber` is declared | §9 |
| Carbon Event Manager, with no crate to lean on | §10 |
| Mach messages, `NDR_record`, the two packed structs | §15 |
| Which `extern "C"` functions Rust hands to C, and what happens if one panics | §19 |
| How `workspace_context` is rebuilt | §20 |
| The four `@try/@catch` sites | §22 |
| The dynamic `objc_msgSend` call | §24 |
| The scripting-addition frame format and every RPC | §25 |
| `build.rs`, `include_bytes!`, the osax | §26 |

---

## 1. Dependencies

### 1.1 The `[dependencies]` table

Verified against crates.io and the published crate sources on 2026-09-19. Every version below is
the current `max_stable_version`; every feature name below was read out of that version's own
`Cargo.toml`, not guessed. `default-features = false` everywhere, because the framework crates
default to *every* header they bind and the daemon needs a small fraction of that.

```toml
[dependencies]
objc2 = { version = "0.6.4", default-features = false, features = [
    "std",
    "exception",
] }

objc2-foundation = { version = "0.3.2", default-features = false, features = [
    "std",
    "NSArray",
    "NSBundle",
    "NSDictionary",
    "NSDistributedNotificationCenter",
    "NSEnumerator",
    "NSGeometry",
    "NSKeyValueCoding",
    "NSKeyValueObserving",
    "NSNotification",
    "NSObjCRuntime",
    "NSObject",
    "NSPathUtilities",
    "NSProcessInfo",
    "NSString",
    "NSUserNotification",
    "NSValue",
    "objc2-core-foundation",
] }

objc2-app-kit = { version = "0.3.2", default-features = false, features = [
    "std",
    "NSApplication",
    "NSGraphics",
    "NSImage",
    "NSResponder",
    "NSRunningApplication",
    "NSScreen",
    "NSWorkspace",
    "libc",
    "objc2-core-foundation",
    "objc2-core-graphics",
] }

objc2-core-foundation = { version = "0.3.2", default-features = false, features = [
    "std",
    "CFArray",
    "CFCGTypes",
    "CFData",
    "CFDate",
    "CFDictionary",
    "CFMachPort",
    "CFNumber",
    "CFRunLoop",
    "CFString",
    "CFUUID",
    "objc2",
] }

objc2-core-graphics = { version = "0.3.2", default-features = false, features = [
    "std",
    "CGAffineTransform",
    "CGBase",
    "CGBitmapContext",
    "CGColorSpace",
    "CGContext",
    "CGDirectDisplay",
    "CGDisplayConfiguration",
    "CGError",
    "CGEvent",
    "CGEventSource",
    "CGEventTypes",
    "CGGeometry",
    "CGImage",
    "CGPath",
    "CGRemoteOperation",
    "CGWindow",
    "CGWindowLevel",
    "libc",
    "objc2",
] }

objc2-application-services = { version = "0.3.2", default-features = false, features = [
    "std",
    "AXError",
    "AXUIElement",
    "AXValue",
    "HIServices",
    "libc",
    "objc2",
] }

objc2-core-video = { version = "0.3.2", default-features = false, features = [
    "std",
    "CVBase",
    "CVDisplayLink",
    "CVHostTime",
    "CVReturn",
    "objc2-core-graphics",
] }

block2 = { version = "0.6.2", default-features = false, features = ["std"] }

dispatch2 = { version = "0.3.1", default-features = false, features = [
    "std",
    "block2",
] }

libc = { version = "0.2.189", default-features = false, features = ["std"] }
```

No `[build-dependencies]`. `build.rs` shells out to `xcrun clang` with
`std::process::Command` and parses `src/osax/common.h` with `std::fs` plus string scanning;
`DECISIONS.md` 11 bars `bindgen`, and nothing else is needed.

### 1.2 Why each feature is on

| Crate / feature | The item it is for |
| --- | --- |
| `objc2/exception` | `objc2::exception::catch`, which is `#[cfg(feature = "exception")]` in `objc2-0.6.4/src/exception.rs:329`. Without it the four `@try/@catch` sites (§22) have no translation. It pulls `objc2-exception-helper`, which compiles one `.m` shim. |
| `objc2-foundation/NSKeyValueObserving` | `addObserver:forKeyPath:options:context:`, `removeObserver:forKeyPath:context:`, `NSKeyValueObservingOptions`, `NSKeyValueChangeNewKey` (`workspace.m:73,83,57,61,94,98,215,227,240,252`) |
| `objc2-foundation/NSKeyValueCoding` | `setValue:forKey:` for the two private `NSUserNotification` keys (`notify.h:41-42`) |
| `objc2-foundation/NSGeometry` | `NSEdgeInsets`, returned by value from `-safeAreaInsets` (`workspace.m:132`) |
| `objc2-foundation/NSPathUtilities` | `NSHomeDirectoryForUser` (`service.h:82`) and `-stringByResolvingSymlinksInPath` (`notify.h:23`) |
| `objc2-foundation/NSUserNotification` | `NSUserNotification`, `NSUserNotificationCenter`, `NSUserNotificationCenterDelegate` (`notify.h:10-43`) |
| `objc2-foundation/NSValue` | `NSNumber`, for `-intValue` on the KVO change value and `@(false)` |
| `objc2-app-kit/NSWorkspace` | `NSWorkspace`, `-notificationCenter`, and the five notification-name statics |
| `objc2-app-kit/NSResponder` | the `NSApplication` **class** itself. `objc2-app-kit-0.3.2/src/generated/NSApplication.rs:445-451` puts `pub struct NSApplication` behind `#[cfg(feature = "NSResponder")]`, because `NSResponder` is its superclass. Without it only the free function `NSApplicationLoad` exists and `NSApplication::sharedApplication(…).run()` — the `[NSApp run]` of `yabai.c:350` — cannot be named |
| `objc2-app-kit/NSGraphics` | `-deviceDescription` on `NSScreen`, which is `#[cfg(feature = "NSGraphics")]` in `NSScreen.rs:60-65`. `workspace.m:131` reads `NSScreenNumber` out of it |
| `objc2-app-kit/libc` | `runningApplicationWithProcessIdentifier:` takes a `libc::pid_t` and is `#[cfg(feature = "libc")]` in `NSRunningApplication.rs:249-254` (`workspace.m:30`) |
| `objc2-core-graphics/CGRemoteOperation` | `CGWarpMouseCursorPosition`, `CGSetLocalEventsSuppressionInterval`, `CGEnableEventStateCombining` |
| `objc2-core-graphics/CGWindowLevel` | `CGWindowLevelForKey` and `CGWindowLevelKey` (`yabai.c:145-147`) |
| `objc2-core-graphics/libc` | `CGEventTapCreate`'s neighbours and `CGEventTapInformation` are `#[cfg(feature = "libc")]` |
| `objc2-application-services/HIServices` | the umbrella feature that makes every `AX*` sub-feature reachable at all. `objc2-application-services-0.3.2/src/generated/mod.rs:23-33` gates the entire tree on it — `#[cfg(feature = "HIServices")] mod __HIServices;` and `#[cfg(feature = "HIServices")] pub use self::__HIServices::*;` — and `AXError`, `AXUIElement` and `AXValue` only gate items *inside* that module. Without `HIServices` the crate exports no AX item whatsoever and every name §8 re-exports is compiled out |
| `objc2-application-services/AXUIElement` | every `AXUIElement*` / `AXObserver*` function, `AXObserverCallback`, `kAXTrustedCheckOptionPrompt` |
| `objc2-application-services/AXValue` | `AXValueCreate`, `AXValueGetValue`, `AXValueType` with `AXValueType::CGPoint` / `::CGSize` |
| `objc2-application-services/libc` | `AXObserverCreate` takes `libc::pid_t` and is `#[cfg(feature = "libc")]` |
| `objc2-core-video/CVBase` | `CVTimeStamp`, whose `hostTime` the animation callback reads |
| `objc2-core-video/CVHostTime` | `CVGetHostClockFrequency` (`yabai.c:144`) |
| `dispatch2/block2` | `DispatchQueue::exec_after_with_block`, which is `#[cfg(feature = "block2")]` |

### 1.3 Three corrections to `sweeps/ffi-surface.md`, verified against the crate sources

1. **The Process Manager functions in `objc2-application-services` cannot be called.**
   `GetProcessPID`, `GetNextProcess`, `CopyProcessName`, `SameProcess` and `IsProcessVisible`
   all take `*const ProcessSerialNumber`, and that type lives in a *private* module:
   `objc2-application-services-0.3.2/src/lib.rs:25` declares `mod mac_types` without `pub`, and
   `:96` re-exports it with `pub(crate) use`. The struct's two fields are private as well, and
   the comment on it reads `// Intentionally don't make this truly public`. A downstream crate
   can neither name the type nor build a value of it, so the whole Process Manager group is
   hand-declared in `src/ffi/carbon_process.rs` (§9). `kNoProcess` is equally unreachable and is
   hand-written as a constant.
2. **`CGPostMouseEvent` is genuinely absent** from `objc2-core-graphics` 0.3.2 — it is the only
   `CGRemoteOperation.h` symbol yabai uses that the crate skips. `CGWarpMouseCursorPosition`,
   `CGSetLocalEventsSuppressionInterval` and `CGEnableEventStateCombining` *are* bound.
3. **`objc2-color-sync` is not a dependency** (`DECISIONS.md` 11). The two display-UUID
   functions are hand-declared against ColorSync in `src/ffi/color_sync.rs` (§7).

### 1.4 The seven dead `extern.h` declarations are not translated

`DECISIONS.md` 5 removes declarations with no caller. These have none in the daemon:
`SLSNewWindow` (`extern.h:25`), `SLSSetWindowTags` (`:28`), `SLSClearWindowTags` (`:29`),
`SLSSetWindowBackgroundBlurRadiusStyle` (`:32`), `SLSMoveWindow` (`:65`),
`SLSSetWindowTransform` (`:87`), `SLSTransactionOrderWindow` (`:91`).
They are dropped, and each is one line in `DEVIATIONS.md`. Every signature below therefore has
at least one live call site.

---

## 2. The `src/ffi/` module layout

### 2.1 Files

```
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
```

One module per *linked framework or library*, not per C header. `src/ffi.rs` sits beside the
`src/ffi/` directory rather than inside it — `TRANSLATION_PLAN.md` §1 fixes the 2018 module form
for the whole crate, and a crate carrying both spellings does not compile. `src/ffi.rs` declares
the sixteen modules and nothing else: no re-export surface, no prelude. Callers write
`use crate::ffi::skylight::SLSGetWindowBounds;`, so a reader of any Rust file can see which
framework a call reaches.

Nothing under `src/ffi/` holds mutable state except `skylight_dynamic`, which holds the two
`OnceLock`s of `DECISIONS.md` 18.

### 2.2 Which framework exports what, and the link attribute each module carries

| Module | Link attribute | Symbols |
| --- | --- | --- |
| `skylight` | `#[link(name = "SkyLight", kind = "framework")]` | every `SLS*`, `SLPS*`, `_SLPS*`, `SLWindowContextCreate` |
| `skylight_dynamic` | none — resolved at run time by `macho` | `CGSGetConnectionPortById`, `SLSPerformAsynchronousBridgedWindowManagementOperation` |
| `core_graphics` | `#[link(name = "CoreGraphics", kind = "framework")]` for the three strays; the rest re-exported from `objc2-core-graphics` | `CGPostMouseEvent`, `CGRegionCreateEmptyRegion`, `CGSNewRegionWithRect` |
| `color_sync` | `#[link(name = "ColorSync", kind = "framework")]` | `CGDisplayCreateUUIDFromDisplayID`, `CGDisplayGetDisplayIDFromUUID` |
| `accessibility` | `#[link(name = "ApplicationServices", kind = "framework")]` for the two private ones | `_AXUIElementGetWindow`, `_AXUIElementCreateWithRemoteToken`; the `kAX*` statics; re-exports |
| `carbon_process` | `#[link(name = "ApplicationServices", kind = "framework")]` | `GetProcessPID`, `GetProcessInformation`, `CopyProcessName`, `GetNextProcess`, `SameProcess`, `IsProcessVisible`, `CoreDockGetAutoHideEnabled`, `CoreDockGetOrientationAndPinning`, `CoreDockSendNotification` |
| `carbon_events` | `#[link(name = "Carbon", kind = "framework")]` | the Carbon Event Manager |
| `carbon_core` | `#[link(name = "CoreServices", kind = "framework")]` | `AbsoluteToNanoseconds` |
| `core_video` | none — re-exports from `objc2-core-video` | `CVDisplayLink*`, `CVGetHostClockFrequency` |
| `core_foundation` | none — re-exports from `objc2-core-foundation` | the CF glue helpers |
| `appkit` / `foundation` | none — re-exports from the two objc2 crates | AppKit / Foundation surface |
| `mach_port` | none — libSystem is always linked | `mach_msg`, `mig_get_special_reply_port`, `bootstrap_look_up`, `task_get_special_port`, `NDR_record` |
| `macho` | none | the symbol-table walker |
| `libsystem` | none | `csr_get_active_config`, `proc_name`, `sysctl`, `_NSGetExecutablePath` |
| `dispatch` | none | one wrapper over `dispatch2` |

`build.rs` emits the search path that makes the SkyLight stub resolvable:

```rust
println!("cargo:rustc-link-search=framework=/System/Library/PrivateFrameworks");
```

The linker re-roots that absolute path through `-syslibroot`, so it finds
`.../MacOSX.sdk/System/Library/PrivateFrameworks/SkyLight.framework/Versions/A/SkyLight.tbd`
exactly as `makefile:1-2` does. Duplicate `-framework` flags contributed by the objc2 crates are
harmless.

### 2.3 The two calling-convention rules

1. **Every function Rust imports is `unsafe extern "C"`.** None of SkyLight, CoreGraphics,
   ColorSync, Accessibility, Carbon, Mach or libc can raise an Objective-C exception, so no
   import needs `"C-unwind"`.
2. **Every function Rust exports to C is `unsafe extern "C-unwind"`.** Four of the seven are
   forced to be: `CGEventTapCallBack`, `CVDisplayLinkOutputCallback`, `AXObserverCallback` and
   `CGDisplayReconfigurationCallBack` are declared `unsafe extern "C-unwind" fn` by the objc2
   crates, and a plain `extern "C" fn` will not coerce to them. The remaining three are written
   the same way so all seven read alike. See §19 for what happens if one panics.

Edition 2024 requires the `unsafe extern` block form:

```rust
#[link(name = "SkyLight", kind = "framework")]
unsafe extern "C" {
    pub fn SLSMainConnectionID() -> c_int;
}
```

---

## 3. `src/ffi/skylight.rs` — the complete declaration list

Every live declaration from `src/misc/extern.h`, in header order, with the Rust signature the
translator writes. Types come from the crates named in §1; nothing in this file redefines
`CGRect`, `CGPoint`, `CGSize`, `CGFloat` or `CGAffineTransform`.

```rust
use core::ffi::{c_int, c_void};
use libc::pid_t;
use objc2_core_foundation::{CGAffineTransform, CGPoint, CGRect, CFArray, CFDictionary, CFString, CFType};
use objc2_core_graphics::{CGContext, CGError};
use crate::ffi::carbon_process::ProcessSerialNumber;
```

Pointer conventions, applied without exception:

* a CoreFoundation parameter is `*const T`; a `Copy`/`Create` return is `*mut T`, wrapped at the
  call site with `CFRetained::from_raw(NonNull::new(raw)?)`;
* an out-parameter is `*mut T`;
* `CGError` and `OSStatus` are returned as-is and compared against `kCGErrorSuccess` / `0`
  exactly where the C did;
* `CFTypeRef` is `*const CFType` in, `*mut CFType` out.

### 3.1 The callback type from `extern.h:1-2`

```rust
pub type ConnectionCallback = unsafe extern "C-unwind" fn(
    r#type: u32,
    data: *mut c_void,
    data_length: usize,
    context: *mut c_void,
    cid: c_int,
);
```

`SLSRegisterConnectionNotifyProc` takes it as a bare function pointer, not an `Option`, because
`yabai.c:322-333` always passes a real function.

### 3.2 Connection lifetime and notifications

```rust
#[link(name = "SkyLight", kind = "framework")]
unsafe extern "C" {
    pub fn SLSMainConnectionID() -> c_int;
    pub fn SLSNewConnection(zero: c_int, cid: *mut c_int) -> CGError;
    pub fn SLSReleaseConnection(cid: c_int) -> CGError;
    pub fn SLSRegisterConnectionNotifyProc(
        cid: c_int,
        handler: ConnectionCallback,
        event: u32,
        context: *mut c_void,
    ) -> CGError;
}
```

`extern.h:9-12`. Callers: `yabai.c:143,275`, `window_manager.c:607,591`, `yabai.c:322-333`.

### 3.3 Window geometry, level and appearance

```rust
    pub fn SLSGetWindowBounds(cid: c_int, wid: u32, frame: *mut CGRect) -> CGError;
    pub fn SLSGetWindowLevel(cid: c_int, wid: u32, level: *mut c_int) -> CGError;
    pub fn SLSGetWindowSubLevel(cid: c_int, wid: u32) -> c_int;
    pub fn SLSGetWindowAlpha(cid: c_int, wid: u32, alpha: *mut f32) -> CGError;
    pub fn SLSSetWindowAlpha(cid: c_int, wid: u32, alpha: f32) -> CGError;
    pub fn SLSSetWindowResolution(cid: c_int, wid: u32, resolution: f64) -> CGError;
    pub fn SLSCopyWindowProperty(
        cid: c_int,
        wid: u32,
        property: *const CFString,
        value: *mut *mut CFType,
    ) -> CGError;
    pub fn SLSNewWindowWithOpaqueShapeAndContext(
        cid: c_int,
        r#type: c_int,
        region: *const CFType,
        opaque_shape: *const CFType,
        options: c_int,
        tags: *mut u64,
        x: f32,
        y: f32,
        tag_size: c_int,
        wid: *mut u32,
        context: *mut c_void,
    ) -> CGError;
    pub fn SLSReleaseWindow(cid: c_int, wid: u32) -> CGError;
    pub fn SLSSetWindowShape(
        cid: c_int,
        wid: u32,
        x_offset: f32,
        y_offset: f32,
        shape: *const CFType,
    ) -> CGError;
    pub fn SLSSetWindowOpacity(cid: c_int, wid: u32, opaque: bool) -> CGError;
    pub fn SLSOrderWindow(cid: c_int, wid: u32, mode: c_int, rel_wid: u32) -> CGError;
    pub fn SLSWindowIsOrderedIn(cid: c_int, wid: u32, value: *mut u8) -> CGError;
    pub fn SLSSetWindowLevel(cid: c_int, wid: u32, level: c_int) -> CGError;
    pub fn SLSSetWindowSubLevel(cid: c_int, wid: u32, sub_level: c_int) -> CGError;
    pub fn SLWindowContextCreate(
        cid: c_int,
        wid: u32,
        options: *const CFDictionary,
    ) -> *mut CGContext;
    pub fn SLSWindowSetShadowProperties(wid: u32, options: *const CFDictionary) -> CGError;
    pub fn SLSRequestNotificationsForWindows(
        cid: c_int,
        window_list: *mut u32,
        window_count: c_int,
    ) -> CGError;
    pub fn SLSHWCaptureWindowList(
        cid: c_int,
        window_list: *mut u32,
        window_count: c_int,
        options: u32,
    ) -> *mut CFArray;
    pub fn SLSDisableUpdate(cid: c_int) -> CGError;
    pub fn SLSReenableUpdate(cid: c_int) -> CGError;
```

`extern.h:13-19,26,27,30,31,33-37,85,86,95,23,24`. Note `SLSGetWindowSubLevel` returns a plain
`c_int`, not a `CGError`, and is reached only from the fallback branch of `window_sub_level`
(`window.c:959`) — see §18.7.

`SLSSetWindowOpacity`'s `opaque` is a C `bool`, one byte, so it maps to Rust `bool` (§18.4).

### 3.4 Transactions — the animation thread only

```rust
    pub fn SLSTransactionCreate(cid: c_int) -> *mut CFType;
    pub fn SLSTransactionCommit(transaction: *const CFType, synchronous: c_int) -> CGError;
    pub fn SLSTransactionSetWindowTransform(
        transaction: *const CFType,
        wid: u32,
        unknown: c_int,
        unknown2: c_int,
        t: CGAffineTransform,
    ) -> CGError;
    pub fn SLSTransactionOrderWindowGroup(
        transaction: *const CFType,
        wid: u32,
        order: c_int,
        rel_wid: u32,
    ) -> CGError;
    pub fn SLSTransactionSetWindowAlpha(
        transaction: *const CFType,
        wid: u32,
        alpha: f32,
    ) -> CGError;
    pub fn SLSTransactionSetWindowSystemAlpha(
        transaction: *const CFType,
        wid: u32,
        alpha: f32,
    ) -> CGError;
```

`extern.h:88-90,92-94`. Called from `window_manager.c:556-573` and `:656-659`. The parameter
names `unknown` and `unknown2` are the C's own.

### 3.5 The window query iterator

```rust
    pub fn SLSWindowQueryWindows(
        cid: c_int,
        windows: *const CFArray,
        count: c_int,
    ) -> *mut CFType;
    pub fn SLSWindowQueryResultCopyWindows(window_query: *const CFType) -> *mut CFType;
    pub fn SLSWindowIteratorGetCount(iterator: *const CFType) -> c_int;
    pub fn SLSWindowIteratorAdvance(iterator: *const CFType) -> bool;
    pub fn SLSWindowIteratorGetParentID(iterator: *const CFType) -> u32;
    pub fn SLSWindowIteratorGetWindowID(iterator: *const CFType) -> u32;
    pub fn SLSWindowIteratorGetTags(iterator: *const CFType) -> u64;
    pub fn SLSWindowIteratorGetAttributes(iterator: *const CFType) -> u64;
    pub fn SLSWindowIteratorGetLevel(iterator: *const CFType) -> c_int;
    pub fn SLSCopyWindowsWithOptionsAndTags(
        cid: c_int,
        owner: u32,
        spaces: *const CFArray,
        options: u32,
        set_tags: *mut u64,
        clear_tags: *mut u64,
    ) -> *mut CFArray;
    pub fn SLSCopyAssociatedWindows(cid: c_int, wid: u32) -> *mut CFArray;
    pub fn SLSCopySpacesForWindows(
        cid: c_int,
        selector: c_int,
        window_list: *const CFArray,
    ) -> *mut CFArray;
```

`extern.h:67-75,58,66,22`.

### 3.6 Displays, menu bar, dock

```rust
    pub fn SLSCopyManagedDisplays(cid: c_int) -> *mut CFArray;
    pub fn SLSCopyManagedDisplaySpaces(cid: c_int) -> *mut CFArray;
    pub fn SLSCopyManagedDisplayForWindow(cid: c_int, wid: u32) -> *mut CFString;
    pub fn SLSCopyManagedDisplayForSpace(cid: c_int, sid: u64) -> *mut CFString;
    pub fn SLSCopyBestManagedDisplayForRect(cid: c_int, rect: CGRect) -> *mut CFString;
    pub fn SLSCopyBestManagedDisplayForPoint(cid: c_int, point: CGPoint) -> *mut CFString;
    pub fn SLSManagedDisplayGetCurrentSpace(cid: c_int, uuid: *const CFString) -> u64;
    pub fn SLSManagedDisplayIsAnimating(cid: c_int, uuid: *const CFString) -> bool;
    pub fn SLSCopyActiveMenuBarDisplayIdentifier(cid: c_int) -> *mut CFString;
    pub fn SLSSetActiveMenuBarDisplayIdentifier(
        cid: c_int,
        uuid: *const CFString,
        repeat_uuid: *const CFString,
    ) -> CGError;
    pub fn SLSSetMenuBarInsetAndAlpha(
        cid: c_int,
        unused1: f64,
        unused2: f64,
        alpha: f32,
    ) -> CGError;
    pub fn SLSGetMenuBarAutohideEnabled(cid: c_int, enabled: *mut c_int) -> CGError;
    pub fn SLSGetRevealedMenuBarBounds(rect: *mut CGRect, cid: c_int, sid: u64) -> CGError;
    pub fn SLSGetDisplayMenubarHeight(did: u32, height: *mut u32) -> CGError;
    pub fn SLSGetDockRectWithReason(
        cid: c_int,
        rect: *mut CGRect,
        reason: *mut c_int,
    ) -> CGError;
    pub fn SLSGetCurrentCursorLocation(cid: c_int, point: *mut CGPoint) -> CGError;
    pub fn SLSFindWindowAndOwner(
        cid: c_int,
        zero: c_int,
        one: c_int,
        zero_again: c_int,
        screen_point: *mut CGPoint,
        window_point: *mut CGPoint,
        wid: *mut u32,
        wcid: *mut c_int,
    ) -> i32;
```

`extern.h:41,60,20,54,21,45,42,46,43,44,47-51,84,83`. `SLSGetRevealedMenuBarBounds` takes the
rect *first* and the connection second — a real asymmetry in the private header, not a typo.
`SLSFindWindowAndOwner` returns `OSStatus`, which is `i32` here (§10.1).

### 3.7 Spaces

```rust
    pub fn SLSSpaceSetFrontPSN(cid: c_int, sid: u64, psn: ProcessSerialNumber) -> CGError;
    pub fn SLSSpaceGetType(cid: c_int, sid: u64) -> c_int;
    pub fn SLSSpaceCopyName(cid: c_int, sid: u64) -> *mut CFString;
    pub fn SLSGetSpaceManagementMode(cid: c_int) -> c_int;
    pub fn SLSProcessAssignToSpace(cid: c_int, pid: pid_t, sid: u64) -> CGError;
    pub fn SLSProcessAssignToAllSpaces(cid: c_int, pid: pid_t) -> CGError;
    pub fn SLSMoveWindowsToManagedSpace(cid: c_int, window_list: *const CFArray, sid: u64);
    pub fn SLSSpaceSetCompatID(cid: c_int, sid: u64, workspace: c_int) -> CGError;
    pub fn SLSSetWindowListWorkspace(
        cid: c_int,
        window_list: *mut u32,
        window_count: c_int,
        workspace: c_int,
    ) -> CGError;
```

`extern.h:55-57,59,61-63,96,97`. `SLSSpaceSetFrontPSN` takes the `ProcessSerialNumber`
**by value** — the only SkyLight entry point that does (§18.3).

### 3.8 Processes, front process and the synthesised event records

```rust
    pub fn _SLPSGetFrontProcess(psn: *mut ProcessSerialNumber) -> i32;
    pub fn SLSGetWindowOwner(cid: c_int, wid: u32, wcid: *mut c_int) -> CGError;
    pub fn SLSGetConnectionPSN(cid: c_int, psn: *mut ProcessSerialNumber) -> CGError;
    pub fn SLSConnectionGetPID(cid: c_int, pid: *mut pid_t) -> CGError;
    pub fn SLSGetConnectionIDForPSN(
        cid: c_int,
        psn: *mut ProcessSerialNumber,
        psn_cid: *mut c_int,
    ) -> CGError;
    pub fn _SLPSSetFrontProcessWithOptions(
        psn: *mut ProcessSerialNumber,
        wid: u32,
        mode: u32,
    ) -> CGError;
    pub fn SLPSPostEventRecordTo(psn: *mut ProcessSerialNumber, bytes: *mut u8) -> CGError;
}
```

`extern.h:76-82`.

### 3.9 The three `kCPS*` constants

From `window_manager.h:4-6`, kept in this module because they are `_SLPSSetFrontProcessWithOptions`
arguments and nothing else:

```rust
pub const kCPSAllWindows: u32 = 0x100;
pub const kCPSUserGenerated: u32 = 0x200;
pub const kCPSNoWindows: u32 = 0x400;
```

---

## 4. `src/ffi/skylight_dynamic.rs` — the two runtime-resolved pointers

`extern.h:4-5` declares these as *static function pointers*, not `extern`, because neither is
exported: `_CGSGetConnectionPortById` is a local symbol and
`SLSPerformAsynchronousBridgedWindowManagementOperation` is a local C++ symbol whose mangled name
is spelled out in full at `yabai.c:149`. `dlsym` cannot see either, which is why yabai walks
`LC_SYMTAB` by hand (§5).

Both are `OnceLock` statics under `DECISIONS.md` 18: written once on the main thread at
`yabai.c:148-149`, read afterwards from the event-loop thread and the animation threads. The
`Option` is load-bearing — `window.c:956`, `space_manager.c:667` and `:688` branch on whether the
symbol was found.

```rust
use core::ffi::{c_int, c_void};
use std::sync::OnceLock;
use libc::mach_port_t;
use crate::ffi::macho::macho_find_symbol;

pub type CGSGetConnectionPortByIdFn = unsafe extern "C" fn(c_int) -> mach_port_t;
pub type SLSPerformAsynchronousBridgedWindowManagementOperationFn =
    unsafe extern "C" fn(*mut c_void) -> i64;

const SKYLIGHT_IMAGE_PATH: &str =
    "/System/Library/PrivateFrameworks/SkyLight.framework/Versions/A/SkyLight";
const CGS_GET_CONNECTION_PORT_BY_ID_SYMBOL: &str = "_CGSGetConnectionPortById";
const SLS_PERFORM_ASYNCHRONOUS_BRIDGED_WINDOW_MANAGEMENT_OPERATION_SYMBOL: &str =
    "__ZL54SLSPerformAsynchronousBridgedWindowManagementOperationP47SLSAsynchronousBridgedWindowManagementOperation";

static CGSGetConnectionPortById: OnceLock<Option<CGSGetConnectionPortByIdFn>> = OnceLock::new();
static SLSPerformAsynchronousBridgedWindowManagementOperation:
    OnceLock<Option<SLSPerformAsynchronousBridgedWindowManagementOperationFn>> = OnceLock::new();

pub fn resolve_dynamic_skylight_symbols() {
    let connection_port_address =
        macho_find_symbol(SKYLIGHT_IMAGE_PATH, CGS_GET_CONNECTION_PORT_BY_ID_SYMBOL);
    let bridged_operation_address = macho_find_symbol(
        SKYLIGHT_IMAGE_PATH,
        SLS_PERFORM_ASYNCHRONOUS_BRIDGED_WINDOW_MANAGEMENT_OPERATION_SYMBOL,
    );

    let _ = CGSGetConnectionPortById.set(
        connection_port_address.map(|address| unsafe {
            core::mem::transmute::<*mut c_void, CGSGetConnectionPortByIdFn>(address)
        }),
    );
    let _ = SLSPerformAsynchronousBridgedWindowManagementOperation.set(
        bridged_operation_address.map(|address| unsafe {
            core::mem::transmute::<
                *mut c_void,
                SLSPerformAsynchronousBridgedWindowManagementOperationFn,
            >(address)
        }),
    );
}

pub fn cgs_get_connection_port_by_id() -> Option<CGSGetConnectionPortByIdFn> {
    *CGSGetConnectionPortById.get().unwrap_or(&None)
}

pub fn sls_perform_asynchronous_bridged_window_management_operation()
-> Option<SLSPerformAsynchronousBridgedWindowManagementOperationFn> {
    *SLSPerformAsynchronousBridgedWindowManagementOperation.get().unwrap_or(&None)
}
```

`resolve_dynamic_skylight_symbols` is called from the Rust transposition of
`configure_settings_and_acquire_lock` (`yabai.c:148-149`), at the same point in the start-up
order. The `unwrap_or(&None)` branch is reached only if some future caller runs before
start-up; it returns `None`, which is the same thing a null C pointer would have meant.

---

## 5. `src/ffi/macho.rs` — the symbol-table walker

A one-for-one transposition of `src/misc/macho_dlsym.h`, three private finders plus
`macho_find_symbol`. `libc` supplies `_dyld_image_count`, `_dyld_get_image_name`,
`_dyld_get_image_header`, `_dyld_get_image_vmaddr_slide`, `mach_header_64`,
`segment_command_64`, `load_command` and `LC_SEGMENT_64`. Four things are missing from `libc`
0.2.189 on both Apple targets and are declared here (`DECISIONS.md` 11 names the Mach-O
symbol-table types as hand-declared):

```rust
#[repr(C)]
pub struct symtab_command {
    pub cmd: u32,
    pub cmdsize: u32,
    pub symoff: u32,
    pub nsyms: u32,
    pub stroff: u32,
    pub strsize: u32,
}

#[repr(C)]
pub struct nlist_64 {
    pub n_strx: u32,
    pub n_type: u8,
    pub n_sect: u8,
    pub n_desc: u16,
    pub n_value: u64,
}

pub const LC_SYMTAB: u32 = 0x2;
pub const SEG_LINKEDIT: &[u8] = b"__LINKEDIT";
```

`struct nlist_64`'s first field is a union `n_un` in C with a single member `n_strx`
(`macho_dlsym.h:73` reads `list->n_un.n_strx`); a plain `u32` is the same layout.

The segment-name comparison at `macho_dlsym.h:27` goes through `strcmp` on a
`[c_char; 16]` that is zero-padded for `__LINKEDIT`. The Rust reads the first ten bytes and
requires the eleventh to be `0`, which is what `strcmp` decides here and is bounded:

```rust
fn segment_name_is_linkedit(segname: &[core::ffi::c_char; 16]) -> bool {
    let bytes: &[u8; 16] = unsafe { &*(segname as *const _ as *const [u8; 16]) };
    &bytes[..SEG_LINKEDIT.len()] == SEG_LINKEDIT && bytes[SEG_LINKEDIT.len()] == 0
}
```

`macho_find_symbol` keeps the C's arithmetic, including the
`(vmaddr - fileoff) + stroff + slide` base computation at `macho_dlsym.h:68-69`:

```rust
pub fn macho_find_symbol(target_image: &str, target_symbol: &str) -> Option<*mut c_void> {
```

It returns `Option<*mut c_void>` instead of a nullable pointer (`DECISIONS.md` 32). The image
name comparison uses `CStr::from_ptr(_dyld_get_image_name(index)).to_bytes() == target_image.as_bytes()`,
which is `string_equals` with the NULL check hoisted into the `Option`.

This whole module is `unsafe` pointer arithmetic, permitted by `DECISIONS.md` 39.

---

## 6. `src/ffi/core_graphics.rs`

Two halves.

**Re-exports.** Everything the daemon uses from `objc2-core-graphics`, so no other Rust file
imports that crate directly:
`CGMainDisplayID`, `CGGetActiveDisplayList`, `CGDisplayBounds`, `CGDisplayIsBuiltin`,
`CGDisplayRegisterReconfigurationCallback`, `CGDisplayChangeSummaryFlags` and the four flags
(`kCGDisplayAddFlag`, `kCGDisplayRemoveFlag`, `kCGDisplayMovedFlag`,
`kCGDisplayDesktopShapeChangedFlag`), `CGDisplayReconfigurationCallBack`,
`CGWindowLevelForKey` with `CGWindowLevelKey` and its three keys used at `misc/macros.h:43-45`,
`CGWindowListCopyWindowInfo` with `kCGWindowListOptionOnScreenOnly`, `kCGWindowName`,
`kCGWindowOwnerName`, `kCGWindowLayer`,
`CGEventTapCreate`, `CGEventTapEnable`, `CGEventTapIsEnabled`, `CGEventTapPostEvent`,
`CGEventTapCallBack`, `CGEventTapProxy`, `CGEventType`, `CGEventFlags`, `CGEventField`,
`kCGHIDEventTap`, `kCGHeadInsertEventTap`, `kCGEventTapOptionDefault`, `kCGSessionEventTap`,
`CGEventCreate`, `CGEventPost`, `CGEventSetIntegerValueField`, `CGEventSetDoubleValueField`,
`CGEventGetLocation`, `CGEventGetFlags`, `CGEventGetIntegerValueField`,
`CGWarpMouseCursorPosition`, `CGSetLocalEventsSuppressionInterval`, `CGEnableEventStateCombining`,
`CGPreflightScreenCaptureAccess`, `CGRequestScreenCaptureAccess`,
`CGBitmapContextCreate`, `CGBitmapContextCreateImage`, `CGColorSpaceCreateDeviceRGB`,
`CGContextDrawImage`, `CGImageGetWidth`, `CGImageGetHeight`,
the `CGContext` drawing calls `view.c` makes, and `CGPathCreateWithRoundedRect`.

`CGColorSpaceRelease`, `CGContextRelease` and `CGPathRelease` have no re-export: those objects
arrive as `CFRetained<T>` and are released by `Drop` at the C's release point.

**Hand-written.** Three symbols CoreGraphics still exports that `objc2-core-graphics` 0.3.2 does
not bind:

```rust
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    pub fn CGPostMouseEvent(
        position: CGPoint,
        update_mouse_cursor_position: bool,
        button_count: u32,
        mouse_button_down: bool,
        ...
    ) -> CGError;
    pub fn CGRegionCreateEmptyRegion() -> *mut CFType;
    pub fn CGSNewRegionWithRect(rect: *mut CGRect, region: *mut *mut CFType) -> CGError;
}
```

`CGPostMouseEvent` is variadic in C (`CGRemoteOperation.h`), and both call sites
(`display_manager.c:476-477`, `space_manager.c:976-977`) pass exactly one button state, so the
declaration keeps the `...` and the calls pass four arguments. This is the one variadic import in
the whole daemon. `CGRegionCreateEmptyRegion` and `CGSNewRegionWithRect` are `extern.h:38-39`.

**The private event-field numbers.** yabai spells them inline as comments
(`space_manager.c:957-969`, `mouse_handler.c:69-77`, `mouse_handler.h:11,19`). They become named
constants here, with the C's own spelling as the name:

```rust
pub const kCGSEventTypeField: CGEventField = CGEventField(55);
pub const kCGSEventDockControl: i64 = 30;
pub const kCGEventGestureHIDType: CGEventField = CGEventField(110);
pub const kCGEventGestureSwipeMotion: CGEventField = CGEventField(123);
pub const kCGEventGestureSwipeProgress: CGEventField = CGEventField(124);
pub const kCGEventGestureSwipeVelocityX: CGEventField = CGEventField(129);
pub const kCGEventGesturePhase: CGEventField = CGEventField(132);
pub const kIOHIDEventTypeDockSwipe: i64 = 23;
pub const kCGGestureMotionHorizontal: i64 = 1;
pub const kCGSGesturePhaseBegan: i64 = 1;
pub const kCGSGesturePhaseEnded: i64 = 4;
pub const kCGSGesturePhaseCancelled: i64 = 8;
```

`kCGSEventDockControl` is used twice with two meanings: as the *value* written into
`kCGSEventTypeField` (`space_manager.c:957`) and as an event *type* in the tap's `switch`
(`mouse_handler.c:69`) and in `MOUSE_EVENT_MASK` (`mouse_handler.h:11,19`), where it is shifted:
`1 << 30`. One constant serves both, exactly as the C's `30` does.

---

## 7. `src/ffi/color_sync.rs`

ColorSync, not CoreGraphics, exports the two display-UUID functions. `DECISIONS.md` 11 keeps
`objc2-color-sync` out of the tree, so both are hand-declared:

```rust
use objc2_core_foundation::CFUUID;

#[link(name = "ColorSync", kind = "framework")]
unsafe extern "C" {
    pub fn CGDisplayCreateUUIDFromDisplayID(did: u32) -> *mut CFUUID;
    pub fn CGDisplayGetDisplayIDFromUUID(uuid: *const CFUUID) -> u32;
}
```

`CGDisplayCreateUUIDFromDisplayID` is `extern.h:40` (`display.c:103`);
`CGDisplayGetDisplayIDFromUUID` has no `extern.h` declaration at all — the C picks it up from the
ColorSync header through the Cocoa umbrella — and is used at `display.c:117`, `space.c:9` and
`window.c:48`. It is a `Create` function, so its result is a `+1` `CFRetained<CFUUID>`.

---

## 8. `src/ffi/accessibility.rs`

### 8.1 Re-exports from `objc2-application-services`

`AXUIElement`, `AXObserver`, `AXError`, `AXValue`, `AXValueType`,
`AXUIElementCreateApplication`, `AXUIElementCreateSystemWide`, `AXUIElementSetMessagingTimeout`,
`AXUIElementCopyAttributeValue`, `AXUIElementSetAttributeValue`, `AXUIElementIsAttributeSettable`,
`AXUIElementPerformAction`, `AXUIElementCopyElementAtPosition`, `AXValueCreate`, `AXValueGetValue`,
`AXObserverCreate`, `AXObserverAddNotification`, `AXObserverRemoveNotification`,
`AXObserverGetRunLoopSource`, `AXIsProcessTrustedWithOptions`, `kAXTrustedCheckOptionPrompt`,
`AXObserverCallback`.

Two type shapes the translator has to respect, read out of the crate source:

```rust
pub type AXObserverCallback = Option<
    unsafe extern "C-unwind" fn(
        NonNull<AXObserver>,
        NonNull<AXUIElement>,
        NonNull<CFString>,
        *mut c_void,
    ),
>;

pub fn AXObserverCreate(
    application: libc::pid_t,
    callback: AXObserverCallback,
    out_observer: NonNull<*mut AXObserver>,
) -> AXError;
```

`AXValueType` is a newtype with associated constants: `AXValueType::CGPoint` and
`AXValueType::CGSize` replace `kAXValueTypeCGPoint` / `kAXValueTypeCGSize`
(`window_manager.c:418,428,743,746`, `window.c:744,761,766`).

### 8.2 The two private HIServices entry points

```rust
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub fn _AXUIElementGetWindow(r#ref: *const AXUIElement, wid: *mut u32) -> AXError;
    pub fn _AXUIElementCreateWithRemoteToken(data: *const CFData) -> *mut AXUIElement;
}
```

`extern.h:7-8`. `_AXUIElementGetWindow` backs `ax_window_id` (`helpers.h:499-504`);
`_AXUIElementCreateWithRemoteToken` backs the inactive-space brute force
(`window_manager.c:1680`, §18.5).

### 8.3 The `kAX*` string constants

The SDK defines these as `#define kAXFooAttribute CFSTR("AXFoo")`, so they are macros, not
exported symbols, and no crate can bind them. Each becomes a `OnceLock<CFRetained<CFString>>`
behind an accessor of the same name, built with `CFString::from_static_str`, which wraps the
`&'static str` without copying. One `macro_rules!` generates all of them, which is the
`DECISIONS.md` 31 treatment of an X-macro list:

```rust
macro_rules! ax_string_constants {
    ($($constant_name:ident => $string_value:literal,)*) => {
        $(
            #[allow(non_snake_case)]
            pub fn $constant_name() -> &'static CFString {
                static CACHED: OnceLock<CFRetained<CFString>> = OnceLock::new();
                CACHED.get_or_init(|| CFString::from_static_str($string_value))
            }
        )*
    };
}

ax_string_constants! {
    kAXWindowsAttribute                 => "AXWindows",
    kAXFocusedWindowAttribute           => "AXFocusedWindow",
    kAXMainWindowAttribute              => "AXMainWindow",
    kAXWindowAttribute                  => "AXWindow",
    kAXPositionAttribute                => "AXPosition",
    kAXSizeAttribute                    => "AXSize",
    kAXRoleAttribute                    => "AXRole",
    kAXSubroleAttribute                 => "AXSubrole",
    kAXTitleAttribute                   => "AXTitle",
    kAXMinimizedAttribute               => "AXMinimized",
    kAXCloseButtonAttribute             => "AXCloseButton",
    kAXParentAttribute                  => "AXParent",
    kAXWindowRole                       => "AXWindow",
    kAXDrawerRole                       => "AXDrawer",
    kAXSheetRole                        => "AXSheet",
    kAXStandardWindowSubrole            => "AXStandardWindow",
    kAXDialogSubrole                    => "AXDialog",
    kAXFloatingWindowSubrole            => "AXFloatingWindow",
    kAXUnknownSubrole                   => "AXUnknown",
    kAXPressAction                      => "AXPress",
    kAXRaiseAction                      => "AXRaise",
    kAXCreatedNotification              => "AXCreated",
    kAXFocusedWindowChangedNotification => "AXFocusedWindowChanged",
    kAXWindowMovedNotification          => "AXWindowMoved",
    kAXWindowResizedNotification        => "AXWindowResized",
    kAXTitleChangedNotification         => "AXTitleChanged",
    kAXMenuOpenedNotification           => "AXMenuOpened",
    kAXMenuClosedNotification           => "AXMenuClosed",
    kAXWindowMiniaturizedNotification   => "AXWindowMiniaturized",
    kAXWindowDeminiaturizedNotification => "AXWindowDeminiaturized",
    kAXUIElementDestroyedNotification   => "AXUIElementDestroyed",
    kAXFullscreenAttribute              => "AXFullScreen",
    kAXEnhancedUserInterface            => "AXEnhancedUserInterface",
    kAXExposeShowAllWindows             => "AXExposeShowAllWindows",
    kAXExposeShowFrontWindows           => "AXExposeShowFrontWindows",
    kAXExposeShowDesktop                => "AXExposeShowDesktop",
    kAXExposeExit                       => "AXExposeExit",
}
```

Every string above was read out of the SDK headers, except the last five, which yabai defines
itself: `kAXFullscreenAttribute` at `window.h:4` (note the capital S in `"AXFullScreen"`, which
does not match the constant's own spelling), `kAXEnhancedUserInterface` at `helpers.h:171`, and
the four `kAXExpose*` at `mission_control.c:52-55`.

The C compares these with `CFEqual` and never by pointer (`application.c:8-27`,
`mission_control.c:61-68`, `window_manager.c:1686`), so a non-interned `CFString` is
behaviour-identical. Every call site becomes `kAXRoleAttribute()` instead of `kAXRoleAttribute`.

### 8.4 The error-string table

`ax_error_str[]` (`application.h:26-44`) is indexed by `-kAXError*`, where the enumerators run
`0, -25200 …`. In `objc2-application-services` `AXError` is a newtype, so the designated-array
becomes a match on the newtype, which keeps the same sixteen strings:

```rust
pub fn ax_error_str(error: AXError) -> &'static str {
    match error {
        AXError::Success => "kAXErrorSuccess",
        AXError::Failure => "kAXErrorFailure",
        ...
    }
}
```

A value outside the sixteen is impossible in C (it would read past the array); in Rust the
catch-all arm returns `"kAXErrorSuccess"`, matching what index 0 of the C table holds for an
out-of-range negative index of 0. This is a bounds check standing in for a C out-of-bounds read
and is one line in `DEVIATIONS.md` per `DECISIONS.md` 4.

`ax_application_notification_str[]` (`application.h:46+`) is a plain seven-entry table indexed by
`AX_APPLICATION_WINDOW_*_INDEX` and becomes a `const [&str; 7]`.

### 8.5 The helpers that live here

`ax_privilege` (`helpers.h:489`), `ax_window_id` (`:499`), `ax_window_pid` (`:506`),
`ax_enhanced_userinterface` (`:511`) and the `AX_ENHANCED_UI_WORKAROUND` macro (`:524-530`).
The macro takes a statement block in C; in Rust it becomes a function taking a closure, which is
the only shape that keeps the "turn it off, do the thing, turn it back on" ordering:

```rust
pub fn ax_enhanced_ui_workaround(r#ref: *const AXUIElement, body: impl FnOnce()) {
    let enhanced_user_interface = ax_enhanced_userinterface(r#ref);
    if enhanced_user_interface {
        unsafe {
            AXUIElementSetAttributeValue(r#ref, kAXEnhancedUserInterface(), kCFBooleanFalse);
        }
    }
    body();
    if enhanced_user_interface {
        unsafe {
            AXUIElementSetAttributeValue(r#ref, kAXEnhancedUserInterface(), kCFBooleanTrue);
        }
    }
}
```

The C names the local `eui`; `DECISIONS.md` 37 forbids the abbreviation, so it is
`enhanced_user_interface`.

`ax_window_pid` is the single most fragile line in the FFI layer and is reproduced verbatim
(§18.6).

---

## 9. `src/ffi/carbon_process.rs`

The whole Process Manager group is hand-written, because the `objc2-application-services`
bindings for it take a `ProcessSerialNumber` that no downstream crate can name (§1.3).

### 9.1 `ProcessSerialNumber`

```rust
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ProcessSerialNumber {
    pub high_long_of_psn: u32,
    pub low_long_of_psn: u32,
}

pub const kNoProcess: u32 = 0;

const _: () = assert!(core::mem::size_of::<ProcessSerialNumber>() == 8);
```

Field names follow `DECISIONS.md` 37 — the C spells them `highLongOfPSN` / `lowLongOfPSN`. The
struct is `Copy`: `process_pid_for_psn` takes it by value (`process_manager.c:24`),
`SLSSpaceSetFrontPSN` takes it by value, and the `dispatch_after` blocks capture it by value
(§23). `PartialEq` is *not* what `psn_equals` uses — see §9.4.

### 9.2 `ProcessInfoRec`

Carbon compiles this under mac68k alignment, so the natural `#[repr(C)]` is wrong: measured on
this SDK, `sizeof(ProcessInfoRec) == 72` with alignment 2, and the field offsets are
`0, 4, 12, 20, 24, 28, 32, 40, 44, 48, 56, 60, 64`. `#[repr(C, packed(2))]` reproduces that
exactly, and the assertions pin it:

```rust
#[repr(C, packed(2))]
pub struct ProcessInfoRec {
    pub process_info_length: u32,
    pub process_name: *mut c_char,
    pub process_number: ProcessSerialNumber,
    pub process_type: u32,
    pub process_signature: u32,
    pub process_mode: u32,
    pub process_location: *mut c_char,
    pub process_size: u32,
    pub process_free_mem: u32,
    pub process_launcher: ProcessSerialNumber,
    pub process_launch_date: u32,
    pub process_active_time: u32,
    pub process_app_ref: *mut c_void,
}

const _: () = assert!(core::mem::size_of::<ProcessInfoRec>() == 72);
const _: () = assert!(core::mem::offset_of!(ProcessInfoRec, process_type) == 20);
const _: () = assert!(core::mem::offset_of!(ProcessInfoRec, process_app_ref) == 64);
```

`process_create` (`process_manager.c:32-33`) builds one with only
`processInfoLength = sizeof(ProcessInfoRec)` set and the rest zeroed by the designated
initialiser — the zeroes for `processName` and `processAppRef` are what stop
`GetProcessInformation` writing into random memory, so the Rust must zero the whole struct too,
not leave it uninitialised.

`process_info.processType == 'XPC!'` at `process_manager.c:47` is the four-char code
`0x58504321`; the Rust compares `process_info.process_type == 0x5850_4321`. Reading a field of a
packed struct needs a copy into a local first, which `{ process_info.process_type }` does.

### 9.3 The declarations

```rust
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub fn GetProcessPID(psn: *const ProcessSerialNumber, pid: *mut pid_t) -> i32;
    pub fn GetProcessInformation(
        psn: *const ProcessSerialNumber,
        info: *mut ProcessInfoRec,
    ) -> i16;
    pub fn CopyProcessName(
        psn: *const ProcessSerialNumber,
        name: *mut *mut CFString,
    ) -> i32;
    pub fn GetNextProcess(psn: *mut ProcessSerialNumber) -> i16;
    pub fn SameProcess(
        psn1: *const ProcessSerialNumber,
        psn2: *const ProcessSerialNumber,
        result: *mut u8,
    ) -> i16;
    pub fn IsProcessVisible(psn: *const ProcessSerialNumber) -> u8;

    pub fn CoreDockGetAutoHideEnabled() -> u8;
    pub fn CoreDockGetOrientationAndPinning(orientation: *mut c_int, pinning: *mut c_int);
    pub fn CoreDockSendNotification(notification: *const CFString, unknown: c_int) -> CGError;
}
```

The return types are not uniform and the SDK header is the authority:
`GetProcessPID` and `CopyProcessName` return `OSStatus` (`SInt32` → `i32`), while
`GetProcessInformation`, `GetNextProcess` and `SameProcess` return `OSErr` (`SInt16` → `i16`).
`noErr` is `0` in both widths and the C compares against it directly
(`process_manager.c:212`). `IsProcessVisible` returns `Boolean`, an `unsigned char`, so it is
`u8` here, and `application_is_hidden` (`application.c:113`) already writes `== 0`; mapping it to
Rust `bool` would be unsound for any byte other than 0 or 1 (§18.4). `SameProcess`'s
out-parameter is a `Boolean` compared against literal `1` and stays `u8` as well. The three
`CoreDock*` are `extern.h:52,53,64` and are exported by HIServices, not by SkyLight, which is why
they sit in this module.

### 9.4 `psn_equals`

`helpers.h:534-539` asks the Process Manager, it does not compare fields:

```rust
pub fn psn_equals(a: *const ProcessSerialNumber, b: *const ProcessSerialNumber) -> bool {
    let mut result: u8 = 0;
    unsafe { SameProcess(a, b, &mut result) };
    result == 1
}
```

The C leaves `result` uninitialised before the call; the Rust zeroes it, which is the
`DECISIONS.md` 4 treatment of an uninitialised read and one line in `DEVIATIONS.md`.
This function is the `compare` half of the process table (`process_manager.c:9-12`), so the
derived `PartialEq` on `ProcessSerialNumber` is never used for that purpose; the hash half is
`lowLongOfPSN` (`process_manager.c:5-7`).

---

## 10. `src/ffi/carbon_events.rs` — the Carbon Event Manager

No `objc2` crate binds HIToolbox, so every line here is hand-written. The symbols are still
exported on SDK 26.1; only the headers carry the 10.9 deprecation, and the C silences it with
`#pragma clang diagnostic ignored "-Wdeprecated-declarations"` (`process_manager.c:22-23`).
Rust has nothing to silence because the declarations are ours.

### 10.1 Types

```rust
pub type OSStatus = i32;
pub const noErr: OSStatus = 0;

pub type EventTime = f64;

#[repr(C)]
pub struct OpaqueEventRef {
    _private: [u8; 0],
}
pub type EventRef = *mut OpaqueEventRef;

#[repr(C)]
pub struct OpaqueEventHandlerRef {
    _private: [u8; 0],
}
pub type EventHandlerRef = *mut OpaqueEventHandlerRef;

#[repr(C)]
pub struct OpaqueEventHandlerCallRef {
    _private: [u8; 0],
}
pub type EventHandlerCallRef = *mut OpaqueEventHandlerCallRef;

#[repr(C)]
pub struct OpaqueEventTargetRef {
    _private: [u8; 0],
}
pub type EventTargetRef = *mut OpaqueEventTargetRef;

pub type EventHandlerProcPtr = unsafe extern "C-unwind" fn(
    in_handler_call_ref: EventHandlerCallRef,
    in_event: EventRef,
    in_user_data: *mut c_void,
) -> OSStatus;
pub type EventHandlerUPP = EventHandlerProcPtr;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct EventTypeSpec {
    pub event_class: u32,
    pub event_kind: u32,
}

const _: () = assert!(core::mem::size_of::<EventTypeSpec>() == 8);
```

`EventTime` is a `double`, not an integer — `process_manager.h:26` stores one in
`switch_event_time` and `event_loop.c` compares it against another. `EventHandlerUPP` is
`STACK_UPP_TYPE(EventHandlerProcPtr)`, which on macOS is the bare function pointer, so the two
aliases are the same type.

**`NewEventHandlerUPP` is not a function and must never be declared.** On `__MACH__` it is a
preprocessor macro, `CarbonEventsCore.h:2512-2519`:

```c
#if __MACH__
  #ifdef __cplusplus
    inline EventHandlerUPP                                      NewEventHandlerUPP(EventHandlerProcPtr userRoutine) { return userRoutine; }
    inline void                                                 DisposeEventHandlerUPP(EventHandlerUPP) { }
  #else
    #define NewEventHandlerUPP(userRoutine)                     ((EventHandlerUPP)userRoutine)
    #define DisposeEventHandlerUPP(userUPP)
  #endif
#endif
```

`HIToolbox.tbd` exports `_InstallEventHandler`, `_GetApplicationEventTarget`,
`_GetEventParameter`, `_GetEventKind` and `_GetCurrentEventTime`, and no `_NewEventHandlerUPP` or
`_DisposeEventHandlerUPP`; no `.tbd` under the SDK does. A Rust `extern` declaration of either
one compiles and then fails to link with
`Undefined symbols for architecture arm64: "_NewEventHandlerUPP"`, which a compiled probe against
SDK 26.1 confirms for both `aarch64-apple-darwin` and `x86_64-apple-darwin`.

So `process_manager.c:233` is a **cast, not a call**: the preprocessor turns
`pm->handler = NewEventHandlerUPP(process_handler);` into
`pm->handler = ((EventHandlerUPP)process_handler);`. `process_manager_begin` in Rust writes that
cast directly:

```rust
process_manager.handler = process_handler as EventHandlerUPP;
```

`DisposeEventHandlerUPP` expands to nothing at all, and the daemon never calls it anyway.

### 10.2 Constants

```rust
pub const kEventClassApplication: u32 = 0x6170_706C;
pub const kEventAppLaunched: u32 = 5;
pub const kEventAppTerminated: u32 = 6;
pub const kEventAppFrontSwitched: u32 = 7;
pub const kEventParamProcessID: u32 = 0x7073_6E20;
pub const typeProcessSerialNumber: u32 = 0x7073_6E20;
```

`kEventParamProcessID` and `typeProcessSerialNumber` really are the same four-char code,
`'psn '`. Both values were read back from a compiled probe against this SDK, not from memory.

### 10.3 Functions

```rust
#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    pub fn GetApplicationEventTarget() -> EventTargetRef;
    pub fn InstallEventHandler(
        in_target: EventTargetRef,
        in_handler: EventHandlerUPP,
        in_num_types: c_ulong,
        in_list: *const EventTypeSpec,
        in_user_data: *mut c_void,
        out_ref: *mut EventHandlerRef,
    ) -> OSStatus;
    pub fn GetEventParameter(
        in_event: EventRef,
        in_name: u32,
        in_desired_type: u32,
        out_actual_type: *mut u32,
        in_buffer_size: c_ulong,
        out_actual_size: *mut c_ulong,
        out_data: *mut c_void,
    ) -> OSStatus;
    pub fn GetEventKind(in_event: EventRef) -> u32;
    pub fn GetCurrentEventTime() -> EventTime;
}
```

Five functions, and only five: `NewEventHandlerUPP` and `DisposeEventHandlerUPP` are macros with
no symbol behind them (§10.1) and are absent from this block on purpose.

`ItemCount` and `ByteCount` are both `unsigned long`, hence `c_ulong`. The three `NULL`
arguments `process_manager.c:156` passes to `GetEventParameter` become `core::ptr::null_mut()`.

### 10.4 The one handler

`process_handler` (`process_manager.c:151`) is the only `EventHandlerProcPtr` in the daemon; its
Rust form and panic policy are in §19.3. `process_manager_begin` (`process_manager.c:227-252`)
takes its address with the `as EventHandlerUPP` cast of §10.1, fills `pm.type[0..3]` with the
three `EventTypeSpec`s and installs the handler with `in_num_types = 3`, which becomes
`pm.r#type.len() as c_ulong` over a `[EventTypeSpec; 3]`.

---

## 11. `src/ffi/carbon_core.rs`

One function, behind CoreServices, plus the two clock readers that use it
(`helpers.h:147-161`):

```rust
#[repr(C)]
#[derive(Clone, Copy)]
pub struct UnsignedWide {
    pub hi: u32,
    pub lo: u32,
}
pub type AbsoluteTime = UnsignedWide;
pub type Nanoseconds = UnsignedWide;

const _: () = assert!(core::mem::size_of::<UnsignedWide>() == 8);

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    pub fn AbsoluteToNanoseconds(absolute_time: AbsoluteTime) -> Nanoseconds;
}

pub fn read_os_timer() -> u64 {
    let result = unsafe { libc::mach_absolute_time() };
    let nano = unsafe { AbsoluteToNanoseconds(core::mem::transmute::<u64, AbsoluteTime>(result)) };
    unsafe { core::mem::transmute::<Nanoseconds, u64>(nano) }
}

pub fn read_os_freq() -> u64 {
    1000000000
}
```

The C punning is `*(AbsoluteTime *) &result` and `*(uint64_t *) &nano` (`helpers.h:151-153`).
The field names are a trap and must be ignored. `MacTypes.h` declares the struct twice under
`#if TARGET_RT_BIG_ENDIAN`, swapping the two halves: `hi, lo` at `:138-142` for big-endian, and
`lo, hi` at `:149-153` for everything the daemon builds for, which is the arm that compiles:

```c
struct UnsignedWide {
  UInt32              lo;
  UInt32              hi;
};
```

So the Rust declaration above has its two fields in the opposite order to the C type on both
build targets, and the C struct's alignment is 2 — `MacTypes.h:60` opens with
`#pragma pack(push, 2)` — against the Rust struct's 4. A compiled probe on SDK 26.1 prints
`sizeof(UnsignedWide)=8 align=2 hi_off=4 lo_off=0` for `-arch arm64` and for `-arch x86_64`
alike.

None of that changes anything, because the value is eight opaque bytes end to end: neither the C
nor `AbsoluteToNanoseconds` ever reads a field, `read_os_timer` only `transmute`s, both types are
8 bytes so the `transmute`s are well-formed, and an 8-byte aggregate of two `u32`s classifies
identically for by-value passing on both ABIs whichever way the halves are named. The field order
of the Rust declaration is therefore immaterial — and nothing in the daemon may read `.hi` or
`.lo`.

`read_os_timer` is called from the main thread (`mission_control.c:24`, `mouse_handler.c:79`) and
from the event-loop thread, and returns **nanoseconds**, not mach ticks.

---

## 12. `src/ffi/core_video.rs`

Pure re-export from `objc2-core-video`: `CVDisplayLink`, `CVTimeStamp`, `CVReturn`,
`CVOptionFlags`, `kCVReturnSuccess`, `CVDisplayLinkOutputCallback`,
`CVDisplayLinkCreateWithActiveCGDisplays`, `CVDisplayLinkSetOutputCallback`,
`CVDisplayLinkStart`, `CVDisplayLinkStop`, `CVGetHostClockFrequency`.

Two shapes to respect:

```rust
pub type CVDisplayLinkOutputCallback = Option<
    unsafe extern "C-unwind" fn(
        NonNull<CVDisplayLink>,
        NonNull<CVTimeStamp>,
        NonNull<CVTimeStamp>,
        CVOptionFlags,
        NonNull<CVOptionFlags>,
        *mut c_void,
    ) -> CVReturn,
>;

pub fn CVDisplayLinkCreateWithActiveCGDisplays(
    display_link_out: NonNull<*mut CVDisplayLink>,
) -> CVReturn;
```

**There is no `CVDisplayLinkRelease` in the crate.** `CVDisplayLink` is declared with
`cf_type!`, so it is a CoreFoundation object with `CFRetained<CVDisplayLink>` semantics. The
`CVDisplayLinkRelease(link)` at `window_manager.c:596` and `:600` becomes dropping the
`CFRetained<CVDisplayLink>`, and the C's ordering — `CVDisplayLinkStop` then release — is kept by
calling `CVDisplayLinkStop` first and dropping immediately after.

The four lifecycle functions carry `#[deprecated = "renamed to CVDisplayLink::..."]` in the
crate. Phase 2 keeps the free-function spellings so the Rust reads against the C
(`DECISIONS.md` 2) and puts `#[allow(deprecated)]` on this module. `CVDisplayLink` is also
soft-deprecated by Apple in favour of `CADisplayLink`; the rewrite does not change that.

---

## 13. `src/ffi/core_foundation.rs`

Re-exports every CF item the daemon touches, so `CFRetained` ownership lives in one place:
`CFArray`, `CFMutableArray`, `CFDictionary`, `CFNumber`, `CFString`, `CFUUID`, `CFData`,
`CFMutableData`, `CFBoolean`, `CFType`, `CFRetained`, `CFRange`, `CFComparisonResult`,
`CFComparatorFunction`, `CFRunLoop`, `CFRunLoopSource`, `CFMachPort`, `CFIndex`, `CFTimeInterval`,
`kCFAllocatorDefault`, `kCFTypeArrayCallBacks`, `kCFTypeDictionaryKeyCallBacks`,
`kCFTypeDictionaryValueCallBacks`, `kCFCopyStringDictionaryKeyCallBacks`,
`kCFStringEncodingUTF8`, `kCFStringEncodingMacRoman`, `kCFNumberSInt32Type`,
`kCFNumberSInt64Type`, `kCFRunLoopDefaultMode`, `kCFRunLoopCommonModes`, `kCFBooleanTrue`,
`kCFBooleanFalse`, and the functions `CFEqual`, `CFArrayGetCount`, `CFArrayGetValueAtIndex`,
`CFArrayCreate`, `CFArrayCreateMutableCopy`, `CFArraySortValues`, `CFDictionaryCreate`,
`CFDictionaryGetValue`, `CFNumberCreate`, `CFNumberGetValue`, `CFNumberGetType`,
`CFStringCreateWithCString`, `CFStringGetCString`, `CFStringGetLength`,
`CFStringGetMaximumSizeForEncoding`, `CFUUIDCreateFromString`, `CFUUIDCreateString`,
`CFDataCreateMutable`, `CFDataIncreaseLength`, `CFDataGetMutableBytePtr`, `CFBooleanGetValue`,
`CFRunLoopGetMain`, `CFRunLoopAddSource`, `CFRunLoopRemoveSource`, `CFRunLoopSourceInvalidate`,
`CFMachPortCreateRunLoopSource`, `CFMachPortInvalidate`, `CFRangeMake`.

`CFRetain` and `CFRelease` have no re-export. A `+1` reference is a `CFRetained<T>` and its
`Drop` is the release; an explicit `CFRetain` in C becomes `CFRetained::retain`. This is the one
place where the Rust does not read line-for-line against the C, and it is forced: a manual
`CFRelease` on an object objc2 already owns would double-release.

The CF-only helpers from `misc/helpers.h` move here because they are pure CF glue and nothing
else: `cfstring_copy` (`:373`), `ts_cfstring_copy` (`:361`, whose arena becomes a `String` under
`DECISIONS.md` 17), `CFSTRINGNUM32` (`:321`), `CFNUM32` (`:328`), `cfarray_of_cfnumbers`
(`:344`) and `sls_window_disable_shadow` (`:333`, a CF dictionary wrapped around one
`SLSWindowSetShadowProperties` call).

`CGRect`, `CGPoint`, `CGSize`, `CGFloat` and `CGAffineTransform` come from here, not from
`objc2-core-graphics` — in objc2 they live in `objc2-core-foundation`'s `CFCGTypes`. No module
may define its own.

The one comparator the daemon hands to CoreFoundation is
`display_manager_coordinate_comparator` (`display_manager.c:180`), whose type is
`CFComparatorFunction = Option<unsafe extern "C-unwind" fn(*const c_void, *const c_void, *mut c_void) -> CFComparisonResult>`;
§19.7.

---

## 14. `src/ffi/appkit.rs` and `src/ffi/foundation.rs`

Thin re-export modules so `workspace.rs`, `sa.rs`, `notify.rs` and `service.rs` each import from
one place.

**`foundation.rs`**: `NSObject`, `NSString`, `NSArray`, `NSDictionary`, `NSNumber`,
`NSNotification`, `NSNotificationCenter`, `NSDistributedNotificationCenter`, `NSProcessInfo`,
`NSOperatingSystemVersion`, `NSBundle`, `NSEdgeInsets`, `NSHomeDirectoryForUser`,
`NSKeyValueObservingOptions`, `NSKeyValueChangeNewKey`,
`NSObjectNSKeyValueObserverRegistration`, `NSObjectNSKeyValueCoding`, `NSUserNotification`,
`NSUserNotificationCenter`, `NSUserNotificationCenterDelegate`, `MainThreadMarker`.

`NSObjectNSKeyValueObserverRegistration` and `NSObjectNSKeyValueCoding` are traits, and they
carry the class name objc2 prefixes onto a category-protocol extension: the Objective-C
categories are `NSKeyValueObserverRegistration` and `NSKeyValueCoding` on `NSObject`, and the
crate spells them `NSObjectNSKeyValueObserverRegistration` (which carries
`addObserver:forKeyPath:options:context:` and `removeObserver:forKeyPath:context:`) and
`NSObjectNSKeyValueCoding` (which carries `setValue:forKey:`). The unprefixed names do not exist
in `objc2-foundation` 0.3.2 and importing them is an `unresolved import`; both traits must be in
scope for §20, §21 and `notify.h:41-42` to compile.

`MainThreadMarker` is `objc2`'s, re-exported by `objc2-foundation`. It is named in exactly two
places: `main`, for `NSApplication::sharedApplication` (§27.3), and
`workspace_display_notch_height`, which mints its own (§20.5).

**`appkit.rs`**: `NSApplication`, `NSApplicationLoad`, `NSApplicationActivationPolicy`,
`NSWorkspace`, `NSRunningApplication`, `NSScreen`, `NSImage`, and the five AppKit notification
names `NSWorkspaceActiveSpaceDidChangeNotification`,
`NSWorkspaceDidHideApplicationNotification`, `NSWorkspaceDidUnhideApplicationNotification`,
`NSWorkspaceDidWakeNotification`, `NSWorkspaceApplicationKey`.

The four notification names that are **plain string literals in the C**, not symbols, are
`CFString`/`NSString` constants built here, so the one place they are spelled is this module:

```rust
pub const NS_WORKSPACE_ACTIVE_DISPLAY_DID_CHANGE_NOTIFICATION: &str =
    "NSWorkspaceActiveDisplayDidChangeNotification";
pub const APPLE_INTERFACE_MENU_BAR_HIDING_CHANGED_NOTIFICATION: &str =
    "AppleInterfaceMenuBarHidingChangedNotification";
pub const NS_APPLICATION_DOCK_DID_RESTART_NOTIFICATION: &str =
    "NSApplicationDockDidRestartNotification";
pub const COM_APPLE_DOCK_PREFCHANGED: &str = "com.apple.dock.prefchanged";
```

`workspace.m:159,184,189,194`. They are not AppKit symbols and never were; the C writes them as
`@"..."` literals.

`NSAutoreleasePool` is **not** re-exported. §21 replaces it with `objc2::rc::autoreleasepool`.

---

## 15. `src/ffi/mach_port.rs`

`DECISIONS.md` 11 bars `mach2`, so the Mach message types are hand-declared here, and
`DECISIONS.md` 35 fixes their shape: `#[repr(C, packed(4))]` with a `const` size assertion each.
Every number below was measured by compiling a probe against SDK 26.1 rather than recalled.

### 15.1 Base types and the message header

```rust
pub type mach_msg_bits_t = u32;
pub type mach_msg_size_t = u32;
pub type mach_msg_id_t = i32;
pub type mach_msg_option_t = i32;
pub type mach_msg_timeout_t = u32;
pub type mach_msg_return_t = libc::kern_return_t;

#[repr(C, packed(4))]
#[derive(Clone, Copy)]
pub struct mach_msg_header_t {
    pub msgh_bits: mach_msg_bits_t,
    pub msgh_size: mach_msg_size_t,
    pub msgh_remote_port: libc::mach_port_t,
    pub msgh_local_port: libc::mach_port_t,
    pub msgh_voucher_port: libc::mach_port_t,
    pub msgh_id: mach_msg_id_t,
}

const _: () = assert!(core::mem::size_of::<mach_msg_header_t>() == 24);
```

### 15.2 The OOL descriptor — why `packed(4)` is mandatory

`<mach/message.h>` wraps these declarations in `#pragma pack(push, 4)`, and the LP64 field order
puts the four bit-fields between the pointer and the size:

```rust
#[repr(C, packed(4))]
#[derive(Clone, Copy)]
pub struct mach_msg_ool_descriptor_t {
    pub address: *mut c_void,
    pub deallocate: u8,
    pub copy: u8,
    pub pad1: u8,
    pub r#type: u8,
    pub size: mach_msg_size_t,
}

const _: () = assert!(core::mem::size_of::<mach_msg_ool_descriptor_t>() == 16);
const _: () = assert!(core::mem::align_of::<mach_msg_ool_descriptor_t>() == 4);
```

The four C bit-fields are each exactly 8 bits wide and each sits in its own byte, so four `u8`s
reproduce them. Without `packed(4)` Rust would give this struct alignment 8, which changes the
*containing* message's size from 44 to 48 and moves the descriptor from offset 28 to 32 — the
send would be malformed. Measured C values: size 16, alignment 4.

### 15.3 `NDR_record`

```rust
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NDR_record_t {
    pub mig_vers: u8,
    pub if_vers: u8,
    pub reserved1: u8,
    pub mig_encoding: u8,
    pub int_rep: u8,
    pub char_rep: u8,
    pub float_rep: u8,
    pub reserved2: u8,
}

const _: () = assert!(core::mem::size_of::<NDR_record_t>() == 8);

unsafe extern "C" {
    pub static NDR_record: NDR_record_t;
}
```

`NDR_record` is a global exported by `libsystem_kernel` and read, not written, at
`window.c:935`. It is all `u8`, so alignment is 1 and no packing attribute is needed. On this
machine it reads `00 00 00 00 01 00 00 00`; the Rust copies whatever the runtime holds, as the C
does.

### 15.4 Constants and functions

```rust
pub const MACH_MSG_TYPE_COPY_SEND: u32 = 19;
pub const MACH_MSGH_BITS_REMOTE_MASK: u32 = 0x1f;
pub const MACH_MSGH_BITS_COMPLEX: u32 = 0x8000_0000;
pub const MACH_MSG_OOL_DESCRIPTOR: u8 = 1;
pub const MACH_MSG_VIRTUAL_COPY: u8 = 1;
pub const MACH_SEND_MSG: mach_msg_option_t = 0x0000_0001;
pub const MACH_RCV_MSG: mach_msg_option_t = 0x0000_0002;
pub const TASK_BOOTSTRAP_PORT: c_int = 4;

unsafe extern "C" {
    pub fn mach_msg(
        msg: *mut mach_msg_header_t,
        option: mach_msg_option_t,
        send_size: mach_msg_size_t,
        rcv_size: mach_msg_size_t,
        rcv_name: libc::mach_port_t,
        timeout: mach_msg_timeout_t,
        notify: libc::mach_port_t,
    ) -> mach_msg_return_t;

    pub fn task_get_special_port(
        task: libc::mach_port_t,
        which_port: c_int,
        special_port: *mut libc::mach_port_t,
    ) -> libc::kern_return_t;

    pub fn bootstrap_look_up(
        bp: libc::mach_port_t,
        service_name: *const c_char,
        sp: *mut libc::mach_port_t,
    ) -> libc::kern_return_t;

    pub fn mig_get_special_reply_port() -> libc::mach_port_t;
}
```

`libc` supplies `mach_task_self()`, `mach_port_t`, `kern_return_t` and `KERN_SUCCESS`; the four
above are not in `libc` 0.2.189 on either Apple target and are declared here. No `#[link]`
attribute: libSystem is linked into every Rust binary. `mig_get_special_reply_port` is
`extern.h:6`.

`g_bs_port` is filled once at `yabai.c:156` by
`task_get_special_port(mach_task_self(), TASK_BOOTSTRAP_PORT, &g_bs_port)` and becomes an
`OnceLock<mach_port_t>` under `DECISIONS.md` 18.

### 15.5 `mach_send` — the JankyBorders OOL send

`helpers.h:204-223`. The message struct is the second `packed(4)` struct of `DECISIONS.md` 35;
measured size 44, with `descriptor_count` at offset 24 and `descriptor` at offset 28:

```rust
#[repr(C, packed(4))]
struct MachSendMessage {
    header: mach_msg_header_t,
    descriptor_count: mach_msg_size_t,
    descriptor: mach_msg_ool_descriptor_t,
}

const _: () = assert!(core::mem::size_of::<MachSendMessage>() == 44);
const _: () = assert!(core::mem::offset_of!(MachSendMessage, descriptor_count) == 24);
const _: () = assert!(core::mem::offset_of!(MachSendMessage, descriptor) == 28);

pub fn mach_send(port: libc::mach_port_t, data: *mut c_void, size: u32) {
    let mut message = MachSendMessage {
        header: mach_msg_header_t {
            msgh_bits: (MACH_MSG_TYPE_COPY_SEND & MACH_MSGH_BITS_REMOTE_MASK)
                | MACH_MSGH_BITS_COMPLEX,
            msgh_size: core::mem::size_of::<MachSendMessage>() as mach_msg_size_t,
            msgh_remote_port: port,
            msgh_local_port: 0,
            msgh_voucher_port: 0,
            msgh_id: 0,
        },
        descriptor_count: 1,
        descriptor: mach_msg_ool_descriptor_t {
            address: data,
            deallocate: 0,
            copy: MACH_MSG_VIRTUAL_COPY,
            pad1: 0,
            r#type: MACH_MSG_OOL_DESCRIPTOR,
            size,
        },
    };

    unsafe {
        mach_msg(
            &mut message.header,
            MACH_SEND_MSG,
            core::mem::size_of::<MachSendMessage>() as mach_msg_size_t,
            0,
            0,
            0,
            0,
        );
    }
}
```

`MACH_MSGH_BITS_SET(remote, 0, 0, complex)` collapses to the `|` above when the local and
voucher halves are zero; the assembled value is `0x80000013`, which a probe against SDK 26.1
confirms. `msgh_size` is `sizeof(msg)` in C and is the same 44 here.

`&mut message.header` needs care: taking a reference into a `packed` struct is not allowed, but
`header` is the first field at offset 0 with the struct's own alignment, so the Rust writes
`core::ptr::addr_of_mut!(message).cast::<mach_msg_header_t>()` and passes that.

### 15.6 The JankyBorders payload

`window_manager_notify_jankyborders` (`window_manager.c:437-458`) builds a 4104-byte struct and
sends it out-of-line, so the 4096-byte scripting-addition limit does not apply to it:

```rust
#[repr(C)]
struct JankyBordersEvent {
    event: u32,
    count: u32,
    proxy_wid: [u32; 512],
    real_wid: [u32; 512],
}

const _: () = assert!(core::mem::size_of::<JankyBordersEvent>() == 4104);
```

Plain `#[repr(C)]`, not packed — every field is `u32` and the C struct has no packing pragma. It
is zero-initialised (`= { event, 0 }` with `-Wmissing-field-initializers` silenced at
`window_manager.c:435-436`), `count` counts only the non-skipped animations, and the loop writes
at most `animation_count` entries. **The C has no bound on `animation_count`**: more than 512
animations overruns both arrays. The Rust stops at 512 and is one line in `DEVIATIONS.md`
(`DECISIONS.md` 4).

The port comes from `bootstrap_look_up(g_bs_port, "git.felix.jbevent", &port)` guarded by
`g_bs_port` being non-zero; the service name is a `c"git.felix.jbevent"` literal.

### 15.7 `SLSGetWindowSubLevel__Internal`

`window.c:930-952` — a hand-rolled MIG call, and the third `packed(4)` struct. It lives in
`window.rs`, next to its only caller, not in this module; only the type and constants come from
here. `#pragma pack(push,4)` is `#[repr(C, packed(4))]`, and the measured size is 0x30, which is
exactly the receive size the C passes:

```rust
#[repr(C, packed(4))]
struct SLSGetWindowSubLevelMessage {
    header: mach_msg_header_t,
    NDR_record: NDR_record_t,
    window_id: u32,
    sub_level: i32,
    padding1: i32,
    padding2: i32,
}

const _: () = assert!(core::mem::size_of::<SLSGetWindowSubLevelMessage>() == 0x30);
const _: () = assert!(core::mem::offset_of!(SLSGetWindowSubLevelMessage, window_id) == 0x20);
const _: () = assert!(core::mem::offset_of!(SLSGetWindowSubLevelMessage, sub_level) == 0x24);

fn SLSGetWindowSubLevel__Internal(cid: c_int, wid: u32) -> c_int {
    let Some(cgs_get_connection_port_by_id) = cgs_get_connection_port_by_id() else {
        return 0;
    };

    let mut message: SLSGetWindowSubLevelMessage = unsafe { core::mem::zeroed() };
    message.NDR_record = unsafe { NDR_record };
    message.window_id = wid;
    message.header.msgh_bits = 0x1513;
    message.header.msgh_remote_port = unsafe { cgs_get_connection_port_by_id(cid) };
    message.header.msgh_local_port = unsafe { mig_get_special_reply_port() };
    message.header.msgh_id = if workspace_is_macos_tahoe() { 0x76E3 } else { 0x73C3 };

    unsafe {
        mach_msg(
            core::ptr::addr_of_mut!(message).cast::<mach_msg_header_t>(),
            MACH_SEND_MSG | MACH_RCV_MSG,
            0x24,
            0x30,
            message.header.msgh_local_port,
            0,
            0,
        );
    }

    message.sub_level
}
```

The literal `0x24` send size is header (24) + `NDR_record` (8) + `window_id` (4) = 36, and the
`0x30` receive size is the whole struct; the two offset assertions are what make those literals
safe to keep. `message.header.msgh_local_port` is read *after* the assignment and again as the
`rcv_name` argument, exactly as the C does, and reading a field out of a packed struct copies it.

The `else { return 0 }` arm replaces C's implicit "call through a NULL function pointer": the
caller `window_sub_level` (`window.c:953-960`) already tests `if (CGSGetConnectionPortById)`
before entering, so this arm is unreachable in practice and exists because Rust has no nullable
`fn`.

---

## 16. `src/ffi/libsystem.rs`

What `libc` 0.2.189 does supply on both `x86_64-apple-darwin` and `aarch64-apple-darwin`, checked
against the crate source: `proc_name`, `PROC_PIDPATHINFO_MAXSIZE`, `_NSGetExecutablePath`,
`_dyld_image_count`, `_dyld_get_image_name`, `_dyld_get_image_header`,
`_dyld_get_image_vmaddr_slide`, `mach_header_64`, `segment_command_64`, `load_command`,
`LC_SEGMENT_64`, `mach_absolute_time`, `mach_task_self`, `mach_port_t`, `kern_return_t`,
`KERN_SUCCESS`, `sysctl`, `sysctlbyname`, `CTL_KERN`, `KERN_PROC`, `KERN_PROC_PID`,
`regcomp`/`regexec`/`regfree`/`regex_t`, `posix_spawn` and the `posix_spawn_file_actions_*`
family, `fcntl` with `F_SETLK` and `struct flock`, `sockaddr_un` and the socket calls,
`getpwuid`, `fork`, `execvp`, `_exit`, `waitpid`, `system`, `popen`, `pclose`.

Four things it does not, declared here:

```rust
unsafe extern "C" {
    pub fn csr_get_active_config(config: *mut u32) -> c_int;
}

pub const CSR_ALLOW_UNRESTRICTED_FS: u32 = 0x02;
pub const CSR_ALLOW_TASK_FOR_PID: u32 = 0x04;

pub const P_TRACED: i32 = 0x0000_0800;
pub const SIZE_OF_KINFO_PROC: usize = 648;
pub const OFFSET_OF_P_FLAG_IN_KINFO_PROC: usize = 32;
```

`csr_get_active_config` is declared by hand at `sa.m:3` in the C too, with the two mask
constants at `:4-5`; it is exported by libSystem.

### 16.1 `process_is_being_debugged`

`process_manager.c:70-80` sysctls a whole `struct kinfo_proc` and reads one field:
`info.kp_proc.p_flag & P_TRACED`. `libc` has neither `kinfo_proc` nor `P_TRACED` on Apple
targets, and transcribing the 648-byte struct would be 100 lines of guesswork. The port reads
the same bytes out of an opaque buffer, with the two constants measured against this SDK
(`sizeof(struct kinfo_proc) == 648`, `offsetof(struct kinfo_proc, kp_proc) == 0`,
`offsetof(struct extern_proc, p_flag) == 32`):

```rust
pub fn process_is_being_debugged(process_id: pid_t) -> bool {
    let mut info = [0u8; SIZE_OF_KINFO_PROC];
    let mut size = core::mem::size_of_val(&info);
    let mut mib: [c_int; 4] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, process_id];

    unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as u32,
            info.as_mut_ptr().cast::<c_void>(),
            &mut size,
            core::ptr::null_mut(),
            0,
        );
    }

    let p_flag = i32::from_ne_bytes(
        info[OFFSET_OF_P_FLAG_IN_KINFO_PROC..OFFSET_OF_P_FLAG_IN_KINFO_PROC + 4]
            .try_into()
            .unwrap(),
    );
    (p_flag & P_TRACED) != 0
}
```

The C leaves the buffer uninitialised apart from `p_flag = 0`, so that a failed `sysctl` reads
back 0; zeroing the whole buffer is the same answer with no uninitialised read, and is one line
in `DEVIATIONS.md`. The `sysctl` call itself is unchanged, and the two constants above are the
only part of the C struct that has to be pinned.

### 16.2 `scripting_addition_is_arm64e_enabled`

`sa.m:317-331`, compiled only under `#ifdef __arm64__`, becomes
`#[cfg(target_arch = "aarch64")]`. `libc` has no `strnstr`; the Rust searches the returned
prefix of the buffer:

```rust
#[cfg(target_arch = "aarch64")]
fn scripting_addition_is_arm64e_enabled() -> bool {
    let mut bootargs = [0u8; 2048];
    let mut length = bootargs.len() - 1;

    let sysctl_result = unsafe {
        libc::sysctlbyname(
            c"kern.bootargs".as_ptr(),
            bootargs.as_mut_ptr().cast::<c_void>(),
            &mut length,
            core::ptr::null_mut(),
            0,
        )
    };

    sysctl_result == 0
        && bootargs[..length]
            .windows(b"-arm64e_preview_abi".len())
            .any(|window| window == b"-arm64e_preview_abi")
}
```

`strnstr` stops at a NUL as well as at `length`; `sysctlbyname` writes `length` back to the
NUL-terminated string length, so the two agree and the search window is the same bytes.

### 16.3 What stays libc, by `DECISIONS.md` 34

`system` (`sa.m:127,130,133,136,198`), `popen`/`pclose` (`sa.m:335,338`),
`posix_spawn` with a NULL environment (`service.h:56-78`), `_NSGetExecutablePath`
(`service.h:115`), and `open`/`fcntl(F_SETLK)` for the lock file whose descriptor is never closed
(`yabai.c:164-177`). None of these becomes a `std::process::Command` or a `std::fs::File` — the
observable behaviour of each, down to the inherited signal dispositions and the lock's lifetime,
is what `DECISIONS.md` 3 pins.

`getpagesize`, `mmap` and `mprotect` do **not** appear: they only ever backed `misc/ts.h` and
`misc/memory_pool.h`, which `DECISIONS.md` 17 replaces with `Vec`.

---

## 17. `src/ffi/dispatch.rs`

One function, so the five call sites in `event_loop.rs` read like the C:

```rust
use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchTime};

pub fn dispatch_after_on_main_queue(delay_in_nanoseconds: i64, work: impl Fn() + 'static) {
    let block = RcBlock::new(work);
    unsafe {
        DispatchQueue::exec_after_with_block(
            DispatchTime::NOW.time(delay_in_nanoseconds),
            DispatchQueue::main(),
            &*block as *const _ as *mut _,
        );
    }
}
```

`dispatch2` 0.3.1 spells `dispatch_after` as the associated function
`DispatchQueue::exec_after_with_block(when, queue, block)`, gated on the crate's `block2`
feature, with `dispatch_block_t = *mut block2::DynBlock<dyn Fn()>`. `dispatch_after` copies the
block, so the `RcBlock` may drop at the end of this function. `DispatchTime::NOW.time(delta)` is
`dispatch_time(DISPATCH_TIME_NOW, delta)` verbatim. §23 has the five call sites.

---

## 18. ABI traps

### 18.1 Aggregates passed or returned by value

Rust `extern "C"` with `#[repr(C)]` lowers through the same LLVM ABI path clang uses, on both
x86_64 SysV and AArch64 AAPCS, so these are correct *provided the struct definitions are the
imported ones*.

| Type | Layout | Crossing by value at |
| --- | --- | --- |
| `CGPoint` | 2 × `f64` | `SLSCopyBestManagedDisplayForPoint` (`extern.h:45`), `CGWarpMouseCursorPosition`, `CGPostMouseEvent` |
| `CGRect` | 4 × `f64` | `SLSCopyBestManagedDisplayForRect` (`extern.h:21`). `SLSGetWindowBounds`, `SLSGetRevealedMenuBarBounds`, `SLSGetDockRectWithReason` and `CGSNewRegionWithRect` take `*mut CGRect` instead |
| `CGAffineTransform` | 6 × `f64` | `SLSTransactionSetWindowTransform` (`extern.h:90`) |
| `ProcessSerialNumber` | 2 × `u32` | `SLSSpaceSetFrontPSN` (`extern.h:55`), `process_pid_for_psn` (`process_manager.c:24`) |
| `NSOperatingSystemVersion` | 3 × `isize` | returned from `-operatingSystemVersion` |
| `NSEdgeInsets` | 4 × `CGFloat` | returned from `-safeAreaInsets` |
| `AbsoluteTime` / `Nanoseconds` | 2 × `u32` | `AbsoluteToNanoseconds` |
| `EventTypeSpec` | 2 × `u32` | array of 3 to `InstallEventHandler` |
| `CVTimeStamp` | mixed | `*const CVTimeStamp` into the display-link callback, never by value |

**The rule: no module under `src/ffi/` and no module outside it may define its own `CGRect`,
`CGPoint`, `CGSize`, `CGFloat` or `CGAffineTransform`.** They are imported from
`objc2-core-foundation` in every file. A second definition would compile and silently mismatch.

### 18.2 `CGFloat`

`CGFloat` is `f64` on both targets yabai builds for, but it is a distinct type in
`objc2-core-foundation` and the daemon's own geometry is `f32` (`struct area` stays `f32` by
`DECISIONS.md` 30). Every crossing between them is an explicit `as` cast at the same point in
the expression the C put the implicit conversion — for example `window_manager.c:565-567` builds
`CGAffineTransform` from `f32` proxy fields, so each argument is `... as CGFloat`.

`workspace_display_notch_height` returns C `int` from `screen.safeAreaInsets.top`, a `CGFloat`
(`workspace.m:132`): that is a C truncating conversion, so the Rust writes
`safe_area_insets.top as c_int`.

### 18.3 `ProcessSerialNumber`

Two `u32`s, 8 bytes, and the only SkyLight function that takes one by value is
`SLSSpaceSetFrontPSN`. Everywhere else it crosses as `*const` or `*mut`. It is `Copy`, so the
event-loop's `dispatch_after` blocks capture it by value (§23) exactly as the C's
`__block ProcessSerialNumber psn` does. Equality is `SameProcess`, never field comparison (§9.4).

### 18.4 `bool` vs `Boolean`

C `bool` is one byte with values 0 and 1 only; Rust `bool` is the same, so
`SLSSetWindowOpacity` (`extern.h:31`), `SLSManagedDisplayIsAnimating` (`:46`) and
`SLSWindowIteratorAdvance` (`:70`) take and return Rust `bool`.

`Boolean` is `unsigned char` and may legally hold any byte. Rust `bool` holding anything but 0
or 1 is undefined, so every `Boolean` maps to `u8` and is compared explicitly:
`CoreDockGetAutoHideEnabled` (`extern.h:52`), `SameProcess`'s out-parameter (`helpers.h:537`,
compared against literal `1`), `IsProcessVisible` (compared against `0` at `application.c:113`),
and `ax_enhanced_userinterface`'s local (`helpers.h:512`).

`AXIsProcessTrustedWithOptions` is the exception: `objc2-application-services` already binds it
returning Rust `bool`, and that binding is used as-is.

`SLSWindowIsOrderedIn` takes `uint8_t *`, not `Boolean *`, and stays `*mut u8`; `sa.m:589-595`
tests it for non-zero.

### 18.5 The 0x14-byte AX remote token

`window_manager.c:1667-1698`. A `CFMutableData` of exactly `0x14` bytes, created empty and then
grown with `CFDataIncreaseLength`, which zero-fills:

| Offset | Width | Value |
| --- | --- | --- |
| `0x0` | `u32` | `application->pid` |
| `0x4` | `u32` | left zero |
| `0x8` | `u32` | `0x636f636f` |
| `0xc` | `u64` | `element_id`, rewritten each iteration |

`0x636f636f` is `"coco"` little-endian. The loop runs `element_id` from `0` to `0x7ffe`
inclusive and calls `_AXUIElementCreateWithRemoteToken` with the same `CFMutableData` each time,
mutating bytes `0xc..0x14` in place. The Rust keeps that: one `CFRetained<CFMutableData>`, a
`*mut u8` from `CFDataGetMutableBytePtr`, and three unaligned writes through
`core::ptr::write_unaligned` — offsets `0x0`, `0x8` and `0xc` are 4-, 4- and 4-aligned inside a
CF allocation, but `0xc` holds a `u64`, so that write is genuinely unaligned and
`write_unaligned` is required, not optional.

### 18.6 `ax_window_pid`

```rust
pub fn ax_window_pid(r#ref: *const AXUIElement) -> pid_t {
    unsafe { *(r#ref.cast::<u8>().add(0x10).cast::<pid_t>()) }
}
```

`helpers.h:506-509` reads a `pid_t` at offset `0x10` inside the opaque `AXUIElement`. There is no
supported equivalent — `AXUIElementGetPid` is a different code path the C deliberately avoids —
and `event_loop.c:559` depends on it. It is ported verbatim and is the single most fragile line
in the FFI layer. No `DEVIATIONS.md` entry: the behaviour is unchanged.

### 18.7 `SLSGetWindowSubLevel` has two spellings

`window_sub_level` (`window.c:953-960`) calls the hand-rolled MIG version when
`CGSGetConnectionPortById` resolved, and the exported `SLSGetWindowSubLevel` (`extern.h:15`)
otherwise. Both must be declared: the first in `window.rs` (§15.7), the second in
`skylight.rs` (§3.3). They return `c_int` and are not interchangeable in the source.

### 18.8 The synthesised `CGEvent` byte records

`window_manager.c:1274-1318`. `g_event_bytes` is a 256-byte heap buffer allocated once at
`yabai.c:141-142` and zeroed; only the first `0xf8` bytes are ever written, and the record handed
to `SLPSPostEventRecordTo` is that buffer. Three distinct records are built in it:

**`window_manager_make_key_window`** (`:1269-1291`):

| Offset | Width | Value |
| --- | --- | --- |
| `0x00..0xf8` | — | zeroed with `memset` |
| `0x04` | `u8` | `0xf8` |
| `0x08` | `u8` | `0x01`, then the record is posted, then `0x02` and posted again |
| `0x20..0x30` | 16 × `u8` | `0xff` |
| `0x3a` | `u8` | `0x10` |
| `0x3c` | `u32` | `window_id` |

**`window_manager_focus_window_without_raise`** (`:1293-1318`), only when the target process is
already frontmost:

| Offset | Width | Value |
| --- | --- | --- |
| `0x00..0xf8` | — | zeroed |
| `0x04` | `u8` | `0xf8` |
| `0x08` | `u8` | `0x0d` |
| `0x8a` | `u8` | `0x02`, posted to the *currently focused* psn, then `0x01`, posted to the target |
| `0x3c` | `u32` | the focused window id for the first post, `window_id` for the second |

with a hard `usleep(40000)` between the two posts, which the C's own `@hack` comment explains
and which is carried over verbatim under `DECISIONS.md` 38.

In Rust `g_event_bytes` becomes a `[u8; 0x100]` owned by `EventLoopOwnedState` rather than a
leaked `malloc`, because both writers run on the event-loop thread. The writes are
`event_bytes[0x04] = 0xf8`, `event_bytes[0x20..0x30].fill(0xff)` and
`event_bytes[0x3c..0x40].copy_from_slice(&window_id.to_ne_bytes())` — the last replacing the C's
`memcpy`, at the same unaligned offset, in native byte order.

`SLPSPostEventRecordTo` takes `*mut u8`, so the call passes `event_bytes.as_mut_ptr()`.

### 18.9 Four-char codes

`'XPC!'` (`process_manager.c:47`) is `0x58504321`; `kEventClassApplication` is `0x6170706C`
(`'appl'`); `kEventParamProcessID` and `typeProcessSerialNumber` are both `0x70736E20`
(`'psn '`). Rust has no four-char-code literal, so each is written as the hexadecimal value with
the C's spelling as a `const` name (§10.2, §9.2).

### 18.10 The SkyLight space-compat identifier

`space_manager.c:680-682` writes `0x79616265` into `SLSSpaceSetCompatID` and
`SLSSetWindowListWorkspace`, then `0x0`. That is `"yabe"` little-endian and it is an observable
protocol value, so it stays a literal at the call site, as in the C.

---

## 19. The functions Rust hands to C

Seven, exactly. Each is `unsafe extern "C-unwind" fn` (§2.3), each is a free function, none is a
closure, and none captures anything: they reach shared state through the statics of
`DECISIONS.md` 18, 20, 21 and 23 and through their own refcon.

### 19.0 The panic policy — `DECISIONS.md` 7

Every profile is `panic = "unwind"`, and the first statement of `main` installs a hook:

```rust
fn main() {
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("yabai: {panic_info}");
        std::process::abort();
    }));
    ...
}
```

Consequences the translator must hold on to:

* **A panic inside any of the seven aborts the process before unwinding starts.** The hook runs
  first, so no unwind ever crosses back into SkyLight, HIToolbox or CoreFoundation, and no
  half-unwound C frame is ever left behind. The `"C-unwind"` on the signatures is a type-system
  requirement of the objc2 aliases, not a live unwinding path.
* **`panic = "abort"` is not an option.** The spike is recorded in `DECISIONS.md` 7: under
  `abort` the process dies on the `removeObserver:` exception yabai deliberately swallows,
  because `objc2::exception::catch` cannot catch anything when the Objective-C runtime's
  personality routine has no unwinder to hand off to. Under `unwind` it catches. The four
  `@try/@catch` sites (§22) depend on this and on the `objc2/exception` feature.
* **A panic on the event-loop thread must not leave a live process behind a dead thread.** The
  hook aborts, so it cannot. Nothing catches panics anywhere in the daemon; there is no
  `catch_unwind`.

### 19.1 The event tap — `mouse_handler` (`mouse_handler.c:21`)

```rust
pub unsafe extern "C-unwind" fn mouse_handler(
    proxy: CGEventTapProxy,
    r#type: CGEventType,
    event: NonNull<CGEvent>,
    context: *mut c_void,
) -> *mut CGEvent;
```

Matches `objc2_core_graphics::CGEventTapCallBack`, which is
`Option<unsafe extern "C-unwind" fn(CGEventTapProxy, CGEventType, NonNull<CGEvent>, *mut c_void) -> *mut CGEvent>`;
the call site passes `Some(mouse_handler)`.

Runs on the **main thread**, installed by `mouse_handler_begin` (`mouse_handler.c:274-290`) with
`CGEventTapCreate(kCGHIDEventTap, kCGHeadInsertEventTap, kCGEventTapOptionDefault, mask, ...)`,
then `CGEventTapIsEnabled`, `CFMachPortCreateRunLoopSource` and
`CFRunLoopAddSource(CFRunLoopGetMain(), source, kCFRunLoopCommonModes)`.

`context` is `&g_mouse_state` in C. Under `DECISIONS.md` 23 the half this callback reads —
`modifier`, the two actions, the drop action, the tap handle and the click-consumption flags — is
a static of atomics, so the Rust passes `core::ptr::null_mut()` as the refcon and reads the
static. The callback returns the event pointer unchanged except in the two arms that consume the click
and return `NULL` (`mouse_handler.c:39`, `:55`), exactly as the C does, and `CFRetain`s the event
before every `event_loop_post` (`mouse_handler.c:34,44,61,67`) — that retain becomes the `Event`
variant owning a `CFRetained<CGEvent>` under `DECISIONS.md` 19. The `consumed_event` held across
a click (retained at `:38`, released at `:54`) is a second retained reference and is part of the click-consumption state
of `DECISIONS.md` 23.

The `kCGSEventDockControl` arm (`mouse_handler.c:69-83`) writes `__pending_gesture` and
`__last_gesture_time`, both `AtomicBool`/`AtomicU64` statics with `Release` stores.

### 19.2 The AX observers — `application_notification_handler` (`application.c:6`) and `mission_control_notification_handler` (`mission_control.c:59`)

```rust
pub unsafe extern "C-unwind" fn application_notification_handler(
    observer: NonNull<AXObserver>,
    element: NonNull<AXUIElement>,
    notification: NonNull<CFString>,
    context: *mut c_void,
);
```

Matches `AXObserverCallback`. Both run on the **main thread**, driven by the run-loop source
`AXObserverGetRunLoopSource` adds at `application.c:57` and `mission_control.c:87`, both in
`kCFRunLoopDefaultMode`.

`mission_control_notification_handler` passes `NULL` as its refcon at every
`AXObserverAddNotification` (`mission_control.c:81-84`) and only compares `notification` against
the four `kAXExpose*` strings with `CFEqual`, so it touches nothing but the event queue.

`application_notification_handler`'s refcon is per-notification: `application_observe`
(`application.c:45-48`) passes the `struct application *`, and the per-window notifications pass
a `struct window *`. Under `DECISIONS.md` 20 a refcon carries either an integer id or a raw
`Arc` pointer to an immutable-plus-atomic cell, never a pointer into event-loop-owned memory,
and that `Arc` is released on the main queue after the notification is removed. The
`kAXUIElementDestroyedNotification` arm's `__sync_bool_compare_and_swap(&window->id_ptr, &window->id, NULL)`
(`application.c:35`) is the claim-for-destruction compare-exchange of `DECISIONS.md` 21, and its
`NOTE(asmvik)` comment carries over verbatim.

### 19.3 The Carbon process handler — `process_handler` (`process_manager.c:151`)

```rust
pub unsafe extern "C-unwind" fn process_handler(
    r#ref: EventHandlerCallRef,
    event: EventRef,
    context: *mut c_void,
) -> OSStatus;
```

Matches the hand-declared `EventHandlerProcPtr` (§10.1). **Main thread.** Installed once by
`process_manager_begin` (`process_manager.c:252`) with the `ProcessManager` itself as refcon.

It returns `-1` when `GetEventParameter` fails (`process_manager.c:157`) and `noErr` otherwise,
including on every early return — that difference is observable to HIToolbox and is kept.

### 19.4 The SkyLight connection notify proc — `connection_handler` (`mission_control.c:7`)

```rust
pub unsafe extern "C-unwind" fn connection_handler(
    r#type: u32,
    data: *mut c_void,
    data_length: usize,
    context: *mut c_void,
    cid: c_int,
);
```

Matches `ConnectionCallback` (§3.1). **Main thread.** Registered up to six times with a
different `event` number each and a `NULL` context (`yabai.c:322-333`); which registrations
happen depends on the macOS version.

`data` is read with `memcpy` into a `u64` for types 1327/1328 and into a `u32` for 804/808
(`mission_control.c:12-22`); the Rust uses `core::ptr::read_unaligned` on `data.cast::<u64>()`
and `data.cast::<u32>()`, because nothing promises the SkyLight buffer is aligned. Type 1202
stores `read_os_timer()` into `__last_cmd_tab_time` with a `Release` store.

### 19.5 Display reconfiguration — `display_handler` (`display.c:6`)

```rust
pub unsafe extern "C-unwind" fn display_handler(
    did: CGDirectDisplayID,
    flags: CGDisplayChangeSummaryFlags,
    context: *mut c_void,
);
```

Matches `objc2_core_graphics::CGDisplayReconfigurationCallBack`. **Main thread.** Registered once
at `display_manager.c:505` with a `NULL` context. The four flag tests are `if / else if` in that
order — `Add`, `Remove`, `Moved`, `DesktopShapeChanged` — and the order is observable when two
flags arrive together, so it is kept as a chain, not a match.

### 19.6 The CVDisplayLink output callback — `window_manager_animate_window_list_thread_proc` (`window_manager.c:536`)

```rust
pub unsafe extern "C-unwind" fn window_manager_animate_window_list_thread_proc(
    link: NonNull<CVDisplayLink>,
    now: NonNull<CVTimeStamp>,
    output_time: NonNull<CVTimeStamp>,
    flags: CVOptionFlags,
    flags_out: NonNull<CVOptionFlags>,
    data: *mut c_void,
) -> CVReturn;
```

Matches `CVDisplayLinkOutputCallback`. Runs on a **CoreVideo-owned thread**, not the main thread
and not the event loop. `data` is the `Arc<AnimationContext>` of `DECISIONS.md` 24, handed over
as a raw pointer by `Arc::into_raw` when the link is started (`window_manager.c:701`) and
reclaimed with `Arc::from_raw` in the `t == 1.0` arm that tears the animation down
(`window_manager.c:591-596`).

Everything this callback touches is atomic or mutex-guarded by `DECISIONS.md` 24: `skip` is an
`AtomicBool` read `Relaxed`, `tx/ty/tw/th` are `AtomicU32` holding `f32` bits, and
`window_animations_table` is taken under its `Mutex` for exactly the C's
`pthread_mutex_lock`/`unlock` span (`window_manager.c:577-589`).

It returns `kCVReturnSuccess` on every path, through the `out:` label.

### 19.7 The array comparator — `display_manager_coordinate_comparator` (`display_manager.c:140`)

```rust
pub unsafe extern "C-unwind" fn display_manager_coordinate_comparator(
    a: *const c_void,
    b: *const c_void,
    context: *mut c_void,
) -> CFComparisonResult;
```

Matches `objc2_core_foundation::CFComparatorFunction`. Runs **synchronously inside
`CFArraySortValues`** on whichever thread called it — the event-loop thread, from
`display_manager_display_id_arrangement` (`display_manager.c:164`, the sort at `:180`). The C's
parameters are `CFTypeRef`, which is `*const c_void` in the objc2 alias; the body casts each to
`*const CFString` and calls `display_id`. `context` is not a pointer at all — the C passes
`(void *)(uintptr_t) g_display_manager.order` — so the Rust reads it back with
`context as usize as u32` and turns it into the `DisplayArrangementOrder` enum.

The comparison is two-level on `f32` coordinates (`display_manager.c:140-161`): the primary axis
first, then the other axis as a tie-break, each returning `kCFCompareLessThan` /
`kCFCompareGreaterThan` in that order of tests, and `kCFCompareEqualTo` only when both are
equal. `display_center` is called for each side on every comparison, as in the C.

---

## 20. `workspace_context`, rebuilt with `define_class!`

`src/workspace.m:153-303` is the only real Objective-C class in the daemon. It is reconstructed
with `objc2::define_class!`, and **the class name and every selector stay byte-identical**:
`NSNotificationCenter` dispatches by selector on the instance, and the class name shows up in
crash logs.

```rust
define_class!(
    #[unsafe(super(NSObject))]
    #[name = "workspace_context"]
    #[ivars = WorkspaceContextIvars]
    pub struct workspace_context;

    impl workspace_context {
        #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
        fn observeValueForKeyPath_ofObject_change_context(
            &self,
            key_path: &NSString,
            object: &AnyObject,
            change: &NSDictionary,
            context: *mut c_void,
        ) { ... }

        #[unsafe(method(didWake:))]
        fn didWake(&self, notification: &NSNotification) { ... }

        #[unsafe(method(didChangeMenuBarHiding:))]
        fn didChangeMenuBarHiding(&self, notification: &NSNotification) { ... }

        #[unsafe(method(didRestartDock:))]
        fn didRestartDock(&self, notification: &NSNotification) { ... }

        #[unsafe(method(didChangeDockPref:))]
        fn didChangeDockPref(&self, notification: &NSNotification) { ... }

        #[unsafe(method(activeDisplayDidChange:))]
        fn activeDisplayDidChange(&self, notification: &NSNotification) { ... }

        #[unsafe(method(activeSpaceDidChange:))]
        fn activeSpaceDidChange(&self, notification: &NSNotification) { ... }

        #[unsafe(method(didHideApplication:))]
        fn didHideApplication(&self, notification: &NSNotification) { ... }

        #[unsafe(method(didUnhideApplication:))]
        fn didUnhideApplication(&self, notification: &NSNotification) { ... }
    }
);
```

Nine selectors: the KVO callback plus the eight notification handlers. The `#[name]` attribute is
what keeps the runtime name `workspace_context` rather than a Rust-derived one.

### 20.1 What `-init` becomes

`workspace.m:154-199` registers eight observers across three notification centres:

| Selector | Centre | Name |
| --- | --- | --- |
| `activeDisplayDidChange:` | `[[NSWorkspace sharedWorkspace] notificationCenter]` | `"NSWorkspaceActiveDisplayDidChangeNotification"` (string literal) |
| `activeSpaceDidChange:` | `[[NSWorkspace sharedWorkspace] notificationCenter]` | `NSWorkspaceActiveSpaceDidChangeNotification` |
| `didHideApplication:` | `[[NSWorkspace sharedWorkspace] notificationCenter]` | `NSWorkspaceDidHideApplicationNotification` |
| `didUnhideApplication:` | `[[NSWorkspace sharedWorkspace] notificationCenter]` | `NSWorkspaceDidUnhideApplicationNotification` |
| `didWake:` | `[[NSWorkspace sharedWorkspace] notificationCenter]` | `NSWorkspaceDidWakeNotification` |
| `didChangeMenuBarHiding:` | `[NSDistributedNotificationCenter defaultCenter]` | `"AppleInterfaceMenuBarHidingChangedNotification"` (string literal) |
| `didRestartDock:` | `[NSNotificationCenter defaultCenter]` | `"NSApplicationDockDidRestartNotification"` (string literal) |
| `didChangeDockPref:` | `[NSDistributedNotificationCenter defaultCenter]` | `"com.apple.dock.prefchanged"` (string literal) |

All eight pass `object: nil`. Registration order is preserved. In Rust the eight
`addObserver:selector:name:object:` calls sit in an inherent `fn init(this: Allocated<Self>) -> Retained<Self>`
that calls `NSObject`'s `init` first, matching `if ((self = [super init]))`.

`workspace_event_handler_begin` (`workspace.m:1-15`) does `[workspace_context alloc]`, checks for
nil, then `[ws_context init]` **discarding the result** and storing the `alloc` pointer. For an
`NSObject` subclass the two agree, but the Rust must not rely on that: it stores what `init`
returns. The version booleans are assigned *before* the allocation, so a nil allocation still
leaves them set — keep that order.

`-dealloc` (`workspace.m:201-207`) removes the observer from all three centres. The instance is
never released in the C; it lives for the process. In Rust it is a
`OnceLock<Retained<workspace_context>>` and a `OnceLock` in a `static` is never dropped, so the
method stays unreachable — but it is written anyway, as the C has it.

It is written as `Drop`, not as a selector. `#[unsafe(method(dealloc))]` inside a `define_class!`
block is a `compile_error!` in objc2 0.6.4 — `objc2-0.6.4/src/macros/define_class.rs:1464-1472`
answers both `method(dealloc)` and `method_id(dealloc)` with
"is not supported. Implement `Drop` for the type instead" — and `define_class.rs:41-42` gives the
supported route: "If the type implements `Drop`, the macro will generate a `dealloc` method for
you, which will call `drop` automatically." So the three `removeObserver:` calls of
`workspace.m:202-204` go in the body of `impl Drop for workspace_context`, in the C's order, and
the `[super dealloc]` of `workspace.m:205` is emitted by the macro and is never written by hand:

```rust
impl Drop for workspace_context {
    fn drop(&mut self) {
        unsafe {
            NSWorkspace::sharedWorkspace().notificationCenter().removeObserver(self);
            NSNotificationCenter::defaultCenter().removeObserver(self);
            NSDistributedNotificationCenter::defaultCenter().removeObserver(self);
        }
    }
}
```

### 20.2 The ivars and the raw context pointer

`@interface workspace_context : NSObject { }` has no ivars, so `#[ivars = ()]` would be the
literal transposition. It is `WorkspaceContextIvars` instead because the KVO callback needs the
event-queue sender, which the C reaches through the global `g_event_loop` (`workspace.m:152`) and
which `DECISIONS.md` 12 moves into `EventLoopOwnedState`:

```rust
pub struct WorkspaceContextIvars {
    event_sender: std::sync::mpsc::Sender<Event>,
}
```

The channel exists from `event_loop_begin` onward (`DECISIONS.md` 12), so the sender can be
cloned into the ivars when the context is built at `yabai.c:295` and every `event_loop_post` in
this file becomes `self.ivars().event_sender.send(...)`.

The `void *context` the KVO callback receives is a different thing and stays a raw pointer. Under
`DECISIONS.md` 22 `struct process` is an `Arc<Process>` with atomic `terminated` and
`ns_application`, and `DECISIONS.md` 20 makes the refcon an **owned** raw `Arc` pointer. So
each `addObserver:forKeyPath:options:context:` mints its own strong count:

```rust
let context = Arc::into_raw(Arc::clone(&process)) as *mut c_void;
```

One owned strong count per live observation — `THREADS.md` §6.3 and its invariant 5. The callback
casts back with `unsafe { &*(context as *const Process) }` and does **not** reclaim: the borrow
leaves the count untouched.

`removeObserver:forKeyPath:context:` is handed the same address and mints nothing, which is what
the §22 helper's `(process as *const Process).cast_mut().cast::<c_void>()` already produces —
`Arc::into_raw` and the address of the `Process` inside the `Arc` are the same pointer value.
A removal that returns without throwing drops exactly one count; a removal that throws drops
none, because the observation is still registered and the refcon must stay valid. That is why
the §22 helper reports whether the removal returned normally, and
`release_kvo_refcon_on_main_queue` (§21.1) — the one place `Arc::from_raw` is written for a KVO
refcon — schedules the reclaimed `Arc<Process>`'s drop on the main queue, never taking it
directly on the event-loop thread (`DECISIONS.md` 20, `THREADS.md` §5.5).

Nothing else keeps that `Arc` alive, and the C shows why the accounting has to be per
observation rather than per table entry. `process_manager.c:189-191` runs
`__atomic_store_n(&process->terminated, …)` (`:189`), then `table_remove(&pm->process, &psn)`
(`:190`), then `workspace_application_unobserve(g_workspace_context, process)` (`:191`) — the
table entry goes **first**, the observation second. And the observation may well survive that
call: the two `:WorstApiEverMade` blocks at `workspace.m:40-54` and `:217-224` record that
`removeObserver:` throws while the observation is still registered, and
`workspace_application_unobserve` (`workspace.m:89-101`) swallows the exception. The observations
registered on the event-loop thread at `event_loop.c:104` and `:123`,
and on the main thread at `window_manager.c:2753`, therefore outlive the process record whenever
that happens, and `process_destroy` at `event_loop.c:344` would take the last count with the
event payload. A borrowed `Arc::as_ptr` refcon would leave the next
`-observeValueForKeyPath:ofObject:change:context:` (`workspace.m:209`) reading `terminated`
(`:213`) and `policy` (`:216`) through freed memory. The owned count is what closes that window.

### 20.3 The KVO callback

`workspace.m:209-259`, two independent `if` blocks — not `else if`, so both can run for one
notification, and that is preserved.

```rust
#[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
fn observeValueForKeyPath_ofObject_change_context(
    &self,
    key_path: &NSString,
    object: &AnyObject,
    change: &NSDictionary,
    context: *mut c_void,
) {
    let process: &Process = unsafe { &*(context as *const Process) };

    if key_path.isEqualToString(ns_string!("activationPolicy")) {
        if process.terminated.load(Ordering::Acquire) {
            return;
        }
        ...
    }

    if key_path.isEqualToString(ns_string!("finishedLaunching")) {
        ...
    }
}
```

The C reads `process->terminated`, a `volatile bool`, with a plain load; `DECISIONS.md` 22 makes
it an `AtomicBool` and the load is `Acquire`, pairing with the `Release` store at
`process_manager.c:189`.

`[change objectForKey:NSKeyValueChangeNewKey]` then `[result intValue]` becomes
`change.objectForKey(unsafe { NSKeyValueChangeNewKey })` downcast to `NSNumber` and `.intValue()`.
The `activationPolicy` arm compares it against `process.policy` and writes the new value back;
the `finishedLaunching` arm compares it against `1`.

Both arms call `removeObserver:forKeyPath:context:` on `object` inside a `@try/@catch` (§22)
before posting `APPLICATION_LAUNCHED`, and both carry the `:WorstApiEverMade` `NOTE(asmvik)`
comment verbatim. A removal that returns normally retires the strong count that arm's
`addObserver:` minted, through `release_kvo_refcon_on_main_queue` (§20.2, §21.1); one that throws
retires nothing. The callback itself never touches the count it was handed — `context` is
borrowed for the duration of the call and nothing more.

**The callback can re-enter synchronously.** `addObserver:` is called with
`NSKeyValueObservingOptionInitial | NSKeyValueObservingOptionNew` (`workspace.m:73,83`), so KVO
fires once on the registering thread before `addObserver:` returns. `DECISIONS.md` 22 is the
consequence: the process table's `Mutex` is never held across an Objective-C or AX call.

### 20.4 The version detection

`workspace.h:4-19` is an X-macro over six `(name, major_version)` pairs generating six statics
and six accessors. Under `DECISIONS.md` 31 it becomes one `macro_rules!`:

```rust
macro_rules! supported_macos_version_list {
    ($($accessor_name:ident, $flag_name:ident, $major_version:literal;)*) => {
        $(
            static $flag_name: OnceLock<bool> = OnceLock::new();

            pub fn $accessor_name() -> bool {
                *$flag_name.get().unwrap_or(&false)
            }
        )*

        pub fn set_supported_macos_version_flags(major_version: isize) {
            $(
                let _ = $flag_name.set(major_version == $major_version);
            )*
        }
    };
}

supported_macos_version_list! {
    workspace_is_macos_tahoe,    WORKSPACE_IS_MACOS_VERSION_TAHOE,    26;
    workspace_is_macos_sequoia,  WORKSPACE_IS_MACOS_VERSION_SEQUOIA,  15;
    workspace_is_macos_sonoma,   WORKSPACE_IS_MACOS_VERSION_SONOMA,   14;
    workspace_is_macos_ventura,  WORKSPACE_IS_MACOS_VERSION_VENTURA,  13;
    workspace_is_macos_monterey, WORKSPACE_IS_MACOS_VERSION_MONTEREY, 12;
    workspace_is_macos_bigsur,   WORKSPACE_IS_MACOS_VERSION_BIGSUR,   11;
}
```

`macro_rules!` cannot build an identifier from two pieces, so the accessor name and the static's
name are both spelled out at the call site rather than derived from `name` the way the C's
`##` does. The six accessors are `workspace_is_macos_tahoe()` … `workspace_is_macos_bigsur()`
over six `OnceLock<bool>`
(`DECISIONS.md` 18 — written once on the main thread at `workspace.m:4-6`, read from the
event-loop thread thereafter). The comparison is `version.majorVersion == major_version`, so at
most one is true and all six are false on an unknown major.

`workspace_use_macos_space_workaround` (`workspace.m:17-26`) deliberately queries
`[[NSProcessInfo processInfo] operatingSystemVersion]` **live** on every call rather than reading
the cached booleans, because it needs the minor version. That stays a live query.

### 20.5 `workspace_display_notch_height`

`workspace.m:125-138`. `__builtin_available(macos 12.0, *)` has no Rust equivalent, and
`objc2-app-kit` exposes `-safeAreaInsets` unconditionally, so calling it on macOS 11 would raise
an unrecognised selector. The guard becomes `if !workspace_is_macos_bigsur() { ... }`, which is
true for exactly the versions `__builtin_available(macos 12.0, *)` admits, given the six version
flags cover 11 through 26 and an unknown major leaves all six false.

The iteration matches each screen's `deviceDescription` `"NSScreenNumber"` against `did` with
`unsignedIntValue` and returns `screen.safeAreaInsets().top as c_int`.

**This function runs off the main thread, and the `MainThreadMarker` is minted at the call site.**
`workspace_display_notch_height` has exactly one caller, `display_bounds_constrained`
(`display.c:141`, inside the function at `display.c:123`), and every caller of *that* is
event-loop-thread code by `THREADS.md` §1.4: `view.c:971`, `window_manager.c:2141`, `:2397`,
`:2420`, `event_loop.c:1257` and `:1440`. Meanwhile `objc2-app-kit` declares `NSScreen` with
`#[thread_kind = MainThreadOnly]` and `pub fn screens(mtm: MainThreadMarker) -> Retained<NSArray<NSScreen>>`.
`MainThreadMarker` is neither `Send` nor `Sync` — `objc2-0.6.4/src/main_thread_marker.rs:199-205`
gives it a `PhantomData<*mut ()>` for exactly that purpose — so no marker can be threaded in from
`main`, and `MainThreadMarker::new()` returns `None` here because this is not the main thread.

The marker is therefore created with `MainThreadMarker::new_unchecked()` inside this function and
used for nothing else. That is the use objc2 documents for it at
`main_thread_marker.rs:247-250`: "you may create this briefly if you know that an API is safe in
a specific case, but is not marked so." The C already makes this call from the event-loop thread
(`workspace.m:130`), and `DECISIONS.md` 3 keeps the behaviour to the pixel, so the transposition
reproduces it rather than hopping to the main queue — a hop would change when the frame is
computed relative to every other event-loop step. §28 records it.

The `unsafe` block is the `NSScreen::screens` call itself, which is `DECISIONS.md` 39's "FFI
calls" category: the marker is the token `objc2-app-kit` demands in order to make that one call,
it is created on the line above it, and it is never stored, returned or passed anywhere else.

```rust
pub fn workspace_display_notch_height(did: u32) -> c_int {
    if !CGDisplayIsBuiltin(did) { return 0; }

    if !workspace_is_macos_bigsur() {
        let screen_list = unsafe {
            let main_thread_marker = MainThreadMarker::new_unchecked();
            NSScreen::screens(main_thread_marker)
        };

        for screen in screen_list {
            let device_description = screen.deviceDescription();
            let Some(screen_number) =
                device_description.objectForKey(ns_string!("NSScreenNumber"))
            else {
                continue;
            };
            let Some(screen_number) = screen_number.downcast_ref::<NSNumber>() else {
                continue;
            };
            if screen_number.unsignedIntValue() == did {
                return screen.safeAreaInsets().top as c_int;
            }
        }
    }

    0
}
```

`-deviceDescription` is `#[cfg(feature = "NSGraphics")]` in the crate, which is why §1.1 carries
that feature.

---

## 21. `NSRunningApplication`, retain/release, autorelease pools

### 21.1 The atomic pointer

`DECISIONS.md` 22 makes `struct process` an `Arc<Process>` whose `ns_application` is atomic. In C
it is a `void *` written with `__ATOMIC_RELEASE` (`process_manager.c:66`, `event_loop.c:87`) and
read with `__ATOMIC_RELAXED` everywhere else (`workspace.m:35,71,81,91,105,117`). The Rust field
is:

```rust
pub struct Process {
    pub psn: ProcessSerialNumber,
    pub pid: pid_t,
    pub name: String,
    pub ns_application: AtomicPtr<NSRunningApplication>,
    pub policy: AtomicI32,
    pub terminated: AtomicBool,
}
```

The pointer is **owning**: the C's `[[NSRunningApplication runningApplicationWithProcessIdentifier:pid] retain]`
(`workspace.m:30`) stores a `+1` reference, and `[application release]` at `workspace.m:65` gives
it back. objc2 retains the result of a non-`new`/`alloc`/`copy` selector automatically, so
`NSRunningApplication::runningApplicationWithProcessIdentifier` already hands back a
`Retained<NSRunningApplication>` at `+1`, and the explicit `retain` in the C has no counterpart
to write:

```rust
pub fn workspace_application_create_running_ns_application(
    process: &Process,
) -> *mut NSRunningApplication {
    match unsafe {
        NSRunningApplication::runningApplicationWithProcessIdentifier(process.pid)
    } {
        Some(running_application) => Retained::into_raw(running_application),
        None => core::ptr::null_mut(),
    }
}
```

and the destroy side takes ownership back exactly once, which is what makes the release happen:

```rust
pub fn workspace_application_destroy_running_ns_application(
    workspace_context: &workspace_context,
    process: &Process,
) {
    let raw = process.ns_application.load(Ordering::Relaxed);
    let Some(running_application) = NonNull::new(raw) else {
        return;
    };
    let running_application: Retained<NSRunningApplication> =
        unsafe { Retained::from_raw(running_application.as_ptr()).unwrap() };

    if unsafe { running_application.observationInfo() }.is_some() {
        //
        // :WorstApiEverMade
        //
        if remove_observer_swallowing_exception(
            &running_application, workspace_context, ns_string!("activationPolicy"), process) {
            release_kvo_refcon_on_main_queue(process);
        }
        if remove_observer_swallowing_exception(
            &running_application, workspace_context, ns_string!("finishedLaunching"), process) {
            release_kvo_refcon_on_main_queue(process);
        }
    }
}
```

The whole `NOTE(asmvik)` block of `workspace.m:40-54` sits under that `:WorstApiEverMade` line
verbatim; the sketch abbreviates it to keep the shape readable.

The `Retained` drops at the end of the function, which is the `[application release]`. The C
does **not** null the field afterwards and neither does the Rust — the process is being destroyed
and nothing reads it again.

`release_kvo_refcon_on_main_queue` is the one place `Arc::from_raw` is written for a KVO refcon:
it takes the `Process`'s address, and schedules the reclaimed `Arc<Process>`'s drop on the main
queue (`DECISIONS.md` 20). Both `workspace_application_unobserve` (`workspace.m:89-101`) and the
two removals inside the KVO callback (`workspace.m:227`, `:252`) pair with it the same way.

`workspace_application_is_observable` (`workspace.m:103-113`) writes `process->policy` in both
branches, including the `Prohibited` value when there is no application, and the Rust keeps that:
the field is an `AtomicI32` and the store is `Relaxed`, matching the C's plain write.

### 21.2 Autorelease pools

`misc/autorelease.h` is dead code (commented out of `manifest.m:49`, `#if 0` at
`yabai.c:158-162`) and is not ported (`DECISIONS.md` 5).

The five live pools all have a lexical scope, so every one becomes
`objc2::rc::autoreleasepool(|_pool| { ... })` — the same `objc_autoreleasePoolPush`/`Pop` pair
that `[[NSAutoreleasePool alloc] init]` / `[pool drain]` compiles to. `NSAutoreleasePool` itself
is never named in the Rust.

| C site | Scope |
| --- | --- |
| `event_loop.c:1653` … `:1677` | one iteration of the outer `while (event_loop->is_running)`: the pool is created at the top and drained at the `empty:` label, immediately before `sem_wait`. This is `DECISIONS.md` 19's "drained when the queue runs empty". The `for (;;)` that drains the queue sits inside the closure. |
| `window_manager.c:2739` … `:2756` | the `table_for` loop over every process in `window_manager_begin`; the code after `[pool drain]` is outside |
| `process_manager.c:242` … `:244` | exactly `process_manager_add_running_processes` |
| `sa.m:176` … `:189` | the whole body of `scripting_addition_check`; the closure returns the `int` result |
| `sa.m:372` … `:414` | the whole body of `scripting_addition_load`, whose four `goto out`s become early returns from the closure |
| `notify.h:31` … `:47` | the whole body of `notify` |

---

## 22. The four `@try/@catch` sites

Four functions, eight `@try` blocks, every one of them
`@try { [... removeObserver:...]; } @catch (NSException * __unused exception) {}`. All four are
about the same defect, which the C documents in two `:WorstApiEverMade` comments carried over
verbatim: `removeObserver:forKeyPath:context:` raises when the key path is not registered, and
there is no way to ask whether it is.

| Function | `@try` blocks | Key paths |
| --- | --- | --- |
| `workspace_application_destroy_running_ns_application` (`workspace.m:33-67`) | `:56`, `:60` | `activationPolicy`, `finishedLaunching`, both guarded by `[application observationInfo]` |
| `workspace_application_unobserve` (`workspace.m:89-101`) | `:93`, `:97` | the same two, **not** guarded by `observationInfo` |
| `-observeValueForKeyPath:ofObject:change:context:` (`workspace.m:209-259`) | `:226`, `:251` | the key path that just fired, removed from `object` |
| `EVENT_HANDLER(APPLICATION_LAUNCHED)` (`event_loop.c:82-140`) | `:112`, `:131` | `finishedLaunching`, `activationPolicy`, each re-checked after subscribing, each guarded by `application && [application observationInfo]` |

One helper covers all eight, and it is the only place `objc2::exception::catch` appears:

```rust
fn remove_observer_swallowing_exception(
    application: &NSRunningApplication,
    workspace_context: &workspace_context,
    key_path: &NSString,
    process: &Process,
) -> bool {
    let context = (process as *const Process).cast_mut().cast::<c_void>();
    objc2::exception::catch(std::panic::AssertUnwindSafe(|| unsafe {
        application.removeObserver_forKeyPath_context(workspace_context, key_path, context);
    }))
    .is_ok()
}
```

`objc2::exception::catch` is
`pub fn catch<R>(closure: impl FnOnce() -> R + UnwindSafe) -> Result<R, Option<Retained<Exception>>>`
and is safe to call. The C discards the exception and so does this — the `Retained<Exception>`
releases on drop, and nothing reads it. What the helper does return is whether the removal came
back normally, because §20.2's refcon accounting turns on exactly that: a `true` drops the one
strong count the matching `addObserver:` minted, a `false` keeps it, since the observation is
still registered. Raw pointers are `UnwindSafe`, but `&`-references to objc2 types are not, so
the closure is wrapped in `AssertUnwindSafe` — there is no state to corrupt, because the
closure's only effect is the call itself.

`event_loop.c`'s two sites call the same helper; `workspace.m:226/251`'s sites take the `object`
the KVO callback was handed rather than the process's own `ns_application`, so they take an
`&AnyObject` instead and are otherwise identical.

**This is what `panic = "unwind"` buys.** Under `panic = "abort"` the Objective-C exception has
no unwinder and the process dies here (`DECISIONS.md` 7).

---

## 23. `dispatch_after`

Five sites, all `dispatch_after(dispatch_time(DISPATCH_TIME_NOW, <delay>), dispatch_get_main_queue(), ^{ ... })`,
all posting to the event loop from the main queue. `0.1f * NSEC_PER_SEC` evaluates to exactly
`100000000` in `f32`, so the Rust passes the integer:

| C site | Delay | Block body |
| --- | --- | --- |
| `event_loop.c:93` | `0.1f * NSEC_PER_SEC` | captures `__block ProcessSerialNumber psn`; looks the process up again and posts `APPLICATION_LAUNCHED` if it is still there |
| `event_loop.c:157` | `0.1f * NSEC_PER_SEC` | identical body, the AX-retry path |
| `event_loop.c:1478` | `0.1f * NSEC_PER_SEC` | posts `MISSION_CONTROL_CHECK_FOR_EXIT` |
| `event_loop.c:1516` | `0.1f * NSEC_PER_SEC` | posts `MISSION_CONTROL_CHECK_FOR_EXIT` |
| `event_loop.c:1520` | `0.0f` | posts `MISSION_CONTROL_EXIT` |

```rust
dispatch_after_on_main_queue(100_000_000, move || {
    let Some(process) = process_manager_find_process(&process_manager, &process_serial_number)
    else {
        return;
    };
    event_sender.send(Event::ApplicationLaunched(process)).ok();
});
```

The `__block ProcessSerialNumber psn` is a `Copy` struct captured by value with `move`; the
`__block` qualifier exists in the C only so the block copies it rather than the enclosing
`process` pointer, which is exactly what a `move` capture does.

The `0.0f` case stays `dispatch_after_on_main_queue(0, ...)` and is **not** rewritten as
`exec_async`. `dispatch_after` with a zero delta and `dispatch_async` enqueue differently against
already-queued work, and `MISSION_CONTROL_EXIT` must land after the `CHECK_FOR_EXIT` chain
(`event_loop.c:1516-1522`).

The block bodies must not touch event-loop-owned state, because they run on the main queue: they
look the process up through the process table of `DECISIONS.md` 22 and then send on the channel.
That is what the C's `process_manager_find_process(&g_process_manager, &psn)` already does — the
re-lookup, rather than a captured pointer, is the whole point of the `__block psn`.

---

## 24. The dynamic `objc_msgSend` call

`space_manager.c:667-673` and `:688-694` — the same four lines twice, once for a window list and
once for a single window:

```c
Class cls = objc_getClass("SLSBridgedMoveWindowsToManagedSpaceOperation");
SEL sel = sel_registerName("initWithWindows:spaceID:");
id operation = ((id (*)(id, SEL, id, uint64_t))objc_msgSend)([cls alloc], sel, (__bridge id)window_list_ref, sid);
SLSPerformAsynchronousBridgedWindowManagementOperation(operation);
[operation release];
```

The class is not known at compile time and the selector is built at run time, so `msg_send!`
cannot express it. The port keeps the cast, which is correct on both architectures because every
argument is pointer-sized or `u64` and the return is a pointer — `objc_msgSend` only needs the
`_stret` / `_fpret` variants for struct and floating-point returns, and there are none here:

```rust
type InitWithWindowsSpaceIdFn =
    unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, u64) -> *mut AnyObject;

fn space_manager_move_window_list_to_space(sid: u64, window_list: &[u32]) {
    if let Some(perform_operation) =
        sls_perform_asynchronous_bridged_window_management_operation()
    {
        let window_list_ref = cfarray_of_cfnumbers(window_list, kCFNumberSInt32Type);
        let Some(class) = AnyClass::get(c"SLSBridgedMoveWindowsToManagedSpaceOperation") else {
            return;
        };
        let selector = Sel::register(c"initWithWindows:spaceID:");
        let allocated: *mut AnyObject = unsafe { msg_send![class, alloc] };
        let init_with_windows_space_id: InitWithWindowsSpaceIdFn = unsafe {
            core::mem::transmute(objc2::ffi::objc_msgSend as *const c_void)
        };
        let operation = unsafe {
            init_with_windows_space_id(
                allocated,
                selector,
                CFRetained::as_ptr(&window_list_ref).as_ptr().cast::<AnyObject>(),
                sid,
            )
        };
        unsafe { perform_operation(operation.cast::<c_void>()) };
        unsafe { let _: () = msg_send![operation, release]; };
    } else if !workspace_use_macos_space_workaround() {
        ...
    } else if !scripting_addition_move_window_list_to_space(sid, window_list) {
        ...
    }
}
```

Three things the C does that the Rust must keep:

1. **The guard is the function pointer, not the class.** Both call sites test
   `if (SLSPerformAsynchronousBridgedWindowManagementOperation)` first; only if that resolved do
   they look the class up. `objc_getClass` returning NULL is not handled in the C at all — it
   would send `alloc` to nil, get nil back, and pass nil onwards. The Rust's
   `let Some(class) = ... else { return }` is a NULL-dereference removal under
   `DECISIONS.md` 4 and is one line in `DEVIATIONS.md`.
2. **`release`, not `Retained`.** `[cls alloc]` and `initWithWindows:spaceID:` return `+1`, and
   the C releases explicitly after the operation is handed off. Wrapping it in `Retained` would
   work, but the raw `msg_send![operation, release]` reads against the C and is what
   `DECISIONS.md` 2 asks for.
3. **The `CFArray` is released after the operation**, not before (`space_manager.c:672`), because
   `initWithWindows:` may not have copied it. The `CFRetained<CFArray>` therefore has to outlive
   `perform_operation`, which it does as a local.

The three-branch fallback chain — bridged operation, then `SLSMoveWindowsToManagedSpace` when
`workspace_use_macos_space_workaround()` is false, then the scripting addition with a
`SLSSpaceSetCompatID`/`SLSSetWindowListWorkspace` last resort — is reproduced exactly, including
the `0x79616265` compat id (§18.10).

---

## 25. The scripting-addition client

`src/osax/` stays C and is never edited (`DECISIONS.md` 1). `src/sa.m` becomes `src/sa.rs`, and
everything below is the daemon's half of the wire protocol.

### 25.1 `OUT_DIR/osax_common.rs`, generated by `build.rs`

`DECISIONS.md` 9 requires `build.rs` to generate this from `src/osax/common.h` and to **fail the
build if the header cannot be parsed**. The daemon and the payload must never disagree about the
version or an opcode (`sa.m:281` compares the handshake's version string against `OSAX_VERSION`),
and a duplicated literal would drift.

`build.rs` scans `src/osax/common.h` for the five `#define`s and the `enum sa_opcode` body,
writes the file below into `OUT_DIR`, and the daemon pulls it in with
`include!(concat!(env!("OUT_DIR"), "/osax_common.rs"));`:

```rust
pub const SA_SOCKET_PATH_FMT: &str = "/tmp/yabai-sa_%s.socket";
pub const SA_SOCKET_BUFF_LEN: usize = 0x1000;

pub const OSAX_VERSION: &str = "2.1.30";

pub const OSAX_ATTRIB_DOCK_SPACES: u32 = 0x01;
pub const OSAX_ATTRIB_DPPM: u32 = 0x02;
pub const OSAX_ATTRIB_ADD_SPACE: u32 = 0x04;
pub const OSAX_ATTRIB_REM_SPACE: u32 = 0x08;
pub const OSAX_ATTRIB_MOV_SPACE: u32 = 0x10;
pub const OSAX_ATTRIB_SET_WINDOW: u32 = 0x20;
pub const OSAX_ATTRIB_ANIM_TIME: u32 = 0x40;

pub const OSAX_ATTRIB_ALL: u32 = OSAX_ATTRIB_DOCK_SPACES
    | OSAX_ATTRIB_DPPM
    | OSAX_ATTRIB_ADD_SPACE
    | OSAX_ATTRIB_REM_SPACE
    | OSAX_ATTRIB_MOV_SPACE
    | OSAX_ATTRIB_SET_WINDOW
    | OSAX_ATTRIB_ANIM_TIME;

#[repr(u8)]
pub enum sa_opcode {
    SA_OPCODE_HANDSHAKE = 0x01,
    SA_OPCODE_SPACE_FOCUS = 0x02,
    SA_OPCODE_SPACE_CREATE = 0x03,
    SA_OPCODE_SPACE_DESTROY = 0x04,
    SA_OPCODE_SPACE_MOVE = 0x05,
    SA_OPCODE_WINDOW_MOVE = 0x06,
    SA_OPCODE_WINDOW_OPACITY = 0x07,
    SA_OPCODE_WINDOW_OPACITY_FADE = 0x08,
    SA_OPCODE_WINDOW_LAYER = 0x09,
    SA_OPCODE_WINDOW_STICKY = 0x0A,
    SA_OPCODE_WINDOW_SHADOW = 0x0B,
    SA_OPCODE_WINDOW_FOCUS = 0x0C,
    SA_OPCODE_WINDOW_SCALE = 0x0D,
    SA_OPCODE_WINDOW_SWAP_PROXY_IN = 0x0E,
    SA_OPCODE_WINDOW_SWAP_PROXY_OUT = 0x0F,
    SA_OPCODE_WINDOW_ORDER = 0x10,
    SA_OPCODE_WINDOW_ORDER_IN = 0x11,
    SA_OPCODE_WINDOW_LIST_TO_SPACE = 0x12,
    SA_OPCODE_WINDOW_TO_SPACE = 0x13,
}
```

`#[repr(u8)]` with explicit discriminants, per `DECISIONS.md` 31 — the values go on the wire.
`SA_SOCKET_PATH_FMT` keeps the C's `%s` because `g_sa_socket_file` is built with `snprintf`
(`yabai.c:135`, `sa.m:158`); the Rust formats it with the same single substitution.

### 25.2 The frame

`sa.m:418-420` is three macros:

```c
#define sa_payload_init() char bytes[SA_SOCKET_BUFF_LEN]; int16_t length = 1+sizeof(length)
#define pack(v) memcpy(bytes+length, &v, sizeof(v)); length += sizeof(v)
#define sa_payload_send(op) *(int16_t*)bytes = length-sizeof(length), bytes[sizeof(length)] = op, scripting_addition_send_bytes(bytes, length)
```

so the frame is:

```
offset 0   1   2   3 ...
      +---+---+---+--------------------------+
      | length (i16, native) | op |   args   |
      +---+---+---+--------------------------+
       \------ 2 -------/ \-1-/ \- length-3 -/
```

* the buffer is a fixed `[u8; SA_SOCKET_BUFF_LEN]`, i.e. 4096 bytes, on the stack;
* `length` starts at 3 — one opcode byte plus the two header bytes — and is the **number of bytes
  sent**;
* the `i16` written at offset 0 is `length - 2`: it counts the opcode but not itself;
* arguments are `memcpy`'d at the running offset: **unaligned, unpadded, native byte order**;
* `bytes` is never zeroed, and only `length` bytes are sent, all of them written.

`DECISIONS.md` 34 keeps the fixed 4096-byte buffer and the `i16` header, and adds the bounds
check the C does not have. One writer type carries all of it:

```rust
struct ScriptingAdditionPayload {
    bytes: [u8; SA_SOCKET_BUFF_LEN],
    length: i16,
}

impl ScriptingAdditionPayload {
    fn new() -> Self {
        Self { bytes: [0; SA_SOCKET_BUFF_LEN], length: 1 + core::mem::size_of::<i16>() as i16 }
    }

    fn pack(&mut self, value: &[u8]) -> bool {
        let offset = self.length as usize;
        if offset + value.len() > SA_SOCKET_BUFF_LEN {
            return false;
        }
        self.bytes[offset..offset + value.len()].copy_from_slice(value);
        self.length += value.len() as i16;
        true
    }

    fn send(mut self, opcode: sa_opcode) -> bool {
        self.bytes[0..2].copy_from_slice(&(self.length - 2).to_ne_bytes());
        self.bytes[2] = opcode as u8;
        scripting_addition_send_bytes(&self.bytes[..self.length as usize])
    }
}
```

Every `pack` call site is `if !payload.pack(&value.to_ne_bytes()) { return false; }`, which is
the "bounds check that fails the call" of `DECISIONS.md` 34. `to_ne_bytes` is the `memcpy`:
native byte order, no alignment requirement.

**What the check catches.** `scripting_addition_swap_window_proxy_in/out` writes 4 bytes for the
count and then 4 or 8 bytes per animation, so 512 animations need
`3 + 4 + 512 * 8 = 4103` bytes and overrun the 4096-byte stack buffer;
`scripting_addition_order_window_in` overruns past 1022 windows;
`scripting_addition_move_window_list_to_space` past 1021. All three are stack buffer overruns in
the C, and all three become a `false` return. One line in `DEVIATIONS.md` (`DECISIONS.md` 4).

### 25.3 The transport

`scripting_addition_send_bytes` (`sa.m:422-440`), one request per connection:

```rust
fn scripting_addition_send_bytes(bytes: &[u8]) -> bool {
    let Some(socket_file_descriptor) = socket_open() else {
        return false;
    };
    let mut result = false;
    if socket_connect(socket_file_descriptor, g_sa_socket_file()) {
        if unsafe {
            libc::send(socket_file_descriptor, bytes.as_ptr().cast(), bytes.len(), 0)
        } != -1
        {
            let mut dummy: u8 = 0;
            unsafe {
                libc::recv(socket_file_descriptor, (&raw mut dummy).cast(), 1, 0);
            }
            result = true;
        }
    }
    socket_close(socket_file_descriptor);
    result
}
```

* `AF_UNIX` / `SOCK_STREAM`, path `/tmp/yabai-sa_<user>.socket`;
* the **one-byte `recv` is the ack**, and its return value is ignored — the payload closes the
  connection after dispatching, so the `recv` returns 0, and that is what makes every scripting
  addition call synchronous with respect to Dock.app. It must not be dropped, and it must not be
  checked;
* `socket_close` is `shutdown(SHUT_RDWR)` then `close` (`helpers.h:198-202`);
* a failed `socket_open` skips the close entirely, as the C's `if` does.

### 25.4 The handshake

The only RPC with a response and the only frame not built by the macros
(`sa.m:239-268`):

```rust
let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
bytes[0] = 0x01;
bytes[1] = 0x00;
bytes[2] = sa_opcode::SA_OPCODE_HANDSHAKE as u8;
// send(sockfd, bytes, 3, 0)
```

The response is `"2.1.30\0" <u32 attrib, native> '\n'`, twelve bytes in one `send`. The C reads
up to `BUFSIZ - 1` bytes, scans to the first NUL, copies `[0 ..= nul]` into `version` and the
next four bytes into `attrib`; the trailing newline is ignored. The Rust scans for the NUL in
the bytes actually received and **bounds-checks that four more bytes follow**, where the C
happily reads past a short response — one line in `DEVIATIONS.md`.

`scripting_addition_perform_validation` (`sa.m:270-299`) then:

1. `string_equals(version, OSAX_VERSION)` and `(attrib & OSAX_ATTRIB_ALL) == OSAX_ATTRIB_ALL` →
   `notify("scripting-addition", "payload v%s", version)`, return 0;
2. version matches but attributes do not → `"payload (0x%X) doesn't support this macOS version!"`,
   return 1;
3. version differs and the installed bundle is stale → `"payload is outdated, updating.."`, then
   `scripting_addition_install()`;
4. version differs but the bundle is current → `"payload is outdated, restarting Dock.app.."`,
   `scripting_addition_restart_dock()`, return 0.

The `%X` and `%s` formats are `DECISIONS.md` 33 macros over the same format text.

### 25.5 Every RPC, with its field packing

Lengths below are argument bytes; the frame adds 3. `bool` packs as one byte.

| Opcode | `sa.m` fn | Packed fields, in order | Arg bytes |
| --- | --- | --- | --- |
| `0x02` `SPACE_FOCUS` | `scripting_addition_focus_space` `:442` | `u64 sid` | 8 |
| `0x03` `SPACE_CREATE` | `scripting_addition_create_space` `:449` | `u64 sid` | 8 |
| `0x04` `SPACE_DESTROY` | `scripting_addition_destroy_space` `:456` | `u64 sid` | 8 |
| `0x05` `SPACE_MOVE` | `scripting_addition_move_space_to_display` `:463` | `u64 src_sid`, `u64 dst_sid`, `u64 src_prev_sid`, `u8 focus` | 25 |
| `0x05` `SPACE_MOVE` | `scripting_addition_move_space_after_space` `:473` | `u64 src_sid`, `u64 dst_sid`, `u64 dummy_sid = 0`, `u8 focus` | 25 |
| `0x06` `WINDOW_MOVE` | `scripting_addition_move_window` `:484` | `u32 wid`, `i32 x`, `i32 y` | 12 |
| `0x07` `WINDOW_OPACITY` | `scripting_addition_set_opacity` `:493` | `u32 wid`, `f32 opacity`, `f32 duration` | 12 |
| `0x08` `WINDOW_OPACITY_FADE` | same fn | same three fields | 12 |
| `0x09` `WINDOW_LAYER` | `scripting_addition_set_layer` `:502` | `u32 wid`, `i32 layer` | 8 |
| `0x0A` `WINDOW_STICKY` | `scripting_addition_set_sticky` `:510` | `u32 wid`, `u8 sticky` | 5 |
| `0x0B` `WINDOW_SHADOW` | `scripting_addition_set_shadow` `:518` | `u32 wid`, `u8 shadow` | 5 |
| `0x0C` `WINDOW_FOCUS` | `scripting_addition_focus_window` `:526` | `u32 wid` | 4 |
| `0x0D` `WINDOW_SCALE` | `scripting_addition_scale_window` `:533` | `u32 wid`, `f32 x`, `f32 y`, `f32 w`, `f32 h` | 20 |
| `0x0E` `WINDOW_SWAP_PROXY_IN` | `scripting_addition_swap_window_proxy_in` `:544` | `i32 animation_count`, then the variable-stride list | 4 + Σ |
| `0x0F` `WINDOW_SWAP_PROXY_OUT` | `scripting_addition_swap_window_proxy_out` `:560` | identical | 4 + Σ |
| `0x10` `WINDOW_ORDER` | `scripting_addition_order_window` `:576` | `u32 a_wid`, `i32 order`, `u32 b_wid` | 12 |
| `0x11` `WINDOW_ORDER_IN` | `scripting_addition_order_window_in` `:586` | `i32 window_count`, then `window_count × u32` | 4 + 4n |
| `0x12` `WINDOW_LIST_TO_SPACE` | `scripting_addition_move_window_list_to_space` `:604` | `u64 sid`, `i32 window_count`, then `window_count × u32` | 12 + 4n |
| `0x13` `WINDOW_TO_SPACE` | `scripting_addition_move_window_to_space` `:615` | `u64 sid`, `u32 wid` | 12 |

Three details that are easy to lose:

* **`SPACE_MOVE` is one opcode with two C entry points.** `move_space_after_space` packs a
  `dummy_sid` of `0` where `move_space_to_display` packs `src_prev_sid`, so the frames are
  byte-identical in shape. Keep both functions.
* **`set_opacity` chooses the opcode from the argument**: `duration > 0.0f` selects
  `WINDOW_OPACITY_FADE`, otherwise `WINDOW_OPACITY` (`sa.m:499`). Both frames carry all three
  fields; the payload reads only the first two for `WINDOW_OPACITY`. The Rust packs all three in
  both cases.
* **`order_window_in` queries SkyLight while packing.** For each window it calls
  `SLSWindowIsOrderedIn(g_connection, window_list[i], &ordered_in)` and packs `0` instead of the
  id when the window is already ordered in (`sa.m:593-600`). `ordered_in` is declared once
  outside the loop and is not reset, so a failing `SLSWindowIsOrderedIn` leaves the previous
  window's answer in place; that is reproduced, with the variable declared once outside the loop.

### 25.6 The variable-stride proxy encoding

`sa.m:544-574`, the one encoding that is not a fixed record:

```rust
fn scripting_addition_swap_window_proxy_in(animation_list: &[WindowAnimation]) -> bool {
    let dummy_wid: u32 = 0;
    let mut payload = ScriptingAdditionPayload::new();
    if !payload.pack(&(animation_list.len() as i32).to_ne_bytes()) {
        return false;
    }
    for animation in animation_list {
        if animation.skip.load(Ordering::Relaxed) {
            if !payload.pack(&dummy_wid.to_ne_bytes()) {
                return false;
            }
        } else {
            if !payload.pack(&animation.wid.to_ne_bytes()) {
                return false;
            }
            if !payload.pack(&animation.proxy.id.to_ne_bytes()) {
                return false;
            }
        }
    }
    payload.send(sa_opcode::SA_OPCODE_WINDOW_SWAP_PROXY_IN)
}
```

A skipped animation contributes **four** bytes — a `u32` zero — and a live one contributes
**eight** — `wid` then `proxy.id`. The count field still says how many animations there are, so
the payload decodes by reading a `u32` and only reading a second one when the first is non-zero.
A window id of 0 can therefore never be sent, which is why the sentinel works.

`animation.skip` is the `AtomicBool` of `DECISIONS.md` 24, read `Relaxed` to match
`__atomic_load_n(..., __ATOMIC_RELAXED)` at `sa.m:550` and `:566`. This function runs on the
CVDisplayLink thread (`window_manager.c:584`) and on the event-loop thread, which is why the flag
is atomic at all.

`swap_window_proxy_out` is the same body with the other opcode; both are kept as separate
functions, as in the C.

### 25.7 The socket path

Two different sources, and they must not be conflated:

* the **daemon** builds `g_sa_socket_file` from `$USER` at `yabai.c:135`;
* the **root `yabai --load-sa` process** builds it in `scripting_addition_set_socket_path`
  (`sa.m:145-160`) from `$SUDO_UID` → `getpwuid` → `pw_name`, after asserting `getuid() == 0`.
  The `assert` becomes `debug_assert!` (`DECISIONS.md` 33). `sscanf(sudo_uid, "%u", &uid)` calls
  libc directly (`DECISIONS.md` 27); `uid` keeps the `getuid()` value if the parse fails, and the
  function returns `false` in that case.

`g_sa_socket_file` is an `OnceLock<String>` (`DECISIONS.md` 18): written once before any thread
starts in the daemon, written once before use in the `--load-sa` process.

---

## 26. Install, load, and `include_bytes!`

### 26.1 `build.rs` — the osax half of `DECISIONS.md` 9

```
xcrun clang src/osax/payload.m -shared -fPIC -O3 -mmacosx-version-min=11.0 \
    -arch x86_64 -arch arm64e -o $OUT_DIR/payload \
    -F/System/Library/PrivateFrameworks -framework SkyLight -framework Foundation -framework Carbon

xcrun clang src/osax/loader.m -O3 -mmacosx-version-min=11.0 \
    -arch x86_64 -arch arm64e -o $OUT_DIR/loader -framework Cocoa
```

`-arch arm64e` is the reason `src/osax/` stays C: it has to go to `clang` directly and it applies
to these two artifacts only, never to the Rust crate. Both invocations must be checked and the
build failed on a non-zero exit.

`rerun-if-changed` covers `src/osax/payload.m`, `src/osax/loader.m`, `src/osax/arm64_payload.m`,
`src/osax/x64_payload.m`, `src/osax/common.h` and `src/misc/hashtable.h` (`payload.m` includes
it), plus `assets/Info.plist` for the `-sectcreate` argument.

The rest of `DECISIONS.md` 9's `build.rs` — the PrivateFrameworks search path,
`MACOSX_DEPLOYMENT_TARGET=11.0`, the absolute `-sectcreate __TEXT __info_plist` link argument and
`OUT_DIR/osax_common.rs` — is one file; only the parts that concern FFI are repeated here.

### 26.2 `include_bytes!` replaces `xxd -i`

`makefile:33-34` runs `xxd -i` over both binaries into `payload_bin.c` / `loader_bin.c`, which
define `__src_osax_payload[]` / `__src_osax_payload_len` and `__src_osax_loader[]` /
`__src_osax_loader_len` (declared at `sa.h:4-7`). Both the array and the length are replaced by
one `include_bytes!` each:

```rust
static __src_osax_payload: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload"));
static __src_osax_loader: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/loader"));
```

`__src_osax_payload_len` and `__src_osax_loader_len` disappear: `.len()` is the same number.
The `xxd` step and the two generated `.c` files are deleted with the C daemon
(`DECISIONS.md` 6).

### 26.3 What lands on disk

`scripting_addition_install` (`sa.m:202-237`), after `umask(S_IWGRP | S_IWOTH)`:

```
/Library/ScriptingAdditions/yabai.osax/                        mkdir 0755
    Contents/                                                  mkdir 0755
        Info.plist                          <- sa_plist        fopen "w"
        MacOS/                                                 mkdir 0755
            loader                          <- __src_osax_loader        fopen "wb"
        Resources/                                             mkdir 0755
            payload.bundle/                                    mkdir 0755
                Contents/                                      mkdir 0755
                    Info.plist              <- sa_bundle_plist fopen "w"
                    MacOS/                                     mkdir 0755
                        payload             <- __src_osax_payload       fopen "wb"
```

Then `scripting_addition_prepare_binaries` (`sa.m:122-137`) runs four `system()` commands in
order — `chmod +x <loader>`, `codesign -f -s - <loader> 2>/dev/null`, `chmod +x <payload>`,
`codesign -f -s - <payload> 2>/dev/null` — and `scripting_addition_restart_dock`
(`sa.m:139-143`) terminates Dock.app with
`[[NSRunningApplication runningApplicationsWithBundleIdentifier:@"com.apple.dock"] makeObjectsPerformSelector:@selector(terminate)]`.

The two plists (`sa.m:21-48` and `:50-76`) are `&str` constants with `{OSAX_VERSION}` substituted
into the two `CFBundleShortVersionString` / `CFBundleVersion` pairs. They are written with
`fwrite(buffer, size, 1, handle)` and the success test is `bytes == 1`, which the Rust keeps by
checking that the whole slice was written. The eleven `MAXLEN` path buffers of `sa.m:9-19`
become `String`s built by `scripting_addition_set_path` (`sa.m:78-94`); `MAXLEN` is 512
(`macros.h:20`) and `snprintf` truncation at that length is observable in the error messages, so
the Rust truncates to 512 bytes at the same points.

Return codes are observable — `scripting_addition_install` returns `0`, `1` (already installed
and removal failed) or `2` (a step failed, after `scripting_addition_remove`) — and
`scripting_addition_load` returns them through to `main`.

### 26.4 Injection

`mach_loader_inject_payload` (`sa.m:333-348`) `popen`s
`/Library/ScriptingAdditions/yabai.osax/Contents/MacOS/loader` with mode `"r"` and inspects
`pclose`'s status with `WIFEXITED` / `WEXITSTATUS` / `WIFSIGNALED` / `WIFSTOPPED`, returning true
only for a clean exit code 0. The path is a literal, not one of the `osax_*` variables. `libc`
has `WIFEXITED` and friends as `const fn`s, so the four-branch chain transposes unchanged.

Everything the loader does — `task_for_pid`, the remote stack and code page, the shellcode
patch, `thread_create_running` versus the arm64e `thread_convert_thread_state` path, the
`0x79616265` sentinel poll — stays in `src/osax/loader.m` and is never touched.

### 26.5 The gates

`scripting_addition_load` (`sa.m:369-416`) checks, in this order, and each failure both `warn`s
and `notify`s with the same text:

1. `is_root()` (`helpers.h:249` — `getuid() == 0 || geteuid() == 0`);
2. `scripting_addition_is_sip_friendly()` — `csr_get_active_config` must report both
   `CSR_ALLOW_UNRESTRICTED_FS` and `CSR_ALLOW_TASK_FOR_PID` (§16);
3. `scripting_addition_check() != 0` → install and return;
4. on aarch64 only, `scripting_addition_is_arm64e_enabled()` — `kern.bootargs` must contain
   `-arm64e_preview_abi`;
5. `mach_loader_inject_payload()`;
6. `scripting_addition_set_socket_path()` → `scripting_addition_perform_validation()`.

`scripting_addition_uninstall` (`sa.m:350-367`) checks SIP first and root second — the opposite
order from `load` — and that difference is observable in which message the user sees. It is kept.

---

## 27. The remaining Objective-C

### 27.1 `notify()` — the one `va_list` that crosses into Objective-C

`misc/notify.h:29-48`. The C formats with
`[[NSString alloc] initWithFormat:<format> arguments:args]`, which is the only place a C
`va_list` reaches the Objective-C runtime. Rust has no `VaList` to hand over and needs none:
`notify` becomes a `macro_rules!` over `format_args!` (`DECISIONS.md` 33), the string is finished
in Rust, and the finished `NSString` is assigned to `informativeText`. The bytes delivered are
identical, because the C also just produces the formatted string.

The rest transposes directly:

* `NotifyDelegate : NSObject <NSUserNotificationCenterDelegate>` (`:10-18`) is a second
  `define_class!`, with `#[unsafe(super(NSObject))]`, the protocol conformance, and one method
  `userNotificationCenter:shouldPresentNotification:` returning `true`;
* `[[NSUserNotificationCenter defaultUserNotificationCenter] setDelegate:[NotifyDelegate alloc]]`
  (`:22`) sets an **unfinished, `alloc`-only** delegate — the C never calls `init`. That is what
  the C does and `NSObject`'s `alloc` is enough for this delegate, so the Rust does the same with
  `NotifyDelegate::alloc()` and no `init`. The class inherits `NSObject`'s `AnyThread` thread
  kind, so `alloc()` takes no `MainThreadMarker` — which it must not, because `notify` is reached
  from the event-loop thread as well (`helpers.h:467,473,485`, `sa.m:277-404`);
* `g_notify_init` and `g_notify_img` (`:4-5`) are `OnceLock`s — the image is
  `[[NSWorkspace sharedWorkspace] iconForFile:[[[NSBundle mainBundle] executablePath] stringByResolvingSymlinksInPath]]`
  retained once;
* the two private KVC keys (`:41-42`) are `setValue:forKey:` with `"_identityImage"` and
  `"_identityImageHasBorder"`, the latter taking `@(false)`, an `NSNumber`. Both stay.

`NSUserNotification` is deprecated since 10.14 and still present in SDK 26.1 and in
`objc2-foundation` 0.3.2. The rewrite keeps it; moving to `UserNotifications.framework` would
require a signed bundle and would change behaviour.

### 27.2 `service.h`

Only one Objective-C call: `NSHomeDirectoryForUser(NULL)` bridged to `CFStringRef` at
`service.h:82`. In objc2 it returns `Option<Retained<NSString>>` and needs no bridge; the C's
`cfstring_copy` becomes `to_string()` on the `NSString`. The `error()` on a NULL result stays.

Everything else in that file is libc and stays libc under `DECISIONS.md` 34: `posix_spawn` with
a NULL environment plus `posix_spawn_file_actions_*` and `waitpid` (`:56-78`),
`_NSGetExecutablePath` into a 4096-byte buffer (`:115`), and the launchd plist text written byte
for byte.

### 27.3 `[NSApp run]`

`yabai.c:139` calls `NSApplicationLoad()` before anything else touches AppKit, and `yabai.c:350`
enters `[NSApp run]`, which never returns. In Rust:

```rust
let main_thread_marker = MainThreadMarker::new().unwrap();
NSApplicationLoad();
NSApplication::sharedApplication(main_thread_marker).run();
```

Both calls are safe in the crate and neither needs an `unsafe` block. `NSApplicationLoad` is a
free function taking no marker; `NSApplication::sharedApplication` is the one call in the daemon
that takes one, so the `MainThreadMarker` is obtained in `main` — `main` *is* the main thread —
and goes no further. It is not threaded through the rest of the program: it is neither `Send` nor
`Sync`, so it cannot cross into the event-loop thread, and the only other main-thread-only AppKit
type the daemon touches is `NSScreen`, which the C reaches from the event-loop thread and which
mints its own marker at the call site (§20.5). `NSWorkspace`, `NSRunningApplication` and `NSImage`
carry no `MainThreadOnly` thread kind and need no marker at all.

`DECISIONS.md` 12 moves the event-loop thread's spawn to after `window_manager_begin` and
`update_window_notifications`, which are both before `[NSApp run]`, so the run loop still starts
last.

---

## 28. What this document registers in `DEVIATIONS.md`

Each line is `C location — what C did — what Rust does`.

| C location | C | Rust |
| --- | --- | --- |
| `extern.h:25,28,29,32,65,87,91` | seven declarations with no caller | not translated (`DECISIONS.md` 5) |
| `misc/autorelease.h` | swizzles `autorelease`/`drain`/`release`; already `#if 0`'d and commented out of `manifest.m:49` | not translated (`DECISIONS.md` 5) |
| `application.h:26-44` | `ax_error_str[-result]` indexes past the table for an unknown `AXError` | a match with a catch-all arm |
| `helpers.h:534-539` | `psn_equals` passes an uninitialised `Boolean` to `SameProcess` | the out-parameter is zeroed first |
| `process_manager.c:70-80` | `process_is_being_debugged` reads an uninitialised `struct kinfo_proc` when `sysctl` fails, relying on one field having been zeroed | the whole buffer is zeroed; `p_flag` is read at a measured offset out of an opaque `[u8; 648]` |
| `window_manager.c:437-458` | `window_manager_notify_jankyborders` writes `animation_count` entries into two `[512]` arrays with no bound | stops at 512 |
| `sa.m:544-574` | `swap_window_proxy_in/out` can write past the 4096-byte stack frame at 512 animations | `pack` bounds-checks and the call returns `false` (`DECISIONS.md` 34) |
| `sa.m:586-602` | `order_window_in` overruns past 1022 windows | same bounds check |
| `sa.m:604-613` | `move_window_list_to_space` overruns past 1021 windows | same bounds check |
| `sa.m:252-257` | the handshake reply is scanned for a NUL and four more bytes are read with no length check | both are bounds-checked against the bytes received |
| `space_manager.c:669,690` | `objc_getClass` returning NULL sends `alloc` to nil and passes nil onward | an early return |
| `window.c:930-952` | `SLSGetWindowSubLevel__Internal` would call through a NULL `CGSGetConnectionPortById` if reached without the caller's guard | returns 0 |
| `yabai.c:141` | `g_event_bytes` is a 256-byte `malloc` that is never freed and is read by `SLPSPostEventRecordTo` | a `[u8; 0x100]` field of `EventLoopOwnedState` |
| `workspace.m:125-138` | `workspace_display_notch_height` calls `[NSScreen screens]` and `-safeAreaInsets` — both main-thread-only AppKit APIs — from the event-loop thread, through `display_bounds_constrained` (`display.c:141`) | the same calls from the same thread, with the `MainThreadMarker` `objc2-app-kit` demands minted on the spot by `MainThreadMarker::new_unchecked()` (§20.5) |

Not deviations, and deliberately so: `ax_window_pid`'s `0x10` offset read (§18.6), the `0x14`
remote token (§18.5), the `usleep(40000)` between the two focus records (§18.8), the ignored
one-byte ack (§25.3), the `0x1513` / `0x76E3` / `0x73C3` MIG literals (§15.7) and the
`0x79616265` compat id (§18.10) are all reproduced exactly.

---

## 29. Order of work for phase 2

`src/ffi/` is written first, in full, before any other Rust file, because every other module
depends on it. Within it:

1. `core_foundation.rs`, `core_graphics.rs`, `color_sync.rs`, `core_video.rs` — re-export shells
   plus the four hand-written CoreGraphics/ColorSync declarations; nothing depends on anything
   else.
2. `carbon_process.rs` — `ProcessSerialNumber` first, because `skylight.rs` needs it.
3. `skylight.rs` — the full declaration list of §3.
4. `macho.rs`, then `skylight_dynamic.rs`, which depends on it.
5. `mach_port.rs`, `carbon_events.rs`, `carbon_core.rs`, `libsystem.rs`.
6. `accessibility.rs` — the `kAX*` macro and the helpers.
7. `foundation.rs`, `appkit.rs`, `dispatch.rs`.

Then `workspace.rs` and `sa.rs`, which are the two files that exercise the whole surface: between
them they cover `define_class!`, KVO, the `@try/@catch` helper, the autorelease pool, the
retain/release of `NSRunningApplication`, `include_bytes!`, the socket client and every packed
frame. If those two compile and behave, the FFI layer is right.
