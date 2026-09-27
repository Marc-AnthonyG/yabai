# Deviations — `w1-repair-src-mouse-handler-rs` (`src/mouse_handler.rs`)

`src/mouse_handler.c:5`, `:21` | `mouse_mod_from_cgflags` and `mouse_handler` are file-`static`, and `state-access/signal-rule-mouse.md` §5 pastes that into its visibility column as `private` | both are `pub(crate)`, because `DECISIONS.md` 48 makes every phase-2 function `pub(crate)` and postdates the state-access tables; phase 3 tightens visibility
