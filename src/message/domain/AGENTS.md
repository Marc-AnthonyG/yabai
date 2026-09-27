# message/domain

One file per `yabai -m` domain (config, display, space, window, query, rule, signal): the command
and argument spellings that domain accepts and the handler that parses a message for it, runs the
command and writes the reply.

## Notes

- The response bytes are the public CLI wire format (decisions 3, 28, 29): every reply, every
  failure text word for word including its trailing newline, the order the lines are written in,
  and how numbers print (`{:.4}` for opacities and the split ratio, printf's six decimals for
  durations). None of it may be reworded or made consistent between domains.
- The command and argument strings are the CLI spelling (decision 3). A word two domains share is
  defined once, among the shared argument words or by the domain that owns it, and imported by
  the other.
- Commands are tried in the C order and the first match wins. An unknown command ends with the
  shared failure text.
- Handlers run on the event-loop thread and take the managers they touch as explicit parameters
  (decision 13).
