# ffi

The boundary with macOS: the single place the objc2 crates are re-exported from, and the bindings
those crates do not provide, declared by hand. That covers private SkyLight, Carbon, ColorSync,
CoreText, the `kAX*` string constants, Mach messages, the Mach-O symbol-table walker, a few
libsystem calls and the wrapper that dispatches work onto the main queue.

## Notes

- The dependency list is closed. A binding the crates lack is declared here, never
  pulled in through another crate or generated.
- A hand-written `extern` signature or struct layout that disagrees with what macOS exports is
  undefined behaviour the compiler cannot see. Packed Mach structs carry a `const` size
  assertion each; hard-coded sizes and offsets are the platform's, not guesses.
- `unsafe` belongs here, at refcon and context casts, `Send`/`Sync` impls, packed Mach messages, the
  Mach-O symbol walker and the Accessibility pid offset read, and nowhere else. Soundness arguments
  live in the AGENTS.md of the module that relies on them, not in comments.
- The two SkyLight functions resolved at run time are written once, before any thread starts. Either may be absent; callers fall back.
- Some re-exported free functions are deprecated by objc2 in favour of methods. They are used on
  purpose because they keep the C names, which is why callers allow `deprecated`.
- What is declared here is the API: call what exists rather than a sketch from a
  pattern document.
