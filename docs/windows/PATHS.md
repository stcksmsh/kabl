# kabl directories

All directories are resolved in `crates/ui/src/paths.rs`. An environment variable that is set but empty counts as unset. An override wins on every platform.

| What | Override | Linux | Windows |
|---|---|---|---|
| User library (`sounds/`, `composites/`, `library.json`, installed modules) | `KABL_USER_DIR` | `$XDG_DATA_HOME/kabl`, else `~/.local/share/kabl` | `%APPDATA%\kabl` |
| Config and themes (`settings.toml`, `themes/`) | `KABL_CONFIG_DIR` | `$XDG_CONFIG_HOME/kabl`, else `~/.config/kabl` | `%APPDATA%\kabl` |
| Logs (`kabl.log`) | `KABL_LOG_DIR`, else `KABL_USER_DIR\logs` | `$XDG_STATE_HOME/kabl/logs`, else `~/.local/state/kabl/logs` | `%LOCALAPPDATA%\kabl\logs` |
| Factory sounds | `KABL_FACTORY_DIR` | `<exe>/../share/kabl/patches`, then each `$XDG_DATA_DIRS/kabl/patches` (default `/usr/local/share:/usr/share`), then the checkout's packed `patches/` (development builds) | `<exe dir>\share\kabl\patches`, then the checkout's packed `patches/` |
| Factory composites | `KABL_FACTORY_COMPOSITES_DIR` | `<exe>/../share/kabl/factory-composites`, then each `$XDG_DATA_DIRS/kabl/factory-composites`, then the checkout's `composites/` | `<exe dir>\share\kabl\factory-composites`, then the checkout's `composites/` |

The first factory candidate that is a directory wins.

## Fallbacks

- Linux without `HOME`: the current directory (`./.local/share/kabl` and so on), as before.
- Windows without `%APPDATA%`: `%USERPROFILE%\AppData\Roaming`. Without `%LOCALAPPDATA%`: `%USERPROFILE%\AppData\Local`.
- Windows with neither: the lookup returns a `PathsError` ("cannot locate the %APPDATA% folder"). The plain functions (`user_dir`, `config_dir`, `log_dir`) print it to stderr and use the current directory instead of panicking.

## Not used yet

There is no cache directory. When one is added it belongs under `%LOCALAPPDATA%\kabl` on Windows and `$XDG_CACHE_HOME/kabl` on Linux, in `paths.rs`.

The installer (W9) must place factory content at `<install dir>\share\kabl\patches` and `<install dir>\share\kabl\factory-composites`, next to the executable.
