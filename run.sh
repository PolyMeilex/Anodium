#!/bin/bash
ssh arch@192.168.56.101 "pkill -f anodium" || echo ""

set -e
cargo build



scp target/debug/anodium arch@192.168.56.101:/tmp/
scp -r resources arch@192.168.56.101:/tmp/resources
scp *.rhai arch@192.168.56.101:/tmp/
scp *.png arch@192.168.56.101:/tmp/

printf "screen -r\n\ncd /tmp\nchmod +x anodium\n./anodium --x11\n" | ssh -tt arch@192.168.56.101
