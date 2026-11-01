# 曲フォルダ設定の保持

- 引数なしの `osu-cli` だけで起動できるように、曲フォルダを `config.toml` に保持する
- パスの優先順位は 1. 引数 2. `config.toml`
- `./songs` のフォールバックは廃止し、どちらも指定がなければ TUI 起動前にエラーで終了する
- 保存場所は `dirs::config_dir()` 配下の `osu-cli/config.toml`
    - Windows: `%APPDATA%\osu-cli\config.toml`
    - Linux: `~/.config/osu-cli/config.toml`
- アプリからは読むだけで、自動保存はしない（開発時の引数で設定を上書きしないため）
- `config.toml` は手で書き、`songs_dir` は絶対パスにする（例: `songs_dir = 'D:\osu!\Songs'`）
- 開発時は `cargo run -- ./songs` で起動する（`.cargo/config.toml` にエイリアス `dev = "run -- ./songs"` を置けば `cargo dev`）
- 追加する依存は `dirs`、`toml`、`serde`（`derive` 機能）
