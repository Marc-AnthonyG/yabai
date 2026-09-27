# Deviations — `w1-repair-src-display-manager-rs` (`src/display_manager.rs`)

No deviations. The earlier `Option<CFRetained<CFString>>` entry is withdrawn: `src/ffi.rs:18` now
defines `CFStringOwned`, so the four uuid-returning functions match `GLOSSARY.md` §2.5 and
`state-access/display.md` §2 exactly.
