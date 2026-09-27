# Deviations — `w0-scaffold` (W0-1, W0-2, W0-3, W0-13)

`src/sa.h:4-7` | `__src_osax_payload_len` and `__src_osax_loader_len` sized the two `xxd` arrays | not translated; `OSAX_PAYLOAD.len()` and `OSAX_LOADER.len()` replace them
`TRANSLATION_PLAN.md:§1.4, §3.1` | each wave 0 unit was to create its own module file and add its own `pub mod` line | every wave 0 module file (sixteen `src/ffi/*.rs`, nine `src/misc/*.rs`) is created empty by `w0-scaffold` and declared up front in `src/ffi.rs` and `src/misc.rs`, so W0-4..W0-12 own disjoint files and can run in parallel
`patterns/ffi-objc-and-os.md:§25.1` | the generated opcode enum was spelled `enum sa_opcode` with `SA_OPCODE_*` variants | `build.rs` emits `SaOpcode` with CamelCase variants without the prefix, per `DECISIONS.md` 37 and `TRANSLATION_PLAN.md` §1.3 item 3
`TRANSLATION_PLAN.md:§1.4` | `src/globals.rs` was to re-export the two runtime-resolved SkyLight pointers | the re-export is absent until W0-4 fills `src/ffi/skylight_dynamic.rs`; it is W0-4's line to add
`TRANSLATION_PLAN.md:§1.4` | `MOUSE_TAP_STATE` (decision 23) was to be a static of atomics in `src/globals.rs` | absent until W0-5 declares `__CFMachPort`, `__CFRunLoopSource` and `CGEvent` in `src/ffi/`
`src/yabai.c:27-37` | `src/state.rs` names real manager types | the ten fields carry empty placeholder types declared in `src/state.rs`; W1-main replaces them, per W0-3's row
