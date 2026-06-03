"""Canonical repository paths (single source of truth for folder layout)."""

from __future__ import annotations

import os

from hub.common import get_project_root

ROOT = get_project_root()

# Surge / Shadowrocket compiled .list rulesets
LIST_DIR = os.path.join(ROOT, "rulesets/list")
SKK_UPSTREAM_DIR = os.path.join(LIST_DIR, "skk_upstream")
ADBLOCK_LIST = os.path.join(LIST_DIR, "AdBlock.list")
HTTPDNS_HIJACK_LIST = os.path.join(LIST_DIR, "HTTPDNS_Hijack.list")

# PROMAX local module sources (upstream-synced sgmodules)
LOCAL_DIR = os.path.join(ROOT, "modules/source/local")

# Other stable trees
ADBLOCK_DIR = os.path.join(ROOT, "rulesets/AdBlock")
SINGBOX_DIR = os.path.join(ROOT, "rulesets/SingBox")
LOCAL_MODULES_DIR = os.path.join(ROOT, "rulesets/Sources/LocalModules")
MODULES_HELPER_DIR = os.path.join(ROOT, "modules/helper")

# Raw GitHub URL prefix for published list rulesets
LIST_RAW_PREFIX = (
    "https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/list/"
)
