# Other-language clients

`furigana serve` は HTTP API なので、HTTP が話せる言語ならどこからでも使えます。

各言語の最小サンプルをここに置いてあります:

| 言語 | ファイル | 主用途 |
|---|---|---|
| Python | [`python/example.py`](./python/example.py) | TTS パイプライン (VOICEVOX / OpenAI TTS 連携)、NLP、data 系 |
| Node.js | [`nodejs/example.mjs`](./nodejs/example.mjs) | Discord bot / web フロントエンド連携 |
| Bash + curl | [`curl/example.sh`](./curl/example.sh) | コピペ動作確認 / shell パイプ |

## 起動

3 言語とも、まずローカルで `furigana serve` を立てておくこと:

```sh
furigana serve              # → http://127.0.0.1:8000
```

`config.toml` で `[auth].tokens` を設定した場合は各サンプル内の API key 部分にコピーしてください (デフォルトは認証無効、ローカル想定)。

## エンドポイント (おさらい)

- `GET  /healthz` — `{"status":"ok","dict_size":44354}`
- `GET  /furigana?text=灰桜の道&mode=ruby`
- `POST /furigana` body `{"text":"灰桜の道","mode":"ruby"}`

`mode` は `tts` (default) | `hiragana` | `ruby` | `kanji` | `romaji` | `romaji-kunrei` の 6 つ。
レスポンスは `{"result":"...","mode":"ruby"}`。

詳細は [プロジェクト README](../../README.md) を参照。

## C++ / C# / その他で使いたい人へ

HTTP クライアントが書ければ呼べます。Python/Node の例を見ればリクエスト形式は分かるので、お好みの HTTP ライブラリ (cpp-httplib, RestSharp, ...) で書いてください。
