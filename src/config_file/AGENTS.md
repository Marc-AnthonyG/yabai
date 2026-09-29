# config_file

The user's config file: finding it, running it in a shell, and watching it so that it runs again
when `--reload-config-file-on-change` is on.

## Notes

- Unless `-c` names one, the config file is the first of `$XDG_CONFIG_HOME/yabai/yabairc`,
  `~/.config/yabai/yabairc` and `~/.yabairc` that exists. It runs in a child
  `/usr/bin/env sh`, with `-c` when the file is executable, that is never waited for.
- A reload runs the file again and resets nothing: settings it no longer mentions keep their
  current value, and rules and signals without a label are added a second time.
- The watcher thread starts the first time the setting is turned on and never stops; turning the
  setting off only makes it skip reloads. It runs the file only when its contents differ from what
  last ran, so a touch or an editor's swap file does not reload, and it compares right after each
  new watch starts, so a save made while no watch was in place is not missed.
