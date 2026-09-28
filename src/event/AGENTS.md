# event

The event loop: the typed events that main-thread callbacks and the message socket post, the
channel that carries them to the event-loop thread, and the loop that hands each one to its
handler together with the state that thread owns.

## Notes

- The queue is an `mpsc` channel of events that own their payloads; dropping an event releases
  its payload. Events are handled in the order they were posted. The
  channel is created before any manager begins, so events posted during start-up wait in it until
  the loop thread starts.
- `EventLoopOwnedState` is moved into the loop thread when it is spawned and nothing else touches
  it afterwards. Posting an event is the only way another thread reaches it.
- A run of events is handled inside one autorelease pool, drained when the channel runs empty.
  Pending signals are flushed after every single event.
