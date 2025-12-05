#!/bin/bash
echo "Testing mihomo update functionality..."
./target/release/singbox-manager --version
echo ""
echo "Config check:"
grep -A 10 "mihomo_core_update" config.json

