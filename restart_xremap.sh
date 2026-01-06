#!/bin/bash

echo "Stopping xremap..."
pkill xremap 2>/dev/null || true
sleep 0.5
pkill xremap 2>/dev/null || true
sleep 0.5
pkill xremap 2>/dev/null || true
sleep 0.5

echo "Stopping espanso..."
pkill espanso 2>/dev/null || true
sleep 0.5
pkill espanso 2>/dev/null || true
sleep 0.5
pkill espanso 2>/dev/null || true
sleep 1

echo "Building xremap..."
cd /home/michael/ahk-wayland || exit 1
cargo build --release

if [ $? -eq 0 ]; then
    echo "Build successful! Launching xremap..."
    sleep 1
    sudo ./target/release/xremap /home/michael/.config/xremap/test_hotstrings.ahk
else
    echo "Build failed!"
    exit 1
fi
