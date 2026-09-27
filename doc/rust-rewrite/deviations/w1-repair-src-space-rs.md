# Deviations — `w1-repair-src-space-rs` (`src/space.rs`)

No deviations. Rounds 5, 6, 7 and 8 required no change: the six signatures match
`state-access/space.md` §2 (rows `space_display_id` … `space_is_visible`) verbatim, including the
bare `Vec<WindowId>` returns that §2 judgement call 11 fixes and the `connection_id: i32` spelling
of `GLOSSARY.md:404`; all four imports (`DisplayId`, `SpaceId`, `WindowId`, `WindowManager`) are
used by those signatures, every function is `pub(crate)` (`DECISIONS.md` 48), `src/space.c` carries
no comment to transpose (`DECISIONS.md` 38), and the omission of `space_is_system` stays recorded
in `deviations/w1-space.md` under `DECISIONS.md` 5 and the "Not translated" table of
`state-access/space.md`.

No other Rust file in the crate calls into `crate::space` yet, so no sibling can disagree with
these signatures; `#![allow(dead_code)]` at `src/main.rs:1` keeps the six unused definitions quiet.

The 11 diagnostics located in the file are `unused variable` warnings on parameters of `todo!()`
bodies, the same shape every stubbed file in the crate carries while phase 2 is stubbed (the same
warning appears 246 times for `window_manager` alone across the crate); prefixing them with `_`
would contradict `DECISIONS.md` 37 and `GLOSSARY.md`, so the spellings stand. The file compiles
with 0 errors for both `aarch64-apple-darwin` and `x86_64-apple-darwin`.
