# serialise

The JSON yabai prints: snapshots of a display, a space and a window (tracked or untracked), the
field selection a query applies to them, the effective configuration `config get` prints, and the
rule and signal listings.

## Notes

- A snapshot is a plain `#[derive(Serialize)]` struct filled from the managers; its snake_case
  keys in declaration order are the JSON. serde_json's `preserve_order` keeps that order through
  the `Value` the field selection filters.
- A query selects fields by their JSON keys: the field-name enums in `command/query.rs` list the
  snapshot keys in the same order, which the tests check.
- Enumerated values print in the spelling `config set` takes, and an unset label, uuid or
  scratchpad is null. The effective configuration's keys are the setting names.
- An untracked window, one yabai holds no AX reference for, prints the same keys as a tracked
  one, with defaults wherever only AX or the window table could answer.
- The rule and signal listings are still hand-rolled JSON written through a `Response`.
- Everything here runs on the event-loop thread and takes the managers it reads as explicit
  parameters.
