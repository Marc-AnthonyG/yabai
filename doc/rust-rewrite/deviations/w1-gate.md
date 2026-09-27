# Deviations — `w1-gate` (the wave 1 gate)

`TRANSLATION_PLAN.md:§3` | `#[allow(dead_code)]` at the crate root was to be the only lint silenced through wave 2 | `src/main.rs` also carries `#![allow(unused_variables)]` and `#![allow(unused_mut)]`. §3.3 requires every body to be `todo!()`, so every one of the 1671 parameters the nine `state-access/*.md` tables name is unused and two rows spell a `mut` binding the tables themselves print (`event_loop.md:114` `mut event_loop_owned_state`, `window-application-process.md:133` `mut window`). Underscoring them would contradict `DECISIONS.md` 37 and the table signatures, so the two crate-root lines are the only way to reach zero warnings with `todo!()` bodies; both are removed by `W3-2` with `dead_code`

`src/misc/helpers.h:534` | `psn_equals` names its two parameters `a` and `b` | `first` and `second` in `src/ffi/carbon_process.rs:68`, the spelling `GLOSSARY.md` §10.3 gives for `a`/`b` in `psn_equals`

`src/misc/helpers.h:328` | `CFNUM32` names its parameter `num` | `number` in `src/ffi/core_foundation.rs:184`, `GLOSSARY.md` §10.4

`src/misc/extern.h:8`, `:40` | the `_AXUIElementGetWindow` and `CGDisplayCreateUUIDFromDisplayID` prototypes name their parameters `wid` and `did` | `window_id` and `display_id` in `src/ffi/accessibility.rs:37` and `src/ffi/color_sync.rs:7`, `GLOSSARY.md` §10.2; the names on an `unsafe extern "C"` declaration bind nothing, so no call site changes

`TRANSLATION_PLAN.md:§3.3` | step 4 keeps a private `fn` "where `STATE_ACCESS.md` shows exactly one caller in the same module", and the `visibility` column of the nine tables marks 33 functions `private` | every one of them is `pub(crate)`, because `DECISIONS.md` 48 is a ruling after wave 0 and a document in this folder may not reopen it; the three groups Rust forbids a visibility modifier on are listed in `signature-changes/w1-gate.md`
