# message/domain

One executor per domain: config, display, space, window, query, rule and signal.

## Notes

- Handlers still parsing an old argument vector write their reply and failures through a
  `Response`; commands are tried in order and the first match wins.
- Handlers run on the event-loop thread and take the managers they touch as explicit parameters.
