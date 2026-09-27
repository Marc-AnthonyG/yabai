# Memory, text and OS objects

Binding elaboration of `DECISIONS.md` 18, 28, 33, 34, 36, and of the CoreFoundation halves of 11
and 19, for phase 2 of the Rust rewrite. Everything here is settled: a translator reading this
file never picks between two shapes. Where an inventory in `files/` or `sweeps/` proposes
something else, this file and `DECISIONS.md` win.

Rust sketches follow `DECISIONS.md` 37-39: names are never abbreviated, and the only comments
inside a sketch are comments that exist in the C source at that place.

Modules this document fixes the shape of:

| C file | Rust module |
|---|---|
| `src/misc/helpers.h` | `src/misc/helpers.rs` |
| `src/misc/log.h` | `src/misc/log.rs` |
| `src/misc/notify.h` | `src/misc/notify.rs` |
| `src/misc/service.h` | `src/misc/service.rs` |
| `src/misc/macho_dlsym.h` | `src/ffi/macho.rs` |
| `src/misc/helpers.h:321,328,333,344,361,373` plus CF glue with no C counterpart | `src/ffi/core_foundation.rs` |

This document adds no module of its own. `TRANSLATION_PLAN.md` §1.5 and
`patterns/ffi-objc-and-os.md` §13 own `src/ffi/core_foundation.rs`; §5 of that document owns
`src/ffi/macho.rs`. Both are built by one work unit, `W0-5`.

The CoreFoundation ownership rules are spread across `helpers.h`, `window.c`, `space.c`,
`space_manager.c`, `display.c`, `display_manager.c`, `event_loop.c` and `process_manager.c`, and
phase 2 needs exactly one place that knows them: `src/ffi/core_foundation.rs`. §1.4 below fixes
the bodies of the ten helpers it grows beyond the six `helpers.h` functions `ffi-objc-and-os.md`
§13 already assigns to it. Every other module calls into it and never touches a raw `CFTypeRef`,
which is also why `objc2-core-foundation` is imported there and nowhere else — `W0-5`'s gate is
that no `objc2-*` crate is imported outside `src/ffi/`.

Sections: 1 CoreFoundation and Accessibility objects · 2 Text is owned `String` ·
3 Fixed `char` buffers and `snprintf` · 4 Process-wide values written once ·
5 Cleanup: `goto` ladders, RAII guards and labelled blocks ·
6 Logging: `debug!`, `warn!`, `error!`, `require!` · 7 `notify` · 8 `src/misc/service.h` ·
9 The easing curves and their X-macro · 10 `cgimage_restore_alpha` · 11 `macho_find_symbol` ·
12 The lock file and the socket helpers · 13 `DEVIATIONS.md` lines this document commits to.

---

## 1. CoreFoundation and Accessibility objects

### 1.1 The two wrapper types

There are exactly two, both from `objc2-core-foundation` (`DECISIONS.md` 11 — no servo
`core-foundation`, no hand-rolled `CfOwned`):

- **Owned:** `CFRetained<T>`. It holds one `+1` reference and calls `CFRelease` in `Drop`. It is
  the return type of every Rust function that translates a C function returning an object the
  caller had to `CFRelease`.
- **Borrowed:** `&T`. It holds no reference at all. It is what a get-rule lookup produces, and it
  may not outlive the container it came out of.

`T` is one of `objc2_core_foundation::{CFString, CFArray, CFDictionary, CFNumber, CFBoolean,
CFType}`, or one of the hand-declared opaque types under `src/ffi/` for anything SkyLight or
Accessibility hands back (`AXUIElementRef`, `AXObserverRef`, `AXValueRef`).

There is no third type. In particular there is no wrapper that is "owned but might be null": a
create/copy that can fail returns `Option<CFRetained<T>>`, and the `Option` is the null.

### 1.2 The create/copy rule

A C call whose name contains `Create` or `Copy` returns `+1`. In Rust it becomes:

```rust
pub unsafe fn take_create_rule_result<T>(pointer: *const T) -> Option<CFRetained<T>> {
    NonNull::new(pointer.cast_mut()).map(|non_null| unsafe { CFRetained::from_raw(non_null) })
}
```

`CFRetained::from_raw` **adopts** the existing reference; it does not add one. Using
`CFRetained::retain` here instead would leak one reference per call, and on the hot paths
(`window_ax_frame`, `window_title`, `space_window_list`) that leak is per window per event.

Every C site that pairs a `Create`/`Copy` with a `CFRelease` therefore loses both the `CFRelease`
and the `goto` label that carried it (§5.1). The sites are:

| C location | C call | Rust |
|---|---|---|
| `src/misc/helpers.h:328-331` | `CFNumberCreate` | `CFNumber::new_i32(num)` → `CFRetained<CFNumber>` |
| `src/misc/helpers.h:321-326` | `CFStringCreateWithCString` | dead, see §1.10 |
| `src/misc/helpers.h:338` | `CFDictionaryCreate` | `CFRetained<CFDictionary>` |
| `src/misc/helpers.h:352` | `CFArrayCreate` | `CFRetained<CFArray>` |
| `src/misc/helpers.h:493` | `CFDictionaryCreate` | `CFRetained<CFDictionary>` |
| `src/misc/helpers.h:516` | `AXUIElementCopyAttributeValue` | `Option<CFRetained<CFType>>` |
| `src/window.c:71,92` | `SLSCopySpacesForWindows` | `Option<CFRetained<CFArray>>` |
| `src/window.c:847,876,904,966` | `cfarray_of_cfnumbers` | `CFRetained<CFArray>` |
| `src/space.c:24` | `cfarray_of_cfnumbers` | `CFRetained<CFArray>` |
| `src/space_manager.c:496,526,562,591,668` | `SLSCopyManagedDisplaySpaces` / `cfarray_of_cfnumbers` | `Option<CFRetained<CFArray>>` |
| `src/display.c:197,225` | `SLSCopyManagedDisplaySpaces` | `Option<CFRetained<CFArray>>` |
| `src/display_manager.c:173,202` | `SLSCopyManagedDisplays` | `Option<CFRetained<CFArray>>` |
| `src/display_manager.c:214` | `CFRetain(CFArrayGetValueAtIndex(...))` | see §1.3 |
| `src/event_loop.c:1489` | `CGWindowListCopyWindowInfo` | `Option<CFRetained<CFArray>>` |
| `src/process_manager.c:112` | `cfarray_of_cfnumbers` | `CFRetained<CFArray>` |
| `src/misc/helpers.h:592,601` | `CGColorSpaceCreateDeviceRGB`, `CGBitmapContextCreate` | see §10 |

### 1.3 The get rule

A C call whose name contains neither `Create` nor `Copy` returns a borrowed reference that the
caller must not release: `CFArrayGetValueAtIndex`, `CFDictionaryGetValue`, `CFStringGetCString`'s
argument, `CFBooleanGetValue`'s argument. In Rust these produce `Option<&T>` tied to the lifetime
of the container, and **never** a `CFRetained`. Wrapping a get-rule result in `CFRetained` is an
over-release: the container drops it once and the wrapper drops it again.

The one place the C converts a get-rule result into an owned one is
`src/display_manager.c:214`:

```c
result = CFRetain(CFArrayGetValueAtIndex(displays, index));
```

That is a deliberate promotion, and it is the only correct use of `CFRetained::retain` in the
tree:

```rust
let display_uuid = unsafe { CFRetained::retain(NonNull::from(borrowed_display_uuid)) };
```

### 1.4 The CF glue in `src/ffi/core_foundation.rs`, in full

`src/ffi/core_foundation.rs` is the only code in the daemon that dereferences a `CFTypeRef`. The
module belongs to `ffi-objc-and-os.md` §13; what follows are the ten helpers and two encoding
constants this document adds to it, on top of the six `helpers.h` functions that document already
puts there. Their signatures are fixed; their bodies call `objc2-core-foundation`. If a method
name in the pinned crate version differs from the one written here, the fix goes inside this
module and never at a call site.

```rust
use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::OnceLock;
use objc2_core_foundation::{
    CFArray, CFBoolean, CFDictionary, CFNumber, CFNumberType, CFRetained, CFString, CFType,
};

pub unsafe fn take_create_rule_result<T>(pointer: *const T) -> Option<CFRetained<T>> {
    NonNull::new(pointer.cast_mut()).map(|non_null| unsafe { CFRetained::from_raw(non_null) })
}

pub fn cfarray_count(array: &CFArray) -> isize {
    array.count()
}

pub unsafe fn cfarray_borrow_value_at_index<T>(array: &CFArray, index: isize) -> Option<&T> {
    let pointer: *const c_void = unsafe { array.value_at_index(index) };
    unsafe { (pointer as *const T).as_ref() }
}

pub unsafe fn cfdictionary_borrow_value<T>(
    dictionary: &CFDictionary,
    key: &CFString,
) -> Option<&T> {
    let pointer: *const c_void =
        unsafe { dictionary.value(key as *const CFString as *const c_void) };
    unsafe { (pointer as *const T).as_ref() }
}

pub fn cfnumber_read_u64_widening(number: &CFNumber) -> u64 {
    let mut destination: u64 = 0;
    let number_type: CFNumberType = number.type_of();
    unsafe {
        number.value(
            number_type,
            &mut destination as *mut u64 as *mut c_void,
        );
    }
    destination
}

pub fn cfnumber_read_i32(number: &CFNumber) -> i32 {
    let mut destination: i32 = 0;
    unsafe {
        number.value(
            CFNumberType::SInt32Type,
            &mut destination as *mut i32 as *mut c_void,
        );
    }
    destination
}

pub fn cfboolean_get_value(boolean: &CFBoolean) -> bool {
    boolean.value()
}

pub fn cfstring_to_string(string: &CFString) -> Option<String> {
    let maximum_size_in_bytes =
        CFString::maximum_size_for_encoding(string.length(), K_CF_STRING_ENCODING_UTF8);
    let mut buffer: Vec<u8> = vec![0; (maximum_size_in_bytes + 1) as usize];
    let converted = unsafe {
        string.c_string(
            buffer.as_mut_ptr() as *mut core::ffi::c_char,
            maximum_size_in_bytes + 1,
            K_CF_STRING_ENCODING_UTF8,
        )
    };
    if !converted {
        return None;
    }
    let nul_position = buffer
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(buffer.len());
    buffer.truncate(nul_position);
    Some(String::from_utf8_lossy(&buffer).into_owned())
}

pub fn cfstring_from_str(text: &str) -> CFRetained<CFString> {
    CFString::from_str(text)
}

pub fn as_cftype<T>(object: &T) -> &CFType {
    unsafe { &*(object as *const T as *const CFType) }
}

pub const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
pub const K_CF_STRING_ENCODING_MAC_ROMAN: u32 = 0;
```

The two constants are the numeric values of the `kCFStringEncodingUTF8` and
`kCFStringEncodingMacRoman` items `ffi-objc-and-os.md` §13 lists. CoreFoundation is handed the
number, so this spelling is the one the module uses and no call site depends on the crate's enum
shape.

### 1.5 Reading a CFArray of CFNumber

The shape appears eight times. `src/window.c:96-106` is the canonical one and every other site is
written the same way:

```c
    *count = CFArrayGetCount(space_list_ref);
    if (!*count) goto out;

    space_list = ts_alloc_list(uint64_t, *count);

    for (int i = 0; i < *count; ++i) {
        CFNumberRef id_ref = CFArrayGetValueAtIndex(space_list_ref, i);
        CFNumberGetValue(id_ref, CFNumberGetType(id_ref), space_list + i);
    }
```

becomes

```rust
let count = core_foundation::cfarray_count(&space_list_ref) as usize;
let mut space_list: Vec<u64> = Vec::with_capacity(count);
for index in 0..count {
    let id_ref: &CFNumber =
        unsafe { core_foundation::cfarray_borrow_value_at_index(&space_list_ref, index as isize) }.unwrap();
    space_list.push(core_foundation::cfnumber_read_u64_widening(id_ref));
}
```

Three rules that are not negotiable:

1. The loop counter in C is `int` and the count comes back as `CFIndex` (`isize`). Cast the count
   once at the top, as above, and never mix the two inside the loop.
2. `CFArrayGetValueAtIndex` gives a borrowed value; it is `&CFNumber`, never `CFRetained<CFNumber>`.
3. A `None` from `cfarray_borrow_value_at_index` cannot happen for an in-range index into a
   CFArray built by CoreFoundation, and the C dereferences the result unconditionally. Use
   `.unwrap()`, not a silent `continue`: a `continue` would shorten the list and change
   `space_window_list`'s output.

The array sites: `src/window.c:78-79`, `src/window.c:102-103`, `src/space.c:33-36`,
`src/space_manager.c:506-508`, `src/space_manager.c:536-538`, `src/space_manager.c:572-574`,
`src/event_loop.c:1377`, `src/display.c:241-243`.

### 1.6 CFDictionary lookups

`src/space_manager.c:501-508` is canonical:

```c
        CFDictionaryRef display_ref = CFArrayGetValueAtIndex(display_spaces_ref, i);
        CFArrayRef spaces_ref = CFDictionaryGetValue(display_ref, CFSTR("Spaces"));
        int spaces_count = CFArrayGetCount(spaces_ref);
```

becomes

```rust
let display_ref: &CFDictionary =
    unsafe { core_foundation::cfarray_borrow_value_at_index(&display_spaces_ref, index) }.unwrap();
let spaces_ref: &CFArray =
    unsafe { core_foundation::cfdictionary_borrow_value(display_ref, key_spaces()) }.unwrap();
let spaces_count = core_foundation::cfarray_count(spaces_ref);
```

The C dereferences `spaces_ref` without a NULL check at `src/space_manager.c:503`,
`:533`, `:569`, `:598`, `src/display.c:207`, `:235` — a missing `"Spaces"` key is a NULL passed to
`CFArrayGetCount`, which is undefined behaviour (`DECISIONS.md` 4). `.unwrap()` is the safe
equivalent and turns the crash into a panic that the hook in `main` converts to an abort
(`DECISIONS.md` 7). Record one line in `DEVIATIONS.md` per site group.

Where the C **does** check, keep the check. `src/event_loop.c:1496-1503` checks three lookups in a
row and `continue`s:

```rust
let name: Option<&CFString> = unsafe { core_foundation::cfdictionary_borrow_value(dictionary, k_cg_window_name()) };
if name.is_some() { continue; }

let Some(owner) = (unsafe { core_foundation::cfdictionary_borrow_value::<CFString>(dictionary, k_cg_window_owner_name()) }) else { continue; };

let Some(layer_ref) = (unsafe { core_foundation::cfdictionary_borrow_value::<CFNumber>(dictionary, k_cg_window_layer()) }) else { continue; };
```

### 1.7 The `CFNumberGetValue` width trap

`CFNumberGetValue(number, type, destination)` writes exactly as many bytes as `type` names. The
daemon uses two forms and they are not interchangeable.

**Form A — the number's own type, widening destination.** `CFNumberGetType(sid_ref)` for a space
id returns `kCFNumberSInt32Type` on some macOS versions and `kCFNumberSInt64Type` on others, so
the call writes 4 or 8 bytes into a `uint64_t`. The C is only correct because the destination is
zero-initialised before the call:

- `src/space_manager.c:493` `uint64_t result = 0;` then `:508`
- `src/space_manager.c:536-538`, `:572-574`, `src/window.c:79`, `:103`, `src/display.c:243`
- `src/event_loop.c:1505-1506` `uint64_t layer = 0;` then the read

In Rust this is `core_foundation::cfnumber_read_u64_widening`, whose
`let mut destination: u64 = 0;` is the zero-init. **Never** write this as `number.value(...)` into an uninitialised `MaybeUninit<u64>`,
and never as a read of `i64` followed by `as u64`: a 4-byte write into an 8-byte slot leaves the
top half at whatever the slot held, and only the zero makes it a correct widening.

`src/window.c:97-106` writes through `space_list + i` into a `ts_alloc_list(uint64_t, *count)`
block. `ts_alloc_*` does **not** zero (`src/misc/ts.h:52-63` is a bump pointer over a fresh
`mmap`, so the first pass through the arena reads as zero and every later pass reads stale
bytes). A 4-byte write there leaves the top half stale on any pass after the first. The Rust
`Vec<u64>` built by `push` of a zero-initialised local is unconditionally correct; record it in
`DEVIATIONS.md` as a latent C defect that Rust closes.

**Form B — an explicit type, exact-width destination.** `src/event_loop.c:1377` passes
`kCFNumberSInt32Type` into a `uint32_t child_wid;` that is *not* zero-initialised, which is
correct because the write covers all four bytes. That is
`core_foundation::cfnumber_read_i32(...) as u32`.

`CFNumberGetValue` returns a `bool` that says whether the value was converted without loss. Both
forms discard it in C (`src/window.c:79`, `src/space_manager.c:508`, …). Discard it in Rust too —
do not turn it into an `Option`, or every caller changes shape.

### 1.8 CFString to String, and back

`ts_cfstring_copy` (`src/misc/helpers.h:361-371`) and `cfstring_copy`
(`src/misc/helpers.h:373-385`) differ only in which allocator they use. Under `DECISIONS.md` 17
and 28 both become one function returning an owned `String`:

```rust
pub fn cfstring_copy(string: &CFString) -> Option<String> {
    core_foundation::cfstring_to_string(string)
}

pub fn ts_cfstring_copy(string: &CFString) -> Option<String> {
    core_foundation::cfstring_to_string(string)
}
```

Both names are kept in phase 2 (`DECISIONS.md` 2) so the Rust reads against the C; phase 3 merges
them. Details that are load-bearing:

- The size comes from `CFStringGetMaximumSizeForEncoding`, not from the actual encoded length, so
  the buffer is over-allocated. Nothing depends on the excess, and the `truncate` at the first NUL
  in `core_foundation::cfstring_to_string` is what makes the `String` the right length.
- `CFStringGetCString` returning `false` is the `NULL` return in C. It stays `None`, and the
  callers keep their `if (!name) continue;` shape.
- `src/misc/helpers.h:367` leaves the arena bytes allocated when the conversion fails — a small
  leak until `ts_reset`. The `Vec` in `core_foundation::cfstring_to_string` is dropped instead.
  No behaviour change; no `DEVIATIONS.md` line needed, because the arena is being replaced
  wholesale by `DECISIONS.md` 17.
- The conversion is UTF-8 in and UTF-8 out. `String::from_utf8_lossy` is the point required by
  `DECISIONS.md` 28; `CFStringGetCString` already refuses a lossy encode, so the lossy path is
  unreachable here and is only there so the function cannot fail a second way.

Going the other way, `CFString::from_str(&str)` is the only constructor used. It returns
`CFRetained<CFString>`, i.e. an owned `+1`, and it is released when the binding drops. The C has
no counterpart to release explicitly except `CFSTRINGNUM32` (§1.10).

### 1.9 The immortal constants

`CFSTR("…")` is a compile-time `__CFConstantString` in `__TEXT`: never retained, never released,
alive for the process. Rust cannot emit one. Every `CFSTR` in the tree becomes a process-lifetime
`OnceLock<SendCFRetained<CFString>>` built once and never dropped, with one accessor function per
constant:

```rust
pub fn k_com_apple_window_shadow_density() -> &'static CFString {
    static K_COM_APPLE_WINDOW_SHADOW_DENSITY: OnceLock<SendCFRetained<CFString>> = OnceLock::new();
    K_COM_APPLE_WINDOW_SHADOW_DENSITY
        .get_or_init(|| SendCFRetained(CFString::from_str("com.apple.WindowShadowDensity")))
        .as_ref()
}
```

The accessor returns `&'static CFString`, so call sites read exactly like the C constant did.
`SendCFRetained` (§1.14) is what lets a `static` hold the value at all: `CFRetained<CFString>` is
neither `Send` nor `Sync`, and a `static` must be `Sync`.

They fall in two families, in two modules:

- The `kAX*` family belongs to `src/ffi/accessibility.rs`, where `ffi-objc-and-os.md` §8.3
  generates every one of them from a single `macro_rules!` and keeps the C spelling of each name,
  so a call site reads `kAXEnhancedUserInterface()`; that generator's `static` needs the
  `SendCFRetained` wrapper for the same reason this one does. The family covers the ones the SDK
  spells `#define kAXFooAttribute CFSTR("AXFoo")` — `kAXSizeAttribute`, `kAXPositionAttribute`
  and the rest — together with the six yabai declares itself: `kAXEnhancedUserInterface`
  (`src/misc/helpers.h:171`), `kAXFullscreenAttribute` (`src/window.h:4`) and the four
  `kAXExpose*` (`src/mission_control.c:52-55`). This document does not respell them.
- Everything else lives in `src/ffi/core_foundation.rs`, next to the helpers of §1.4:
  `"com.apple.WindowShadowDensity"` (`src/misc/helpers.h:336`), `"Spaces"`, `"id64"`,
  `"Display Identifier"`, `"type"`, `"uuid"`, `"ManagedSpaceID"`, `"Current Space"` and the rest
  of the `CFSTR` literals in `src/space_manager.c`, `src/display.c` and `src/mission_control.c`.

Two rules:

- Never build one per call. `AXUIElementCopyAttributeValue(ref, kAXEnhancedUserInterface, …)` runs
  on every window move (`src/window_manager.c:361,405,741`).
- Only the string constants that are *exported symbols* are hand-declared `extern` statics under
  `src/ffi/` (`DECISIONS.md` 11), read as `&'static CFString` and never wrapped in `CFRetained`:
  `kAXTrustedCheckOptionPrompt` (`AXUIElement.h:66`, `extern CFStringRef`), `kCGWindowName`,
  `kCGWindowOwnerName` and `kCGWindowLayer`. A `CFSTR` macro is not a symbol and cannot be bound
  this way, which is why the two families above exist.

### 1.10 `CFSTRINGNUM32`

`src/misc/helpers.h:321-326` has no caller anywhere in `src/`. `DECISIONS.md` 5 removes it. Write
one line in `DEVIATIONS.md`: *`src/misc/helpers.h:321-326` `CFSTRINGNUM32` had no caller; not
translated.*

### 1.11 `CFNUM32`, `sls_window_disable_shadow`, `cfarray_of_cfnumbers`

```rust
pub fn CFNUM32(num: i32) -> CFRetained<CFNumber> {
    CFNumber::new_i32(num)
}

pub fn sls_window_disable_shadow(id: u32) {
    let density = CFNUM32(0);
    let keys: [*const c_void; 1] = [core_foundation::k_com_apple_window_shadow_density() as *const CFString as *const c_void];
    let values: [*const c_void; 1] = [&*density as *const CFNumber as *const c_void];
    let options = unsafe {
        core_foundation::take_create_rule_result(CFDictionaryCreate(
            core::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        ))
    }
    .unwrap();
    unsafe { SLSWindowSetShadowProperties(id, &*options) };
}

pub fn cfarray_of_cfnumbers<T: Copy>(values: &[T], number_type: CFNumberType) -> CFRetained<CFArray> {
    let numbers: Vec<CFRetained<CFNumber>> = values
        .iter()
        .map(|value| unsafe {
            core_foundation::take_create_rule_result(CFNumberCreate(
                core::ptr::null(),
                number_type,
                value as *const T as *const c_void,
            ))
            .unwrap()
        })
        .collect();
    let pointers: Vec<*const c_void> = numbers
        .iter()
        .map(|number| &**number as *const CFNumber as *const c_void)
        .collect();
    unsafe {
        core_foundation::take_create_rule_result(CFArrayCreate(
            core::ptr::null(),
            pointers.as_ptr(),
            pointers.len() as isize,
            &kCFTypeArrayCallBacks,
        ))
    }
    .unwrap()
}
```

`cfarray_of_cfnumbers` drops the C `(void *values, size_t size, int count, CFNumberType type)`
quadruple for a typed slice, which removes the raw pointer walk at `src/misc/helpers.h:348` and
the stack VLA at `:346` (`src/space_manager.c:668` passes a whole window list through it). Callers
pass `&[wid]`, `&[u32]` or `&[u64]`. The explicit `CFRelease` loop at `src/misc/helpers.h:355-357`
disappears: the `Vec<CFRetained<CFNumber>>` drops after `CFArrayCreate` has retained each element,
which is the same order.

`sls_window_disable_shadow` runs concurrently on the proxy-build threads and the event-loop thread
(`src/window_manager.c:473,652`, `src/view.c:21`). CF object creation is thread-safe; nothing is
shared between the calls. No lock.

### 1.12 The Accessibility helpers

```rust
pub fn ax_privilege() -> bool {
    let keys: [*const c_void; 1] = [unsafe { kAXTrustedCheckOptionPrompt } as *const c_void];
    let values: [*const c_void; 1] = [unsafe { kCFBooleanTrue } as *const c_void];
    let options = unsafe {
        core_foundation::take_create_rule_result(CFDictionaryCreate(
            core::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            keys.len() as isize,
            &kCFCopyStringDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        ))
    }
    .unwrap();
    unsafe { AXIsProcessTrustedWithOptions(&*options) }
}

pub fn ax_window_id(reference: AXUIElementRef) -> u32 {
    let mut window_id: u32 = 0;
    unsafe { _AXUIElementGetWindow(reference, &mut window_id) };
    window_id
}

pub unsafe fn ax_window_pid(reference: AXUIElementRef) -> libc::pid_t {
    unsafe { *((reference as *const u8).add(0x10) as *const libc::pid_t) }
}

pub fn ax_enhanced_userinterface(reference: AXUIElementRef) -> bool {
    let mut result = false;
    let mut value: *const CFType = core::ptr::null();
    if unsafe {
        AXUIElementCopyAttributeValue(
            reference,
            kAXEnhancedUserInterface(),
            &mut value,
        )
    } == kAXErrorSuccess
    {
        if let Some(owned_value) = unsafe { core_foundation::take_create_rule_result(value) } {
            result = core_foundation::cfboolean_get_value(unsafe {
                &*(&*owned_value as *const CFType as *const CFBoolean)
            });
        }
    }
    result
}
```

Notes that decide the shape:

- `ax_privilege` uses `kCFCopyStringDictionaryKeyCallBacks`, **not**
  `kCFTypeDictionaryKeyCallBacks` (`src/misc/helpers.h:493`). Keep it; they differ in whether the
  key is copied.
- `ax_window_id` returns `0` when `_AXUIElementGetWindow` fails, because `wid` is pre-zeroed
  (`src/misc/helpers.h:501`). That sentinel is checked by nine callers. It stays a `u32`, not a
  `Result`, not an `Option`.
- `ax_window_pid` reads a private struct field at a hardcoded `0x10`
  (`src/misc/helpers.h:508`). It is the single most ABI-fragile line in the daemon. Keep the
  literal, keep the name, keep it `unsafe` (`DECISIONS.md` 39). Its one caller is
  `src/event_loop.c:559`, on the main thread.
- `ax_enhanced_userinterface` casts the copied value to `CFBoolean` without checking the type, as
  the C does at `src/misc/helpers.h:517`. A non-boolean attribute produces whatever
  `CFBooleanGetValue` makes of it; this is preserved, not fixed. The `Option` wrapper around the
  copied value is the one addition: the C reads `value` even when `AXUIElementCopyAttributeValue`
  leaves it uninitialised on a non-success error other than the one it checks — it does check, so
  the C is fine, and the `if let` costs nothing.

### 1.13 `AX_ENHANCED_UI_WORKAROUND` as a closure-taking function

The C macro (`src/misc/helpers.h:524-530`):

```c
#define AX_ENHANCED_UI_WORKAROUND(r, c) \
{\
    bool eui = ax_enhanced_userinterface(r); \
    if (eui) AXUIElementSetAttributeValue(r, kAXEnhancedUserInterface, kCFBooleanFalse); \
    c \
    if (eui) AXUIElementSetAttributeValue(r, kAXEnhancedUserInterface, kCFBooleanTrue); \
}
```

Rust:

```rust
struct RestoreEnhancedUserInterfaceOnDrop {
    application_reference: AXUIElementRef,
    was_enabled: bool,
}

impl Drop for RestoreEnhancedUserInterfaceOnDrop {
    fn drop(&mut self) {
        if self.was_enabled {
            unsafe {
                AXUIElementSetAttributeValue(
                    self.application_reference,
                    kAXEnhancedUserInterface(),
                    kCFBooleanTrue,
                )
            };
        }
    }
}

pub fn with_enhanced_user_interface_disabled<Result>(
    application_reference: AXUIElementRef,
    body: impl FnOnce() -> Result,
) -> Result {
    let was_enabled = ax_enhanced_userinterface(application_reference);
    if was_enabled {
        unsafe {
            AXUIElementSetAttributeValue(
                application_reference,
                kAXEnhancedUserInterface(),
                kCFBooleanFalse,
            )
        };
    }
    let _restore_enhanced_user_interface_on_drop = RestoreEnhancedUserInterfaceOnDrop {
        application_reference,
        was_enabled,
    };
    body()
}
```

The three call sites are `src/window_manager.c:361`, `:405` and `:741`, all on the event-loop
thread. Each body mutates window-manager state, so the closure must receive what it needs as
parameters rather than capturing a manager (`DECISIONS.md` 13). `src/window_manager.c:361`:

```rust
with_enhanced_user_interface_disabled(application_reference, || {
    window_manager_move_window(window, fx, fy);
    window_manager_resize_window(window, fw, fh);
});
```

Where the borrow checker objects because the body needs `&mut WindowManager` that the caller also
holds, split the call: take the `AXUIElementRef` out of the manager first, then pass the `&mut`
into the closure by move. Do not reach for a global (`DECISIONS.md` 13) and do not inline the
macro's three statements at the call site — the guard is what makes the restore run on a panic,
which the C macro does not do.

### 1.14 `SendCFRetained`, and CF objects inside an `Event`

`CFRetained<T>` is neither `Send` nor `Sync`, and `DECISIONS.md` 19 puts owned payloads inside the
`Event` enum that crosses from the posting thread to the event-loop thread. One newtype covers
every such payload:

```rust
pub struct SendCFRetained<T: ?Sized>(pub CFRetained<T>);

unsafe impl<T: ?Sized> Send for SendCFRetained<T> {}
unsafe impl<T: ?Sized> Sync for SendCFRetained<T> {}

impl<T: ?Sized> SendCFRetained<T> {
    pub fn as_ref(&self) -> &T {
        &self.0
    }
}
```

`Sync` is there for the `static OnceLock<SendCFRetained<CFString>>` of §1.9, which a `static`
cannot hold without it. It is the same newtype for both jobs on purpose: one type, one argument in
`THREADS.md`, nothing to choose between at a call site.

The soundness argument goes in `THREADS.md`, not here and not in a comment
(`DECISIONS.md` 38). It rests on CoreFoundation's retain/release being atomic and on each
`Event` payload having exactly one owner at a time: the posting thread constructs it and gives it
up, the event-loop thread receives it and drops it.

Where it is used:

| C | payload | Rust |
|---|---|---|
| `src/mouse_handler.c:34,44` `(void *) CFRetain(event)` | `CGEventRef` | `SendCFRetained<CGEvent>` in the `MouseDown` / `MouseUp` / `MouseDragged` / `MouseMoved` variants |
| `src/application.c:9` `(void *) CFRetain(element)` | `AXUIElementRef` | `SendCFRetained<AXUIElement>` in the `WindowCreated` variant |

`Drop` on the variant replaces every `CFRelease(context)` in the tree:
`src/event_loop.c:554,557,560,563` (`WINDOW_CREATED`), `:1151` (`MOUSE_DOWN`), `:1232`
(`MOUSE_UP`), `:1244` and `:1342` (`MOUSE_DRAGGED`), `:1449` (`MOUSE_MOVED`)
(`DECISIONS.md` 19).

The one place this changes the C's shape is `WINDOW_CREATED`: the C releases on each of its four
early-out paths (`:554,557,560,563`) but **not** at `:566`, because
`window_manager_create_and_add_window` has taken ownership of the `AXUIElementRef` by then. In
Rust the call at `:565` moves the `SendCFRetained` into the new `Window`, so no `Drop` runs at the
handler; every other path drops it. Same reference count, no `CFRelease` written by hand.

`SendCFRetained` is also what holds the `OnceLock`-ed `CFSTR` replacements (§1.9), because a
`static` must be `Sync`; those are immortal and immutable, and the accessor hands out
`&'static CFString`.

---

## 2. Text is owned `String`

`DECISIONS.md` 28 settles it: every `char *` that the daemon owns becomes a `String`, every
`char *` it borrows becomes `&str`, and the `ts` arena behind `ts_string_copy`,
`ts_cfstring_copy` and `ts_string_escape` is gone (`DECISIONS.md` 17). There is no `Cow`, no
arena lifetime, no `&'ts str`. The `files/` inventory's option of keeping arena-backed strings and
the sweep's rule "never substitute `String`/`Vec` for a `ts_*` result" are both overridden.

The one consequence a translator must not undo: `ts_*` results in C die at the next `ts_reset()`
(`src/event_loop.c:1671`), so the C is free to hand the same pointer to several consumers within
one event. In Rust each consumer gets its own `String`, or borrows one that outlives it. Where the
C stored an arena pointer into a longer-lived struct — `struct event_signal`'s `arg_name` /
`arg_value` arrays (`src/event_signal.c:129-338`) — the Rust struct owns `String`s
(`DECISIONS.md` 17, "Queued signals own their strings").

### 2.1 `string_copy` and `ts_string_copy`

`src/misc/helpers.h:387-406`. Both become one thing, and both keep their names for phase 2:

```rust
pub fn string_copy(source: &str) -> String {
    source.to_owned()
}

pub fn ts_string_copy(source: &str) -> String {
    source.to_owned()
}
```

`string_copy` returns `NULL` on allocation failure (`src/misc/helpers.h:401`) and every caller
ignores that; Rust aborts on allocation failure instead. One line in `DEVIATIONS.md`.

`src/event_signal.c:209` stores either a `ts_string_copy` result or the literal `"<unknown>"` in
the same field, so the C field is not uniformly owned. In Rust the field is `String` and the
literal branch writes `String::from("<unknown>")`. Do not reach for `Cow<'static, str>`: nothing
measures this, and one type per field is the phase-2 rule.

`length` is `int` in C (`src/misc/helpers.h:389,399`). In Rust it is `usize`, which removes an
overflow at 2 GB that no caller can reach.

### 2.2 `string_equals`

`src/misc/helpers.h:254-257` is `a && b && strcmp(a, b) == 0` — NULL on either side is false,
including NULL against NULL. 89 call sites, several with a genuinely nullable side
(`src/rule.c:148`, `src/event_signal.c:389`, `src/window_manager.c:180-205`).

Two forms, and which one to use is decided by the C type at the site, never by taste:

```rust
pub fn string_equals(left: Option<&str>, right: Option<&str>) -> bool {
    matches!((left, right), (Some(left), Some(right)) if left == right)
}
```

- The C argument can be NULL → `string_equals(left, right)` with `Option<&str>` on both sides.
- Both C arguments are known non-NULL, which is every `string_equals(argv[i], "--verbose")`-shaped
  site in `src/yabai.c:183-250` → plain `left == right`.
- One side is a C string from the OS (`src/misc/macho_dlsym.h:9,27,74`) → compare `&CStr` or
  `&[u8]`, see §11.

### 2.3 `ts_string_escape` and the "NULL means nothing was escaped" contract

`src/misc/helpers.h:259-319`. It JSON-escapes `"`, `\`, `\b`, `\f`, `\n`, `\r`, `\t` and the C0
controls (as `\u00xx`), and **returns `NULL` when no character needed escaping**. Every call site
relies on that: `src/window.c:177,187,477,487`, `src/rule.c:13-16,42-45`,
`src/event_signal.c:408-410,424-428`, all written as `escaped ? escaped : original`.

```rust
pub fn ts_string_escape(source: &str) -> Option<String> {
    let mut number_of_replacements = 0;
    for character in source.chars() {
        match character {
            '"' | '\\' | '\u{8}' | '\u{c}' | '\n' | '\r' | '\t' => number_of_replacements += 1,
            '\u{0}'..='\u{1f}' => number_of_replacements += 5,
            _ => {}
        }
    }

    if number_of_replacements == 0 {
        return None;
    }

    let mut result = String::with_capacity(source.len() + number_of_replacements);
    for character in source.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\u{8}' => result.push_str("\\b"),
            '\u{c}' => result.push_str("\\f"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\u{0}'..='\u{1f}' => result.push_str(&format!("\\u{:04x}", character as u32)),
            other => result.push(other),
        }
    }
    Some(result)
}
```

`None` is the `NULL`, and the call sites become `escaped.unwrap_or(original)` — or, where the
original is itself nullable as in `src/rule.c:42` (`escaped_app ? escaped_app : app ? app : ""`),
`escaped.as_deref().or(app).unwrap_or("")`. Returning `Some(String::new())` or `Some(source.to_owned())`
for the unescaped case would change `src/rule.c:13-16`, which distinguishes the three cases.

Four details that are load-bearing:

1. **`char` is signed on both targets**, so the C test `*cursor >= 0x00 && *cursor <= 0x1f`
   (`src/misc/helpers.h:273,308`) deliberately excludes every byte ≥ 0x80 — as a signed `char`
   those are negative and fail the lower bound — and UTF-8 therefore passes through untouched.
   Do not port the `>= 0x00` half literally: on an unsigned type it is always true, and the
   escape set would then depend entirely on getting the upper bound right.
2. **Iterate `chars()`, not `bytes()`.** Pushing `byte as char` would re-encode every byte in
   `0x80..=0xff` as two UTF-8 bytes and corrupt every non-ASCII title. Iterating `chars()` and
   pushing the whole `char` in the default arm is byte-preserving, and the two passes still agree
   on the size because the C also adds nothing for a multi-byte sequence: `source.len()` is the
   byte length, `number_of_replacements` counts only ASCII matches, and that is exactly
   `(cursor - s) + num_replacements` at `src/misc/helpers.h:282`.
3. `sprintf(dst, "%04x", (int)*cursor)` at `src/misc/helpers.h:311` writes five bytes (four hex
   digits plus a NUL) and advances four, so the NUL is overwritten by the next iteration or lands
   exactly on `result[size_in_bytes]`. It is in bounds but only just. `format!("\\u{:04x}", character as u32)`
   writes the same four digits, lowercase, zero-padded, and no NUL. Do not reproduce the overlap.
4. The C sizes the buffer before filling it, and the two passes must agree or the write runs past
   the allocation. `String::with_capacity` makes the first pass an optimisation only, but keep it:
   it is the C's structure and it documents the +1/+5 accounting.

### 2.4 `json_bool` and `json_optional_bool`

`src/misc/helpers.h:225-236`. Both return string literals; nothing is allocated, nothing is freed.

```rust
pub fn json_optional_bool(value: i32) -> &'static str {
    if value == 0 {
        return "null";
    }
    if value == 1 {
        return "true";
    }
    "false"
}

pub fn json_bool(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}
```

`json_optional_bool` keeps its `i32` parameter in phase 2 because the callers
(`src/rule.c:50-54`, `src/event_signal.c:426`) store an `int` tri-state in `rule_effects`. Keep the
"anything that is neither 0 nor 1 is false" rule; it is reachable, because `rule->effects.manage`
is initialised to a value outside {0, 1} in some paths.

### 2.5 Where socket bytes become text

`DECISIONS.md` 28 puts the conversion at the point the bytes are stored, and there is exactly one
such point: the `DAEMON_MESSAGE` handler, `src/event_loop.c:1614-1640`.

```c
    if (read(param1, &bytes_to_read, sizeof(int)) == sizeof(int)) {
        char *message = ts_alloc_unaligned(bytes_to_read);

        do {
            int cur_read = read(param1, message+bytes_read, bytes_to_read-bytes_read);
            if (cur_read <= 0) break;

            bytes_read += cur_read;
        } while (bytes_read < bytes_to_read);
```

The `Vec<u8>` filled by that loop becomes a `String` with `String::from_utf8_lossy(&buffer).into_owned()`
immediately after the loop and before `debug_message`, and nothing downstream ever sees the bytes
again. The token ranges of `DECISIONS.md` 27 are `(start, length)` over **that `String`**, so they
stay self-consistent.

The message is a run of NUL-terminated argv elements followed by one extra NUL
(`src/yabai.c:73-82`). NUL is valid UTF-8 and survives `from_utf8_lossy` unchanged, so the
tokeniser and `debug_message` both still find their separators.

Two consequences to record in `DEVIATIONS.md`:

- A non-UTF-8 argument is echoed back by the C as raw bytes through `%.*s`
  (`src/message.c:2997` `daemon_fail(rsp, "unknown domain '%.*s'\n", …)`) and by the Rust as
  U+FFFD. The socket wire format is unchanged; one malformed argument echoes differently.
- `from_utf8_lossy` can lengthen the buffer (one invalid byte becomes three). Nothing compares the
  length against `bytes_to_read` after the read loop, so nothing breaks.

### 2.6 The `Response` sink

`DECISIONS.md` 28 replaces `FILE *rsp` with one type that owns both the failure prefix byte and
the "no response wanted" case.

```rust
pub struct Response {
    sink: Option<std::io::BufWriter<std::fs::File>>,
}

impl Response {
    pub fn to_socket(file: std::fs::File) -> Response {
        Response { sink: Some(std::io::BufWriter::new(file)) }
    }

    pub fn none() -> Response {
        Response { sink: None }
    }

    pub fn write_fmt(&mut self, arguments: std::fmt::Arguments<'_>) {
        if let Some(sink) = self.sink.as_mut() {
            let _ = std::io::Write::write_fmt(sink, arguments);
        }
    }

    pub fn write_failure_prefix(&mut self) {
        if let Some(sink) = self.sink.as_mut() {
            let _ = std::io::Write::write_all(sink, &[FAILURE_MESSAGE]);
        }
    }

    pub fn flush(&mut self) {
        if let Some(sink) = self.sink.as_mut() {
            let _ = std::io::Write::flush(sink);
        }
    }
}
```

`FAILURE_MESSAGE` is `"\x07"` (`src/misc/macros.h:18`) and is written at
`src/message.c:424` with `fprintf(rsp, FAILURE_MESSAGE)` — a format call with no arguments. In
Rust it is the raw byte `0x07`, never a format string; `write_failure_prefix` is the only writer
of it, and `daemon_fail` calls it before the message, exactly as `src/message.c:423-425` does.

`Response::none()` is the `if (!rsp) return;` of `src/message.c:420` and `:431`. Today the only
caller passes a real socket (`src/event_loop.c:1632-1634` requires `fdopen` to have succeeded), so
`none()` is unreachable; keep it, because `DECISIONS.md` 28 names it and because dropping it would
make `daemon_fail` change shape.

`Result` appears nowhere above: `DECISIONS.md` 32 confines it to `io::Write`, and every write here
discards its error exactly as `fprintf` does.

---

## 3. Fixed `char` buffers and `snprintf`

`MAXLEN` is `512` (`src/misc/macros.h:20`). A `char[MAXLEN]` plus one `snprintf` becomes a
`String` built with `format!`, except for the three buffers that are handed to the kernel.

### 3.1 The process-wide paths

`src/yabai.c:44-47` declares four of them; `src/yabai.c:1-3` the three formats:

```c
#define SA_SOCKET_PATH_FMT      "/tmp/yabai-sa_%s.socket"
#define SOCKET_PATH_FMT         "/tmp/yabai_%s.socket"
#define LCFILE_PATH_FMT         "/tmp/yabai_%s.lock"
```

`DECISIONS.md` 18 makes them `OnceLock` statics:

```rust
pub static G_SA_SOCKET_FILE: OnceLock<String> = OnceLock::new();
pub static G_SOCKET_FILE: OnceLock<String> = OnceLock::new();
pub static G_LOCK_FILE: OnceLock<String> = OnceLock::new();
pub static G_CONFIG_FILE: OnceLock<String> = OnceLock::new();
```

set at `src/yabai.c:135-137` with

```rust
G_SA_SOCKET_FILE.set(format!("/tmp/yabai-sa_{}.socket", user)).unwrap();
G_SOCKET_FILE.set(format!("/tmp/yabai_{}.socket", user)).unwrap();
G_LOCK_FILE.set(format!("/tmp/yabai_{}.lock", user)).unwrap();
```

and `G_CONFIG_FILE` set earlier than those three, in `main` at `src/yabai.c:264`, once
`parse_arguments` has returned the `--config` value it collected:

```rust
G_CONFIG_FILE.set(config_file_argument.unwrap_or_default()).unwrap();
```

`G_SA_SOCKET_FILE`, `G_SOCKET_FILE` and `G_LOCK_FILE` are sound as `OnceLock`s because the C
writes each at most once per process:

- `g_socket_file` and `g_lock_file` have one write each, `src/yabai.c:136-137`.
- `g_sa_socket_file` is written at `src/yabai.c:135` on the daemon path, **or** at
  `src/sa.m:158` from `scripting_addition_set_socket_path`, which is only reached through the
  `--load-sa` / `--uninstall-sa` branches of `parse_arguments` (`src/yabai.c:216-220`) and those
  branches `exit` before `main` reaches `configure_settings_and_acquire_lock` at
  `src/yabai.c:287`. Never both.

`g_config_file` is different, and the difference decides where `set` goes. The C writes that
buffer **twice** on inputs a user can type:

- `src/yabai.c:243-257` loops over the whole of `argv` without stopping at the first match, so
  `yabai -c one -c two` runs `src/yabai.c:253` twice and the last value wins.
- `src/yabai.c:252` rejects only a NULL `val`, so `yabai -c ""` stores the empty string. Then
  `src/misc/helpers.h:465` tests `config_file[0] == '\0'`, finds it true, and lets
  `get_config_file("yabairc", …)` write the buffer a second time through `exec_config_file`'s
  buffer parameter. The C runs the discovered `yabairc`; a second `G_CONFIG_FILE.set(…).unwrap()`
  would `Err` and the panic hook of `DECISIONS.md` 7 would abort instead.

So `G_CONFIG_FILE` has exactly one `set`, and neither C write is it:

- `parse_arguments` returns the `--config` value as an `Option<String>` — last occurrence wins, as
  in C — and `main` calls `G_CONFIG_FILE.set(…)` once, after the loop, with `String::new()` when
  the flag was absent. The empty string is a legitimate value and reaches the static unchanged.
- the `get_config_file` fallback never touches the static. `exec_config_file` takes the path by
  value and resolves it in a local (§3.2); the write-back through the caller's buffer disappears
  with the buffer. Nothing reads the path after `src/yabai.c:348`, which is the only read in the
  tree — `src/yabai.c:46,253,348` are all three mentions of `g_config_file`.

With that placement all four are set on the main thread before `event_loop_begin`
(`src/yabai.c:291`), which is the shape `DECISIONS.md` 18 names. The *resolution* the C performs
at `src/yabai.c:348`, after the event-loop, message-loop and workspace threads are up, stays
exactly where it is: it happens inside `exec_config_file`, on a value, and touches no static.

### 3.2 `get_config_file` and `exec_config_file`

`get_config_file` (`src/misc/helpers.h:445-461`) probes three paths and leaves the buffer holding
the last candidate even when it returns false. That residue is only observable through
`exec_config_file`, which returns immediately on false (`src/misc/helpers.h:465-469`), so it is
dropped:

```rust
pub fn get_config_file(filename: &str) -> Option<String> {
    if let Some(xdg_config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        if !xdg_config_home.is_empty() {
            let candidate = format!("{}/yabai/{}", xdg_config_home.to_string_lossy(), filename);
            if file_exists(&candidate) {
                return Some(candidate);
            }
        }
    }

    let home = std::env::var_os("HOME")?;

    let candidate = format!("{}/.config/yabai/{}", home.to_string_lossy(), filename);
    if file_exists(&candidate) {
        return Some(candidate);
    }

    let candidate = format!("{}/.{}", home.to_string_lossy(), filename);
    if file_exists(&candidate) {
        return Some(candidate);
    }
    None
}
```

The `xdg_home && *xdg_home` test at `src/misc/helpers.h:447` means "set **and** non-empty"; an
empty `XDG_CONFIG_HOME` falls through to `$HOME`. Keep both halves.

`file_exists` (`src/misc/helpers.h:419-432`) is `stat` then false for a directory, true for
anything else, so a socket, fifo or symlink-to-file counts as existing:

```rust
pub fn file_exists(filename: &str) -> bool {
    match std::fs::metadata(filename) {
        Ok(metadata) => !metadata.is_dir(),
        Err(_) => false,
    }
}

pub fn directory_exists(filename: &str) -> bool {
    match std::fs::metadata(filename) {
        Ok(metadata) => metadata.is_dir(),
        Err(_) => false,
    }
}

pub fn file_can_execute(filename: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(filename) {
        Ok(metadata) => metadata.permissions().mode() & 0o100 != 0,
        Err(_) => false,
    }
}
```

`std::fs::metadata` follows symlinks, as `stat` does. `file_can_execute` tests `S_IXUSR` only
(`src/misc/helpers.h:442`), which can disagree with what `exec` would do; it decides between
`sh -c <path>` and `sh <path>` at `src/misc/helpers.h:479`, so do not "improve" it to
`access(X_OK)`.

`exec_config_file` takes the path **by value** — `pub fn exec_config_file(config_file: String)` —
and resolves the fallback itself: when `config_file.is_empty()` it calls `get_config_file`, and on
`None` it warns, notifies and returns, which is what `src/misc/helpers.h:465-469` does. The `&&`
there short-circuits and so does the `if`, so `get_config_file` still runs only for an empty path,
and `yabai -c ""` still ends up running the discovered `yabairc`. The `config_file_size` parameter
disappears with the buffer, and with it the write-back that made the C write `g_config_file`
twice (§3.1).

`exec_config_file` itself (`src/misc/helpers.h:463-487`) is `fork` + `execvp`, and
`DECISIONS.md` 25 keeps it that way: `libc::fork`, the argv chosen in the parent from
`file_can_execute`, and `libc::execvp("/usr/bin/env", …)` in the child, which calls nothing that
is not async-signal-safe and leaves through `_exit`. It is **not** `std::process::Command`, which
would `posix_spawn`, reset `SIGPIPE` to `SIG_DFL` in the child (the C child inherits `SIG_IGN`
from `src/yabai.c:152`) and change what a `yabairc` containing a pipeline does. The mechanics of
the fork belong to whoever owns `DECISIONS.md` 25; what this section fixes is the two inputs it
consumes — the resolved path and the executable-bit verdict — and that both are computed in the
parent, before the fork.

### 3.3 Where truncation was observable

Every `snprintf` outside `src/osax/` and outside `src/sa.m`, with a verdict. "Observable" means a
user-reachable input can reach the cap.

| C site | buffer | cap | verdict |
|---|---|---|---|
| `src/yabai.c:86` | `socket_file[MAXLEN]` | 512 | a `$USER` longer than 492 bytes truncates the client's socket path. `format!` does not. The connect fails either way, with a different path in the error. `DEVIATIONS.md` line. |
| `src/yabai.c:135,136,137` | `g_sa_socket_file`, `g_socket_file`, `g_lock_file` `[MAXLEN]` | 512 | same, daemon side. One `DEVIATIONS.md` line covering all four. |
| `src/yabai.c:253` | `g_config_file[4096]` | 4096 | a `--config` argument longer than 4095 bytes truncates; `format!` keeps it whole and the subsequent `file_exists` fails honestly. `DEVIATIONS.md` line. |
| `src/misc/helpers.h:449,456,459` | caller's buffer, `4096` via `exec_config_file` | 4096 | same class; the `Option<String>` above removes it. Covered by the `g_config_file` line. |
| `src/misc/helpers.h:194` | `sockaddr_un.sun_path` | **104** | **keep the truncation.** See §3.4. |
| `src/message.c:3020` | `sockaddr_un.sun_path` | **104** | **keep the truncation.** See §3.4. |
| `src/misc/service.h:206,209,258,272,275` | `service_target`, `domain_target` `[MAXLEN]` | 512 | unreachable: the content is `gui/<uid>/com.asmvik.yabai`, at most 40 bytes. `format!`, no note. |
| `src/misc/service.h:96` | `malloc`'d, sized `strlen(_PATH_YABAI_PLIST)-2 + strlen(home) + 1` | exact | the C sizing is exact only if `home` has no `%`; `format!` cannot truncate. §8.3. |
| `src/misc/service.h:126` | `malloc`'d, sized `strlen(_YABAI_PLIST)-8 + …` | exact | same. §8.3. |
| `src/misc/helpers.h:324` | `num_str[255]` | 255 | dead code, §1.10. |
| `src/event_signal.c:132-338` | arena `arg_size = 128` (`src/event_signal.c:104`) | 128 | `YABAI_*` names and values are all short integers or fixed literals; the longest is `"YABAI_RECENT_DISPLAY_INDEX"` at 26 bytes. `String` via `format!`. No note. Owned by the `event_signal` writer. |

Two fixed buffers in the tree are **not** `snprintf` targets but are the same hazard, and they are
flagged here because they are the only unchecked writes into a fixed array left in the daemon.
They get **different** fixes, because only one of them is private to the daemon:

- `uint32_t window_list[1024]` (`src/event_loop.c:19`), filled by a `table_for` with no bounds
  check at `src/event_loop.c:23-25`. More than 1024 managed windows overruns the stack. It leaves
  as a pointer plus a count — `SLSRequestNotificationsForWindows(g_connection, window_list,
  window_count)` (`src/event_loop.c:33`) — so its length is part of no contract. The Rust is a
  `Vec<u32>` and the overrun is gone. `DEVIATIONS.md` line.
- `uint32_t proxy_wid[512]` / `real_wid[512]` (`src/window_manager.c:444-445`) **stay
  `[u32; 512]`**. They are two fields of the anonymous struct that `src/window_manager.c:457`
  sends whole — `mach_send(port, &data, sizeof(data))` — to the JankyBorders process looked up
  as `"git.felix.jbevent"` (`src/window_manager.c:440`). `sizeof(data)` is 4104 bytes and that number
  is the wire contract with a third-party program, so a `Vec` would change the bytes on the
  channel. The struct is `ffi-objc-and-os.md` §15.6's `#[repr(C)] JankyBordersEvent`, carrying
  `const _: () = assert!(core::mem::size_of::<JankyBordersEvent>() == 4104);`, and that document
  owns it; nothing here may respell it. The fix for the overrun is a bound on the fill loop
  (`src/window_manager.c:448-455`): stop at 512 entries. That is one `DEVIATIONS.md` line, and it
  is `ffi-objc-and-os.md`'s line, so §13 below does not repeat it.

### 3.4 The three buffers that stay bytes

`struct sockaddr_un.sun_path` is 104 bytes on Darwin, and the C truncates into it
(`src/misc/helpers.h:194`, `src/message.c:3020`). That truncation is part of the observable
behaviour: the kernel sees 104 bytes and binds or connects to the truncated name. Keep a byte
array and keep the truncation:

```rust
pub fn socket_connect(socket_file_descriptor: i32, socket_path: &str) -> bool {
    let mut socket_address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    socket_address.sun_family = libc::AF_UNIX as libc::sa_family_t;

    let path_bytes = socket_path.as_bytes();
    let writable = socket_address.sun_path.len() - 1;
    let copied = path_bytes.len().min(writable);
    for index in 0..copied {
        socket_address.sun_path[index] = path_bytes[index] as libc::c_char;
    }

    unsafe {
        libc::connect(
            socket_file_descriptor,
            &socket_address as *const libc::sockaddr_un as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
        ) != -1
    }
}
```

Three facts that the `std::os::unix::net::UnixStream` shortcut would lose, which is why the
hand-rolled form is what phase 2 writes:

1. The length passed to `connect` is `sizeof(struct sockaddr_un)` = 106, not `SUN_LEN`
   (`src/misc/helpers.h:195`).
2. `socket_open` hands the raw `int` to `event_loop_post` as `param1`
   (`src/message.c:3009` → `src/event_loop.c:1613`), so the descriptor must be a plain `i32` that
   crosses the queue, not an owned handle.
3. The C leaves `sun_len` and the tail of `sun_path` uninitialised. The `zeroed()` above fixes
   that; Darwin ignores both, so there is no behaviour change, and it is one `DEVIATIONS.md` line
   under `DECISIONS.md` 4.

The third byte buffer is `char bytes[SA_SOCKET_BUFF_LEN]` (`0x1000`, `src/osax/common.h:5`) used at
`src/sa.m:244,418`; it belongs to the `sa.m` writer and stays a `[u8; 0x1000]` because the
scripting-addition frame layout is wire format (`DECISIONS.md` 3, 34).

---

## 4. Process-wide values written once

`DECISIONS.md` 18 names exactly which globals become `OnceLock` statics. All of them are written
on the main thread before `event_loop_begin` at `src/yabai.c:291` and read from every thread
afterwards; all but `g_config_file` and `g_verbose` are written in
`configure_settings_and_acquire_lock` (`src/yabai.c:129-178`), and those two come out of
`parse_arguments` (`src/yabai.c:181-258`), which runs earlier still, at `src/yabai.c:264`.

| C global | C site | Rust |
|---|---|---|
| `g_pid` | `src/yabai.c:40-52`, set `:140` | `static G_PID: OnceLock<libc::pid_t>` |
| `g_connection` | set `src/yabai.c:143` | `static G_CONNECTION: OnceLock<i32>` |
| `g_cv_host_clock_frequency` | set `src/yabai.c:144` | `static G_CV_HOST_CLOCK_FREQUENCY: OnceLock<f64>` |
| `g_layer_normal_window_level` | set `src/yabai.c:145` | `static G_LAYER_NORMAL_WINDOW_LEVEL: OnceLock<i32>` |
| `g_layer_below_window_level` | set `src/yabai.c:146` | `static G_LAYER_BELOW_WINDOW_LEVEL: OnceLock<i32>` |
| `g_layer_above_window_level` | set `src/yabai.c:147` | `static G_LAYER_ABOVE_WINDOW_LEVEL: OnceLock<i32>` |
| `g_bs_port` | set `src/yabai.c:156` | `static G_BS_PORT: OnceLock<libc::mach_port_t>` |
| `CGSGetConnectionPortById` | set `src/yabai.c:148` | §11 |
| `SLSPerformAsynchronousBridgedWindowManagementOperation` | set `src/yabai.c:149` | §11 |
| `g_sa_socket_file`, `g_socket_file`, `g_lock_file`, `g_config_file` | §3.1 | §3.1 |
| `g_verbose` | `src/yabai.c:51`, set `:248` and `src/message.c:1175,1177` | `static G_VERBOSE: AtomicBool` |

Rules:

- A read is `*G_CONNECTION.get().unwrap()`, or a one-line accessor `pub fn g_connection() -> i32`
  per global in the module that owns it. Use the accessor; it keeps call sites reading like the C.
- `task_get_special_port` writes `g_bs_port` through an out-param and its result is discarded
  (`src/yabai.c:156`); on failure the port stays 0, and `src/window_manager.c:440` tests
  `g_bs_port &&`. So the Rust sets the `OnceLock` unconditionally, with whatever the out-param
  holds, and the `!= 0` test survives.
- `g_verbose` is a plain `bool` in C that the event-loop thread writes
  (`src/message.c:1175,1177`, from `yabai -m config debug_output on|off`) while every thread reads
  it. That is a data race (`DECISIONS.md` 4). `AtomicBool` with `Relaxed` load and store is the
  fix; one `DEVIATIONS.md` line.
- `g_event_bytes` (`src/yabai.c:42,141-142`) is **not** in this set: it is a 0x100 scratch buffer
  that `src/window_manager.c:1280-1289` rewrites on every call. It belongs to the
  `window_manager` module, not to a `OnceLock`, and is named here only so nobody adds it to the
  list by pattern-matching on `src/yabai.c:129-157`.

---

## 5. Cleanup: `goto` ladders, RAII guards and labelled blocks

84 `goto`s across ten files, in four shapes. The rule for all four: **every tail assignment the C
reaches still runs, on every path, in the same order.** Never duplicate a tail into two branches.

### 5.1 Cascading CF release

The dominant shape, and the one that disappears entirely. `src/window.c:67-84`:

```c
    CFArrayRef window_list_ref = cfarray_of_cfnumbers(&wid, sizeof(uint32_t), 1, kCFNumberSInt32Type);
    CFArrayRef space_list_ref = SLSCopySpacesForWindows(g_connection, 0x7, window_list_ref);
    if (!space_list_ref) goto err;

    int count = CFArrayGetCount(space_list_ref);
    if (!count) goto free;

    CFNumberRef id_ref = CFArrayGetValueAtIndex(space_list_ref, 0);
    CFNumberGetValue(id_ref, CFNumberGetType(id_ref), &sid);

free:
    CFRelease(space_list_ref);
err:
    CFRelease(window_list_ref);

    return sid ? sid : window_display_space(wid);
```

becomes

```rust
pub fn window_space(window_id: u32) -> u64 {
    let mut space_id: u64 = 0;

    let window_list_ref = cfarray_of_cfnumbers(&[window_id], CFNumberType::SInt32Type);
    let space_list_ref =
        unsafe { core_foundation::take_create_rule_result(SLSCopySpacesForWindows(g_connection(), 0x7, &*window_list_ref)) };

    if let Some(space_list_ref) = space_list_ref {
        let count = core_foundation::cfarray_count(&space_list_ref);
        if count != 0 {
            let id_ref: &CFNumber =
                unsafe { core_foundation::cfarray_borrow_value_at_index(&space_list_ref, 0) }.unwrap();
            space_id = core_foundation::cfnumber_read_u64_widening(id_ref);
        }
    }

    if space_id != 0 {
        space_id
    } else {
        window_display_space(window_id)
    }
}
```

Both labels are gone. The release order is identical because Rust drops in reverse declaration
order and the C labels are in reverse acquisition order. **Verify that per function.** In the four
sites where this shape occurs — `src/window.c:67-84`, `src/window.c:86-108`, `src/space.c:26-80`,
`src/display_manager.c:165-197` — it holds. Where a future site does not have reverse-acquisition
labels, write an explicit `drop(object)` at the point the C released it rather than reordering the
bindings.

`src/window.c:906-921` releases through an `err1`/`err2` ladder around
`cfarray_of_cfnumbers`; same treatment.

### 5.2 Multi-label fallthrough — `src/event_loop.c:1154-1232`

This is the worst case in the tree: three labels, each falling through to the next, with three
different entry points plus the fallthrough from the body.

```c
static EVENT_HANDLER(MOUSE_UP)
{
    if (mission_control_is_active()) goto out;
    if (!g_mouse_state.window)       goto res;
    /* ... 70 lines ... */
err:
    g_mouse_state.window = NULL;
res:
    g_mouse_state.current_action = MOUSE_MODE_NONE;
out:
    CFRelease(context);
}
```

Four paths, and the tails they run:

| entry | `window = NULL` | `current_action = NONE` | `CFRelease(context)` |
|---|---|---|---|
| `goto out` (`:1156`) | no | no | yes |
| `goto res` (`:1157`) | no | yes | yes |
| `goto err` (`:1161`, `:1166`, `:1173`) | yes | yes | yes |
| fallthrough (`:1226`) | yes | yes | yes |

The Rust splits it into two nested labelled blocks plus a `Drop`, so each tail is written once and
each entry picks how far up it leaves:

```rust
fn event_handler_mouse_up(context: SendCFRetained<CGEvent>, param1: i32, mouse_state: &mut MouseState, window_manager: &mut WindowManager, space_manager: &mut SpaceManager) {
    'set_current_action_to_none: {
        if mission_control_is_active() {
            return;
        }

        'clear_mouse_state_window: {
            if mouse_state.window.is_none() {
                break 'set_current_action_to_none;
            }

            if !mouse_state.claim_window_is_still_alive() {
                debug!("{}: {} has been marked invalid by the system, ignoring event..\n", "EVENT_HANDLER_MOUSE_UP", mouse_state.window_id());
                break 'clear_mouse_state_window;
            }

            if window_check_flag(mouse_state.window_id(), WindowFlag::Fullscreen) {
                debug!("{}: {} is transitioning into native-fullscreen mode, ignoring event..\n", "EVENT_HANDLER_MOUSE_UP", mouse_state.window_id());
                break 'clear_mouse_state_window;
            }

            /* ... the body, unchanged ... */
        }

        mouse_state.window = None;
    }

    mouse_state.current_action = MouseMode::None;
}
```

Read it against the table: `return` is `goto out`, `break 'set_current_action_to_none` is
`goto res`, `break 'clear_mouse_state_window` is `goto err`, and falling off the end of the inner
block is the fallthrough. Each label's tail is written exactly once, immediately after the block
that carries its name.

`CFRelease(context)` is not written at all: `context` is the owned `SendCFRetained<CGEvent>`
parameter, so it drops when the function returns, on every path including a panic. That is the
`out:` label, and it is the reason `goto out` can be a bare `return`.

Note the asymmetry the table makes visible and that a careless translation loses: `goto out` at
`:1156` skips `current_action = MOUSE_MODE_NONE`, so a mouse-up that arrives while Mission Control
is active leaves the drag action latched. Writing the handler as one labelled block with both
assignments in its tail, or as an early `return` that also resets `current_action`, changes that.
Keep the two blocks nested in this order.

The three neighbouring handlers are simpler and must not be given this treatment by analogy:
`MOUSE_DOWN` (`src/event_loop.c:1117-1152`), `MOUSE_DRAGGED` (`:1235-1343`) and `MOUSE_MOVED`
(`:1345-1450`) each have a single `out:` whose only tail is `CFRelease(context)`, so each `goto
out` is a plain `return` once `context` owns itself. `MOUSE_DRAGGED` also duplicates the tail
inline once at `:1242-1245` (`window = NULL`, `current_action = MOUSE_MODE_NONE`,
`CFRelease`, `return`) instead of jumping; write those three statements followed by `return`,
exactly there, rather than restructuring the function to share them.

### 5.3 Single-exit accumulator

`src/sa.m:202-237` (`scripting_addition_install`, whose `cleanup:` removes a partially installed
bundle) and `src/sa.m:96-108` (`scripting_addition_create_directory`, seven `mkdir` checks all
jumping to one `err:`). The Rust is an inner function returning `bool`, with the cleanup after the
call:

```rust
fn scripting_addition_install() -> i32 {
    if !scripting_addition_install_steps() {
        scripting_addition_remove_partial_install();
        return 1;
    }
    0
}
```

Not a `Drop` guard: the cleanup runs only on failure, and a guard would have to carry a
"succeeded" flag, which is the same code with more machinery. These belong to the `sa.m` writer;
the shape is fixed here so both writers produce the same thing.

### 5.4 Loop escape and shared tails

- `src/event_loop.c:1661` `if (!next) goto empty;` jumps out of two nested loops to `[pool drain]`
  and `sem_wait`. `DECISIONS.md` 19 replaces the whole lock-free queue with `mpsc`, so this
  `goto` has no counterpart: the drain happens when `try_recv` returns empty, exactly where the C
  drained. Nothing to translate.
- `src/event_loop.c:343` `out:` is reached both by `goto out` from an early failure and by
  falling off the end of the body, and its tail is `process_destroy(process)`. Write the body in a
  labelled block and the tail after it, as in §5.2. Do **not** duplicate `process_destroy` into
  the early-out branch.
- `src/event_loop.c:1119-1121`, `src/event_loop.c:1441-1448` and `src/window_manager.c:575` use
  `goto out` purely as an early return to a shared `return` statement. Plain `return` in Rust.

---

## 6. Logging: `debug!`, `warn!`, `error!`, `require!`

`src/misc/log.h` is the daemon's entire logging and fatal-error story. `DECISIONS.md` 33 makes the
four functions macros that take the C format text.

### 6.1 The four macros, in full

`src/misc/log.rs`:

```rust
#[macro_export]
macro_rules! debug {
    ($($argument:tt)*) => {{
        if $crate::misc::log::g_verbose() {
            print!($($argument)*);
        }
    }};
}

#[macro_export]
macro_rules! warn {
    ($($argument:tt)*) => {{
        eprint!($($argument)*);
    }};
}

#[macro_export]
macro_rules! error {
    ($($argument:tt)*) => {{
        eprint!($($argument)*);
        std::process::exit(libc::EXIT_FAILURE)
    }};
}

#[macro_export]
macro_rules! require {
    ($($argument:tt)*) => {{
        eprint!($($argument)*);
        std::process::exit(libc::EXIT_SUCCESS)
    }};
}
```

The missing semicolon after `std::process::exit` in the last two is deliberate: it makes the block
expression's type `!`, so `error!(...)` can stand where a value is expected, as it does in the
`match` arms of §8.3. Putting the semicolon back types the block as `()` and those call sites stop
compiling.

Fixed points:

- `error!` exits `EXIT_FAILURE` (`src/misc/log.h:34`) and `require!` exits `EXIT_SUCCESS`
  (`src/misc/log.h:45`). The `require!` exit code is not cosmetic: the launchd job has
  `KeepAlive.SuccessfulExit = false` (`src/misc/service.h:26-32`), so a non-zero exit from the
  three refusal paths at `src/yabai.c:268,272,276` would put launchd into an endless respawn loop.
- Both call `std::process::exit`, never `panic!`. `error!` is reachable from the event-loop
  thread (any `error` inside message handling) and must take the whole process down without
  unwinding, which is what the C `exit` does. `panic!` would unwind one thread and leave a live
  process with a dead event loop (`DECISIONS.md` 7).
- `require!` writes to **stderr and nothing else**. It does not call `notify` from
  `src/misc/notify.h`: `src/misc/log.h:37-46` contains no such call, and adding one would put a
  desktop notification where the C emits none (`DECISIONS.md` 3). The word "notify" in
  `DECISIONS.md` 33 names the role — the stand-down notice the user reads — not the function.
- Every call site of `error!`/`require!` that C followed with unreachable code keeps that code.
  C's `error` is not `noreturn`-annotated, so `src/yabai.c:252` falls through to a `return` the
  compiler still considers reachable; in Rust the `!` expansion makes it dead but legal. Do not
  restructure the call sites to remove it.
- `debug!` gates on `g_verbose` **before** formatting, where C evaluates its arguments first and
  then early-returns (`src/misc/log.h:9`). The only call site whose arguments do work is
  `src/window_manager.c:1657` (`ts_buf_len`, a pure read); every other side-effecting expression
  is hoisted into a local before the call, as at `src/window_manager.c:1442-1445`. So the guard
  changes nothing. It must stay in front, because it is what keeps the 87 `debug!` sites free in a
  non-verbose daemon.
- Name the `warn!` macro `warn`. Do not depend on the `log` crate (`DECISIONS.md` 11) and do not
  rename it to `warn_`.

`g_verbose` is the `AtomicBool` of §4:

```rust
pub fn g_verbose() -> bool {
    G_VERBOSE.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn set_g_verbose(value: bool) {
    G_VERBOSE.store(value, std::sync::atomic::Ordering::Relaxed);
}
```

### 6.2 `__FUNCTION__` becomes a literal

All 87 `debug` calls in the tree pass `__FUNCTION__` as their first argument, and every one of
them opens its format string with `"%s"`. Rust has no `__FUNCTION__`, and
`DECISIONS.md` 33 settles the substitute: **the function's own name, written as a string literal
at the call site.** Not a `function_name!()` macro built on `std::any::type_name`, not a
`#[track_caller]` trick, not a module path.

```rust
debug!("{}: {} ({})\n", "event_handler_application_launched", process.name, process.pid);
```

The literal is spelled exactly as the Rust function is named, which by `DECISIONS.md` 2 and 37 is
the C function's name. For the X-macro-generated handlers the C `__FUNCTION__` expands to
`EVENT_HANDLER_APPLICATION_LAUNCHED` (`src/event_loop.h:4`), so that — not the lowercase Rust
spelling — is the literal to write, or the log lines change. Check the expansion, not the
declaration, at every generated site: `src/event_loop.c` has 40 of them.

`tests/` greps daemon output, so these lines are part of the tested surface even though they are
not part of the byte-identical socket contract.

### 6.3 Every printf conversion in the tree, and its Rust spelling

Live code uses ten conversions. `DECISIONS.md` 29 fixes the casts.

| C | where | Rust | notes |
|---|---|---|---|
| `%s` | 385 sites | `{}` | a `&str` or `String`. A NULL `char *` prints `(null)`; see §6.4. |
| `%.*s` | 88 sites in `src/message.c` | `{}` of `&text[start..start + length]` | the precision is the token length of `DECISIONS.md` 27; slice, do not pass a width |
| `%d` | 177 sites | `{}` | cast the value `as i32` first when the C type is `uint32_t` (`DECISIONS.md` 29) — window ids, display ids and `getuid()` all print as signed |
| `%lld` | `src/event_loop.c:974,985,1004,1047,1056`, `src/event_signal.c:238,249,269,271`, `src/view.c:870` | `{}` of the value `as i64` | space ids are `uint64_t`; the C prints them signed |
| `%c` | `src/display_manager.c:16`, `src/space_manager.c:40,61,90` | `{}` of a `char` | the `,`/`]` separator of the query JSON |
| `%x` | `src/message.c:1373` | `{:x}` | `0x%x\n` of `insert_feedback_color.p`, a `u32`; lowercase, no padding |
| `%08x` | `src/rule.c:38` | `{:08x}` | inside `"\t\"flags\":\"0x%08x\"\n"` |
| `%04x` | `src/misc/helpers.h:311` | `{:04x}` | §2.3 |
| `%X` | `src/sa.m:287` | `{:X}` | uppercase, no width, of a `uint32_t` attribute mask |
| `%f` | `src/message.c:1291,1300` | `{:.6}` of the value `as f64` | printf's default precision is 6; `{}` alone prints the shortest round-trip and is wrong here |
| `%.2f` | `src/event_loop.c:1123,1170,1249`, two per line | `{:.2}` of the value `as f64` | the `CGPoint` of a mouse event, whose components are `CGFloat` already |
| `%.4f` | 19 sites in `src/window.c`, `src/display.c`, `src/rule.c`, `src/message.c`, `src/view.c` | `{:.4}` of the value `as f64` | this is query JSON, so it is byte-identical surface (`DECISIONS.md` 3) |

`%llu`, `%0.4f` and `%%` appear only inside the `PROFILE` machinery of `src/misc/timer.h:78-87`,
which `DECISIONS.md` 5 does not translate.

Three rules that go with the table:

1. **Cast before formatting, never after.** `{}` of a `u32` prints the unsigned value; the C
   `%d` prints it reinterpreted as `int`. Every `%d` of an unsigned becomes `{}` of `value as
   i32`, every `%lld` of a `uint64_t` becomes `{}` of `value as i64` (`DECISIONS.md` 29).
2. **Floats are cast to `f64` before formatting** (`DECISIONS.md` 29), matching the default
   argument promotion `printf` performs. Both `printf` and Rust round to nearest with ties to
   even, so `{:.4}` reproduces `%.4f` digit for digit once the value is an `f64`.
3. **`\n` stays in the format string.** These are `print!`/`eprint!`, not `println!`/`eprintln!`;
   the C format strings carry their own newline and several deliberately do not end with one
   (`src/misc/log.h:53` prints `"%s:"` with no newline).

### 6.4 `%s` with a NULL pointer

macOS libc prints `(null)`. The daemon relies on it: `window_title_ts` and friends return NULL
when `CFStringGetCString` fails (`src/misc/helpers.h:367`) and are passed straight to `debug` at
`src/window_manager.c:1445`. In Rust the value is an `Option<String>` and `{}` of a `None` does
not compile, so one helper in `src/misc/log.rs` does the job and is used at every such site:

```rust
pub fn or_null(value: Option<&str>) -> &str {
    match value {
        Some(text) => text,
        None => "(null)",
    }
}
```

`debug!("{}:{} {} - {} ({}:{}:{})\n", "window_manager_create_and_add_window", window.id as i32, application.name, or_null(window_title.as_deref()), or_null(window_role.as_deref()), or_null(window_subrole.as_deref()), window.is_root as i32)`.

`DECISIONS.md` 29 requires the same in the JSON writers, where a NULL `%s` also prints `(null)`
and that string reaches the socket.

### 6.5 stdout flushing

C's `stdout` is line-buffered on a tty and **fully buffered when piped**, which is how launchd
runs yabai (`StandardOutPath` in the plist, §8.1). Rust's `Stdout` is always a `LineWriter`. So
the Rust daemon flushes `debug!` output on every newline where the C batched it into 4 KiB
chunks, and the interleaving of yabai's stdout with its stderr and with the config script's output
in `/tmp/yabai_<user>.out.log` changes.

This is a change, and it is the one this document accepts rather than fights: matching C would
mean wrapping stdout in a `BufWriter` with an explicit flush discipline, which would then have to
be threaded through `error!` and `require!` so a fatal message is not lost. One line in
`DEVIATIONS.md`: *`src/misc/log.h:6-15` — C stdout is fully buffered under launchd, Rust's is
line-buffered; debug output interleaves differently in the launchd log. No content changes.*

`debug_message` already forces a flush of its own (`src/misc/log.h:60`); keep it.

### 6.6 `debug_message`

`src/misc/log.h:48-61`:

```c
static inline void
debug_message(const char *prefix, char *message)
{
    if (!g_verbose) return;

    fprintf(stdout, "%s:", prefix);

    for (;*message;) {
        message += fprintf(stdout, " %s", message);
    }

    putc('\n', stdout);
    fflush(stdout);
}
```

The loop advances by `fprintf`'s return value, which is `1 + strlen(message)` because of the
leading space, so it steps over each NUL of the double-NUL-terminated message built by
`client_send_message` (`src/yabai.c:73-82`). It is a plain function, not a macro, and it has one
call site: `src/event_loop.c:1633`.

```rust
pub fn debug_message(prefix: &str, message: &str) {
    if !g_verbose() {
        return;
    }

    print!("{}:", prefix);
    for token in message.split('\0') {
        if token.is_empty() {
            break;
        }
        print!(" {}", token);
    }
    print!("\n");
    let _ = std::io::Write::flush(&mut std::io::stdout());
}
```

The `break` on the first empty token is the C's `for (;*message;)`: the loop stops at the extra
trailing NUL, and an empty argv element would stop it early in C too. Do not reproduce the
return-value pointer walk.

---

## 7. `notify`

`src/misc/notify.h` surfaces scripting-addition and configuration failures as desktop
notifications, through the deprecated `NSUserNotification` API. Fourteen call sites, all on the
main thread: `src/sa.m:277,283,287,292,296,354,360,376,383,396,404` and
`src/misc/helpers.h:467,473,485`. `NSUserNotificationCenter` is not thread-safe; keep every call
site on the thread it is on today.

### 7.1 The delegate class

`src/misc/notify.h:10-18` declares a one-method delegate whose only job is to return `YES` from
`-userNotificationCenter:shouldPresentNotification:`, which forces the notification to appear even
while yabai is frontmost.

```rust
objc2::define_class!(
    #[unsafe(super(objc2_foundation::NSObject))]
    #[name = "NotifyDelegate"]
    struct NotifyDelegate;

    impl NotifyDelegate {
        #[unsafe(method(userNotificationCenter:shouldPresentNotification:))]
        fn user_notification_center_should_present_notification(
            &self,
            _center: *mut objc2::runtime::AnyObject,
            _notification: *mut objc2::runtime::AnyObject,
        ) -> bool {
            true
        }
    }
);
```

The protocol conformance is declared by implementing the selector, not by naming
`NSUserNotificationCenterDelegate`: the delegate property is untyped `id` and the runtime only
sends that one selector. `NSUserNotification` and `NSUserNotificationCenter` themselves are
hand-declared in `src/misc/notify.rs` with `objc2::extern_class!` plus `extern_methods!`, not
taken from `objc2-foundation`, because they are deprecated and gated there.

### 7.2 The deliberate leak

`src/misc/notify.h:22`:

```c
    [[NSUserNotificationCenter defaultUserNotificationCenter] setDelegate:[NotifyDelegate alloc]];
```

`alloc` without `init`, into an unretained (`assign`) delegate property. The object is leaked on
purpose so the property does not dangle. Rust reproduces the effect — an immortal delegate — with
a `static`, and fixes the `alloc`-without-`init`:

```rust
static G_NOTIFY_DELEGATE: OnceLock<SendRetained<NotifyDelegate>> = OnceLock::new();
static G_NOTIFY_IMG: OnceLock<SendRetained<NSImage>> = OnceLock::new();

fn notify_init() -> bool {
    let delegate = G_NOTIFY_DELEGATE.get_or_init(|| {
        SendRetained(unsafe { objc2::msg_send![NotifyDelegate::alloc(), init] })
    });
    unsafe {
        let center: *mut objc2::runtime::AnyObject =
            objc2::msg_send![class!(NSUserNotificationCenter), defaultUserNotificationCenter];
        let _: () = objc2::msg_send![center, setDelegate: &*delegate.0];
    }

    G_NOTIFY_IMG.get_or_init(|| {
        let bundle = NSBundle::mainBundle();
        let executable_path = unsafe { bundle.executablePath() }.unwrap();
        let resolved_path = unsafe { executable_path.stringByResolvingSymlinksInPath() };
        let workspace = unsafe { NSWorkspace::sharedWorkspace() };
        SendRetained(unsafe { workspace.iconForFile(&resolved_path) })
    });

    true
}
```

`SendRetained<T>` is the objc2 twin of `SendCFRetained<T>` (§1.14), in the same module:

```rust
pub struct SendRetained<T: ?Sized>(pub objc2::rc::Retained<T>);

unsafe impl<T: ?Sized> Send for SendRetained<T> {}
unsafe impl<T: ?Sized> Sync for SendRetained<T> {}
```

The explicit `retain` on `g_notify_img` (`src/misc/notify.h:23`) is the `Retained<NSImage>` the
`OnceLock` holds; the never-matching `release` is the `OnceLock` never dropping. Two
`DEVIATIONS.md` lines: the `alloc`-without-`init` becomes `alloc` + `init`, and the two leaks
become immortal statics with the same lifetime.

**`notify_init` stays lazy.** `src/misc/notify.h:33` calls it on the first `notify`, not at
start-up, and that ordering matters: it must run after `NSApplicationLoad()`
(`src/yabai.c:139`). An eager initialisation at process start would change that.

### 7.3 `notify` itself

`src/misc/notify.h:29-48` is variadic. In Rust it is a macro over a two-argument function, so the
format string stays verbatim at the call site and nothing builds a `va_list`:

```rust
#[macro_export]
macro_rules! notify {
    ($subtitle:expr, $($argument:tt)*) => {
        $crate::misc::notify::notify($subtitle, &format!($($argument)*))
    };
}

pub fn notify(subtitle: &str, informative_text: &str) {
    objc2::rc::autoreleasepool(|_pool| {
        if G_NOTIFY_DELEGATE.get().is_none() {
            notify_init();
        }

        unsafe {
            let notification: *mut objc2::runtime::AnyObject =
                objc2::msg_send![objc2::msg_send![class!(NSUserNotification), alloc], init];
            let _: () = objc2::msg_send![notification, setTitle: &*NSString::from_str("yabai")];
            let _: () = objc2::msg_send![notification, setSubtitle: &*NSString::from_str(subtitle)];
            let _: () = objc2::msg_send![notification, setInformativeText: &*NSString::from_str(informative_text)];
            let _: () = objc2::msg_send![notification, setValue: &*G_NOTIFY_IMG.get().unwrap().0, forKey: &*NSString::from_str("_identityImage")];
            let _: () = objc2::msg_send![notification, setValue: &*NSNumber::new_bool(false), forKey: &*NSString::from_str("_identityImageHasBorder")];
            let center: *mut objc2::runtime::AnyObject =
                objc2::msg_send![class!(NSUserNotificationCenter), defaultUserNotificationCenter];
            let _: () = objc2::msg_send![center, deliverNotification: notification];
            let _: () = objc2::msg_send![notification, release];
        }
    });
}
```

Fixed points:

- `[[NSAutoreleasePool alloc] init]` / `[pool drain]` (`src/misc/notify.h:31,47`) become
  `objc2::rc::autoreleasepool`. The build is `-fno-objc-arc` (`makefile:4`), so the manual
  retain/release maps onto objc2's explicit `Retained<T>` with no ARC assumptions.
- **`initWithFormat:arguments:` is dropped entirely.** All fourteen call sites use only `%s`
  (`src/misc/helpers.h:473,485`), `%X` (`src/sa.m:287`) and a `%s` version string
  (`src/sa.m:283`), and every one of them is expressible with `format!` per §6.3. Passing a
  Rust-built `va_list` into ObjC is not attempted.
- `[NSString stringWithUTF8String:]` returns `nil` for invalid UTF-8; `NSString::from_str` takes
  a `&str` that is valid by construction. The difference is confined to the error path where the
  C string came from the OS (`src/sa.m` version strings), and the Rust side already holds a
  `String` there.
- The two private KVC keys `"_identityImage"` and `"_identityImageHasBorder"` stay as
  `setValue:forKey:` message sends; there is no typed binding for them.
- The `release` of the notification (`src/misc/notify.h:44`) is written explicitly because the
  object is created through `msg_send!` and is not held in a `Retained`.
- `#[allow(deprecated)]` on the module is the counterpart of the
  `#pragma clang diagnostic ignored "-Wdeprecated-declarations"` pair at
  `src/misc/notify.h:7-8,50`.

---

## 8. `src/misc/service.h`

Five CLI verbs that write a launchd `LaunchAgent` plist and drive `/bin/launchctl`. Everything
runs once, on the main thread, before any other thread exists; every return value goes straight to
`exit` (`src/yabai.c:224-241`).

### 8.1 The constants and the plist template

```rust
const _PATH_LAUNCHCTL: &str = "/bin/launchctl";
const _NAME_YABAI_PLIST: &str = "com.asmvik.yabai";
const _PATH_YABAI_PLIST: &str = "{0}/Library/LaunchAgents/com.asmvik.yabai.plist";
```

The plist template is carried **byte for byte**, including the indentation anomaly at
`src/misc/service.h:29-31`, where three lines are indented with a space, a TAB and five spaces
instead of the eight spaces every other line uses. Verified against the source bytes: the
indentation on those three lines is exactly `0x20 0x09 0x20 0x20 0x20 0x20 0x20`. Write it as one
Rust string literal per C line, in the same order, with `\t` spelled as an escape so the anomaly
survives every editor and every `rustfmt` run:

```rust
const _YABAI_PLIST: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
    "<plist version=\"1.0\">\n",
    "<dict>\n",
    "    <key>Label</key>\n",
    "    <string>com.asmvik.yabai</string>\n",
    "    <key>ProgramArguments</key>\n",
    "    <array>\n",
    "        <string>{0}</string>\n",
    "    </array>\n",
    "    <key>EnvironmentVariables</key>\n",
    "    <dict>\n",
    "        <key>PATH</key>\n",
    "        <string>{1}</string>\n",
    "    </dict>\n",
    "    <key>RunAtLoad</key>\n",
    "    <true/>\n",
    "    <key>KeepAlive</key>\n",
    "    <dict>\n",
    "        <key>SuccessfulExit</key>\n",
    " \t     <false/>\n",
    " \t     <key>Crashed</key>\n",
    " \t     <true/>\n",
    "    </dict>\n",
    "    <key>StandardOutPath</key>\n",
    "    <string>/tmp/yabai_{2}.out.log</string>\n",
    "    <key>StandardErrorPath</key>\n",
    "    <string>/tmp/yabai_{2}.err.log</string>\n",
    "    <key>ProcessType</key>\n",
    "    <string>Interactive</string>\n",
    "    <key>Nice</key>\n",
    "    <integer>-20</integer>\n",
    "</dict>\n",
    "</plist>",
);
```

Three things that are easy to lose and change the file on disk:

1. The last line has **no trailing newline** (`src/misc/service.h:42`).
2. `_NAME_YABAI_PLIST` is concatenated into the `Label` line by the preprocessor
   (`src/misc/service.h:14`); inlining the literal, as above, produces the same bytes.
3. The four `%s` become `{0}`, `{1}`, `{2}`, `{2}` — executable path, `PATH`, user, user. The
   template contains no `{` or `}` of its own, so no escaping is needed.

Since `concat!` produces a `&'static str` and `format!` needs a literal, write the template
directly inside the `format!` invocation, or keep the `const` above and use
`runtime_format`-free positional substitution by declaring the template as a macro:

```rust
macro_rules! yabai_plist_template {
    () => {
        concat!(/* the 34 lines above */)
    };
}

let yabai_plist = format!(yabai_plist_template!(), executable_path, path_env, user);
```

That is the form to use: a `macro_rules!` whose expansion is the `concat!`, passed to `format!` as
its literal.

### 8.2 `safe_exec`

`DECISIONS.md` 34 keeps the libc calls. This overrides the `files/` inventory's
`std::process::Command` recommendation: `Command` inherits the environment, and
`src/misc/service.h:64` passes `envp == NULL`.

```rust
fn safe_exec(argv: &[&str], suppress_output: bool) -> i32 {
    let argument_strings: Vec<std::ffi::CString> = argv
        .iter()
        .map(|argument| std::ffi::CString::new(*argument).unwrap())
        .collect();
    let mut argument_pointers: Vec<*mut libc::c_char> = argument_strings
        .iter()
        .map(|argument| argument.as_ptr() as *mut libc::c_char)
        .collect();
    argument_pointers.push(core::ptr::null_mut());

    let mut process_id: libc::pid_t = 0;
    let mut actions: libc::posix_spawn_file_actions_t = unsafe { std::mem::zeroed() };
    unsafe { libc::posix_spawn_file_actions_init(&mut actions) };

    if suppress_output {
        let dev_null = std::ffi::CString::new("/dev/null").unwrap();
        unsafe {
            libc::posix_spawn_file_actions_addopen(
                &mut actions,
                libc::STDOUT_FILENO,
                dev_null.as_ptr(),
                libc::O_WRONLY | libc::O_APPEND,
                0,
            );
            libc::posix_spawn_file_actions_addopen(
                &mut actions,
                libc::STDERR_FILENO,
                dev_null.as_ptr(),
                libc::O_WRONLY | libc::O_APPEND,
                0,
            );
        }
    }

    let mut status: libc::c_int = unsafe {
        libc::posix_spawn(
            &mut process_id,
            argument_pointers[0],
            &actions,
            core::ptr::null(),
            argument_pointers.as_ptr(),
            core::ptr::null(),
        )
    };
    if status != 0 {
        return 1;
    }

    while unsafe { libc::waitpid(process_id, &mut status, 0) } == -1
        && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
    {
        unsafe { libc::usleep(1000) };
    }

    if libc::WIFSIGNALED(status) {
        1
    } else if libc::WIFSTOPPED(status) {
        1
    } else {
        libc::WEXITSTATUS(status)
    }
}
```

Every detail of the C survives:

- `posix_spawn`, **not** `posix_spawnp` — no PATH search; `argv[0]` is always the absolute
  `/bin/launchctl`.
- `envp == NULL`, the third `null` above.
- `/dev/null` opened `O_WRONLY|O_APPEND` twice, once per descriptor, not once shared.
- `status` is the same variable for the spawn result and the wait status
  (`src/misc/service.h:64,67`). If `waitpid` fails with something other than `EINTR`, `status`
  still holds the spawn result `0`, and the function returns `WEXITSTATUS(0)` = 0. Keep the
  single variable so that path is reproduced.
- `posix_spawn_file_actions_destroy` is never called (`src/misc/service.h:53-78`); the process
  exits immediately. Do not add it — adding it is harmless, but the point of this file is that
  nothing is added.

### 8.3 `populate_plist_path` and `populate_plist`

```rust
fn populate_plist_path() -> String {
    let home_reference = unsafe { objc2_foundation::NSHomeDirectoryForUser(None) };
    let home = match home_reference {
        Some(home_reference) => home_reference.to_string(),
        None => error!("yabai: unable to retrieve home directory! abort..\n"),
    };
    format!("{}/Library/LaunchAgents/com.asmvik.yabai.plist", home)
}

fn populate_plist() -> String {
    let user = match std::env::var("USER") {
        Ok(user) => user,
        Err(_) => error!("yabai: 'env USER' not set! abort..\n"),
    };

    let path_env = match std::env::var("PATH") {
        Ok(path_env) => path_env,
        Err(_) => error!("yabai: 'env PATH' not set! abort..\n"),
    };

    let mut executable_path_buffer = [0u8; 4096];
    let mut executable_path_size: u32 = executable_path_buffer.len() as u32;
    if unsafe {
        _NSGetExecutablePath(
            executable_path_buffer.as_mut_ptr() as *mut libc::c_char,
            &mut executable_path_size,
        )
    } < 0
    {
        error!("yabai: unable to retrieve path of executable! abort..\n");
    }
    let nul_position = executable_path_buffer
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(executable_path_buffer.len());
    let executable_path =
        String::from_utf8_lossy(&executable_path_buffer[..nul_position]).into_owned();

    format!(yabai_plist_template!(), executable_path, path_env, user)
}
```

- **`_NSGetExecutablePath`, never `std::env::current_exe()`** (`DECISIONS.md` 34).
  `current_exe()` resolves through `realpath`, so a yabai started through Homebrew's
  `/opt/homebrew/bin/yabai` symlink would get a different `ProgramArguments` string baked into the
  plist than the C writes. The declaration goes in `src/ffi/`:
  `unsafe extern "C" { fn _NSGetExecutablePath(buffer: *mut libc::c_char, size: *mut u32) -> i32; }`
- **`NSHomeDirectoryForUser(nil)`, never `$HOME`.** They agree in a normal session and differ
  under `sudo` or a sandboxed launch. This is the only ObjC dependency in `service.rs`.
- The C's manual sizing — `strlen(_PATH_YABAI_PLIST)-2 + strlen(home) + 1` and
  `strlen(_YABAI_PLIST)-8 + strlen(exe) + strlen(path) + 2*strlen(user) + 1` — exists only to size
  a `malloc`. `format!` removes it, together with the `memset` and the two permanent leaks
  (`src/misc/service.h:90-96,120-126`; neither buffer is ever freed, and `home` is leaked too).
  No behaviour change: the process exits either way.
- `*length = size-1` (`src/misc/service.h:127`) is the string length, not the allocation size. In
  Rust it is `yabai_plist.len()` at the one place it is used, §8.5.

### 8.4 `ensure_directory_exists`

`src/misc/service.h:132-153` truncates the path in place at the last `/`, `mkdir`s if missing, and
restores the slash. Use `Path::parent()`:

```rust
fn ensure_directory_exists(yabai_plist_path: &str) {
    let parent = std::path::Path::new(yabai_plist_path).parent().unwrap();
    if !directory_exists(&parent.to_string_lossy()) {
        let _ = std::fs::create_dir(parent);
    }
}
```

`create_dir`, not `create_dir_all`: the C creates exactly one level and ignores the error.

The two `NOTE(asmvik):` comments at `src/misc/service.h:134-139` and `:148-150` explain the
in-place truncation. That trick is gone, so **both comments are dropped** — a comment describing
code that no longer exists is worse than none (`DECISIONS.md` 38 carries comments over "at the
matching place", and here there is no matching place). Every other comment in the file is carried
verbatim; see §8.6.

### 8.5 The five verbs

```rust
fn service_install_internal(yabai_plist_path: &str) -> i32 {
    let yabai_plist = populate_plist();
    ensure_directory_exists(yabai_plist_path);

    let Ok(mut handle) = std::fs::File::create(yabai_plist_path) else {
        return 1;
    };

    match std::io::Write::write_all(&mut handle, yabai_plist.as_bytes()) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}
```

`File::create` + `write_all`, **not** `fs::write`: the C uses `fwrite(plist, length, 1, handle)`
with the whole plist as one item, so a short write yields 0 items and the function returns 1
(`src/misc/service.h:164-165`). `write_all` maps a short write to an error, which is the same
verdict; `fs::write` would hide it.

The rest transcribe directly, with `char service_target[MAXLEN]` / `char domain_target[MAXLEN]`
becoming `format!`:

```rust
let service_target = format!("gui/{}/{}", unsafe { libc::getuid() } as i32, _NAME_YABAI_PLIST);
let domain_target = format!("gui/{}", unsafe { libc::getuid() } as i32);
```

`getuid()` returns `uid_t` (`u32`) and the C prints it with `%d` (`src/misc/service.h:206,209`),
so the cast to `i32` is required by `DECISIONS.md` 29 even though no real uid can differ.

- `service_install` (`:171-180`) — `error!` if the plist already exists, else
  `service_install_internal`.
- `service_uninstall` (`:182-191`) — `error!` if missing, else
  `if std::fs::remove_file(path).is_ok() { 0 } else { 1 }`.
- `service_start` (`:193-248`) — install first if missing (`warn!` + install, `error!` if the
  install fails); probe `launchctl print <service_target>` with output suppressed; if that fails,
  `launchctl enable <service_target>` (result discarded) then return
  `launchctl bootstrap <domain_target> <plist_path>`; if it succeeds, return
  `launchctl kickstart <service_target>`.
- `service_restart` (`:250-262`) — `error!` if missing; return
  `launchctl kickstart -k <service_target>`.
- `service_stop` (`:264-312`) — `error!` if missing; same probe; if not bootstrapped, return
  `launchctl kill SIGTERM <service_target>`; if bootstrapped,
  `launchctl bootout <domain_target> <plist_path>` with the result **discarded**, then return
  `launchctl disable <service_target>`.

The inner `args` arrays shadow the outer one in C (`src/misc/service.h:215,227,235,245,285,299,303`).
In Rust give each its own binding named after the verb — `print_arguments`,
`enable_arguments`, `bootstrap_arguments`, `kickstart_arguments`, `kill_arguments`,
`bootout_arguments`, `disable_arguments` — and pass `&[&str]` without the trailing `NULL`, which
`safe_exec` appends itself.

### 8.6 Comments carried over verbatim

Eight `NOTE(asmvik):` blocks survive, at the matching place, with their original wording including
the typos (`boostrapped`, `**iff*`): `src/misc/service.h:44-51`, `:211-213`, `:220-225`,
`:230-233`, `:239-243`, `:277-279`, `:286-290`, `:296-304`. The two at `:134-139` and `:148-150`
are dropped, §8.4.

### 8.7 `system` and `popen`

`DECISIONS.md` 34 also keeps `system` (`src/sa.m:127,130,133,136,198`) and `popen`
(`src/sa.m:335`) as libc calls. Those lines belong to the `sa.m` writer; they are named here only
so nobody translates them into `std::process::Command` on the strength of `service.rs` doing
something else.

---

## 9. The easing curves and their X-macro

`ANIMATION_EASING_TYPE_LIST` (`src/misc/helpers.h:4-25`) is expanded three times: into the enum
constants (`:29-31`), into the name table (`:35-40`), and into the dispatch `switch` inside the
CVDisplayLink callback (`src/window_manager.c:551-553`). `DECISIONS.md` 31 makes it one
`macro_rules!` that generates all three.

### 9.1 The macro

```rust
macro_rules! animation_easing_type_list {
    ($callback:ident) => {
        $callback! {
            (EaseInSine, ease_in_sine, "ease_in_sine", 0),
            (EaseOutSine, ease_out_sine, "ease_out_sine", 1),
            (EaseInOutSine, ease_in_out_sine, "ease_in_out_sine", 2),
            (EaseInQuad, ease_in_quad, "ease_in_quad", 3),
            (EaseOutQuad, ease_out_quad, "ease_out_quad", 4),
            (EaseInOutQuad, ease_in_out_quad, "ease_in_out_quad", 5),
            (EaseInCubic, ease_in_cubic, "ease_in_cubic", 6),
            (EaseOutCubic, ease_out_cubic, "ease_out_cubic", 7),
            (EaseInOutCubic, ease_in_out_cubic, "ease_in_out_cubic", 8),
            (EaseInQuart, ease_in_quart, "ease_in_quart", 9),
            (EaseOutQuart, ease_out_quart, "ease_out_quart", 10),
            (EaseInOutQuart, ease_in_out_quart, "ease_in_out_quart", 11),
            (EaseInQuint, ease_in_quint, "ease_in_quint", 12),
            (EaseOutQuint, ease_out_quint, "ease_out_quint", 13),
            (EaseInOutQuint, ease_in_out_quint, "ease_in_out_quint", 14),
            (EaseInExpo, ease_in_expo, "ease_in_expo", 15),
            (EaseOutExpo, ease_out_expo, "ease_out_expo", 16),
            (EaseInOutExpo, ease_in_out_expo, "ease_in_out_expo", 17),
            (EaseInCirc, ease_in_circ, "ease_in_circ", 18),
            (EaseOutCirc, ease_out_circ, "ease_out_circ", 19),
            (EaseInOutCirc, ease_in_out_circ, "ease_in_out_circ", 20),
        }
    };
}

macro_rules! define_animation_easing_type {
    ($(($variant:ident, $function:ident, $name:literal, $value:literal)),* $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq)]
        #[repr(i32)]
        pub enum AnimationEasingType {
            $($variant = $value),*
        }

        pub static ANIMATION_EASING_TYPE_STR: [&str; EASING_TYPE_COUNT as usize] = [$($name),*];

        impl AnimationEasingType {
            pub fn apply(self, t: f32) -> f32 {
                match self {
                    $(AnimationEasingType::$variant => $function(t)),*
                }
            }

            pub fn from_index(index: i32) -> Option<AnimationEasingType> {
                match index {
                    $($value => Some(AnimationEasingType::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

animation_easing_type_list!(define_animation_easing_type);

pub const EASING_TYPE_COUNT: i32 = 21;
```

Fixed points:

- The 21 strings are the user-facing names of `yabai -m config window_animation_easing <name>`
  and are echoed back by `src/message.c:1318`. They are byte-exact and in the C order; the table
  is indexed by the discriminant at `src/message.c:1318` and scanned by the loop at
  `src/message.c:1321-1327`, which is why the discriminants are explicit (`DECISIONS.md` 31).
- `EASING_TYPE_COUNT` is 21 and is the loop bound at `src/message.c:1321`.
- `struct window_manager`'s field is `int window_animation_easing` (`src/window_manager.h:100`).
  In Rust it is an `AnimationEasingType`, converted once by `from_index` at the single write site
  (`src/message.c:1323`), which is already inside the validating loop. The C `switch` at
  `src/window_manager.c:550-554` has **no `default`**, so an out-of-range value would leave
  `float mt` uninitialised and produce a garbage transform. The typed field makes that
  unrepresentable. One `DEVIATIONS.md` line; no observable change, because the value can only come
  from the validated loop.
- The enum never crosses a thread boundary as a global: the value is snapshotted into the
  animation context at `src/window_manager.c:611` on the event-loop thread before the
  CVDisplayLink thread reads it.

### 9.2 The float/double promotion, curve by curve

This settles the inventory's open question. **Eighteen of the twenty-one curves are pure `f32`.
Three are not, and they are exactly the three that mention `M_PI`**, because `M_PI` is a `double`
and promotes the whole subexpression; `cosf`/`sinf` then narrow the *argument* back to `float`.

| curve | C | promotion |
|---|---|---|
| `ease_in_sine` (`:44`) | `1.0f - cosf((t * M_PI) / 2.0f)` | `t * M_PI` and `/ 2.0f` in `f64`, narrowed to `f32` at `cosf` |
| `ease_out_sine` (`:49`) | `sinf((t * M_PI) / 2.0f)` | same |
| `ease_in_out_sine` (`:54`) | `-(cosf(M_PI * t) - 1.0f) / 2.0f` | `M_PI * t` in `f64`, narrowed at `cosf`; the rest `f32` |
| the other eighteen (`:57-145`) | only `f` literals and `powf`/`sqrtf` | entirely `f32` |

So the rule for all 21, applied uniformly:

```rust
fn ease_in_sine(t: f32) -> f32 {
    1.0 - (((t as f64 * std::f64::consts::PI) / 2.0) as f32).cos()
}

fn ease_out_sine(t: f32) -> f32 {
    (((t as f64 * std::f64::consts::PI) / 2.0) as f32).sin()
}

fn ease_in_out_sine(t: f32) -> f32 {
    -(((std::f64::consts::PI * t as f64) as f32).cos() - 1.0) / 2.0
}

fn ease_in_quad(t: f32) -> f32 {
    t * t
}

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powf(3.0)
}
```

Two more details that apply to the eighteen:

- `powf(x, 3)` at `src/misc/helpers.h:79,94,109` passes an `int` literal that is converted to
  `3.0f`. Write `x.powf(3.0)`, **not** `x.powi(3)`: `powi` has a different rounding path and
  changes the last pixel of a long animation.
- `ease_in_expo`, `ease_out_expo` and `ease_in_out_expo` compare `t` against `0.0f` and `1.0f`
  with `==` (`src/misc/helpers.h:114,119,124`). Keep the exact comparisons; the caller clamps `t`
  to `[0.0, 1.0]` at `src/window_manager.c:546-547` precisely so those branches fire.

### 9.3 The call site narrowing

`src/window_manager.c:545` computes `double t` and `src/window_manager.c:551` passes it into a
`float` parameter, an implicit narrowing. `DECISIONS.md` 30 makes it explicit at the same point:

```rust
let mt: f32 = context.animation_easing.apply(t as f32);
```

`lerp` (`src/misc/macros.h:16`) is `(((1.0-t)*a) + (t*b))` with **unparenthesised arguments** and a
`double` literal `1.0`, so the whole expression is computed in `f64` and narrowed on assignment
into the `float tx/ty/tw/th` of `struct window_proxy` (`src/view.h:61`):

```rust
fn lerp(a: f64, t: f64, b: f64) -> f64 {
    (1.0 - t) * a + t * b
}
```

Call it as `lerp(proxy.frame.origin.x, mt as f64, animation.x as f64) as f32`. Computing this in
`f32` gives different pixel positions over a long animation; that is the single easiest way to
break `DECISIONS.md` 3's "window placement arithmetic to the pixel".

---

## 10. `cgimage_restore_alpha`

`src/misc/helpers.h:588-675`. Draws the captured window image into a fresh 32-bit
premultiplied-last bitmap context, un-premultiplies in place with SSE2 or NEON, and returns a new
CGImage the caller owns (released at `src/window_manager.c:492`).

`DECISIONS.md` 36: `std::arch` intrinsics **instruction for instruction**, both architectures, and
a 1-3 pixel tail through the same routine on a padded scratch copy. No scalar fallback: the SIMD
converts float to int with round-to-nearest-even (`_mm_cvtps_epi32`, `vcvtnq_s32_f32`) while a
Rust `as u8` truncates and saturates, so a scalar tail changes the last pixels.

### 10.1 The shape

```rust
pub fn cgimage_restore_alpha(image: &CGImage) -> Option<CFRetained<CGImage>> {
    let width = unsafe { CGImageGetWidth(image) };
    let height = unsafe { CGImageGetHeight(image) };
    let pitch = width * 4;

    let pixel_count = width * height;
    let mut data: Vec<u8> = vec![0; height * pitch];

    let color_space = unsafe { core_foundation::take_create_rule_result(CGColorSpaceCreateDeviceRGB()) }.unwrap();
    let context = unsafe {
        core_foundation::take_create_rule_result(CGBitmapContextCreate(
            data.as_mut_ptr() as *mut c_void,
            width,
            height,
            8,
            pitch,
            &*color_space,
            K_CG_BITMAP_BYTE_ORDER_32_BIG | K_CG_IMAGE_ALPHA_PREMULTIPLIED_LAST,
        ))
    }?;
    drop(color_space);
    unsafe { CGContextDrawImage(&*context, CGRectMake(0.0, 0.0, width as f64, height as f64), image) };

    let whole_groups = pixel_count / 4;
    for group in 0..whole_groups {
        unsafe { restore_alpha_four_pixels(data.as_mut_ptr().add(group * 16) as *mut u32) };
    }

    let remainder = pixel_count - whole_groups * 4;
    if remainder != 0 {
        let mut scratch: [u32; 4] = [0; 4];
        let tail = whole_groups * 16;
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr().add(tail),
                scratch.as_mut_ptr() as *mut u8,
                remainder * 4,
            );
            restore_alpha_four_pixels(scratch.as_mut_ptr());
            std::ptr::copy_nonoverlapping(
                scratch.as_ptr() as *const u8,
                data.as_mut_ptr().add(tail),
                remainder * 4,
            );
        }
    }

    let result = unsafe { core_foundation::take_create_rule_result(CGBitmapContextCreateImage(&*context)) };
    result
}
```

The 4-element `scratch` is the "padded scratch copy" of `DECISIONS.md` 36. It is zeroed, so the
`a > 0` mask discards the padding lanes exactly as the C's garbage lanes were discarded, and the
1-3 real lanes compute bit-identically because SIMD lanes are independent.

The C's loop (`src/misc/helpers.h:613`) runs `for (int i = 0; i < height*width; i += 4)` with no
tail handling, so its last iteration reads **and writes** up to 12 bytes past the
`calloc(height*pitch, 1)` buffer whenever `width*height % 4 != 0`. That is the undefined
behaviour `DECISIONS.md` 4 forbids reproducing; the scratch removes it with identical output.
One `DEVIATIONS.md` line.

`height*width` and `pitch` are `int` in C (`src/misc/helpers.h:591,613`); they are `usize` in
Rust. A greater-than-2-gigapixel image would have overflowed; it cannot now.

`r / a` with `a == 0` yields inf or NaN, and the lane is discarded by the `a > 0` mask
(`src/misc/helpers.h:620,646`). With intrinsics that is identical in Rust — do not add a guard.

### 10.2 The x86_64 lane routine

```rust
#[cfg(target_arch = "x86_64")]
#[inline]
unsafe fn restore_alpha_four_pixels(pixel: *mut u32) {
    use std::arch::x86_64::*;

    let inv255 = _mm_set1_ps(1.0 / 255.0);
    let one255 = _mm_set1_ps(255.0);
    let zero = _mm_set1_ps(0.0);
    let mask_ff = _mm_set1_epi32(0xff);

    let source = _mm_loadu_si128(pixel as *const __m128i);
    let mut r = _mm_cvtepi32_ps(_mm_and_si128(source, mask_ff));
    let mut g = _mm_cvtepi32_ps(_mm_and_si128(_mm_srli_epi32::<8>(source), mask_ff));
    let mut b = _mm_cvtepi32_ps(_mm_and_si128(_mm_srli_epi32::<16>(source), mask_ff));
    let mut a = _mm_cvtepi32_ps(_mm_and_si128(_mm_srli_epi32::<24>(source), mask_ff));
    let mask = _mm_castps_si128(_mm_cmpgt_ps(a, zero));

    r = _mm_mul_ps(one255, _mm_div_ps(r, a));
    g = _mm_mul_ps(one255, _mm_div_ps(g, a));
    b = _mm_mul_ps(one255, _mm_div_ps(b, a));

    a = one255;

    r = _mm_mul_ps(inv255, _mm_mul_ps(r, a));
    g = _mm_mul_ps(inv255, _mm_mul_ps(g, a));
    b = _mm_mul_ps(inv255, _mm_mul_ps(b, a));

    let sr = _mm_cvtps_epi32(r);
    let sg = _mm_slli_epi32::<8>(_mm_cvtps_epi32(g));
    let sb = _mm_slli_epi32::<16>(_mm_cvtps_epi32(b));
    let sa = _mm_slli_epi32::<24>(_mm_cvtps_epi32(a));

    let color = _mm_or_si128(_mm_or_si128(_mm_or_si128(sr, sg), sb), sa);
    let masked_color = _mm_or_si128(_mm_and_si128(mask, color), _mm_andnot_si128(mask, source));
    _mm_storeu_si128(pixel as *mut __m128i, masked_color);
}
```

`_mm_srli_epi32` and `_mm_slli_epi32` take the shift as a const generic in Rust, hence the
turbofish. SSE2 is baseline on `x86_64-apple-darwin`, so no `#[target_feature]` is needed.

The `a = one255` then multiply-by-`a` then multiply-by-`inv255` round trip
(`src/misc/helpers.h:625-633`) is a no-op algebraically and **two extra roundings** numerically.
Keep it. `DECISIONS.md` 36 says instruction for instruction, and simplifying it changes the low
bit of some channels.

### 10.3 The aarch64 lane routine

```rust
#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn restore_alpha_four_pixels(pixel: *mut u32) {
    use std::arch::aarch64::*;

    let inv255 = vdupq_n_f32(1.0 / 255.0);
    let one255 = vdupq_n_f32(255.0);
    let zero = vdupq_n_f32(0.0);
    let mask_ff = vdupq_n_s32(0xff);

    let source = vld1q_s32(pixel as *const i32);
    let shift_by_8 = vshlq_u32(vreinterpretq_u32_s32(source), vdupq_n_s32(-8));
    let shift_by_16 = vshlq_u32(vreinterpretq_u32_s32(source), vdupq_n_s32(-16));
    let shift_by_24 = vshlq_u32(vreinterpretq_u32_s32(source), vdupq_n_s32(-24));

    let mut r = vcvtq_f32_s32(vandq_s32(source, mask_ff));
    let mut g = vcvtq_f32_s32(vandq_s32(vreinterpretq_s32_u32(shift_by_8), mask_ff));
    let mut b = vcvtq_f32_s32(vandq_s32(vreinterpretq_s32_u32(shift_by_16), mask_ff));
    let mut a = vcvtq_f32_s32(vandq_s32(vreinterpretq_s32_u32(shift_by_24), mask_ff));
    let mask = vreinterpretq_s32_u32(vcgtq_f32(a, zero));

    r = vmulq_f32(one255, vdivq_f32(r, a));
    g = vmulq_f32(one255, vdivq_f32(g, a));
    b = vmulq_f32(one255, vdivq_f32(b, a));

    a = one255;

    r = vmulq_f32(inv255, vmulq_f32(r, a));
    g = vmulq_f32(inv255, vmulq_f32(g, a));
    b = vmulq_f32(inv255, vmulq_f32(b, a));

    let sr = vcvtnq_s32_f32(r);
    let sg = vshlq_s32(vcvtnq_s32_f32(g), vdupq_n_s32(8));
    let sb = vshlq_s32(vcvtnq_s32_f32(b), vdupq_n_s32(16));
    let sa = vshlq_s32(vcvtnq_s32_f32(a), vdupq_n_s32(24));

    let color = vorrq_s32(vorrq_s32(vorrq_s32(sr, sg), sb), sa);
    let masked_color = vorrq_s32(vandq_s32(color, mask), vbicq_s32(source, mask));
    vst1q_s32(pixel as *mut i32, masked_color);
}
```

Two spellings differ from the C because the C relies on Clang's lax vector conversions, which
Rust does not have. Both are exact in effect:

- `vshlq_u32(source, vdupq_n_s32(-8))` in C passes an `int32x4_t` where a `uint32x4_t` is
  expected. The Rust reinterprets explicitly. Logical versus arithmetic right shift cannot change
  the result here, because every shifted value is immediately masked with `0xff`.
- `vreinterpretq_s32_f32(vcgtq_f32(a, zero))` in C reinterprets from the wrong source type;
  `vcgtq_f32` yields `uint32x4_t`, so the Rust uses `vreinterpretq_s32_u32`. Same bits.

NEON is baseline on `aarch64-apple-darwin`; no `#[target_feature]` is needed. The two routines
share one name and one call site, so the `#[cfg]` choice is made once.

---

## 11. `macho_find_symbol`

`src/misc/macho_dlsym.h` reaches two SkyLight symbols `dlsym` cannot see, by walking an
already-mapped image's load commands and symbol table. It becomes `src/ffi/macho.rs`
(`TRANSLATION_PLAN.md` §1.5, `ffi-objc-and-os.md` §5), not a `src/misc/` module: it is
hand-declared Mach-O ABI, which `DECISIONS.md` 11 puts under `src/ffi/`. All four functions run on
the main thread before any other exists (`src/yabai.c:148-149`, inside
`configure_settings_and_acquire_lock`, called at `src/yabai.c:287`, before `event_loop_begin` at
`:291`). The whole module is `unsafe` (`DECISIONS.md` 39).

### 11.1 The hand-declared types

`DECISIONS.md` 11 puts the Mach-O symbol-table types under `src/ffi/`, hand-declared. No
`goblin`, no `object`, no `mach2`: those parse *files*, and what is needed is the in-memory layout
of a mapped image.

```rust
#[repr(C)]
pub struct mach_header_64 {
    pub magic: u32,
    pub cputype: i32,
    pub cpusubtype: i32,
    pub filetype: u32,
    pub ncmds: u32,
    pub sizeofcmds: u32,
    pub flags: u32,
    pub reserved: u32,
}

#[repr(C)]
pub struct load_command {
    pub cmd: u32,
    pub cmdsize: u32,
}

#[repr(C)]
pub struct segment_command_64 {
    pub cmd: u32,
    pub cmdsize: u32,
    pub segname: [u8; 16],
    pub vmaddr: u64,
    pub vmsize: u64,
    pub fileoff: u64,
    pub filesize: u64,
    pub maxprot: i32,
    pub initprot: i32,
    pub nsects: u32,
    pub flags: u32,
}

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

pub const LC_SEGMENT_64: u32 = 0x19;
pub const LC_SYMTAB: u32 = 0x2;
pub const SEG_LINKEDIT: &[u8] = b"__LINKEDIT";
```

`struct nlist_64`'s first field is a union in the SDK; only `n_strx` is ever read
(`src/misc/macho_dlsym.h:73`), so it is declared as the one `u32` field.

### 11.2 The walk

```rust
unsafe fn macho_find_image_header(target_name: &CStr, slide: &mut u64) -> *const mach_header_64 {
    let image_count = unsafe { libc::_dyld_image_count() };

    for index in 0..image_count {
        let image_name = unsafe { libc::_dyld_get_image_name(index) };
        if image_name.is_null() {
            continue;
        }

        if unsafe { CStr::from_ptr(image_name) } == target_name {
            *slide = unsafe { libc::_dyld_get_image_vmaddr_slide(index) } as u64;
            return unsafe { libc::_dyld_get_image_header(index) } as *const mach_header_64;
        }
    }

    core::ptr::null()
}

unsafe fn macho_find_linkedit_segment(header: *const mach_header_64) -> *const segment_command_64 {
    let mut offset = core::mem::size_of::<mach_header_64>();

    for _ in 0..unsafe { (*header).ncmds } {
        let command = unsafe { (header as *const u8).add(offset) } as *const load_command;

        if unsafe { (*command).cmd } == LC_SEGMENT_64 {
            let segment = command as *const segment_command_64;
            let segname = unsafe { &(*segment).segname };
            let name_length = segname.iter().position(|byte| *byte == 0).unwrap_or(16);
            if &segname[..name_length] == SEG_LINKEDIT {
                return segment;
            }
        }

        offset += unsafe { (*command).cmdsize } as usize;
    }

    core::ptr::null()
}

unsafe fn macho_find_symtab_command(header: *const mach_header_64) -> *const symtab_command {
    let mut offset = core::mem::size_of::<mach_header_64>();

    for _ in 0..unsafe { (*header).ncmds } {
        let command = unsafe { (header as *const u8).add(offset) } as *const load_command;

        if unsafe { (*command).cmd } == LC_SYMTAB {
            return command as *const symtab_command;
        }

        offset += unsafe { (*command).cmdsize } as usize;
    }

    core::ptr::null()
}

pub unsafe fn macho_find_symbol(target_image: &CStr, target_symbol: &CStr) -> Option<*mut c_void> {
    let mut slide: u64 = 0;
    let header = unsafe { macho_find_image_header(target_image, &mut slide) };
    if header.is_null() {
        return None;
    }

    let linkedit_segment = unsafe { macho_find_linkedit_segment(header) };
    if linkedit_segment.is_null() {
        return None;
    }

    let symtab_command = unsafe { macho_find_symtab_command(header) };
    if symtab_command.is_null() {
        return None;
    }

    let symbol_count = unsafe { (*symtab_command).nsyms };
    let linkedit_base =
        (unsafe { (*linkedit_segment).vmaddr } - unsafe { (*linkedit_segment).fileoff }) as usize;
    let symbol_str =
        (linkedit_base + unsafe { (*symtab_command).stroff } as usize + slide as usize) as *const u8;
    let symbol_sym =
        (linkedit_base + unsafe { (*symtab_command).symoff } as usize + slide as usize) as *const u8;

    for index in 0..symbol_count {
        let list = unsafe {
            symbol_sym.add(index as usize * core::mem::size_of::<nlist_64>())
        } as *const nlist_64;
        let symbol_name = unsafe { symbol_str.add((*list).n_strx as usize) } as *const libc::c_char;
        if unsafe { CStr::from_ptr(symbol_name) } == target_symbol {
            return Some((unsafe { (*list).n_value } + slide) as *mut c_void);
        }
    }

    None
}
```

Bounds and trust, exactly as far as the C goes and no further:

- **`segname` is `char[16]` and is not guaranteed NUL-terminated.**
  `string_equals(segment->segname, SEG_LINKEDIT)` (`src/misc/macho_dlsym.h:27`) `strcmp`s it, and
  a segment whose name fills all 16 bytes would over-read. The Rust compares the first 16 bytes up
  to the first NUL, which is what the C means and what the C does for every real image
  (`"__LINKEDIT"` is 10 bytes). Do **not** use `CStr::from_ptr` on that array.
- **Symbol names in the string table are NUL-terminated**, so `CStr::from_ptr` is correct at
  `src/misc/macho_dlsym.h:74`. A bogus `n_strx` reads out of bounds in both languages; the C
  trusts dyld and so does this.
- A `cmdsize` of 0 does **not** hang: both loops are bounded by `ncmds`, not by the offset
  (`src/misc/macho_dlsym.h:22,42`). It re-reads the same command `ncmds` times and returns NULL.
  Reproduce that, and do not add a `cmdsize == 0` guard. (The
  `files/misc-containers-and-allocators.md` §10.6 claim of an infinite loop is wrong; the C is the
  ground truth here.)
- Pointer arithmetic on `void *` is a GNU byte-granularity extension
  (`src/misc/macho_dlsym.h:23,43,68,69,72`). Every cast above goes through `*const u8` first.
  Getting this wrong by an element size is the likeliest bug in the file.
- `(int)header->ncmds` and `int symbol_count = symtab_command->nsyms` narrow `u32` to `int`
  (`:22,42,67`). Keep them `u32`; no image has 2^31 load commands or symbols.
- The image is found by its **exact path string**, including
  `/System/Library/PrivateFrameworks/SkyLight.framework/Versions/A/SkyLight`
  (`src/yabai.c:148-149`). On a system where the dyld shared cache reports a different name the
  lookup returns `None` and yabai falls back, unchanged.
- No fat/universal handling: by the time `_dyld_get_image_header` returns, dyld has chosen the
  slice.

### 11.3 The two consumers

`src/misc/extern.h:4-5` declares them as `static` function pointers, assigned at
`src/yabai.c:148-149`. They live in `src/ffi/skylight_dynamic.rs` (`TRANSLATION_PLAN.md` §1.5),
not in `src/ffi/macho.rs`. `DECISIONS.md` 18 makes them `OnceLock` statics, and `DECISIONS.md` 11
keeps the `Option` so the NULL checks survive.

```rust
pub static CGS_GET_CONNECTION_PORT_BY_ID: OnceLock<
    Option<unsafe extern "C" fn(i32) -> libc::mach_port_t>,
> = OnceLock::new();

pub static SLS_PERFORM_ASYNCHRONOUS_BRIDGED_WINDOW_MANAGEMENT_OPERATION: OnceLock<
    Option<unsafe extern "C" fn(*mut c_void) -> i64>,
> = OnceLock::new();
```

set once, from `configure_settings_and_acquire_lock`:

```rust
CGS_GET_CONNECTION_PORT_BY_ID
    .set(unsafe {
        macho_find_symbol(
            c"/System/Library/PrivateFrameworks/SkyLight.framework/Versions/A/SkyLight",
            c"_CGSGetConnectionPortById",
        )
    }
    .map(|address| unsafe { core::mem::transmute(address) }))
    .unwrap();
```

and the second with the mangled local symbol
`__ZL54SLSPerformAsynchronousBridgedWindowManagementOperationP47SLSAsynchronousBridgedWindowManagementOperation`.

The `Option` is what `src/window.c:956` and `src/space_manager.c:667,688` test, and both fall back
to a public API when it is `None`. Read them as
`if let Some(function) = *CGS_GET_CONNECTION_PORT_BY_ID.get().unwrap() { … }`. Note
`src/window.c:946` dereferences `CGSGetConnectionPortById` **without** a check, but that line is
only reachable through the checked branch at `:956`; keep the same structure rather than adding a
second check.

---

## 12. The lock file and the socket helpers

### 12.1 The lock file

`src/yabai.c:164-177`, inside the deprecation pragma pair at `:125,179`. `DECISIONS.md` 34 keeps
the libc calls and the descriptor that is never closed.

```rust
let lock_file = std::ffi::CString::new(g_lock_file()).unwrap();
let handle = unsafe {
    libc::open(
        lock_file.as_ptr(),
        libc::O_CREAT | libc::O_WRONLY | libc::O_CLOEXEC,
        0o600,
    )
};
if handle == -1 {
    error!("yabai: could not create lock-file! abort..\n");
}

let lock_file_description = libc::flock {
    l_start: 0,
    l_len: 0,
    l_pid: g_pid(),
    l_type: libc::F_WRLCK as i16,
    l_whence: libc::SEEK_SET as i16,
};

unsafe { libc::fcntl(handle, libc::F_SETLK, &lock_file_description) != -1 }
```

The descriptor is never closed and must not be: the advisory write lock is released the moment it
is, and a second yabai would start. So `handle` is a plain `i32` local that goes out of scope with
no `Drop` — do **not** wrap it in `File::from_raw_fd`, which would close it at the end of the
function and silently drop the lock. That is the single most damaging possible mistake in this
section.

`libc::flock`'s field order is not the declaration order in `src/yabai.c:169-175`; the C uses
designated initialisers. Name every field in Rust, as above, and the order stops mattering.

### 12.2 `socket_open` and `socket_close`

```rust
pub fn socket_open(socket_file_descriptor: &mut i32) -> bool {
    *socket_file_descriptor = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
    *socket_file_descriptor != -1
}

pub fn socket_close(socket_file_descriptor: i32) {
    unsafe {
        libc::shutdown(socket_file_descriptor, libc::SHUT_RDWR);
        libc::close(socket_file_descriptor);
    }
}
```

The out-param plus `bool` shape is kept rather than returning `Option<i32>`, because the
descriptor is handed to `event_loop_post` as a plain `int` `param1`
(`src/message.c:3009` → `src/event_loop.c:1613`) and crosses the queue as one. `UnixStream` is not
used anywhere in the daemon for the same reason; see §3.4.

`close` errors are ignored in C; ignore them in Rust. Callers: `src/yabai.c:122`,
`src/event_loop.c:1643` (the `DAEMON_MESSAGE` error path), `src/sa.m:246-266,428`.

`socket_connect` is §3.4.

### 12.3 `mach_send`

`src/misc/helpers.h:204-223` builds a complex Mach message with one out-of-line descriptor and
sends it fire-and-forget, discarding `mach_msg`'s return value. `DECISIONS.md` 35 pins the layout:

```rust
#[repr(C, packed(4))]
struct MachSendMessage {
    header: mach_msg_header_t,
    descriptor_count: u32,
    descriptor: MachMsgOolDescriptor,
}

const _: () = assert!(core::mem::size_of::<MachSendMessage>() == 44);
```

`mach_send` and this struct live in `src/ffi/mach_port.rs`; `ffi-objc-and-os.md` §15.5 owns both
and spells the struct `MachSendMessage`.

`<mach/message.h>` wraps the descriptor typedefs in `#pragma pack(push, 4)`, so
`sizeof(mach_msg_header_t)` is 24, the descriptor is 16 bytes with alignment 4, and the whole
message is **44 bytes with the descriptor at offset 28** — where the kernel expects descriptors to
start. A naturally aligned `#[repr(C)]` struct would be 48 bytes with the descriptor at 32 and the
message would be misparsed. `msgh_bits` is `0x8000_0013`
(`MACH_MSG_TYPE_COPY_SEND` in the remote field, or-ed with `MACH_MSGH_BITS_COMPLEX`); write the
constant with the C expression kept beside it as the name of a `const`. `msgh_size` and the
`send_size` argument are both 44.

Callers are on the event-loop thread (`src/window_manager.c:653,654,689`) and the CVDisplayLink
thread (`src/window_manager.c:579`); the data pointer is a caller stack struct, the 4104-byte
`JankyBordersEvent` of `ffi-objc-and-os.md` §15.6 (`src/window_manager.c:457`), which is fine
because the copy happens inside `mach_msg`.

---

## 13. `DEVIATIONS.md` lines this document commits to

One line each, in the form `DECISIONS.md` 4 and 5 require: C location, what C did, what Rust does.

| C location | C | Rust |
|---|---|---|
| `src/space_manager.c:503,533,569,598`, `src/display.c:207,235` | NULL `"Spaces"` value passed to `CFArrayGetCount` | `.unwrap()` panics, which the panic hook turns into an abort |
| `src/window.c:97-106` | `CFNumberGetValue` widening write into non-zeroed arena memory | zero-initialised local per element |
| `src/misc/helpers.h:401` | `string_copy` returns NULL on allocation failure | Rust aborts on allocation failure |
| `src/misc/helpers.h:191-195` | `sockaddr_un` handed to the kernel with `sun_len` and the `sun_path` tail uninitialised | zero-initialised; length still `sizeof(struct sockaddr_un)` |
| `src/yabai.c:86,135,136,137` | socket and lock paths truncate at `MAXLEN` | `format!`, no truncation |
| `src/yabai.c:253`, `src/misc/helpers.h:449,456,459` | config path truncates at 4096 | `format!`, no truncation |
| `src/event_loop.c:19,23-25` | `uint32_t window_list[1024]` filled with no bounds check | `Vec<u32>` |
| `src/misc/log.h:6-15` | stdout fully buffered under launchd | line-buffered; debug output interleaves differently in the launchd log |
| `src/yabai.c:51`, `src/message.c:1175,1177` | `g_verbose` is a plain `bool` written by one thread and read by all | `AtomicBool`, `Relaxed` |
| `src/misc/notify.h:22` | `[NotifyDelegate alloc]` without `init`, leaked | `alloc` + `init`, held in an immortal `static` |
| `src/misc/notify.h:23` | `g_notify_img` retained once, never released | immortal `static` |
| `src/misc/helpers.h:321-326` | `CFSTRINGNUM32` has no caller | not translated |
| `src/misc/helpers.h:613` | SIMD loop reads and writes up to 12 bytes past the buffer on a pixel count not divisible by 4 | 1-3 pixel tail through the same routine on a zeroed 4-pixel scratch; identical output |
| `src/window_manager.c:550-554` | easing `switch` has no `default`, leaving `float mt` uninitialised for an out-of-range value | `AnimationEasingType` makes it unrepresentable |
| `src/misc/service.h:90-96,120-126` | plist path and plist contents malloc'd and never freed | `String` |
| `src/misc/service.h:53-78` | `posix_spawn_file_actions_destroy` never called | unchanged; the process exits |
| `src/message.c:2997` and the other `%.*s` echoes | non-UTF-8 argument bytes echoed raw | `from_utf8_lossy` replaces them with U+FFFD |
| `src/misc/helpers.h:367` | arena bytes not rewound when `CFStringGetCString` fails | the `Vec` is dropped |

One line that belongs to the same family is deliberately **not** here:
`src/window_manager.c:437-458`, the JankyBorders fill loop that writes `animation_count` entries
into two `[512]` arrays. `ffi-objc-and-os.md` §15.6 owns `JankyBordersEvent` and carries that
line, and the arrays stay `[u32; 512]` because their size is the wire format (§3.3).
