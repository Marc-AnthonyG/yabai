# message

The daemon side of client commands: the socket that accepts clients and decodes their requests,
the dispatch of a typed command to its domain, the resolution of typed selectors against live
state and, until window, space and display are typed, the bridge that runs their old argument
vector through the tokenizer, selectors, labels, argument words and failure texts they share.

## Notes

- The accept thread reads a request under a one-second timeout, decodes it and checks its
  version, answering a failure itself. It posts a decoded command to the event loop together with
  the stream to reply on; executing and replying happen on the event-loop thread, which passes
  the managers explicitly.
- A selector resolves relative to the focused display, space or window unless a command names
  another; a selector that does not resolve is a failure naming it.
- The bridge rebuilds the old message: every argument null-terminated, then padding nulls. Tokens
  are ranges over that buffer and `take_next_token` never steps past the final terminator. Some
  parsers call libc's `sscanf` on a token.
- An untyped handler writes through `support::response::Response`; a selector that parses but
  cannot be resolved has already written its failure, and the caller acts only on a resolved
  value.
