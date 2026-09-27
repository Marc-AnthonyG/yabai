# Signature changes — `w2b-model` (second halves of `src/view.rs`, `src/window.rs`, `src/space_manager.rs`)

No frozen signature was changed, in these three files or in any other module, and no other module
was asked for one. One item with no C original was **added** to `src/space_manager.rs`, private to
the module:

`src/space_manager.c:670`, `:691` | `((id (*)(id, SEL, id, uint64_t))objc_msgSend)` cast inline at both call sites | `type InitWithWindowsSpaceIdFn = unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, u64) -> *mut AnyObject;` — the function-pointer type `objc2::ffi::objc_msgSend` is transmuted to, spelled as in `patterns/ffi-objc-and-os.md` §24
