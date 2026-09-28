# signal

Signals: the subscriptions users add (an event type, app and title filters, an active filter, a
command and a label), the queue of events waiting to be delivered, and delivery by running each
matching subscriber's command in a child process.

## Notes

- Everything here runs on the event-loop thread. The subscriptions and the pending queue are
  owned by `EventLoopOwnedState` and passed in explicitly; queued signals own
  their strings.
- Delivery forks once per flush and the child forks again per command. The argv,
  the environment and the filter verdict are all computed in the parent before the first fork;
  the children only swap `environ`, exec and `_exit`.
- The signal type names are CLI and JSON spellings, and each discriminant indexes the per-type
  subscription table; the first and last names are sentinels.
- Removal is `swap_remove`, so the indices a listing shows can change order after a removal.
