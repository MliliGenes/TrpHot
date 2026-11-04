#!/bin/sh
set -eu
base=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
prefix=${PREFIX:-"$HOME/.local"}
command -v npm >/dev/null 2>&1 || { echo 'Install Node.js + npm first (used to install the OpenTUI runtime).' >&2; exit 1; }
case "$(uname -s)" in Linux|Darwin) ;; *) echo 'TrpHot currently supports Unix systems.' >&2; exit 1;; esac
cd "$base"
# Source builds take priority; otherwise use the included Linux x86_64 binary.
if command -v cargo >/dev/null 2>&1; then
    cargo build --release --locked
    engine="$base/target/release/trphot"
elif [ "$(uname -s)-$(uname -m)" = 'Linux-x86_64' ] && [ -x "$base/bin/trphot" ]; then
    engine="$base/bin/trphot"
else
    echo 'Install Rust/Cargo, then rerun ./install.sh.' >&2; exit 1
fi
npm ci --include=dev --no-audit --no-fund
# Confirm that the downloaded Bun runtime is executable before changing installation.
./node_modules/.bin/bun --version
mkdir -p "$prefix/bin" "$prefix/share/trphot/ui"
cp "$engine" "$prefix/bin/trphot"
cp ui/app.ts ui/main.ts "$prefix/share/trphot/ui/"
cp package.json package-lock.json "$prefix/share/trphot/"
# Keep a project-local dependency tree so other tools are unaffected.
rm -rf "$prefix/share/trphot/node_modules"
cp -R node_modules "$prefix/share/trphot/node_modules"
printf '\nInstalled %s/bin/trphot\nEnsure %s/bin is in your PATH.\n' "$prefix" "$prefix"
