# serialise

The JSON yabai prints for one window (tracked or untracked), space, display, rule or signal, the
arrays the rule and signal listings print, the property tables that name each field a query can
select, and the effective configuration `config get` prints.

## Notes

- The effective configuration is a serde struct whose snake_case keys are the setting names;
  each value prints in the spelling `config set` takes.
- The query bytes are the public query wire format and are written with hand-rolled
  format strings: field names and their order, the tab indentation, the `,\n`
  between fields, the `, ` inside the `spaces` and `windows` arrays of a display and a space,
  `{:.4}` for frames and opacity, ids printed as signed integers. None of it is to be "cleaned
  up", reordered or made consistent between objects.
- A property table's names are both the CLI spelling a query selects fields with and the JSON
  key; its values are the selection bits, and a selection of zero prints every field.
- An untracked window, one yabai holds no AX reference for, prints the same keys as a tracked
  one, with fixed placeholder values wherever only AX or the window table could answer.
- Everything here runs on the event-loop thread, writes through a `Response`
  and takes the managers it reads as explicit parameters.
