# message

The daemon side of `yabai -m`: the socket that accepts client connections, the cursor that walks a
message's arguments, the selectors, labels, property lists and argument words the domains share,
the failure texts they share, and the switch that hands a message to its domain.

## Notes

- The accept thread only posts each connection to the event loop as an event (decision 19).
  Reading, parsing and answering a message happen on the event-loop thread, which passes the
  managers explicitly (decision 13).
- A message is the client's argument vector with every argument null-terminated. Tokens are
  ranges over that buffer and the cursor reproduces `get_token` exactly, including never stepping
  past the final terminator (decision 27). Some parsers write nulls into the buffer to split a
  token in place. Number parsing and the `sscanf` formats call libc rather than reimplementing it.
- Selector, label and argument spellings are the CLI (decision 3). A reserved selector word can
  never become a label.
- A selector that parses but cannot be resolved has already written its failure; the caller acts
  only on a resolved value. Failure texts and the failure prefix byte are part of the wire format
  (decisions 3, 28).
