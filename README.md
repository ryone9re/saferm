# saferm

A safe `rm` replacement — moves files to trash instead of permanent deletion.

`rm` の安全な代替ツール — ファイルを永久削除せずゴミ箱に移動します。

## Features / 特徴

- **Drop-in replacement for `rm`** — supports `-r`, `-f`, `-i`, `-d`, `-v`, `--` flags
- **OS-native trash** — uses macOS Trash / FreeDesktop Trash on Linux desktop environments
- **Managed trash fallback** — self-managed `~/.local/share/saferm/trash/` for headless Linux servers
- **Confirmation prompt** — always asks before deletion (y/N) in interactive terminals; `-f` skips only in non-TTY (scripts/CI)
- **Trash cleanup** — `--cleanup` to empty the trash
- **Bilingual** — English & Japanese (auto-detected from system locale)

---

- **`rm` のドロップイン代替** — `-r`, `-f`, `-i`, `-d`, `-v`, `--` フラグに対応
- **OS標準ゴミ箱** — macOS Trash / Linux デスクトップ環境の FreeDesktop Trash を使用
- **管理ゴミ箱フォールバック** — ヘッドレスLinuxサーバー向けに `~/.local/share/saferm/trash/` を自動管理
- **確認プロンプト** — 対話端末では削除前に必ず確認 (y/N)、`-f` は非TTY環境（スクリプト/CI）でのみスキップ
- **ゴミ箱クリーンアップ** — `--cleanup` でゴミ箱を空に
- **バイリンガル** — 英語・日本語（システムロケールから自動検出）

## Installation / インストール

```bash
cargo install --path .
```

## Usage / 使い方

```bash
# Move a file to trash / ファイルをゴミ箱に移動
saferm file.txt

# Move multiple files / 複数ファイルをゴミ箱に移動
saferm file1.txt file2.txt file3.txt

# Force (skip confirmation in non-TTY) / 非TTYで確認をスキップ
saferm -f file.txt

# Remove a directory recursively / ディレクトリを再帰的に削除
saferm -rf my_directory/

# Verbose output / 詳細表示
saferm -fv file.txt

# Empty the trash / ゴミ箱を空にする
saferm --cleanup
```

## Options / オプション

| Flag | Description | 説明 |
|------|-------------|------|
| `-r`, `-R`, `--recursive` | Remove directories and contents | ディレクトリとその中身を再帰的に削除 |
| `-f`, `--force` | Skip confirmation in non-TTY, ignore nonexistent files | 非TTYで確認をスキップ、存在しないファイルを無視 |
| `-i`, `--interactive` | Prompt before every removal (default) | 毎回確認する（デフォルト動作） |
| `-d`, `--dir` | Remove empty directories | 空ディレクトリを削除 |
| `-v`, `--verbose` | Explain what is being done | 実行内容を表示 |
| `--cleanup` | Empty the trash | ゴミ箱を空にする |

## Trash Backend / ゴミ箱バックエンド

| Platform | Backend | Notes |
|----------|---------|-------|
| macOS | OS Trash | Always available. Cleanup via Finder. |
| Linux (desktop) | FreeDesktop Trash | Detected via `$XDG_CURRENT_DESKTOP` / `$DESKTOP_SESSION`. |
| Linux (headless) | Managed Trash | `~/.local/share/saferm/trash/` with `.trashinfo` metadata. |

| プラットフォーム | バックエンド | 備考 |
|----------|---------|-------|
| macOS | OS ゴミ箱 | 常に利用可能。クリーンアップは Finder から。 |
| Linux (デスクトップ) | FreeDesktop Trash | `$XDG_CURRENT_DESKTOP` / `$DESKTOP_SESSION` で検出。 |
| Linux (ヘッドレス) | 管理ゴミ箱 | `~/.local/share/saferm/trash/` に `.trashinfo` メタデータ付きで保存。 |

## CI / CD

Pull requests and pushes to `main` are checked automatically on both Ubuntu and macOS:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`

Releases are started manually from GitHub Actions via `Actions > Release > Run workflow`. The workflow always prepares the release from `main`, updates `Cargo.toml` and `Cargo.lock`, validates the release candidate, and builds the supported release artifacts before publish.

Leave the version input empty to auto-bump the current patch version. Provide a version such as `1.2.0` to override the automatic bump. After approval from the GitHub `release` environment, the workflow publishes the prepared release by updating `main` when needed, ensuring the matching version tag (`v<release_version>`) exists, and creating the GitHub Release:

| Target | Binary type |
|--------|-------------|
| `x86_64-unknown-linux-musl` | Static (musl, no glibc dependency) |
| `aarch64-apple-darwin` | Native (Apple Silicon) |

---

`main` への PR / push 時に Ubuntu と macOS の両方で自動チェックされます:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`

リリースは GitHub Actions の `Actions > Release > Run workflow` から手動で開始します。ワークフローは常に `main` からリリース候補を作成し、`Cargo.toml` と `Cargo.lock` を更新し、publish 前にリリース候補を検証して対応する release artifact をビルドします。

version 入力を空のままにすると現在の patch バージョンを自動でインクリメントします。`1.2.0` のようなバージョンを指定すると自動 bump を上書きします。GitHub の `release` environment で承認されると、ワークフローは必要に応じて `main` を更新し、対応する version tag (`v<release_version>`) の存在を保証して、GitHub Release を公開します:

| ターゲット | バイナリ種別 |
|--------|-------------|
| `x86_64-unknown-linux-musl` | 静的リンク (musl、glibc 依存なし) |
| `aarch64-apple-darwin` | ネイティブ (Apple Silicon) |

## Development / 開発

```bash
cargo build              # Debug build / デバッグビルド
cargo build --release    # Release build / リリースビルド
cargo test               # Run all tests / 全テスト実行
cargo clippy             # Lint / リント
cargo fmt                # Format / フォーマット
```

## License / ライセンス

MIT
