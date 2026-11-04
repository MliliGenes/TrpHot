# TrpHot

**Watch. Build. Restart.** A Rust project watcher with a live, local web dashboard.

Run `trphot .` to inspect your project in the browser: watched files, live changes, build results, application output, and editable build rules. It starts in spectator mode, so no build commands execute until you enable automatic builds or click **Build now**.

## Install

Requires Linux or macOS, Rust/Cargo, and Make for your project builds:

```sh
./install.sh
export PATH="$HOME/.local/bin:$PATH"
trphot .
```

The dashboard is embedded in the Rust binary. No Node.js, Bun, or frontend installation is needed. Installation builds from source; the legacy binary in `bin/` predates the web dashboard.

Use `PREFIX="$HOME/apps/trphot" ./install.sh` for another prefix, then add its `bin` directory to your PATH. To uninstall, delete the installed `bin/trphot`. Older installations may also have a `share/trphot` directory that can be removed.

## Use cases

### Explore a project without running commands

```sh
trphot .
```

The browser opens a local dashboard. Search the file list, inspect changes as you edit, and configure targets, application commands, and ignore patterns before enabling builds. **Build now** runs a single build even in spectator mode.

### Automatically rebuild and restart an application

```sh
trphot . --run --web --target all --exec './bin/app'
```

Build immediately, then rebuild after source changes and restart the application when the build succeeds. Follow compiler and application output in **Live activity**. A failed build keeps the previously running application alive.

### Watch a library or build-only project

```sh
trphot . --run --web --target all
```

Omit `--exec` to run Make without launching an application. Add generated output directories to the ignore rules to prevent rebuild loops.

### Use custom Make targets and generated files

```sh
trphot . --run --web --target generate --target all \
  --makefile dev.mk --ignore 'generated/**' --interval 500 --debounce 250
```

Targets are passed to one Make invocation, in the order supplied. All commands execute in the project directory. Use ordinary build targets rather than an indefinitely running `make run`.

### Work in a terminal or automation script

```sh
trphot . --run --target all --exec './bin/app'  # Plain terminal output
trphot . --headless                           # Watch without building
trphot . --once --target all                  # One build, then exit
trphot . --once --exec './bin/app'             # Build, then wait for app exit
```

`--once` returns a nonzero exit code if the build or application fails. `--run` uses plain terminal output unless `--web` is supplied.

### View a remote project over SSH

On the remote machine:

```sh
trphot . --port 8787 --no-open
```

On your local machine:

```sh
ssh -L 8787:127.0.0.1:8787 user@host
```

Open the complete dashboard URL printed by TrpHot, including its `#` token, in your local browser. The server binds only to `127.0.0.1`. By default it chooses an available port; `--port` provides a fixed port for forwarding.

### Try the included C example

```sh
trphot examples/hello
```

Click **Enable auto-build**, then edit and save `examples/hello/main.c`. Watch the build and restarted application's output in the dashboard. The example requires a C compiler.

## Dashboard controls

- **Enable/Disable auto-build:** switch between automatic builds and spectator mode.
- **Build now:** request a build using the current rules.
- **Pause/Resume:** suspend or resume build scheduling.
- **Stop app:** terminate the managed application.
- **Apply rules:** validate and use edited rules for this session.
- **Save current rules:** persist the engine's current rules to `.trphot.json`; apply edits first.
- Filter activity by output, file changes, or engine messages; disable **Follow output** to read earlier logs.

Changing rules does not itself trigger a build. Disabling auto-build or pausing does not terminate an existing build/application. Pending builds wait while paused. Closing or reloading the browser leaves the watcher running; press **Ctrl+C in the terminal** to stop it and its managed processes. The dashboard reconnects automatically while the engine is available.

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

Missing fields use defaults. CLI options override saved values; repeated `--ignore` options append rules. Paths and globs are relative to the project root. External configuration edits are loaded on the next launch. Save current rules only when you want CLI overrides to become project defaults.

Always excluded: `.git`, `node_modules`, `target`, `.trphot.json`, and its temporary save file. Symlinks are not followed. A directly executed relative program path (the first shell word) is automatically ignored to avoid rebuilding itself. For commands with quoted paths, wrappers, redirects, or custom output locations, explicitly ignore generated output.

## How it works

- Poll recursively for changes in file paths, sizes, and modification timestamps.
- Debounce edits and run at most one build at a time. Changes during a build queue another build.
- Keep the old application running during builds; replace it only after success.
- Stop process groups on exit, including SIGINT, SIGTERM, and SIGHUP.

Polling detects saved files, not editor buffers. Edits preserving both size and timestamp are not detectable. Scans are capped at 50,000 files; exclude large generated directories. The dashboard retains the latest 2,000 activity events and shows up to 1,000 matching files at a time; use search to narrow the list. Scan errors appear in activity. Child stdin is closed, so interactive/full-screen applications are unsupported. Programs that detach into their own sessions are outside process-group cleanup.

Make recipes and `--exec` execute project code. TrpHot never cleans project files or runs a clean target automatically. This is process restarting, not in-process code replacement. Windows is not currently supported.

## Development

Requires Rust/Cargo, Python 3, and Make. No JavaScript dependencies are needed.

```sh
make test
cargo clippy -- -D warnings
```

- `src/main.rs`: CLI, configuration, polling scheduler, JSON-lines engine interface.
- `src/watch.rs`: snapshots, ignore globs, change detection.
- `src/process.rs`: Make/app jobs, output channel, process-group cleanup.
- `src/web.rs`: loopback HTTP server, authenticated commands, bounded dashboard state.
- `web/`: embedded HTML, CSS, and JavaScript dashboard.
- `tests/`: engine lifecycle and dashboard HTTP integration checks.
