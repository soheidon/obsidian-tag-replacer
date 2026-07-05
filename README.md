# Obsidian Tag Replacer

Obsidian Vault 内のタグを一括置換するデスクトップアプリ。YAML frontmatter の `tags:` リストと本文中の `#タグ` の両方を安全に置換します。

## 特徴

- **検索 → プレビュー → バックアップ → 置換** の安全な4ステップフロー
- YAML frontmatter の `tags:` リスト（クォート付き・なし対応）
- 本文中のインライン `#タグ`（日本語・記号タグ対応）
- コードブロック・URL内の `#` を誤置換しない
- バックアップを作成してからの置換（またはバックアップなしでも実行可能）
- 日本語 UI

## 技術スタック

- [Tauri v2](https://v2.tauri.app/) + [React 19](https://react.dev/) + [TypeScript](https://www.typescriptlang.org/) + [Vite](https://vite.dev/)
- Rust バックエンド（`regex`, `walkdir`, `chrono`）

## インストール

[Releases](https://github.com/soheidon/obsidian-tag-replacer/releases) から最新の `.msi` または `.exe` インストーラーをダウンロードしてください。

## 使い方

1. **Vaultフォルダ** を選択（またはパスを直接入力）
2. **変更前タグ** と **変更後タグ** を入力
3. **検索** をクリック → 該当ファイル一覧を表示
4. **変更内容を確認** で置換前後の差分をプレビュー
5. **置換** または **バックアップして置換** で実行

## 開発

```bash
# 依存関係のインストール
npm install

# 開発モードで起動
npx tauri dev

# リリースビルド
npx tauri build
```

## ライセンス

MIT

## 作者

soheidon
