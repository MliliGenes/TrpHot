#!/bin/sh
set -eu
base=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
prefix=${PREFIX:-"$HOME/.local"}
case "$(uname -s)" in Linux|Darwin) ;; *) echo 'TrpHot currently supports Unix systems.' >&2; exit 1;; esac
cd "$base"
# The dashboard assets are embedded in the Rust binary.
if command -v cargo >/dev/null 2>&1; then
    cargo build --release --locked
    engine="$base/target/release/trphot"
else
    echo 'Install Rust/Cargo, then rerun ./install.sh.' >&2; exit 1
fi
mkdir -p "$prefix/bin"
cp "$engine" "$prefix/bin/trphot"
printf '\nInstalled %s/bin/trphot\nEnsure %s/bin is in your PATH.\n' "$prefix" "$prefix"
