#!/usr/bin/env python3
# ruff: noqa: T201, S603
"""
実用例文の検証ループ用スクリプト。

`tools/check_samples.txt` に書かれた例文を 1 行ずつ `furigana lookup` に流し、
hiragana / ruby / tts の 3 mode の結果を表形式で出力する。

誤読を見つけたら:
- 単語不足 → core/jukugo/*.toml に追加
- 文脈次第 → rules/context/*.toml に追加
- 機械学習領域 → tests/corpus/out_of_scope.toml に記録

Usage:
    python3 tools/check_samples.py
    python3 tools/check_samples.py --binary /path/to/furigana
    python3 tools/check_samples.py --data-dir /path/to/data-staging
    python3 tools/check_samples.py --mode hiragana    # 単一 mode のみ
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from pathlib import Path

# Windows 上で stdout を cp932 にされると日本語が壊れるので UTF-8 に強制
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")  # type: ignore[attr-defined]

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SAMPLES = REPO_ROOT / "tools" / "check_samples.txt"
DEFAULT_MODES = ["hiragana", "ruby", "tts"]


def find_binary(override: str | None) -> str:
    if override:
        return override
    # release > debug > PATH
    for candidate in (
        REPO_ROOT / "target" / "release" / "furigana.exe",
        REPO_ROOT / "target" / "release" / "furigana",
        REPO_ROOT / "target" / "debug" / "furigana.exe",
        REPO_ROOT / "target" / "debug" / "furigana",
    ):
        if candidate.is_file():
            return str(candidate)
    found = shutil.which("furigana")
    if found:
        return found
    sys.exit("[FAIL] furigana バイナリが見つかりません (--binary で明示してください)")


def run(binary: str, text: str, mode: str, data_dir: str | None) -> str:
    cmd = [binary]
    if data_dir:
        cmd += ["--data-dir", data_dir]
    cmd += ["lookup", "--mode", mode, text]
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            encoding="utf-8",
            timeout=15,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return "<TIMEOUT>"
    if result.returncode != 0:
        return f"<ERROR: {result.stderr.strip().splitlines()[-1] if result.stderr else 'exit ' + str(result.returncode)}>"
    return result.stdout.rstrip("\n")


def main() -> int:
    parser = argparse.ArgumentParser(description="実用例文の検証ループ")
    parser.add_argument("--samples", type=Path, default=DEFAULT_SAMPLES)
    parser.add_argument("--binary", help="furigana バイナリ path")
    parser.add_argument("--data-dir", help="--data-dir に渡す path")
    parser.add_argument(
        "--mode",
        choices=["hiragana", "ruby", "tts", "all"],
        default="all",
        help="all = hiragana + ruby + tts を併記、それ以外は単一 mode のみ",
    )
    args = parser.parse_args()

    binary = find_binary(args.binary)
    if not args.samples.is_file():
        sys.exit(f"[FAIL] samples file not found: {args.samples}")

    modes = DEFAULT_MODES if args.mode == "all" else [args.mode]

    print(f"[info] binary  : {binary}")
    print(f"[info] samples : {args.samples}")
    if args.data_dir:
        print(f"[info] data-dir: {args.data_dir}")
    print(f"[info] modes   : {' / '.join(modes)}")
    print()

    with args.samples.open(encoding="utf-8") as f:
        lines = [ln.rstrip("\n") for ln in f if ln.strip() and not ln.startswith("#")]

    for i, text in enumerate(lines, 1):
        print(f"{i:>3}. {text}")
        for mode in modes:
            result = run(binary, text, mode, args.data_dir)
            print(f"    [{mode:8s}] {result}")
        print()

    return 0


if __name__ == "__main__":
    sys.exit(main())
