#!/bin/bash

VBoxManage startvm "Wayland" --type headless
xfreerdp /v:127.0.0.1 /w:1920 /h:1080 +auto-reconnect &
sleep 10
cargo watch -s ./run.sh