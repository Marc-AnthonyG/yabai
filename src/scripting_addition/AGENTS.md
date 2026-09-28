# scripting_addition

The payload yabai injects into Dock.app to do what the public APIs cannot: the bundle that carries
it and how it is installed, loaded and validated, and the socket protocol yabai speaks to it.

## Notes

- The payload itself is C under `src/osax/` and is never edited. The opcodes, the
  buffer length, the payload version, its attribute bits and the socket path format are generated
  from `src/osax/common.h` at build time: change the header, never a Rust copy.
- A frame is a fixed 4096-byte buffer: an `i16` length that excludes itself, the opcode byte,
  then native-endian fields. A field that would overflow fails the whole request.
  The payload parses these bytes, so no layout detail may drift.
- Requests are sent from the event-loop thread and, for the proxy swap out, from the window
  animation completion thread. They share nothing but the socket path, which is set once before
  either thread runs.
- Install, uninstall and load run on the command-line path as root. They shell out through
  `system` and `popen` and build their paths as a C `snprintf` into a 512-byte buffer would. The bundle layout under `/Library/ScriptingAdditions/yabai.osax` and both plist
  texts are externally observable.
- The loader and the payload are linked with `-no_mac_public_arm64e`. Xcode 26 stamps arm64e
  executables with pointer-authentication ABI version 1, while Dock on macOS 15 runs version 0 and
  refuses a thread created from a version 1 loader ("could not spawn remote thread: (os/kern)
  protection failure"). build.rs fails the build if either binary's arm64e slice is stamped
  anything but version 0. `--load-sa` only replaces an installed scripting addition whose version
  string differs, so after a change to how these binaries are built, `sudo yabai --uninstall-sa`
  must run before `sudo yabai --load-sa`.
