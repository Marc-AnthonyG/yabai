# message

The daemon side of client commands: the socket that accepts clients and decodes their requests,
the dispatch of a typed command to its domain and the resolution of typed selectors against live
state.

## Notes

- The accept thread reads a request under a one-second timeout, decodes it and checks its
  version, answering a failure itself. It posts a decoded command to the event loop together with
  the stream to reply on; executing and replying happen on the event-loop thread, which passes
  the managers explicitly.
- A selector resolves relative to the focused display, space or window unless a command names
  another; a selector that does not resolve is a failure naming it.
