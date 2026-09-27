# Sweep: FFI surface and crate selection

Phase 1 mapping. No Rust is written here. Everything below is derived from reading the C/ObjC
sources under `src/` (excluding `src/osax/`, which stays in C), from the SDK `.tbd` stubs in
`MacOSX26.1.sdk`, and from crates.io / docs.rs as of 2026-09.

Toolchain actually present on this machine: `rustc 1.98.0`, `cargo 1.98.0`, SDK 26.1, host macOS 15.8.

---

## 1. The single most important finding: the daemon does not link what `extern.h` implies

`src/misc/extern.h` declares 90-odd symbols in one flat block and the makefile links
`-F/System/Library/PrivateFrameworks -framework Carbon -framework Cocoa -framework CoreServices
-framework CoreVideo -framework SkyLight` (`makefile:1-2`). Because it is one unity translation unit
with five frameworks on the link line, nothing in the C source says *which* framework exports
*which* symbol. Rust wants that answer per `#[link]` block, so it was resolved against the SDK stubs:

| Symbol group | Actually exported by | Evidence |
| --- | --- | --- |
| every `SLS*`, `SLPS*`, `_SLPS*`, `SLWindowContextCreate` | **SkyLight** (private) | `SDK/System/Library/PrivateFrameworks/SkyLight.framework/Versions/A/SkyLight.tbd`; all of them verified present except one (below) |
| `CGRegionCreateEmptyRegion`, `CGSNewRegionWithRect`, `CGPostMouseEvent` | **CoreGraphics** (private/deprecated, still exported) | `CoreGraphics.tbd` |
| `CGDisplayCreateUUIDFromDisplayID`, `CGDisplayGetDisplayIDFromUUID` | **ColorSync** | `ColorSync.tbd` — *not* CoreGraphics |
| `CoreDockGetAutoHideEnabled`, `CoreDockGetOrientationAndPinning`, `CoreDockSendNotification`, `_AXUIElementGetWindow`, `_AXUIElementCreateWithRemoteToken`, all Process Manager, all `AX*` | **ApplicationServices / HIServices** | `HIServices.tbd` |
| `mig_get_special_reply_port`, `csr_get_active_config`, `NDR_record` | **libSystem** | `SDK/usr/lib/libSystem.tbd` |
| `CVGetHostClockFrequency`, `CVDisplayLink*` | **CoreVideo** | `CoreVideo.tbd` |
| `CGSGetConnectionPortById`, `SLSPerformAsynchronousBridgedWindowManagementOperation` | **nothing** — resolved at runtime | not in any `.tbd`; see §4 |

The two runtime-resolved ones are declared as *static function pointers*, not `extern`
(`src/misc/extern.h:4-5`), and filled in by `macho_find_symbol` at `src/yabai.c:148-149`.
`SLSPerformAsynchronousBridgedWindowManagementOperation` is a **local** C++ symbol
(`__ZL54SLSPerformAsynchronousBridgedWindowManagementOperationP47SLSAsynchronousBridgedWindowManagementOperation`),
which is exactly why `dlsym` will not find it and yabai walks `LC_SYMTAB` by hand.

Verification run: every `SLS|SLW|_SLPS|SLPS` identifier in `src/misc/extern.h` was checked against
`SkyLight.tbd`; the only miss is `SLSPerformAsynchronousBridgedWindowManagementOperation`.

Seven declarations in `extern.h` are **dead** — declared but never called by the daemon:
`SLSClearWindowTags` (`:29`), `SLSMoveWindow` (`:65`), `SLSNewWindow` (`:25`),
`SLSSetWindowBackgroundBlurRadiusStyle` (`:32`), `SLSSetWindowTags` (`:28`),
`SLSSetWindowTransform` (`:87`), `SLSTransactionOrderWindow` (`:91`).
Phase 2 should still declare them (cheap, keeps the header a faithful mirror) but mark them `#[allow(dead_code)]`.

---

## 2. SkyLight — the private surface

All from `src/misc/extern.h`. Signatures reproduced verbatim with the Rust type each C type maps to.
`CGError` = `objc2_core_graphics::CGError` (i32 newtype), `CGRect`/`CGPoint`/`CGAffineTransform` =
`objc2_core_foundation::{CGRect, CGPoint, CGAffineTransform}`, `CFStringRef`/`CFArrayRef`/`CFTypeRef`
= `*const CFString` / `*const CFArray` / `*const CFType` at the boundary (wrapped into
`CFRetained<_>` by hand for the `Copy`/`Create` ones).

### 2.1 Connection lifecycle and notifications

| C declaration (`extern.h` line) | Used by |
| --- | --- |
| `int SLSMainConnectionID(void)` (`:9`) | `yabai.c:143`, `yabai.c:275` |
| `CGError SLSNewConnection(int zero, int *cid)` (`:10`) | `window_manager.c:607` |
| `CGError SLSReleaseConnection(int cid)` (`:11`) | `window_manager.c:591` |
| `CGError SLSRegisterConnectionNotifyProc(int cid, connection_callback *handler, uint32_t event, void *context)` (`:12`) | `yabai.c:322,323,326,329,330,333` |
| `CGError SLSGetWindowOwner(int cid, uint32_t wid, int *wcid)` (`:77`) | `display_manager.c:441`, `window.c` |
| `CGError SLSGetConnectionPSN(int cid, ProcessSerialNumber *psn)` (`:78`) | `display_manager.c:442` |
| `CGError SLSConnectionGetPID(int cid, pid_t *pid)` (`:79`) | `window_manager.c:938`, `window.c` |
| `CGError SLSGetConnectionIDForPSN(int cid, ProcessSerialNumber *psn, int *psn_cid)` (`:80`) | `application.c:135` |
| `OSStatus _SLPSGetFrontProcess(ProcessSerialNumber *psn)` (`:76`) | `application.c:106`, `process_manager.c:247`, `window_manager.c` |
| `CGError _SLPSSetFrontProcessWithOptions(ProcessSerialNumber *psn, uint32_t wid, uint32_t mode)` (`:81`) | `window_manager.c:1320,1329,1927,2104,2475` |
| `CGError SLPSPostEventRecordTo(ProcessSerialNumber *psn, uint8_t *bytes)` (`:82`) | `window_manager.c:1287,1290,1304,1317` |

`connection_callback` is `void (*)(uint32_t type, void *data, size_t data_length, void *context, int cid)`
(`src/misc/extern.h:1-2`), implemented at `src/mission_control.c:7-26`.

### 2.2 Window geometry / appearance

`SLSGetWindowBounds(int, uint32_t, CGRect*)` (`:13`), `SLSGetWindowLevel(int, uint32_t, int*)` (`:14`),
`int SLSGetWindowSubLevel(int, uint32_t)` (`:15`), `SLSGetWindowAlpha(int, uint32_t, float*)` (`:16`),
`SLSSetWindowAlpha(int, uint32_t, float)` (`:17`), `SLSSetWindowResolution(int, uint32_t, double)` (`:18`),
`SLSCopyWindowProperty(int, uint32_t, CFStringRef, CFTypeRef*)` (`:19`),
`SLSSetWindowOpacity(int, uint32_t, bool)` (`:31`), `SLSSetWindowLevel(int, uint32_t, int)` (`:35`),
`SLSSetWindowSubLevel(int, uint32_t, int)` (`:36`), `SLSOrderWindow(int, uint32_t, int, uint32_t)` (`:33`),
`SLSWindowIsOrderedIn(int, uint32_t, uint8_t*)` (`:34`), `SLSSetWindowShape(int, uint32_t, float, float, CFTypeRef)` (`:30`),
`SLSWindowSetShadowProperties(uint32_t, CFDictionaryRef)` (`:85`),
`SLSSetWindowTransform(int, uint32_t, CGAffineTransform)` (`:87`, dead),
`CGContextRef SLWindowContextCreate(int, uint32_t, CFDictionaryRef)` (`:37`),
`SLSNewWindowWithOpaqueShapeAndContext(...)` (`:26`), `SLSReleaseWindow(int, uint32_t)` (`:27`),
`SLSDisableUpdate` / `SLSReenableUpdate` (`:23-24`),
`SLSRequestNotificationsForWindows(int, uint32_t*, int)` (`:86`),
`SLSHWCaptureWindowList(int, uint32_t*, int, uint32_t) -> CFArrayRef` (`:95`).

Callers: `view.c` (feedback window: `:12,17,92`), `window_manager.c` (proxy windows: `:468-484`,
capture `:521`), `window.c` (levels/bounds/title).

### 2.3 Transactions (used only by the animation thread)

`SLSTransactionCreate(int) -> CFTypeRef` (`:88`), `SLSTransactionCommit(CFTypeRef, int)` (`:89`),
`SLSTransactionSetWindowTransform(CFTypeRef, uint32_t, int, int, CGAffineTransform)` (`:90`),
`SLSTransactionOrderWindow` (`:91`, dead), `SLSTransactionOrderWindowGroup` (`:92`),
`SLSTransactionSetWindowAlpha` (`:93`), `SLSTransactionSetWindowSystemAlpha` (`:94`).
All called from `window_manager.c:556-574` and `:656-660`.

### 2.4 Window query iterator

`SLSWindowQueryWindows(int, CFArrayRef, int) -> CFTypeRef` (`:67`),
`SLSWindowQueryResultCopyWindows(CFTypeRef) -> CFTypeRef` (`:68`),
`SLSWindowIteratorGetCount` (`:69`), `SLSWindowIteratorAdvance -> bool` (`:70`),
`SLSWindowIteratorGetParentID` (`:71`), `...GetWindowID` (`:72`), `...GetTags` (`:73`),
`...GetAttributes` (`:74`), `...GetLevel` (`:75`),
plus `SLSCopyWindowsWithOptionsAndTags(int, uint32_t, CFArrayRef, uint32_t, uint64_t*, uint64_t*)` (`:58`),
`SLSCopyAssociatedWindows(int, uint32_t) -> CFArrayRef` (`:66`),
`SLSCopySpacesForWindows(int, int, CFArrayRef) -> CFArrayRef` (`:22`).

Callers: `process_manager.c:112-140`, `space.c`, `window.c:900-928`, `window_manager.c`.

### 2.5 Displays, menubar, dock

`SLSCopyManagedDisplays(int) -> CFArrayRef` (`:41`) — `display_manager.c:172,202`;
`SLSCopyManagedDisplaySpaces(int) -> CFArrayRef` (`:60`) — `display.c` ×2, `space_manager.c` ×6;
`SLSManagedDisplayGetCurrentSpace(int, CFStringRef) -> uint64_t` (`:42`);
`SLSCopyActiveMenuBarDisplayIdentifier(int) -> CFStringRef` (`:43`);
`SLSSetActiveMenuBarDisplayIdentifier(int, CFStringRef, CFStringRef)` (`:44`) — `display_manager.c:457`;
`SLSCopyBestManagedDisplayForPoint(int, CGPoint) -> CFStringRef` (`:45`) — `display_manager.c:126`;
`SLSCopyBestManagedDisplayForRect(int, CGRect) -> CFStringRef` (`:21`) — `display_manager.c:110`, `window.c:37`;
`SLSCopyManagedDisplayForWindow(int, uint32_t) -> CFStringRef` (`:20`);
`SLSCopyManagedDisplayForSpace(int, uint64_t) -> CFStringRef` (`:54`);
`SLSManagedDisplayIsAnimating(int, CFStringRef) -> bool` (`:46`) — `display_manager.c:382`;
`SLSSetMenuBarInsetAndAlpha(int, double, double, float)` (`:47`) — `event_loop.c:1534` +2, `window_manager.c`;
`SLSGetMenuBarAutohideEnabled(int, int*)` (`:48`);
`SLSGetRevealedMenuBarBounds(CGRect*, int, uint64_t)` (`:49`) — `display_manager.c:309`;
`SLSGetDisplayMenubarHeight(uint32_t, uint32_t*)` (`:50`);
`SLSGetDockRectWithReason(int, CGRect*, int*)` (`:51`);
`SLSGetCurrentCursorLocation(int, CGPoint*)` (`:84`);
`SLSFindWindowAndOwner(int, int, int, int, CGPoint*, CGPoint*, uint32_t*, int*) -> OSStatus` (`:83`) —
`window_manager.c:950-955` (four call sites in one function).

### 2.6 Spaces

`SLSSpaceSetFrontPSN(int, uint64_t, ProcessSerialNumber)` (`:55`, **PSN by value**) —
`event_loop.c:371,666`, `window_manager.c:2116`;
`int SLSSpaceGetType(int, uint64_t)` (`:56`); `SLSSpaceCopyName(int, uint64_t) -> CFStringRef` (`:57`);
`int SLSGetSpaceManagementMode(int)` (`:59`) — `yabai.c:275`;
`SLSProcessAssignToSpace(int, pid_t, uint64_t)` (`:61`); `SLSProcessAssignToAllSpaces(int, pid_t)` (`:62`);
`void SLSMoveWindowsToManagedSpace(int, CFArrayRef, uint64_t)` (`:63`) — `space_manager.c:677,698`;
`SLSSpaceSetCompatID(int, uint64_t, int)` (`:96`) — `space_manager.c:680,682` ×2 pairs;
`SLSSetWindowListWorkspace(int, uint32_t*, int, int)` (`:97`) — `space_manager.c:681`.

---

## 3. Every other framework, by user file

### 3.1 CoreGraphics (public)

| Symbol | Used at |
| --- | --- |
| `CGMainDisplayID` | `display_manager.c:88` |
| `CGGetActiveDisplayList` | `display_manager.c:394,402` |
| `CGDisplayBounds` | `display.c`, `display_manager.c`, `window_manager.c` |
| `CGDisplayRegisterReconfigurationCallback` + `CGDisplayChangeSummaryFlags` | `display_manager.c:505`; callback typedef at `display.h:4-5` |
| `kCGDisplayAddFlag/RemoveFlag/MovedFlag/DesktopShapeChangedFlag` | `display.c` |
| `CGDisplayIsBuiltin` | `workspace.m:127` |
| `CGWindowLevelForKey`, `kCGBackstopMenuLevelKey`, `kCGNormalWindowLevelKey`, `kCGFloatingWindowLevelKey` | `yabai.c:145-147`, `misc/macros.h:43-45` |
| `CGWindowListCopyWindowInfo`, `kCGWindowListOptionOnScreenOnly`, `kCGWindowName`, `kCGWindowOwnerName`, `kCGWindowLayer` | `event_loop.c:1489-1502` |
| `CGEventTapCreate`, `CGEventTapEnable`, `CGEventTapIsEnabled`, `CGEventTapPostEvent` | `mouse_handler.c:278,281,297,48-49` |
| `CGEventCreate`, `CGEventPost`, `CGEventSetIntegerValueField`, `CGEventSetDoubleValueField` | `space_manager.c:955-969` |
| `CGEventGetLocation`, `CGEventGetFlags`, `CGEventGetIntegerValueField` | `event_loop.c`, `mouse_handler.c` |
| `CGWarpMouseCursorPosition` | `display_manager.c:472`, `space_manager.c:942`, `window_manager.c:269` |
| `CGSetLocalEventsSuppressionInterval`, `CGEnableEventStateCombining` | `yabai.c:153-154` |
| `CGPreflightScreenCaptureAccess`, `CGRequestScreenCaptureAccess` | `message.c:1306,1310` |
| `CGBitmapContextCreate`, `CGBitmapContextCreateImage`, `CGColorSpaceCreateDeviceRGB`, `CGColorSpaceRelease`, `CGContextDrawImage`, `CGContextRelease`, `CGImageGetWidth/Height` | `misc/helpers.h:588-675` |
| `CGContext*` drawing (`AddPath`, `ClearRect`, `ClipToRect`, `FillRect`, `Flush`, `ResetClip`, `SetLineWidth`, `SetRGBFillColor`, `SetRGBStrokeColor`, `StrokePath`) | `view.c` |
| `CGPathCreateWithRoundedRect`, `CGPathRelease` | `view.c:89` |
| `CGAffineTransformMakeTranslation/MakeScale/Concat` | `window_manager.c:565-567` |
| `CGRectMake/Inset/GetMidX/GetMidY/GetWidth/GetHeight/ContainsPoint/ContainsRect/EqualToRect`, `CGPointMake`, `CGSizeMake`, `CGPointEqualToPoint` | everywhere |

**Not in `objc2-core-graphics` 0.3.2** (checked against its `all.html`): `CGPostMouseEvent`
(`display_manager.c:476-477`, `space_manager.c:976-977`) and the two display-UUID functions.

### 3.2 ColorSync

`CGDisplayCreateUUIDFromDisplayID` (`display.c:103`, declared in `extern.h:40`) and
`CGDisplayGetDisplayIDFromUUID` (`display.c:117`, `space.c:9`, `window.c:48`). Both are in
`objc2-color-sync` 0.3.2, oddly enough — that is the crate that binds them because that is the
framework that exports them.

### 3.3 Accessibility (ApplicationServices / HIServices)

Functions: `AXIsProcessTrustedWithOptions` (`helpers.h:494`), `AXUIElementCreateApplication`
(`application.c:130`, `mission_control.c:77`), `AXUIElementCreateSystemWide` (`window_manager.c:2711`),
`AXUIElementSetMessagingTimeout` (`window_manager.c:2712`), `AXUIElementCopyAttributeValue`,
`AXUIElementSetAttributeValue`, `AXUIElementIsAttributeSettable` (`window.c:776,790,804`),
`AXUIElementPerformAction` (`window_manager.c:1331,2086`), `AXUIElementCopyElementAtPosition`
(`display_manager.c:409`), `AXValueCreate` (`window_manager.c:418,428,743,746`), `AXValueGetValue`
(`window.c:744,761,766`), `AXObserverCreate` (`application.c:45`, `mission_control.c:80`),
`AXObserverAddNotification`, `AXObserverRemoveNotification`, `AXObserverGetRunLoopSource`.

Private: `_AXUIElementGetWindow(AXUIElementRef, uint32_t*)` (`extern.h:8`, used by
`helpers.h:499-504`) and `_AXUIElementCreateWithRemoteToken(CFDataRef)` (`extern.h:7`, used by
`window_manager.c:1680` for the inactive-space brute force).

Callback typedef: `OBSERVER_CALLBACK` at `application.h:4-5` —
`void (*)(AXObserverRef, AXUIElementRef, CFStringRef, void*)`.

**The `kAX*` constants are `#define … CFSTR("…")` in the SDK and therefore are NOT in
`objc2-application-services`** (verified: only `kAXTrustedCheckOptionPrompt`, which is a real
exported `CFStringRef`, is present). Everything below must be hand-written as CFString literals:

- attributes: `kAXWindowsAttribute`, `kAXFocusedWindowAttribute`, `kAXMainWindowAttribute`,
  `kAXWindowAttribute`, `kAXPositionAttribute`, `kAXSizeAttribute`, `kAXRoleAttribute`,
  `kAXSubroleAttribute`, `kAXTitleAttribute`, `kAXMinimizedAttribute`, `kAXFullscreenAttribute`,
  `kAXCloseButtonAttribute`, `kAXParentAttribute`
- roles/subroles: `kAXWindowRole`, `kAXDrawerRole` (`event_loop.c`), `kAXSheetRole` (`event_loop.c`),
  `kAXStandardWindowSubrole`, `kAXDialogSubrole`, `kAXFloatingWindowSubrole`, `kAXUnknownSubrole`
- actions: `kAXPressAction`, `kAXRaiseAction`
- notifications: `kAXCreatedNotification`, `kAXFocusedWindowChangedNotification`,
  `kAXWindowMovedNotification`, `kAXWindowResizedNotification`, `kAXTitleChangedNotification`,
  `kAXMenuOpenedNotification`, `kAXMenuClosedNotification`, `kAXWindowMiniaturizedNotification`,
  `kAXWindowDeminiaturizedNotification`, `kAXUIElementDestroyedNotification`
- value types: `kAXValueTypeCGPoint`, `kAXValueTypeCGSize` (these *are* an enum — `AXValueType` — and
  are bound)
- yabai-local: `kAXEnhancedUserInterface` (`helpers.h:171`), `kAXExposeShowAllWindows`,
  `kAXExposeShowFrontWindows`, `kAXExposeShowDesktop`, `kAXExposeExit` (`mission_control.c:52-55`)

`ax_error_str[]` (`application.h:26-44`) indexes by `-kAXError*`; `AXError` in
`objc2-application-services` is a newtype with associated constants, so the table becomes a match arm
or a const array keyed on `-(error.0)`.

### 3.4 Process Manager (HIServices, all deprecated since 10.9)

`GetProcessPID` (`process_manager.c:27,248`, `window_manager.c`), `GetProcessInformation`
(`process_manager.c:34`), `CopyProcessName` (`:37`), `GetNextProcess` (`:211`), `SameProcess`
(`helpers.h:537`), `IsProcessVisible` (`application.c:114`), plus `ProcessSerialNumber`,
`ProcessInfoRec`, `kNoProcess`, `'XPC!'` process type check (`process_manager.c:47`).

Bound in `objc2-application-services` 0.3.2: `GetProcessPID`, `GetNextProcess`, `CopyProcessName`,
`SameProcess`, `IsProcessVisible`, `kNoProcess`.
**Not bound anywhere**: `ProcessSerialNumber` (the struct), `ProcessInfoRec`, `GetProcessInformation`.
(`objc2-core-services` only has `keyProcessSerialNumber` / `typeProcessSerialNumber` AE constants.)

### 3.5 Carbon Event Manager (HIToolbox) — **no Rust bindings exist**

Used only by `process_manager.c` and `event_signal.c`:
`GetApplicationEventTarget` (`:232`), `NewEventHandlerUPP` (`:233`), `InstallEventHandler` (`:251`),
`GetEventParameter` (`:156`), `GetEventKind` (`:160`), `GetCurrentEventTime`
(`process_manager.c:250`, `event_loop.c`, `event_signal.c`),
constants `kEventClassApplication`, `kEventAppLaunched`, `kEventAppTerminated`,
`kEventAppFrontSwitched`, `kEventParamProcessID`, `typeProcessSerialNumber`, `noErr`,
types `EventTargetRef`, `EventHandlerUPP`, `EventTypeSpec`, `EventHandlerRef`, `EventTime`,
`EventRef`, `EventHandlerCallRef` (`process_manager.h:4,20-27`).
Handler typedef `PROCESS_EVENT_HANDLER` at `process_manager.h:4-5`.

There is no `objc2-carbon` / `objc2-hi-toolbox` crate (checked the 100 most-downloaded `objc2*` crates).
This whole group is hand-written `extern "C"` against `#[link(name = "Carbon", kind = "framework")]`.
It links fine on both arches on SDK 26.1 — the symbols are still exported by HIToolbox; only the
headers carry `AVAILABLE_…_BUT_DEPRECATED_IN_MAC_OS_X_VERSION_10_9`, and the C build already silences
that with `#pragma clang diagnostic ignored "-Wdeprecated-declarations"`
(e.g. `process_manager.c:22-23`, `application.c:110-111`, `helpers.h:532-533`, `yabai.c:126-127`).
Rust has no deprecation to silence because we write the declarations ourselves.

### 3.6 CarbonCore / CoreServices

`AbsoluteToNanoseconds(AbsoluteTime) -> Nanoseconds` (`helpers.h:149-154`), with the
`*(AbsoluteTime*)&u64` / `*(uint64_t*)&nano` type-punning. Exported by CoreServices; not bound.
Note `AbsoluteTime` and `Nanoseconds` are both `{ UInt32 hi; UInt32 lo; }` — the punning works
because on little-endian the pair reads back as the same `u64`. In Rust this is
`mach_absolute_time()` → `AbsoluteToNanoseconds` with `#[repr(C)] struct UnsignedWide { lo: u32, hi: u32 }`
and `u64::from_ne_bytes`/transmute.

### 3.7 CoreVideo

`CVGetHostClockFrequency` (`yabai.c:144`), `CVDisplayLinkCreateWithActiveCGDisplays`
(`window_manager.c:700`), `CVDisplayLinkSetOutputCallback` (`:701`), `CVDisplayLinkStart` (`:702`),
`CVDisplayLinkStop` / `CVDisplayLinkRelease` (`:595-596`), types `CVDisplayLinkRef`, `CVTimeStamp`
(`->hostTime` read at `window_manager.c:542-543`), `CVReturn`, `CVOptionFlags`, `kCVReturnSuccess`.
All bound in `objc2-core-video` 0.3.2 (`CVDisplayLink` and `CVHostTime` features, both default-on).
`CVDisplayLink` is soft-deprecated in favour of `CADisplayLink` on macOS 14+; it still works and the
rewrite must not change that.

### 3.8 CoreFoundation

`CFArray*` (`Create`, `CreateMutableCopy`, `GetCount`, `GetValueAtIndex`, `SortValues`),
`CFDictionary*` (`Create`, `GetValue`), `CFNumber*` (`Create`, `GetValue`, `GetType`),
`CFString*` (`CreateWithCString`, `GetCString`, `GetLength`, `GetMaximumSizeForEncoding`, `CFSTR`),
`CFUUID*` (`CreateFromString`, `CreateString`), `CFData*` (`CreateMutable`, `IncreaseLength`,
`GetMutableBytePtr` — `window_manager.c:1668-1671`), `CFBooleanGetValue`, `CFEqual`, `CFRetain`,
`CFRelease`, `CFRunLoop*` (`GetMain`, `AddSource`, `RemoveSource`, `SourceInvalidate`),
`CFMachPort*` (`CreateRunLoopSource`, `Invalidate`), `CFRangeMake`, `CFComparisonResult`,
callbacks `kCFTypeArrayCallBacks`, `kCFTypeDictionaryKeyCallBacks`, `kCFTypeDictionaryValueCallBacks`,
`kCFCopyStringDictionaryKeyCallBacks`, encodings `kCFStringEncodingUTF8`, `kCFStringEncodingMacRoman`,
numbers `kCFNumberSInt32Type`, `kCFNumberSInt64Type`, `kCFRunLoopDefaultMode`, `kCFRunLoopCommonModes`,
`kCFBooleanTrue`, `kCFBooleanFalse`.
All present in `objc2-core-foundation` 0.3.2 (`CFRelease`/`CFRetain` are replaced by the
`CFRetained<T>` smart pointer / `CFRetained::retain`).

`CGRect`, `CGPoint`, `CGSize`, `CGFloat`, `CGAffineTransform` also live in
`objc2-core-foundation` (`CFCGTypes`), not in `objc2-core-graphics`.

### 3.9 Objective-C, by file

**`src/workspace.m`** (the only real ObjC class in the daemon):

- `workspace_context : NSObject` (`workspace.h:21-24`, `@implementation` at `workspace.m:153-303`)
  with `init`, `dealloc`, `observeValueForKeyPath:ofObject:change:context:`, and the eight
  notification selectors `didWake:`, `didChangeMenuBarHiding:`, `didRestartDock:`,
  `didChangeDockPref:`, `activeDisplayDidChange:`, `activeSpaceDidChange:`, `didHideApplication:`,
  `didUnhideApplication:`.
- `[[NSProcessInfo processInfo] operatingSystemVersion]` (`:3`, `:19`) → `NSOperatingSystemVersion`
  returned **by value** (3 × `NSInteger`).
- `[NSRunningApplication runningApplicationWithProcessIdentifier:]` + `retain` (`:30`),
  `release` (`:65`), `observationInfo` (`:38`), `activationPolicy` (`:107`),
  `isFinishedLaunching` (`:119`), `processIdentifier` (`:146`),
  `runningApplicationsWithBundleIdentifier:` (`:142`).
- KVO: `addObserver:forKeyPath:options:context:` (`:73`, `:83`) and
  `removeObserver:forKeyPath:context:` (`:57`, `:61`, `:94`, `:98`, `:227`, `:252`), every one
  wrapped in `@try/@catch(NSException)`.
- `[[NSWorkspace sharedWorkspace] notificationCenter]`, `[NSNotificationCenter defaultCenter]`,
  `[NSDistributedNotificationCenter defaultCenter]`, each with
  `addObserver:selector:name:object:` (`:157-195`) and `removeObserver:` (`:203-205`).
- Notification names — three are plain string literals, not symbols:
  `@"NSWorkspaceActiveDisplayDidChangeNotification"` (`:159`),
  `@"AppleInterfaceMenuBarHidingChangedNotification"` (`:184`),
  `@"NSApplicationDockDidRestartNotification"` (`:189`), `@"com.apple.dock.prefchanged"` (`:194`);
  the rest are AppKit constants (`NSWorkspaceActiveSpaceDidChangeNotification`,
  `NSWorkspaceDidHideApplicationNotification`, `NSWorkspaceDidUnhideApplicationNotification`,
  `NSWorkspaceDidWakeNotification`, `NSWorkspaceApplicationKey`).
- `[NSScreen screens]`, `deviceDescription`, `objectForKey:@"NSScreenNumber"`, `unsignedIntValue`,
  `screen.safeAreaInsets.top` (`:130-132`) — `NSEdgeInsets` returned **by value** behind a
  `__builtin_available(macos 12.0, *)` guard.
- `NSApplicationActivationPolicyRegular` / `…Prohibited` (`:108`, `:110`).
- `[change objectForKey:NSKeyValueChangeNewKey]`, `intValue` (`:215-216`, `:240-241`).

**`src/sa.m`**: `NSAutoreleasePool` alloc/init/drain (`:176`, `:189`, `:372`, `:414`);
`[NSString stringWithUTF8String:]` / `UTF8String` (`:179`, `:183`);
`[NSBundle bundleWithPath:]` + `objectForInfoDictionaryKey:@"CFBundleVersion"` (`:180-181`);
`[NSRunningApplication runningApplicationsWithBundleIdentifier:@"com.apple.dock"]` +
`makeObjectsPerformSelector:@selector(terminate)` (`:141-142`).

**`src/misc/notify.h`**: `NotifyDelegate : NSObject <NSUserNotificationCenterDelegate>` (`:10-18`);
`[NSUserNotificationCenter defaultUserNotificationCenter] setDelegate:` / `deliverNotification:`
(`:22`, `:43`); `[[NSWorkspace sharedWorkspace] iconForFile:[[[NSBundle mainBundle] executablePath]
stringByResolvingSymlinksInPath]]` (`:23`); `NSUserNotification` `title`/`subtitle`/`informativeText`
(`:38-40`); private KVC `setValue:forKey:@"_identityImage"` and `@"_identityImageHasBorder"`
(`:41-42`); `[[NSString alloc] initWithFormat:… arguments:va_list]` (`:40`).
`NSUserNotification*` are Foundation, not AppKit, and **are** in `objc2-foundation` 0.3.2
(`NSUserNotification` feature) and still in SDK 26.1.

**`src/misc/service.h`**: `NSHomeDirectoryForUser(NULL)` bridged to `CFStringRef` (`:82`).

**`src/misc/autorelease.h`**: swizzles `-[NSObject autorelease]`, `-[NSAutoreleasePool drain]`,
`-[NSAutoreleasePool release]` with `objc_getClass` / `class_getInstanceMethod` /
`method_setImplementation`. **Dead code** — commented out of the unity build at `manifest.m:49` and
guarded by `#if 0` at `yabai.c:158-162`. Do not port it.

**`src/space_manager.c:669-672` and `:690-692`**: dynamic ObjC —
`objc_getClass("SLSBridgedMoveWindowsToManagedSpaceOperation")`,
`sel_registerName("initWithWindows:spaceID:")`, and a hand-cast
`objc_msgSend` of type `id (*)(id, SEL, id, uint64_t)` applied to `[cls alloc]`, then
`[operation release]`.

**`src/event_loop.c`**: `NSAutoreleasePool` per drain cycle (`:1653`, `:1677`);
`observationInfo` + `removeObserver:forKeyPath:context:` in `@try/@catch` (`:112-118`, `:131-137`).

**`src/process_manager.c:242-244`** and **`src/window_manager.c:2739,2756`**: `NSAutoreleasePool`.

**`src/yabai.c`**: `NSApplicationLoad()` (`:139`) and `[NSApp run]` (`:350`).

### 3.10 libSystem / Mach / POSIX / dyld

- `csr_get_active_config(uint32_t*)` — declared by hand at `sa.m:3`, called at `sa.m:304`;
  exported by `libSystem.tbd`.
- `mig_get_special_reply_port()` — `extern.h:6`, called at `window.c:947`; `libSystem.tbd`.
- `NDR_record` (global) — `window.c:935,943`; `libsystem_kernel.tbd`.
- `mach_msg` — `helpers.h:222` (OOL send) and `window.c:949` (send+recv MIG call);
  `mach_msg_header_t`, `mach_msg_size_t`, `mach_msg_ool_descriptor_t`, `MACH_MSGH_BITS_SET`,
  `MACH_MSG_TYPE_COPY_SEND`, `MACH_MSGH_BITS_COMPLEX`, `MACH_MSG_OOL_DESCRIPTOR`,
  `MACH_MSG_VIRTUAL_COPY`, `MACH_SEND_MSG`, `MACH_RCV_MSG`.
- `task_get_special_port(mach_task_self(), TASK_BOOTSTRAP_PORT, &g_bs_port)` — `yabai.c:156`.
- `bootstrap_look_up(g_bs_port, "git.felix.jbevent", &port)` — `window_manager.c:440` (JankyBorders IPC).
- dyld: `_dyld_image_count`, `_dyld_get_image_name`, `_dyld_get_image_header`,
  `_dyld_get_image_vmaddr_slide` — `misc/macho_dlsym.h:3-11`; Mach-O structs `mach_header_64`,
  `load_command`, `segment_command_64`, `symtab_command`, `nlist_64`, constants `LC_SEGMENT_64`,
  `LC_SYMTAB`, `SEG_LINKEDIT`.
- `_NSGetExecutablePath` — `service.h:115`.
- `proc_name(pid, buf, size)` + `PROC_PIDPATHINFO_MAXSIZE` — `window.c:174`, `window_manager.c:939`.
- `sysctl(CTL_KERN, KERN_PROC, KERN_PROC_PID, pid)` into `struct kinfo_proc`, testing
  `kp_proc.p_flag & P_TRACED` — `process_manager.c:70-80`.
- `sysctlbyname("kern.bootargs")` + `strnstr` — `sa.m:323-324` (arm64 only).
- POSIX: `socket`/`connect`/`bind`/`listen`/`accept`/`send`/`recv`/`shutdown`/`close` with
  `sockaddr_un` (`helpers.h:183-202`, `message.c:3016-3045`, `yabai.c:84-122`), `fcntl` `F_SETLK`
  with `struct flock` (`yabai.c:164-177`) and `F_SETFD` (`message.c:3039`), `open`, `chmod`, `unlink`,
  `mkdir`, `umask`, `stat`, `opendir`/`closedir`, `fopen`/`fwrite`/`fclose`/`fdopen`/`fflush`,
  `popen`/`pclose` (`sa.m:335-338`), `system` (`sa.m:127-136`, `:198`), `fork`+`execvp`
  (`helpers.h:477-483`), `posix_spawn` + `posix_spawn_file_actions_*` + `waitpid` (`service.h:53-78`),
  `signal(SIGCHLD|SIGPIPE, SIG_IGN)` (`yabai.c:151-152`), `getpwuid` (`sa.m:155`), `getenv`,
  `getuid`/`geteuid`, `usleep`, `mmap`/`mprotect`/`getpagesize` (`ts.h:12-23`, `memory_pool.h:13-24`),
  `regcomp`/`regexec` (`helpers.h:573-579`, `rule.c`),
  `sem_open`/`sem_unlink`/`sem_wait`/`sem_post` (`event_loop.c:1678,1702,1709-1711`),
  `pthread_create`/`pthread_join`/`pthread_mutex_*` (`event_loop.c:1718`, `message.c:3042`,
  `window_manager.c:577,589,666,680,2734`).
- GCD: `dispatch_after(dispatch_time(DISPATCH_TIME_NOW, …), dispatch_get_main_queue(), ^{ … })` at
  `event_loop.c:93`, `:157`, `:1478`, `:1516`, `:1520`.

---

## 4. ABI hazards to get right

**By-value aggregates across the boundary.** These are the ones where a wrong `#[repr]` silently
corrupts arguments:

| Type | Layout | Passed by value to |
| --- | --- | --- |
| `CGPoint` | `{ f64, f64 }` | `SLSCopyBestManagedDisplayForPoint` (`extern.h:45`), `CGWarpMouseCursorPosition`, `CGPostMouseEvent` |
| `CGRect` | `{ CGPoint, CGSize }` = 4×f64 | `SLSCopyBestManagedDisplayForRect` (`:21`) — by value; `SLSGetWindowBounds`/`SLSGetRevealedMenuBarBounds`/`CGSNewRegionWithRect` take `CGRect*` |
| `CGAffineTransform` | 6×f64 | `SLSSetWindowTransform` (`:87`), `SLSTransactionSetWindowTransform` (`:90`) |
| `ProcessSerialNumber` | `{ u32 highLongOfPSN, u32 lowLongOfPSN }` | `SLSSpaceSetFrontPSN` (`:55`), `process_pid_for_psn` (`process_manager.c:24`) |
| `NSOperatingSystemVersion` | 3×`isize` | returned by value from `-operatingSystemVersion` |
| `NSEdgeInsets` | 4×f64 | returned by value from `-safeAreaInsets` |
| `AbsoluteTime`/`Nanoseconds` | `{ u32, u32 }` | `AbsoluteToNanoseconds` |
| `CVTimeStamp` | mixed struct | passed as `const CVTimeStamp*` to the display-link callback |
| `EventTypeSpec` | `{ u32 eventClass, u32 eventKind }` | array of 3 to `InstallEventHandler` |

Rust `extern "C"` + `#[repr(C)]` lowers these through the same LLVM ABI code clang uses, on both
x86_64 SysV and AArch64 AAPCS, so as long as the struct definitions come from
`objc2-core-foundation` (or are `#[repr(C)]` mirrors) this is a non-issue. The rule for phase 2:
**never redefine `CGRect`/`CGPoint`/`CGSize`/`CGAffineTransform` locally**, always import them.

**`bool` vs `Boolean`.** C `bool` (1 byte) is used by `SLSSetWindowOpacity` (`:31`),
`SLSManagedDisplayIsAnimating` (`:46`), `SLSWindowIteratorAdvance` (`:70`) — maps to Rust `bool`.
`Boolean` (CoreServices, `unsigned char`) is used by `CoreDockGetAutoHideEnabled` (`:52`),
`SameProcess`'s out-param, `AXIsProcessTrustedWithOptions` — map to `u8` and compare against 1,
exactly as `psn_equals` does at `helpers.h:538`.

**Variadics.** No SkyLight/CF/AX call is variadic. The variadic C functions are all logging /
formatting: `debug`/`warn`/`error`/`require`/`debug_message` (`misc/log.h`), `notify`
(`misc/notify.h:29`), `daemon_fail` (`message.c`), plus `snprintf`/`fprintf`/`sprintf`. All become
Rust `macro_rules!` over `format_args!` — no `VaList` needed anywhere.

The one genuine `va_list` crossing into ObjC is
`[[NSString alloc] initWithFormat:… arguments:args]` at `notify.h:40`. Phase 2 formats in Rust and
assigns the finished string to `informativeText`; that is byte-identical output, since the C code
also just produces the formatted string.

**Blocks.** Only `dispatch_after` (5 sites). Each block captures a `__block ProcessSerialNumber psn`
by value or nothing at all, and only calls `process_manager_find_process` + `event_loop_post`.

**Function pointers crossing into C.** `connection_callback` (`extern.h:1`),
`observer_callback` (`application.h:4`), `display_callback` (`display.h:4`),
`MOUSE_HANDLER` (`mouse_handler.h:21`), `PROCESS_EVENT_HANDLER` (`process_manager.h:4`),
`CVDisplayLinkOutputCallback` (`window_manager.c:537`), `CFArraySortValues` comparator
(`display_manager.c:180`), plus two `pthread` entry points
(`window_manager.c:507`, `event_loop.c:1647`, `message.c:3003`).
All must be `unsafe extern "C-unwind" fn` and must not panic across the boundary.

---

## 5. Crate selection

Versions are the current stable ones on crates.io as of 2026-09.

| Crate | Version | Verdict |
| --- | --- | --- |
| `objc2` | 0.6.4 | **yes** — `define_class!`, `msg_send!`, `sel!`, `rc::Retained`, `exception::catch`, `MainThreadMarker`, `runtime::{AnyClass, Sel, AnyObject}` |
| `objc2-foundation` | 0.3.2 | **yes** — NSString/NSArray/NSDictionary/NSNumber, NSProcessInfo, NSBundle, NSNotificationCenter, NSDistributedNotificationCenter, NSException, NSKeyValueObserving, NSUserNotification(+Center,+Delegate), NSHomeDirectoryForUser, NSAutoreleasePool, NSOperatingSystemVersion |
| `objc2-app-kit` | 0.3.2 | **yes** — NSApplication/`NSApplicationLoad`, NSWorkspace(+notification names), NSRunningApplication, NSScreen, NSImage, NSApplicationActivationPolicy |
| `objc2-core-foundation` | 0.3.2 | **yes** — all CF types with `CFRetained<T>`, plus `CGRect`/`CGPoint`/`CGSize`/`CGFloat`/`CGAffineTransform` |
| `objc2-core-graphics` | 0.3.2 | **yes** — CGEvent + event taps, CGWindowList, display config, CGContext/CGImage/CGPath/CGColorSpace, CGWindowLevelForKey, screen-capture permission. Gaps: `CGPostMouseEvent`, and the display-UUID pair (see ColorSync) |
| `objc2-application-services` | 0.3.2 | **yes** for the AX *functions* and the Process Manager functions. All `kAX*` string constants and `ProcessSerialNumber`/`ProcessInfoRec`/`GetProcessInformation` are **not** there and are hand-written |
| `objc2-color-sync` | 0.3.2 | **yes** — the only crate that binds `CGDisplayCreateUUIDFromDisplayID` / `CGDisplayGetDisplayIDFromUUID`. Alternative: hand-declare them in the CoreGraphics module with `#[link(name="ColorSync")]`. Prefer the crate; it is 6 features and pulls only `objc2-core-foundation` |
| `objc2-core-video` | 0.3.2 | **yes** — CVDisplayLink + `CVGetHostClockFrequency` |
| `block2` | 0.6.2 | **yes** — only for the 5 `dispatch_after` blocks (and it is a transitive dep of the objc2 framework crates anyway) |
| `dispatch2` | 0.3.1 | **yes** — `DispatchQueue::main()` + `exec_after_with_block` / `after` |
| `libc` | 0.2.189 | **yes** — POSIX, `proc_name`, `_NSGetExecutablePath`, `_dyld_*`, `mach_header_64`, `segment_command_64`, `load_command`, `LC_SEGMENT_64`, `sysctlbyname`, `proc_bsdinfo`/`PROC_PIDTBSDINFO`, `regcomp`/`regexec`, `sem_*`, `posix_spawn*`, `mmap`/`mprotect`. Gaps found: `symtab_command`, `nlist_64`, `LC_SYMTAB`, `SEG_LINKEDIT`, `kinfo_proc`, `P_TRACED`, `getpagesize` |
| `mach2` | 0.7.0 | **yes** — `mach_msg`, `mach_msg_header_t`, `mach_msg_ool_descriptor_t`, all the `MACH_*` bits, `task_get_special_port`, `mach_task_self`, `TASK_BOOTSTRAP_PORT`, `bootstrap_look_up`, `NDR_record`/`NDR_record_t`, `KERN_SUCCESS` |
| `cc` (build-dep) | 1.x | only if we compile the osax through it; `xcrun clang` invoked directly is simpler because of `-arch arm64e` |

**Rejected: the servo `core-foundation` 0.10.1 / `core-graphics` 0.25.0 pair.** They are older-style,
cover less of CoreGraphics (no event taps in `core-graphics`; that lives in a separate
`core-graphics-types` / `cocoa` split), and mixing them with `objc2-*` means two incompatible
`CGRect` definitions in the same crate. One family only.

**Rejected: `cocoa` / `objc` (0.2).** Unmaintained, `objc_msgSend` ABI unsound on aarch64 for
struct returns. `objc2` supersedes both.

**Hand-written `extern "C"` blocks** (no crate exists or the crate has a gap): SkyLight in full,
Carbon Event Manager in full, `ProcessSerialNumber`/`ProcessInfoRec`/`GetProcessInformation`,
`AbsoluteToNanoseconds`, every `kAX*` CFString, `CGPostMouseEvent`, `csr_get_active_config`,
`mig_get_special_reply_port`, `symtab_command`/`nlist_64`, the `kCGS*` event-field numbers.

---

## 6. Linking and build

### 6.1 SkyLight

`SkyLight.framework` ships a stub in the SDK
(`.../MacOSX.sdk/System/Library/PrivateFrameworks/SkyLight.framework/Versions/A/SkyLight.tbd`), so it
links the same way the makefile does it. Two halves:

`build.rs`:

```
println!("cargo:rustc-link-search=framework=/System/Library/PrivateFrameworks");
```

(the linker re-roots absolute `-F` paths through `-syslibroot`, which is why the absolute system path
works against the SDK stub).

In `src/ffi/skylight.rs`:

```
#[link(name = "SkyLight", kind = "framework")]
unsafe extern "C-unwind" { … }
```

Same pattern, different framework name, for the small hand-written blocks:
`#[link(name = "Carbon", kind = "framework")]`,
`#[link(name = "ApplicationServices", kind = "framework")]` (for `_AXUIElementGetWindow`,
`_AXUIElementCreateWithRemoteToken`, `CoreDock*`, `GetProcessInformation`),
`#[link(name = "CoreGraphics", kind = "framework")]` (for `CGPostMouseEvent`,
`CGRegionCreateEmptyRegion`, `CGSNewRegionWithRect`),
`#[link(name = "CoreServices", kind = "framework")]` (for `AbsoluteToNanoseconds`).
Duplicate `-framework` flags from the objc2 crates are harmless.

### 6.2 Deployment target

The makefile passes `-mmacosx-version-min=11.0` to every compile (`makefile:4`). In Cargo this is
`MACOSX_DEPLOYMENT_TARGET=11.0` in `.cargo/config.toml` `[env]` — rustc reads it for the
`LC_BUILD_VERSION` load command and `cc`/`clang` read it in `build.rs`.

Consequences of 11.0 that phase 2 must honour:
- `workspace.m:129` guards `safeAreaInsets` behind `__builtin_available(macos 12.0, *)`. Rust has no
  `@available`; the equivalent is a runtime check. The existing
  `workspace_is_macos_*` flags (`workspace.h:4-19`, set from `NSProcessInfo` at `workspace.m:3-6`)
  already carry the version, so the port becomes `if !workspace_is_macos_bigsur() { … }` — but note
  `safeAreaInsets` is an NSScreen property that objc2-app-kit exposes unconditionally, so calling it
  on 11.0 would be an unrecognised selector. Keep a guard.
- `objc2-*` 0.3.2 crates are compiled against a recent SDK but gate nothing on the deployment target;
  availability is the caller's problem, same as in C.
- `CGPreflightScreenCaptureAccess` is 10.15+, fine.

### 6.3 Universal binary

Cargo cannot emit a fat binary. The makefile's `-arch x86_64 -arch arm64` becomes two builds plus
`lipo`:

```
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
lipo -create -output bin/yabai \
    target/x86_64-apple-darwin/release/yabai \
    target/aarch64-apple-darwin/release/yabai
```

Keep this in the existing `makefile` (or a `cargo xtask`), not in `build.rs`.
`cargo-lipo` is iOS-only and abandoned; do not use it.

### 6.4 Embedded `__info_plist`

`makefile:4` ends with `-sectcreate __TEXT __info_plist $(INFO_PLIST)`. In Cargo:

```
println!("cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,{abs_path_to_assets_Info_plist}");
```

`-link-arg-bins` (not `-link-arg`) so it does not leak into build-script or test link lines.
The path must be absolute — resolve it from `CARGO_MANIFEST_DIR` in `build.rs`.

### 6.5 The scripting addition

`makefile:32-38` builds `src/osax/payload.m` (`-arch x86_64 -arch arm64e`, linking SkyLight,
Foundation, Carbon) and `src/osax/loader.m` (`-arch x86_64 -arch arm64e`, linking Cocoa), then
`xxd -i` both into `payload_bin.c` / `loader_bin.c`, which define
`__src_osax_payload[]`/`__src_osax_payload_len` and `__src_osax_loader[]`/`__src_osax_loader_len`
(declared at `src/sa.h:4-7`, written to disk at `sa.m:222-228`).

Rust replacement in `build.rs`:

1. `xcrun clang src/osax/payload.m -shared -fPIC -O3 -mmacosx-version-min=11.0 -arch x86_64 -arch arm64e -o $OUT_DIR/payload -F/System/Library/PrivateFrameworks -framework SkyLight -framework Foundation -framework Carbon`
2. `xcrun clang src/osax/loader.m -O3 -mmacosx-version-min=11.0 -arch x86_64 -arch arm64e -o $OUT_DIR/loader -framework Cocoa`
3. `println!("cargo:rerun-if-changed=src/osax/payload.m");` and the same for `loader.m`, `common.h`.
4. In `sa.rs`: `static OSAX_PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload"));`

No `xxd` step — `include_bytes!` replaces it and gives the length for free.
`-arch arm64e` is the reason this stays C; it must be passed to `clang` directly, and it applies only
to these two artifacts, never to the Rust crate.

Also mirror `makefile:7` `OSAX_VERSION` handling: `src/osax/common.h:7` defines `"2.1.30"` and both
`sa.m` (plist strings, version check) and the payload consume it. Phase 2 should either `include!` a
generated constant from `build.rs` parsing `common.h`, or duplicate the literal with a build-time
assertion — the daemon and the payload must never disagree (`sa.m:281`).

---

## 7. The hard parts

### 7.1 `macho_find_symbol` (`src/misc/macho_dlsym.h`)

Walks the loaded images for a path match (`:9`), takes the ASLR slide (`:10`), finds `__LINKEDIT`
(`:27`) and `LC_SYMTAB` (`:45`), then scans `nlist_64` entries comparing
`strtab + n_un.n_strx` (`:73`) and returns `n_value + slide` (`:75`). This is how yabai reaches
*local* (non-exported) symbols, which `dlsym` cannot do.

Rust port: straight pointer arithmetic in one `unsafe` function. `libc` gives `_dyld_image_count`,
`_dyld_get_image_name`, `_dyld_get_image_header`, `_dyld_get_image_vmaddr_slide`, `mach_header_64`,
`load_command`, `segment_command_64`, `LC_SEGMENT_64`. Missing and hand-declared:

```
#[repr(C)] struct symtab_command { cmd: u32, cmdsize: u32, symoff: u32, nsyms: u32, stroff: u32, strsize: u32 }
#[repr(C)] struct nlist_64 { n_strx: u32, n_type: u8, n_sect: u8, n_desc: u16, n_value: u64 }
const LC_SYMTAB: u32 = 0x2;
const SEG_LINKEDIT: &CStr = c"__LINKEDIT";
```

Watch `segname` comparison: it is a `[c_char; 16]` that may not be NUL-terminated when the name is
exactly 16 bytes; `string_equals` at `:27` relies on `strcmp` and the C struct is zero-padded here, so
compare the first 10 bytes against `b"__LINKEDIT"` and require the 11th to be 0.

The two symbols resolved: `_CGSGetConnectionPortById` and the mangled
`__ZL54SLSPerformAsynchronousBridgedWindowManagementOperation…` (`yabai.c:148-149`). Both become
`static AtomicPtr<c_void>` (or a `OnceLock<Option<fn…>>`) because the code tests them for null before
calling (`window.c:956`, `space_manager.c:667`, `:688`).

### 7.2 The workspace observer class

`objc2::define_class!` with `#[unsafe(super(NSObject))]`, `#[name = "workspace_context"]`,
`#[ivars = ()]`, and one `#[unsafe(method(selector:))]` per selector, exactly matching
`workspace.m:153-303`. The class must keep the same ObjC name because
`addObserver:selector:name:object:` looks up by selector on the instance, not by class name — but the
name also shows in crash logs, so keep it identical.

`observeValueForKeyPath:ofObject:change:context:` takes a raw `void *context` that is a
`*mut process` (`workspace.m:212`, `:237`). In Rust that stays `*mut Process` and the handler does the
same `terminated` check. KVO registration is `NSKeyValueObserverRegistration` on `NSObject`
(`objc2-foundation`, `NSKeyValueObserving` feature).

`@try { … } @catch (NSException *) {}` (8 sites) → `let _ = objc2::exception::catch(|| unsafe { … });`
(`pub fn catch<R>(closure: impl FnOnce() -> R + UnwindSafe) -> Result<R, Option<Retained<Exception>>>`).
The closure must be `UnwindSafe`; raw pointers are, so this is mechanical.

### 7.3 `dispatch_after` blocks

```
DispatchQueue::main().exec_after_with_block(Duration::from_secs_f32(0.1), &block);
```
with `block2::RcBlock::new(move || { … })`. The `0.0f` case at `event_loop.c:1520` is a
`Duration::ZERO`, which `dispatch_after` treats as "as soon as possible on the main queue" — keep
`exec_after` rather than switching to `exec_async`, to preserve ordering semantics exactly.
The captured `__block ProcessSerialNumber psn` becomes a plain `move` capture of a `Copy` struct.

### 7.4 `ax_window_pid` (`src/misc/helpers.h:506-509`)

```
return *(pid_t *)((void *) ref + 0x10);
```
Reads a `pid_t` at offset 0x10 inside the opaque `AXUIElement`. Used at `event_loop.c:559`.
There is no safe equivalent (`AXUIElementGetPid` is the public call, but it is a different code path
and yabai deliberately avoids it). Port verbatim as
`unsafe { *(ptr.cast::<u8>().add(0x10).cast::<libc::pid_t>()) }` and leave a note — this is the single
most fragile line in the FFI layer.

### 7.5 `SLSGetWindowSubLevel__Internal` (`src/window.c:930-952`)

A hand-rolled MIG call: `#pragma pack(push,4)` struct of
`mach_msg_header_t` + `NDR_record_t` + `u32 window_id` + `i32 sub_level` + two `i32` pads,
`msgh_bits = 0x1513`, remote port from `CGSGetConnectionPortById(cid)`, local port from
`mig_get_special_reply_port()`, `msgh_id` 0x76E3 on Tahoe else 0x73C3, then
`mach_msg(&header, MACH_SEND_MSG|MACH_RCV_MSG, 0x24, 0x30, local_port, 0, 0)`.

`#pragma pack(4)` has no direct Rust equivalent; `#[repr(C, packed(4))]` is the right spelling.
Verify the resulting size is 0x30 with a `const` assertion, because the literal `0x24`/`0x30` sizes
are load-bearing. Same for the OOL send struct at `helpers.h:206-210` (`mach_send`), whose
`sizeof(msg)` is passed as the send size.

### 7.6 `objc_msgSend` cast at `space_manager.c:669-672`

```
id operation = ((id (*)(id, SEL, id, uint64_t))objc_msgSend)([cls alloc], sel, (__bridge id)window_list_ref, sid);
```

In objc2 this is `msg_send![…]` only if the class were known at compile time; it is not. The port is
`AnyClass::get(c"SLSBridgedMoveWindowsToManagedSpaceOperation")`, `Sel::register(c"initWithWindows:spaceID:")`,
`msg_send![cls, alloc]` then `msg_send![alloc_obj, initWithWindows: cf_array, spaceID: sid]`
through `objc2::runtime::AnyObject` with an explicit selector — objc2 supports dynamic selectors via
`msg_send![obj, performSelector: sel]`-style only for zero/one-arg, so the honest port is a
`transmute` of `objc2::ffi::objc_msgSend` to the exact fn type, same as the C. Keep the cast; it is
correct on both arches for pointer-sized args.

### 7.7 SIMD in `cgimage_restore_alpha` (`src/misc/helpers.h:588-675`)

Two arch-specific bodies (`_mm_*` for x86_64, `v*q_*` for arm64) that un-premultiply alpha over a
bitmap. Rust equivalents: `core::arch::x86_64` / `core::arch::aarch64` intrinsics one-for-one under
`#[cfg(target_arch = …)]`. `vdivq_f32` and `vcvtnq_s32_f32` both exist in `core::arch::aarch64`.
Note the loop at `:613` steps 4 pixels at a time over `height*width` without a tail guard — it
over-reads up to 3 pixels when `width*height % 4 != 0`; the allocation at `:593` is
`height*pitch = height*width*4` bytes so the over-read stays in-bounds only by luck of the 4-byte
stride. Reproduce as-is (behaviour-preserving) but flag it.

### 7.8 `kAX*` constants

Roughly 35 `CFSTR("…")` macros with no binding. The idiom in objc2 is
`CFString::from_static_str("AXWindows")` at first use behind a `OnceLock`, or a small
`cfstr!` macro that builds a `&'static CFString` from a literal. The C code compares them with
`CFEqual` (`application.c:8-27`, `mission_control.c:61-68`), never by pointer, so a non-interned
CFString is fine.

### 7.9 `__attribute((cleanup))` and the profiler

`misc/timer.h:136-142` uses `__attribute((cleanup(END_TIME_BLOCK)))` for `TIME_FUNCTION` /
`TIME_BLOCK`. The whole file is behind `#if PROFILE >= 1` and is a no-op in the shipped build
(`makefile` never defines `PROFILE`). Port as a `Drop` guard behind a Cargo feature, or drop it; the
`TIME_FUNCTION;` call sites in `window_manager.c`/`message.c`/`event_loop.c` expand to nothing today.

### 7.10 `process_is_being_debugged` (`src/process_manager.c:70-80`)

`libc` (aarch64-apple-darwin) has no `kinfo_proc` and no `P_TRACED`. Two options:
1. Faithful: hand-declare enough of `struct extern_proc` to reach `p_flag`. On LP64 it is at
   offset 32 (`p_un` 16 bytes, `p_vmspace` 8, `p_sigacts` 8), and `P_TRACED = 0x00000800`. Read
   `sysctl` into a `[u8; size_of::<kinfo_proc>()]` and index. Brittle.
2. Equivalent: `proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &mut proc_bsdinfo, …)` then
   `pbi_flags & PROC_FLAG_TRACED`. `libc` has `proc_bsdinfo` and `PROC_PIDTBSDINFO` on Apple targets.
   Same answer, no layout guessing.

Recommend (2) with a note in the module's `AGENTS.md`; flag for the user since it is the one place a
faithful transposition is genuinely worse than the alternative.

---

## 8. Proposed FFI layer layout

One module per *linked framework*, written first in phase 2, depended on by every other module.
Nothing here holds state except the two runtime-resolved pointers.

```
src/ffi/mod.rs
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

### `skylight.rs`
`#[link(name = "SkyLight", kind = "framework")]`. Contains, in `extern.h` order, every declaration
from `src/misc/extern.h:6-97` **except** lines 4-5 (dynamic), 7-8 (HIServices), 38-40 (CoreGraphics /
ColorSync), 52-53 and 64 (HIServices CoreDock). That is 82 functions. Also the
`ConnectionCallback` type alias from `extern.h:1-2`, and the yabai-local
`kCPSAllWindows`/`kCPSUserGenerated`/`kCPSNoWindows` from `window_manager.h:4-6`.

### `skylight_dynamic.rs`
The two `macho_find_symbol`-resolved pointers (`extern.h:4-5`, filled at `yabai.c:148-149`):
`CGSGetConnectionPortById: unsafe extern "C" fn(c_int) -> mach_port_t` and
`SLSPerformAsynchronousBridgedWindowManagementOperation: unsafe extern "C" fn(*mut c_void) -> i64`,
each behind an `Option<…>` accessor so callers can keep the null test. Plus the two literal
symbol/image strings.

### `core_graphics.rs`
Re-exports from `objc2_core_graphics` (so no other module imports the crate directly), plus a
hand-written block under `#[link(name = "CoreGraphics", kind = "framework")]` for
`CGPostMouseEvent`, `CGRegionCreateEmptyRegion` (`extern.h:38`), `CGSNewRegionWithRect`
(`extern.h:39`). Plus the private `kCGS*` numeric event fields yabai spells inline as comments:
`kCGSEventTypeField = 55`, `kCGSEventDockControl = 30`, `kCGEventGestureHIDType = 110`,
`kCGEventGestureSwipeMotion = 123`, `kCGEventGestureSwipeProgress = 124`,
`kCGEventGestureSwipeVelocityX = 129`, `kCGEventGesturePhase = 132`,
`kIOHIDEventTypeDockSwipe = 23`, `kCGGestureMotionHorizontal = 1`, `kCGSGesturePhaseBegan = 1`,
`kCGSGesturePhaseEnded = 4`, `kCGSGesturePhaseCancelled` (`space_manager.c:959-969`,
`mouse_handler.c:69`, `mouse_handler.h:11,19`).

### `color_sync.rs`
`CGDisplayCreateUUIDFromDisplayID`, `CGDisplayGetDisplayIDFromUUID` — re-exported from
`objc2-color-sync`.

### `accessibility.rs`
Re-exports the AX functions/types from `objc2_application_services`; defines all ~35 `kAX*`
CFString constants (§3.3); defines the private
`_AXUIElementGetWindow` / `_AXUIElementCreateWithRemoteToken` under
`#[link(name = "ApplicationServices", kind = "framework")]`; defines the `ObserverCallback` alias
matching `application.h:4`; ports `ax_privilege` (`helpers.h:489`), `ax_window_id` (`:499`),
`ax_window_pid` (`:506`), `ax_enhanced_userinterface` (`:511`) and the
`AX_ENHANCED_UI_WORKAROUND` macro (`:524-530`) as a closure-taking helper; the `ax_error_str` table
(`application.h:26-44`).

### `carbon_process.rs`
`#[repr(C)] ProcessSerialNumber`, `ProcessInfoRec`, `kNoProcess`; re-export `GetProcessPID`,
`GetNextProcess`, `CopyProcessName`, `SameProcess`, `IsProcessVisible` from
`objc2-application-services`; hand-declare `GetProcessInformation`; `psn_equals` (`helpers.h:534`).

### `carbon_events.rs`
`#[link(name = "Carbon", kind = "framework")]`: `EventRef`, `EventHandlerRef`, `EventHandlerCallRef`,
`EventTargetRef`, `EventHandlerUPP`, `EventTypeSpec`, `EventTime`, `OSStatus`, `noErr`,
`GetApplicationEventTarget`, `NewEventHandlerUPP`, `InstallEventHandler`, `GetEventParameter`,
`GetEventKind`, `GetCurrentEventTime`, and the four-char-code constants
`kEventClassApplication`, `kEventAppLaunched`, `kEventAppTerminated`, `kEventAppFrontSwitched`,
`kEventParamProcessID`, `typeProcessSerialNumber`.

### `carbon_core.rs`
`AbsoluteTime`/`Nanoseconds` as `#[repr(C)] { lo: u32, hi: u32 }`, `AbsoluteToNanoseconds` under
`#[link(name = "CoreServices", kind = "framework")]`, and `read_os_timer` / `read_os_freq`
(`helpers.h:149-160`).

### `core_video.rs`
Re-exports `CVDisplayLink`, the four lifecycle functions, `CVTimeStamp`, `CVReturn`, `CVOptionFlags`,
`kCVReturnSuccess`, `CVGetHostClockFrequency`, `CVDisplayLinkOutputCallback`.

### `core_foundation.rs`
Re-exports every CF type/function used, and the helpers that are pure CF glue in `helpers.h`:
`cfstring_copy` (`:373`), `ts_cfstring_copy` (`:361`), `CFSTRINGNUM32` (`:321`), `CFNUM32` (`:328`),
`cfarray_of_cfnumbers` (`:344`), `sls_window_disable_shadow` (`:333` — lives here because it is
CF-dictionary construction wrapping one SkyLight call).

### `appkit.rs` / `foundation.rs`
Thin re-export modules so `workspace.rs`, `sa.rs`, `notify.rs`, `service.rs` import from one place:
NSApplication/`NSApplicationLoad`/`NSApp`, NSWorkspace + the five notification-name constants and the
four string-literal names, NSRunningApplication, NSScreen, NSImage, NSProcessInfo, NSBundle,
NSNotificationCenter, NSDistributedNotificationCenter, NSException, NSUserNotification(+Center,
+Delegate), NSHomeDirectoryForUser, `NSKeyValueObservingOptions`, `NSKeyValueChangeNewKey`.

### `mach_port.rs`
Re-exports from `mach2`; `mach_send` (`helpers.h:204-223`) with its `#[repr(C)]` OOL message struct;
`mig_get_special_reply_port` under `#[link(name = "System", kind = "dylib")]` (or a bare extern —
libSystem is always linked); the `SLSGetWindowSubLevel__Internal` MIG struct
(`window.c:930-952`) belongs in `window.rs`, not here, since it is one call site — but the
`#[repr(C, packed(4))]` pattern and the `NDR_record` import come from here.

### `macho.rs`
`symtab_command`, `nlist_64`, `LC_SYMTAB`, `SEG_LINKEDIT`, and the three private finders plus
`macho_find_symbol`, transposed from `src/misc/macho_dlsym.h` one-for-one.

### `libsystem.rs`
`csr_get_active_config` + `CSR_ALLOW_UNRESTRICTED_FS`/`CSR_ALLOW_TASK_FOR_PID` (`sa.m:3-5`),
`_NSGetExecutablePath`, `proc_name`, `PROC_PIDPATHINFO_MAXSIZE`, the process-traced check, and
`getpagesize`.

### `dispatch.rs`
One function: `post_to_main_queue_after(delay: Duration, work: impl Fn() + 'static)` wrapping
`DispatchQueue::main()` + `RcBlock`, so the five call sites in `event_loop.rs` read like the C.

---

## 9. Open questions for the user

1. **`kinfo_proc` vs `proc_pidinfo`** (§7.10). Faithful port needs a hand-guessed struct offset;
   the equivalent-behaviour port uses a supported API. Which?
2. **`objc2-color-sync` as a dependency** just for two functions, versus hand-declaring them with
   `#[link(name = "ColorSync", kind = "framework")]`. The crate is tiny but it is one more thing to
   keep in step with the rest of the 0.3.2 family.
3. **`misc/autorelease.h`** is dead (commented out at `manifest.m:49`, `#if 0` at `yabai.c:158-162`).
   Drop it entirely, or port it behind a feature?
4. **`misc/timer.h` profiler** is compiled out in every shipped configuration. Drop, or port behind a
   Cargo feature?
5. **Seven dead `extern.h` declarations** (§1). Keep as `#[allow(dead_code)]` for fidelity, or prune?
6. **The `asan` / `tsan` makefile targets** (`makefile:20-26`) have no direct Cargo equivalent
   (`-Zsanitizer` is nightly-only). Drop them, keep them as a nightly-gated xtask, or leave the C
   build around for that?
7. **`NSUserNotification`** is deprecated since 10.14 and Apple has signalled removal. It still exists
   in SDK 26.1 and in `objc2-foundation` 0.3.2, so a faithful port is possible — but is this the
   moment to move `notify()` to `UserNotifications.framework`? (That would change behaviour: it
   requires a signed bundle, which the yabai binary is not.)
