# query

Which displays, spaces and windows a query covers, and the snapshots of the windows on a set of
spaces.

## Notes

- A space query is not read-only: once the space manager has begun, looking up a space creates
  its view if it does not exist yet. A space without a view is left out.
- A window yabai does not track is still listed, with its untracked snapshot.
- Everything here runs on the event-loop thread and takes the managers it reads as explicit
  parameters.
