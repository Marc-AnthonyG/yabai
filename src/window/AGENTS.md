# window

Windows as yabai tracks and drives them: the window struct and what AX and SkyLight report about
it, the window manager's tables and settings, discovering windows and applying rules to them, the
lookups that select a window, what changes when a window gains focus, and every window command
(focus, frame, opacity, layer, shadow, fullscreen and zoom, floating, placement in a tree, on a
grid or on another space, the scratchpad), including the animation that carries windows to their
new frames through proxy windows.

## Notes

- Apart from the callbacks and threads named below, everything here runs on the event-loop
  thread and takes the managers it touches as explicit parameters (decision 13). Windows and
  applications are named by id and looked up at each use (decision 14); a lookup miss is an early
  return.
- The liveness cell is shared between a window and its AX refcon (decision 21). Claiming a
  window for destruction is a compare-exchange from alive to dead; every other probe is a load of
  the same cell. An event for a window that is dead or no longer tracked is dropped.
- One animator drives every window animation (decision 55), shared through an `Arc` by four
  kinds of thread. The event-loop thread takes each request: it retargets windows already moving,
  captures and builds proxies for the others on scoped builder threads it joins, swaps them in,
  sets the real frames through AX and sets the request in motion. The CVDisplayLink thread moves
  every proxy once per frame. One long-lived completion thread notifies JankyBorders, waits its
  20 ms, swaps finished proxies out and releases them, merging every job already queued into one
  batch. The SkyLight connection and the completion thread start with the first animation and
  last as long as the animator; a display link exists only while some window is moving.
- One mutex guards every window's animation phase and the current display link. It is never held
  across anything that can block or call back: no socket, sleep, AX call, SkyLight commit or
  CoreVideo call. The tick holds it only to compute the displayed frames and collect the windows
  that came to rest, then commits one transaction and hands work to the completion thread after
  releasing it. CoreVideo is kept out of the lock because stopping a link from another thread
  waits for its running callback, and that callback may be waiting for the mutex.
- A window is moving (it owns its proxy and its layers), awaiting its proxy swap-out, or absent.
  Only the event loop adds one; only a tick, or a display link that failed to start, turns a moving
  window into one awaiting swap-out, handing its proxy to a completion job; only the completion
  thread removes it, after the swap-out has returned. All of it happens under the mutex, so a
  request either finds the window moving and adds a layer, or finds it absent and builds a new
  proxy: a proxy has exactly one owner and is never animated and swapped out at once. A request
  holds the layers it adds until all its windows are in motion, so a retargeted window cannot come
  to rest in between. A request for a window awaiting swap-out waits on the condition variable
  until the swap-out returns, so its swap-in and JankyBorders notification always follow the old
  ones.
- The tick retires the display link, under the mutex, when no window is left moving, and a
  request starts a new one when it admits a moving window and none exists; a tick from a link that
  is not the current one does nothing. The link's callback reads the animator through a raw `Arc`
  the link owns. A retired link is released on the completion thread, whose stop waits for the
  last callback, so that `Arc` is never dropped under a running callback. That is why the link
  handle is `Send`; a proxy crosses threads because its CoreGraphics context is created and drawn
  on one builder thread and afterwards only released, on the completion thread.
- A proxy takes its real window's alpha when it is built and refreshes it at most every 1/30 s
  while it moves, so an opacity fade that runs during the animation still ends right without a
  window-server query per window per frame.
- Rule matching and reapplication take the rules they work on out of the window manager and put
  them back afterwards; code reached in between sees an empty rule list, or a default rule in the
  slot of the rule being reapplied.
- Bytes other programs read are fixed: the synthesized focus event records and their offsets,
  the sub-level Mach message (decision 35, its message id differs on Tahoe) and the JankyBorders
  notification, whose size is asserted at compile time and whose service name and event numbers
  JankyBorders listens for (decision 3).
- The sleeps are timing behaviour and stay (decision 3): 40 ms between the two focus events,
  100 ms per spin while a native-fullscreen transition finishes, 20 ms after a JankyBorders
  notification that waits.
