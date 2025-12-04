#!/bin/bash
echo "=== 性能基准测试 ==="
echo ""
echo "测试 1: 完整更新时间"
time sudo ./target/release/singbox-manager --once 2>&1 | grep -E "(Updating|complete)" | tail -5
echo ""
echo "测试 2: 二进制大小"
ls -lh target/release/singbox-manager | awk '{print "二进制大小:", $5}'
echo ""
echo "测试 3: 内存占用"
/usr/bin/time -l sudo ./target/release/singbox-manager --once 2>&1 | grep "maximum resident"
