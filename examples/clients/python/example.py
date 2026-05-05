#!/usr/bin/env python3
"""
ja-furigana の HTTP API を Python から叩く最小サンプル。

事前準備:
    1. `furigana serve` をローカルで起動
    2. `pip install requests`
    3. このスクリプトを実行: `python example.py`

主用途: TTS パイプライン (VOICEVOX / OpenAI TTS 連携)、NLP / 言語処理。
"""

from __future__ import annotations

import requests

BASE_URL = "http://127.0.0.1:8000"
API_KEY: str | None = None  # config.toml の [auth].tokens を設定した場合のみ


def _headers() -> dict[str, str]:
    headers = {"Content-Type": "application/json"}
    if API_KEY:
        headers["X-API-Key"] = API_KEY
    return headers


def healthz() -> dict:
    """`GET /healthz` で生死確認 + dict_size 取得。"""
    r = requests.get(f"{BASE_URL}/healthz", timeout=5)
    r.raise_for_status()
    return r.json()


def lookup(text: str, mode: str = "tts") -> str:
    """`POST /furigana` で 1 ショット変換。

    Args:
        text: 変換対象の日本語テキスト
        mode: "tts" (default、TTS 用ひらがな + 句読点後 pause)
              | "hiragana" (素のひらがな化)
              | "ruby" ("{灰桜|はいざくら}" 形式)
              | "kanji" (入力そのまま、辞書だけ参照したい時に)

    Returns:
        変換後テキスト。
    """
    r = requests.post(
        f"{BASE_URL}/furigana",
        json={"text": text, "mode": mode},
        headers=_headers(),
        timeout=10,
    )
    r.raise_for_status()
    return r.json()["result"]


def main() -> None:
    print("=== healthz ===")
    print(healthz())

    print("\n=== ruby (1 件) ===")
    print(lookup("灰桜の散る道", mode="ruby"))
    # → {灰桜|はいざくら}の{散る|ちる}{道|みち}

    print("\n=== TTS パイプライン的な使い方 ===")
    text = "今日は5KMを30分で走った。次回は10kmにしたい。"
    spoken = lookup(text, mode="tts")
    print(f"input:  {text}")
    print(f"spoken: {spoken}")
    # ここから VOICEVOX や OpenAI TTS の synthesize に投げる


if __name__ == "__main__":
    main()
