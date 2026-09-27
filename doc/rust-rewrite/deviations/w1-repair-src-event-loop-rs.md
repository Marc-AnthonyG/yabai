# Deviations — `w1-repair-src-event-loop-rs` (`src/event_loop.rs`)

src/event_loop.h:6-46 | the `EVENT_TYPE_LIST` X-macro was expanded twice, into `enum event_type` (`:48-53`) and into the dispatch `switch` (`src/event_loop.c:1665-1667`) | a plain `pub(crate) enum Event`, forty variants written out in the C order; the `macro_rules! event_type_list` / `event_type_entry` pair that stood here is gone, because `files/entry-and-event-loop.md` §6.1 forbids `macro_rules!` for this list and `THREADS.md` §3.1 gives the enum as literal code, and the wave-2 dispatch `match` gets the exhaustiveness the X-macro bought for free

This withdraws the last entry of `deviations/w1-event_loop.md`, which recorded the macro.
