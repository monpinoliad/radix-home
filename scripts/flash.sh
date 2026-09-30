#!/usr/bin/env bash

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

# Serves http://localhost:8000 - open it in Chrome/Edge and flash over WebSerial.
web-flash --chip esp32s3 target/xtensa-esp32s3-none-elf/${BUILD_MODE}/radix-home
