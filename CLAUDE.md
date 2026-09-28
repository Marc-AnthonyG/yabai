# CLAUDE.md

This window manager began as a line-by-line Rust port of yabai and has since gone its own way.
Staying compatible with yabai is not a goal.

- Anything yabai users or scripts could observe may change when the change makes the code
  simpler or better: the `-m` message grammar and its spellings, command-line options, help and
  failure texts, exit codes, query output (JSON layout, number formatting, ordering), the regex
  dialect of rules and signals, and numeric results reproduced from the C.
- A note in a folder's AGENTS.md that preserves C or yabai behaviour for its own sake no longer
  binds. Drop it when you change that code rather than working around it.
- What still binds is what other running code depends on: the protocol shared with the
  scripting addition (`src/osax/common.h`), the byte layouts macOS, the Dock and JankyBorders
  read, and the options a running daemon invokes its own binary with.
- The dependency list is open. Prefer a well-established crate to hand-written code whenever it
  removes code.
