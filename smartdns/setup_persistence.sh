#!/bin/bash

# --- 1. 配置准备 ---
SMARTDNS_DIR="$HOME/.config/smartdns"
mkdir -p "$SMARTDNS_DIR"
# (不再复制 cn.txt 等临时文件，配置文件将直接挂载 GitHub 仓库路径中的 rules 文件夹)

# --- 2. 持久化 PF 规则脚本 ---
PF_SCRIPT="$SMARTDNS_DIR/pf_setup.sh"
cat <<EOF > "$PF_SCRIPT"
#!/bin/bash
# 打通 53 -> 6053 隧道
sudo pfctl -d 2>/dev/null
echo "rdr pass on lo0 proto { udp, tcp } from any to 127.0.0.1 port 53 -> 127.0.0.1 port 6053" | sudo pfctl -ef -
EOF
chmod +x "$PF_SCRIPT"

# --- 3. 生成 LaunchAgent 守护进程 ---
PLIST_PATH="$HOME/Library/LaunchAgents/com.smartdns.service.plist"
cat <<EOF > "$PLIST_PATH"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.smartdns.service</string>
    <key>ProgramArguments</key>
    <array>
        <string>/opt/homebrew/sbin/smartdns</string>
        <string>-c</string>
        <string>/Users/nyamiiko/Downloads/GitHub/script_hub/smartdns/smartdns.conf</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/smartdns.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/smartdns.stderr.log</string>
</dict>
</plist>
EOF

echo "--- [完成] 持久化配置已就位 ---"
echo "1. 配置文件已备份至: $SMARTDNS_DIR"
echo "2. 开机启动项已创建: $PLIST_PATH"
echo "3. 以后如需手动重启: launchctl load -w $PLIST_PATH"
