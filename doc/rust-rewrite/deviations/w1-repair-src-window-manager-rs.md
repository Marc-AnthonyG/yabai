# Deviations — `w1-repair-src-window-manager-rs` (wave 1 repair, rounds 3, 4, 5, 6, 7 and 8)

No signature, type or import changed in round 3, round 4, round 5, round 6, round 7 or round 8: `src/window_manager.rs` had 0 errors
on both `aarch64-apple-darwin` and `x86_64-apple-darwin`, its 123 function signatures already match
`state-access/window_manager.md` §1 verbatim (the 17 rows the table marks `private` are `pub(crate)`
per decision 48, already recorded in `w1-window_manager.md`), and `WindowManager`, `Scratchpad`,
`WindowOpError`, `PurifyMode`, `FfmMode`, `WindowOriginMode`, `PURIFY_MODE_STR`, `FFM_MODE_STR` and
`WINDOW_ORIGIN_MODE_STR` match `GLOSSARY.md` §3.1, §3.18, §4.4, §4.6 and §6 field for field. The
453 warnings left in the file are all `unused variable` on parameters of `todo!()` bodies; the
parameter names are fixed by `GLOSSARY.md` and `state-access/window_manager.md`, so they are kept
as spelled and the warnings go away in phase 3. The one entry below records an addition made in
wave 1 that `w1-window_manager.md` did not list.

`src/window_manager.c:461` | `mach_send(port, &data, sizeof(data))` sends the anonymous `data` struct, whose size the receiver `git.felix.jbevent` depends on | `JankyBordersEvent` is `#[repr(C)]` and carries `const _: () = assert!(core::mem::size_of::<JankyBordersEvent>() == 4104);`, which pins the wire size the C `sizeof` computed
