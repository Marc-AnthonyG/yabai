# message/domain

One executor per domain. Config takes a typed command; display, space, window, query, rule and
signal still parse the old argument vector the bridge hands them.

## Notes

- A typed executor returns the text to print, ending with a newline, or its failure. Only
  `config set` reports several failures: its settings are independent, so one the daemon refuses
  (an animation duration without the scripting addition) does not undo the others. Clap checks
  every value before the command is sent; what only the daemon can check (the space resolves, it
  is not a fullscreen space) is checked before anything changes.
- `config set --space` gives the view its own value and flags it, so later global changes skip
  it; `space layout` is a different command that re-tiles without the flag.
- Handlers still parsing an old argument vector write through a `Response`; commands are tried
  in order and the first match wins.
- Handlers run on the event-loop thread and take the managers they touch as explicit parameters.
