# ffi

The boundary with macOS: the single place the objc2 crates are re-exported from, and the bindings
those crates do not provide, declared by hand. That covers private SkyLight, Carbon, ColorSync,
the `kAX*` string constants, Mach messages, the Mach-O symbol-table walker, a few libsystem
calls and the wrapper that dispatches work onto the main queue.

## Notes

- The dependency list is closed (decision 11). A binding the crates lack is declared here, never
  pulled in through another crate or generated.
- A hand-written `extern` signature or struct layout that disagrees with what macOS exports is
  undefined behaviour the compiler cannot see. Packed Mach structs carry a `const` size
  assertion each (decision 35); hard-coded sizes and offsets are the platform's, not guesses.
- `unsafe` belongs here and at the few other sites decision 39 lists. Soundness arguments live in
  `THREADS.md`, not in comments (decision 38).
- The two SkyLight functions resolved at run time are written once, before any thread starts
  (decision 18). Either may be absent; callers fall back.
- Some re-exported free functions are deprecated by objc2 in favour of methods. They are used on
  purpose because they keep the C names (decision 37), which is why callers allow `deprecated`.
- What is declared here is the API (decision 42): call what exists rather than a sketch from a
  pattern document.
