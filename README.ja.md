# CodePrism

<p align="center">
  <strong>🔬 Git リポジトリ向け高性能コード分析ツール</strong>
</p>

<p align="center">
  <a href="#-クイックスタート">クイックスタート</a> •
  <a href="#-インストール">インストール</a> •
  <a href="#-cli-リファレンス">CLI リファレンス</a> •
  <a href="#-設定">設定</a>
</p>

<p align="center">
  <a href="./README.md">English</a> |
  <a href="./README.zh-CN.md">简体中文</a> |
  <a href="./README.ja.md">日本語</a>
</p>

<p align="center">
  <a href="https://github.com/yougikou/code-prism/releases"><img src="https://img.shields.io/github/v/release/yougikou/code-prism?style=flat-square" alt="Release"></a>
  <a href="https://github.com/yougikou/code-prism/actions"><img src="https://img.shields.io/github/actions/workflow/status/yougikou/code-prism/release.yml?style=flat-square" alt="Build"></a>
  <a href="./LICENSE"><img src="https://img.shields.io/github/license/yougikou/code-prism?style=flat-square" alt="License"></a>
</p>

---

CodePrism は Rust で構築された**高性能コード分析ツール**です。Git リポジトリをスキャンし、メトリクスを抽出し、直感的な Web ダッシュボードで実用的なインサイトを提供します。

## ✨ 機能

- 🚀 **高性能** - Rust で構築された最高速度
- 📊 **豊富な分析** - 複数の集計タイプとチャート可視化
- 🔄 **Git 統合** - スナップショットと差分スキャンモード
- 🎨 **サーバー駆動 UI** - YAML で設定可能なダッシュボード
- 📦 **マルチプロジェクト対応** - 1つの設定で複数プロジェクトを管理
- 🔌 **拡張可能なアナライザー** - 組み込み、正規表現、Python、WASM アナライザー

## 📥 インストール

### ビルド済みバイナリをダウンロード（推奨）

[GitHub Releases](https://github.com/yougikou/code-prism/releases) からプラットフォームに合った最新版をダウンロード：

| プラットフォーム | ダウンロードファイル |
|------------------|----------------------|
| **Linux x86_64** | `codeprism-x86_64-unknown-linux-gnu.tar.gz` |
| **Linux ARM64** | `codeprism-aarch64-unknown-linux-gnu.tar.gz` |
| **macOS Intel** | `codeprism-x86_64-apple-darwin.tar.gz` |
| **macOS Apple Silicon** | `codeprism-aarch64-apple-darwin.tar.gz` |
| **Windows x86_64** | `codeprism-x86_64-pc-windows-msvc.zip` |

```bash
# Linux / macOS
tar xzf codeprism-*.tar.gz
chmod +x codeprism
sudo mv codeprism /usr/local/bin/

# インストール確認
codeprism --version
```

### ソースからビルド

```bash
git clone https://github.com/yougikou/code-prism.git
cd code-prism
cargo build --release
# バイナリは target/release/codeprism にあります
```

### Docker

```bash
docker pull ghcr.io/yougikou/code-prism:latest
docker run -v $(pwd):/workspace ghcr.io/yougikou/code-prism scan .
```

## 🚀 クイックスタート

```bash
# 1. データベースを初期化
codeprism init

# 2. リポジトリをスキャン
codeprism scan /path/to/your/repo

# 3. Web ダッシュボードを起動
codeprism serve
```

ブラウザで **http://localhost:3000** を開きます。

## 📖 CLI リファレンス

### グローバルオプション

```
codeprism [オプション] <コマンド>

オプション:
  --config <パス>    設定ファイルのパス（デフォルト: codeprism.yaml）
  --help             ヘルプ情報を表示
  --version          バージョン情報を表示
```

### コマンド

#### `init` - データベースを初期化

```bash
codeprism init
```

SQLite データベース（`.codeprism.db`）を作成し、必要なスキーマを適用します。

#### `scan` - リポジトリをスキャン

```bash
codeprism scan <パス> [オプション]

引数:
  <パス>  Git リポジトリのパス（デフォルト: .）

オプション:
  -p, --project <名前>     プロジェクト名（デフォルト: ディレクトリ名）
  --mode <モード>          スキャンモード: snapshot または diff（デフォルト: snapshot）
  --commit <ハッシュ>      スキャンする特定のコミット（スナップショットモード）
  --base <ハッシュ>        比較のベースコミット（差分モード、必須）
  --target <ハッシュ>      比較のターゲットコミット（差分モード、デフォルト: HEAD）
```

**例：**

```bash
# 現在のディレクトリをスナップショットスキャン
codeprism scan .

# 特定のコミットをスキャン
codeprism scan . --commit abc123

# 2つのコミット間の差分スキャン
codeprism scan . --mode diff --base abc123 --target def456

# カスタムプロジェクト名でスキャン
codeprism scan ../my-project --project "MyApp"
```

#### `serve` - Web ダッシュボードを起動

```bash
codeprism serve [オプション]

オプション:
  --port <ポート>    サーバーポート（デフォルト: 3000）
```

**例：**

```bash
# デフォルトポートで起動
codeprism serve

# カスタムポートで起動
codeprism serve --port 8080

# カスタム設定を使用
codeprism serve --config production.yaml
```

#### `init-config` - 設定ファイルを生成

```bash
codeprism init-config [パス]

引数:
  [パス]  出力ファイルパス（デフォルト: codeprism.yaml）
```

#### `check-config` - 設定ファイルを検証

```bash
codeprism check-config
```

### 終了コード

| コード | 説明 |
|--------|------|
| `0` | 成功 |
| `1` | 一般エラー |
| `2` | 設定エラー |
| `3` | データベースエラー |
| `4` | Git エラー |

## ⚙️ 設定

CodePrism は YAML 設定ファイルを使用します。詳細は[設定ガイド](#設定ファイル形式)を参照してください。

```bash
# デフォルト設定を生成
codeprism init-config

# カスタム設定を使用
codeprism --config my-config.yaml scan .
```

### 設定ファイル形式

```yaml
database_url: "sqlite:.codeprism.db"

global_excludes:
  - "**/.git/**"
  - "**/node_modules/**"

tech_stacks:
  - name: "Rust"
    extensions: ["rs", "toml"]
    analyzers: ["char_count"]

aggregation_views:
  top_files:
    title: "Top 10 最大ファイル"
    tech_stacks: ["Rust"]
    func:
      type: "top_n"
      metric_key: "char_count"
      limit: 10
    chart_type: "bar_row"
```

### マルチプロジェクト設定

```yaml
projects:
  - name: "frontend"
    tech_stacks:
      - name: "React"
        extensions: ["tsx", "ts"]
        analyzers: ["char_count"]
    aggregation_views: {}

  - name: "backend"
    tech_stacks:
      - name: "Rust"
        extensions: ["rs"]
        analyzers: ["char_count"]
    aggregation_views: {}
```

## 📊 集計とチャートタイプ

### 集計タイプ

| タイプ | 説明 |
|--------|------|
| `top_n` | 値による上位 N 件 |
| `sum` | 値の合計 |
| `avg` | 平均値 |
| `min` / `max` | 最小/最大値 |
| `distribution` | バケット分布 |

### チャートタイプ

| タイプ | 説明 |
|--------|------|
| `bar_row` | 横棒グラフ |
| `bar_col` | 縦棒グラフ |
| `pie` | 円グラフ |
| `table` | データテーブル |
| `gauge` | ゲージメーター |
| `radar` | レーダーチャート |
| `line` | 折れ線グラフ |
| `heatmap` | ヒートマップ |

## 🏗️ アーキテクチャ

```
┌─────────────────────────────────────────┐
│           Web ダッシュボード            │
│         (React + ECharts)               │
└─────────────────┬───────────────────────┘
                  │ REST API
┌─────────────────▼───────────────────────┐
│            API サーバー                 │
│    (Axum + サーバー駆動設定)            │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│          スキャンエンジン               │
│     (Git 統合 + アナライザー)           │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│         SQLite データベース             │
│      (メトリクス + スキャン履歴)        │
└─────────────────────────────────────────┘
```

## 📚 ドキュメント

- [プロジェクトブループリント](./PROJECT_BLUEPRINT.md)
- [モジュール構造](./STRUCTURE_AND_MODULES.md)
- [API ドキュメント](http://localhost:3000/swagger-ui)（サーバー実行中）

## 🤝 コントリビューション

コントリビューションを歓迎します！上記のドキュメントでガイドラインをご確認ください。

## 📄 ライセンス

MIT License - 詳細は [LICENSE](./LICENSE) を参照
