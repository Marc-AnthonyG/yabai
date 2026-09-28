# query

The answers to `yabai -m query --windows`, `--spaces` and `--displays`: choosing which windows,
spaces or displays a query covers and framing their JSON objects as one array.

## Notes

- The array framing is part of the public query wire format, quirks included. The
  space and display arrays write their `]` in place of the separator after the last element
  instead of after the loop, so when the last element is skipped (a space without a view, a
  display without a space list) the array ends on a dangling `,` with no `]`, and an empty
  display list prints a lone `[`. This asymmetry is the C output and is kept on purpose; only
  the window array is closed after its loop.
- A space query is not read-only: once the space manager has begun, looking up a space creates
  its view if it does not exist yet, exactly as C did.
- Everything here runs on the event-loop thread and takes the managers it reads as explicit
  parameters.
