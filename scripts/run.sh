#!/usr/bin/env bash
# Build (with wifi.env), flash over USB with espflash, then show the serial log.
# Needs the board attached to WSL: `usbipd attach --wsl --busid <id>` in Windows PowerShell.
# Keeps the Wi-Fi network saved on the board (nothing is erased).

set -e

BUILD_MODE=""
case "$1" in
"" | "release")
    bash scripts/build.sh
    BUILD_MODE="release"
    ;;
"debug")
    bash scripts/build.sh debug
    BUILD_MODE="debug"
    ;;
*)
    echo "Wrong argument. Only \"debug\"/\"release\" arguments are supported"
    exit 1
    ;;
esac

espflash flash --monitor --chip esp32s3 target/xtensa-esp32s3-none-elf/${BUILD_MODE}/radix-home
