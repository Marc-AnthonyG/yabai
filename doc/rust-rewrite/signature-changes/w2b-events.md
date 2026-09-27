# Signature changes — `w2b-events` (`src/event_loop.rs`, `src/mouse_handler.rs`, `src/main.rs`)

No frozen signature was changed, in these three files or in any other module. One item was
**added** to `src/mouse_handler.rs`:

`src/event_loop.c:1137`, `:1139` | — | `impl MouseMode { pub(crate) fn from_discriminant(discriminant: u8) -> MouseMode }` | `g_mouse_state.current_action = g_mouse_state.action1` / `action2` copied one `enum mouse_mode` into another; `DECISIONS.md` 23 and `GLOSSARY.md` §3.23 store `action1` / `action2` in `MOUSE_TAP_STATE` as an `AtomicU8` holding a `MouseMode`, while `MouseDragState::current_action` is a `MouseMode`, so the load needs the reverse of `as u8`. Only `MouseMode` discriminants are ever stored, so the `_ => MouseMode::None` arm for any other byte is unreachable
