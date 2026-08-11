#!/usr/bin/env python3
"""Compatibility adapter for the Rust-owned PROMAX compiler."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from hub.project_paths import ROOT


class AdBlockManager:
    """Keep the legacy Python update entrypoint without duplicating PROMAX logic."""

    def __init__(self, root: str | Path = ROOT) -> None:
        self.root = Path(root).resolve()

    def merge(self, execute: bool = False) -> bool:
        release_binary = self.root / "target" / "release" / "promax"
        if release_binary.is_file():
            command = [str(release_binary), "--root", str(self.root)]
        else:
            command = [
                "cargo",
                "run",
                "--quiet",
                "--manifest-path",
                str(self.root / "create" / "processor" / "Cargo.toml"),
                "--bin",
                "promax",
                "--",
                "--root",
                str(self.root),
            ]
        if execute:
            command.append("--execute")

        completed = subprocess.run(command, cwd=self.root, check=False)
        if completed.returncode != 0:
            raise RuntimeError(f"Rust PROMAX compiler failed with exit code {completed.returncode}")
        return True


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Run the Rust PROMAX compiler")
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--execute", action="store_true")
    options = parser.parse_args()
    AdBlockManager(options.root).merge(execute=options.execute)
