#!/usr/bin/env python3
"""Small, network-free checks for the legacy Python PROMAX entrypoint."""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from pipeline.adblock_manager import AdBlockManager


def test_cargo_fallback_invokes_rust_cli() -> None:
    with tempfile.TemporaryDirectory() as temporary_root:
        root = Path(temporary_root).resolve()
        with patch("pipeline.adblock_manager.subprocess.run") as run:
            run.return_value = subprocess.CompletedProcess([], 0)
            assert AdBlockManager(root).merge(execute=True)

        command = run.call_args.args[0]
        assert command[:3] == ["cargo", "run", "--quiet"]
        assert command[-3:] == ["--root", str(root), "--execute"]
        assert run.call_args.kwargs["cwd"] == root


def test_rust_failure_is_not_swallowed() -> None:
    with tempfile.TemporaryDirectory() as temporary_root:
        with patch("pipeline.adblock_manager.subprocess.run") as run:
            run.return_value = subprocess.CompletedProcess([], 7)
            try:
                AdBlockManager(temporary_root).merge()
            except RuntimeError as error:
                assert "exit code 7" in str(error)
            else:
                raise AssertionError("Rust PROMAX failure did not propagate")


def main() -> int:
    test_cargo_fallback_invokes_rust_cli()
    test_rust_failure_is_not_swallowed()
    print("PROMAX Rust adapter checks passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
