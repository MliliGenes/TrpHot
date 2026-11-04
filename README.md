# TrpHot

**Watch. Build. Restart.** A Rust project watcher with a real OpenTUI cockpit.

`trphot .` opens a spectator: browse files, see changes live, and configure rules before enabling builds. The engine runs independently of the interface; the CLI needs no JavaScript runtime.

## Install

On Linux or macOS, install Make, Node.js/npm, and optionally Rust/Cargo, then:

```sh
./install.sh
export PATH="$HOME/.local/bin:$PATH"
trphot .
```

The ZIP includes a static Linux x86_64 engine; no Rust compiler is required on that platform. Other platforms build from source using Cargo. The installer downloads pinned OpenTUI and a local Bun runtime using npm; internet access is needed for this step. No root privileges or global npm packages are required. Node 22+ is recommended for the installer. OpenTUI itself runs under the installed Bun, not Node. npm may report OpenTUI's Node engine requirement because your Node is older than 26.4; this does not affect the Bun runtime. If your npm configuration enforces `engine-strict`, use a supported Node version for installation.

Use `PREFIX="$HOME/goinfre/apps/trphot" ./install.sh` for another prefix. Add that prefix's `bin` to PATH. Keep its `bin` and `share` directories together. To remove the default install, delete `~/.local/bin/trphot` and `~/.local/share/trphot`.

## Commands

```sh
trphot .                                      # OpenTUI spectator; no builds
trphot . --target all --exec './bin/app'        # Prefill TUI rules
trphot . --run --target all                    # Automatic builds, plain terminal output
trphot . --run --target all --exec './bin/app'  # Build and restart automatically
trphot . --run --tui --exec './bin/app'         # Start armed inside the TUI
trphot . --headless                            # Spectate without OpenTUI
trphot . --once --target all                   # One build; exit with build result
trphot . --once --exec './bin/app'              # One build; run until app exits
trphot . --run --target generate --target all \
  --makefile dev.mk --ignore 'generated/**' --interval 500 --debounce 250
```

`--run` enables automatic builds. `--exec` sets the application command; without it TrpHot only builds. Targets are passed to a single Make invocation. Use ordinary build targets, not an indefinitely running `make run`. All commands execute in the project directory. `--once` enables a build even without `--run`, then optionally waits for the executable. Nonzero build/application exits produce a nonzero TrpHot exit code in this mode.

## TUI keys

| Key | Action |
| --- | --- |
| Tab | Files / Activity / Rules |
| Up / Down | Select a file or scroll activity/rules |
| `/` | Filter the file list by path |
| `x` | Ignore/unignore the selected file |
| `t` | Edit space-separated Make targets |
| `m` | Edit Makefile path |
| `e` | Edit application command; empty means build only |
| `g` | Add an ignore glob |
| `c` in Rules | Clear custom ignore globs |
| `i` / `d` | Edit polling interval / debounce in milliseconds |
| `a` | Arm/disarm automatic builds |
| `b` | Request a build now, including in spectator mode |
| `p` | Pause/resume build scheduling |
| `k` | Stop the current application |
| `s` | Save current rules in the project |
| `q` / Ctrl+C | Quit and stop managed processes |
| Enter / Escape in a field | Apply / cancel editing |

Rule changes apply immediately in memory; press `s` to persist. Invalid values are rejected by the Rust engine. Editing a rule does not itself run a build. Disarming or pausing does not terminate a current build/app; `k` stops the application. Pending builds wait while paused. A file covered by a broad ignore glob must be unignored by clearing/replacing that rule, rather than the single-file toggle.

## Saved configuration

Project-relative `.trphot.json`:

```json
{
  "targets": ["all"],
  "makefile": "Makefile",
  "exec": "./bin/app",
  "ignores": ["**/*.o", "**/*.a", "build/**", "bin/**"],
  "interval": 300,
  "debounce": 200
}
```

Missing fields use defaults. CLI options override saved values; repeated `--ignore` options append rules. Paths/globs are relative to the project root; `generated/**` excludes a directory. Save only when you want current CLI overrides to become project defaults. External configuration edits are loaded on the next launch.

Always excluded: `.git`, `node_modules`, `target`, `.trphot.json`, and its temporary save file. Symlinks are not followed. A directly executed relative program path (the first shell word) is automatically ignored to avoid rebuilding itself. For commands containing quoted paths with spaces, wrappers, redirects, generators or custom output paths, explicitly ignore their output. This is especially necessary in build-only mode: TrpHot cannot infer every output produced by an arbitrary Makefile.

## Lifecycle

- Poll recursively, comparing file path, size, and modification timestamp. Detect additions, changes, and deletions.
- Debounce edits; run at most one build at a time. Changes while building queue another build.
- Build while the existing app continues running. Failed builds preserve it.
- On success, terminate the old process group, wait briefly, kill remaining children if necessary, then launch the replacement.
- Quit handles SIGINT, SIGTERM, SIGHUP, or interface disconnection and stops the build/application groups.

Polling detects saved files, not editor buffers. An edit that preserves both size and timestamp is not detectable. Scans are capped at 50,000 files: exclude large generated directories. Persistent scan errors appear in activity; resolve them before relying on automatic rebuilds. The interface keeps the latest 2,000 output lines, with long individual lines chunked by the engine. Child stdin is closed: interactive/full-screen applications are not supported in this version. Programs that detach into their own sessions are outside process-group cleanup.

Only run projects and commands you trust: Make recipes and `--exec` intentionally execute project code. TrpHot never cleans project files or runs a clean target automatically.

## Try the included example

```sh
trphot examples/hello
```

Press `a` to build and start the C example, Tab to Activity, then edit `examples/hello/main.c` in another terminal. Save to rebuild and restart. Press `q` to stop.

## Development and checks

Requires Rust/Cargo, npm, Python 3, and Make. The example additionally needs a C compiler.

```sh
npm ci
make test
python3 tests/terminal.py   # Linux PTY check of the actual OpenTUI launcher
cargo clippy -- -D warnings
```

Architecture:

- `src/main.rs`: CLI, configuration, polling scheduler, JSON-lines interface.
- `src/watch.rs`: snapshots, ignore globs, change detection.
- `src/process.rs`: Make/app jobs, bounded output channel, process-group cleanup.
- `ui/app.ts`: OpenTUI files/activity/rules interface.
- `ui/main.ts`: interface lifecycle and engine transport.

OpenTUI: https://github.com/anomalyco/opentui — pinned to `@opentui/core` 0.8.1. The engine is Rust; OpenTUI's supported TypeScript API drives its Zig renderer. This is process restarting, not in-process code replacement. Windows is not currently supported.
