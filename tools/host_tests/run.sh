#!/usr/bin/env bash
# Runs the pure-logic tests on the PC (no board needed): weather/sky logic, saved Wi-Fi record,
# captive-portal DNS replies, setup form parsing, HTML escaping, the BOOT hold,
# bin nights, the location lookup, the Home Assistant calendar and media player, and slides.
# The setup test also writes setup_page.html / saved_page.html to the temp dir for a look.
set -e
cd "$(dirname "$0")"
out=$(mktemp -d)
for t in weather setup input bins location join calendar media slides; do
    rustup run stable rustc --edition 2024 --test "$t.rs" -o "$out/$t" 2>/dev/null \
        || rustup run stable rustc --edition 2024 --test "$t.rs" -o "$out/$t"
    "$out/$t" -q
done
