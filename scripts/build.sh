#!/usr/bin/env bash

which idf.py >/dev/null || {
    source ~/export-esp.sh >/dev/null 2>&1
}

# Optional preset Wi-Fi network (git-ignored); see wifi.env.example.
# Without it, the board starts in setup mode: join its "Radix-Setup" hotspot to pick a network.
if [ -f wifi.env ]; then
    set -a
    source wifi.env
    set +a
fi

# Optional Home Assistant calendar (git-ignored); see calendar.env.example.
if [ -f calendar.env ]; then
    set -a
    source calendar.env
    set +a
fi

case "$1" in
"" | "release")
    cargo build --release
    ;;
"debug")
    cargo build
    ;;
*)
    echo "Wrong argument. Only \"debug\"/\"release\" arguments are supported"
    exit 1
    ;;
esac
