# Obsidian Tag Replacer — 仕様書

## 概要

Obsidian Vault 内の `.md` ファイルを対象に、YAML frontmatter の `tags:` リストと本文中の `#タグ` を一括置換するデスクトップアプリ。

## アーキテクチャ

```
┌──────────────────────────┐
│  React 19 (TypeScript)   │  ← UI層
│  Vite 8                  │
├──────────────────────────┤
│  invoke() ←→ commands.rs │  ← Tauri IPC
├──────────────────────────┤
│  Rust std::fs (同期I/O)  │  ← ファイル操作
└──────────────────────────┘
```

## 対応するタグ形式

### YAML frontmatter

```yaml
tags:
  - old_tag        # クォートなし
  - "old_tag"      # ダブルクォート
  - 'old_tag'      # シングルクォート
```

- ファイル先頭の `--- ... ---` 内のみ対象
- 行末完全一致（部分一致はしない）
- 置換後もクォートを維持

### 本文インラインタグ

```
#tag または #日本語タグ
```

- `\b`（単語境界）は日本語・記号タグで機能しないため、独自の `is_tag_boundary()` で終端判定
- コードブロック（\`\`\`）内は除外
- URL 内の `/#tag` は除外（直前文字が `/` ならスキップ）

## フェーズ遷移

```
idle → searching → searched → previewing → previewed → replacing → done
                ↘ error       ↘ searched     ↘ previewed
```

## 安全機能

- **バックアップ**: `.tag-replace-backup/YYYY-MM-DD-HHMM/` に元ファイルを保存
- **確認ダイアログ**: バックアップなし置換時に警告
- **プレビュー**: 置換前後の行をファイルごとに表示

## 拡張子・除外パス

- 対象: `.md` ファイルのみ
- 除外: `.obsidian/`, `.trash/`, `.git/`, `node_modules/`, `.tag-replace-backup*`

## Tauri コマンド

| コマンド | 引数 | 戻り値 |
|---|---|---|
| `search_files` | `vaultPath`, `oldTag` | `FileMatch[]` |
| `preview_changes` | `vaultPath`, `oldTag`, `newTag`, `relativePaths` | `FileDiff[]` |
| `execute_replace` | `vaultPath`, `oldTag`, `newTag`, `relativePaths`, `createBackup` | `ReplaceResult` |
| `get_app_version` | — | `string` |
| `load_settings` | — | `AppSettings` |
| `save_settings` | `vaultPath` | — |

## データ構造

```typescript
interface FileMatch {
  relativePath: string;
  yamlTagCount: number;
  inlineTagCount: number;
}

interface FileDiff {
  relativePath: string;
  diffs: SingleDiff[];
}

interface SingleDiff {
  lineNumber: number;
  tagType: "yaml" | "inline";
  oldLine: string;
  newLine: string;
}

interface ReplaceResult {
  filesModified: number;
  totalYamlReplacements: number;
  totalInlineReplacements: number;
  backupPath: string;
  errors: string[];
}

interface AppSettings {
  vaultPath: string;
}
```

## v0.1 非対応

- `tags: [a, b]` インライン配列形式
- 複数タグの一括変換表（CSV）
- ファイルごとのチェックボックス ON/OFF
- Undo（バックアップからの復元）
- 正規表現でのタグ指定

## 技術スタック

- [Tauri v2](https://v2.tauri.app/) + [React 19](https://react.dev/) + [TypeScript](https://www.typescriptlang.org/) + [Vite](https://vite.dev/)
- Rust バックエンド（`regex`, `walkdir`, `chrono`）

## 開発

```bash
# 依存関係のインストール
npm install

# 開発モードで起動
npx tauri dev

# リリースビルド
npx tauri build
```
