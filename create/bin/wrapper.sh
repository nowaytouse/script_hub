#!/bin/bash
# mihomo-wrapper template for Surge/SubStore posix_spawn fix
# Save as /usr/local/bin/mihomo and rename original to /usr/local/bin/mihomo-bin

REAL_BIN="/usr/local/bin/mihomo-bin"
LOG_FILE="/tmp/mihomo.log"

if [ ! -f "$REAL_BIN" ]; then
    echo "Error: $REAL_BIN not found." >&2
    exit 1
fi

# Redirect stdin from /dev/null to avoid errno 9 (EBADF)
# Redirect stderr to log file for debugging
exec "$REAL_BIN" "$@" </dev/null 2>>"$LOG_FILE"
