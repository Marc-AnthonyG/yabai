# w1-repair-src-handles-rs

`GLOSSARY.md` section 1 | lists `ProcessSerialNumber` among the handle newtypes of `src/handles.rs` | it stays where the code on disk already defines it, `crate::ffi::carbon_process::ProcessSerialNumber` (`src/ffi/carbon_process.rs:8-13`), because `DECISIONS.md` 42 makes `src/ffi/` ground truth for the API it defines and the five files that use it already import it from there
