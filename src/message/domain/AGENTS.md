# message/domain

One executor per domain. Every domain but window takes a typed command; window still parses the
old argument vector the bridge hands it.

## Notes

- A typed executor returns the text to print, ending with a newline, or its failure. Only
  `config set` reports several failures: its settings are independent, so one the daemon refuses
  (an animation duration without the scripting addition) does not undo the others. Clap checks
  every value before the command is sent, patterns included; what only the daemon can check (a
  selector resolves, a space is not a fullscreen space, a rule has a matcher) is checked before
  anything changes.
- A selector left out means the focused display, space or window, and relative selectors
  (`next`, `west`, `stack.prev`…) resolve from it. A plural query prints an array, a singular one
  an object.
- `config set --space` gives the view its own value and flags it, so later global changes skip
  it; `space layout` is a different command that re-tiles without the flag.
- `rule apply` with neither a rule nor a flag re-applies every stored rule but the one-shot ones;
  with flags it applies that rule once without storing it.
- Handlers still parsing an old argument vector write through a `Response`; commands are tried
  in order and the first match wins.
- Handlers run on the event-loop thread and take the managers they touch as explicit parameters.
