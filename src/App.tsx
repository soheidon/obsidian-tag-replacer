import { useTagReplacer } from './hooks/useTagReplacer';
import { FolderSelector } from './components/FolderSelector';
import { TagInput } from './components/TagInput';
import { ActionBar } from './components/ActionBar';
import { ResultsTable } from './components/ResultsTable';
import { DiffPreview } from './components/DiffPreview';
import './App.css';

export default function App() {
  const { state, actions, version, canSearch, canPreview, canExecute, totalMatches } = useTagReplacer();

  const busy = state.phase === 'searching' || state.phase === 'previewing' || state.phase === 'replacing';

  return (
    <div className="app-layout">
      <header className="app-header">
        <h1 className="app-title">Obsidianタグ置換</h1>

        <div className="header-row">
          <FolderSelector
            value={state.vaultPath}
            onChange={actions.setVaultPath}
            disabled={busy}
          />
        </div>

        <div className="tag-replace-section">
          <h2 className="section-heading">タグ置換</h2>
          <TagInput
            oldTag={state.oldTag}
            newTag={state.newTag}
            onOldChange={actions.setOldTag}
            onNewChange={actions.setNewTag}
            disabled={state.phase === 'searching' || state.phase === 'replacing'}
          />
        </div>

        <div className="header-row">
          <ActionBar
            phase={state.phase}
            canSearch={canSearch}
            canExecute={canExecute}
            onSearch={actions.search}
            onReplace={actions.replace}
            onReplaceWithBackup={actions.replaceWithBackup}
          />
        </div>
      </header>

      <main className="app-main">
        {state.error && (
          <div className="error-banner">{state.error}</div>
        )}

        {(state.phase === 'searching' || state.phase === 'previewing' || state.phase === 'replacing') && (
          <div className="status-message">
            {state.phase === 'searching' && 'タグを検索しています...'}
            {state.phase === 'previewing' && 'プレビューを作成しています...'}
            {state.phase === 'replacing' && '置換しています...'}
          </div>
        )}

        {state.fileMatches.length > 0 && (
          <>
            <div className="results-header">
              <div className="results-summary">
                <span className="summary-stat">該当ファイル: {state.fileMatches.length}件</span>
                <span className="summary-stat">該当タグ: {totalMatches}件</span>
              </div>
              {canPreview && (
                <button
                  className="btn btn-secondary"
                  disabled={!canPreview}
                  onClick={actions.preview}
                >
                  {state.phase === 'previewing' ? '確認中...' : '変更内容を確認'}
                </button>
              )}
            </div>
            <ResultsTable matches={state.fileMatches} />
          </>
        )}

        {state.fileDiffs.length > 0 && (
          <DiffPreview diffs={state.fileDiffs} />
        )}

        {state.replaceResult && (
          <div className="completion-card">
            <h2 className="section-title">置換完了</h2>
            <div className="completion-stats">
              <div className="stat">
                <span className="stat-value">{state.replaceResult.filesModified}</span>
                <span className="stat-label">変更ファイル</span>
              </div>
              <div className="stat">
                <span className="stat-value">{state.replaceResult.totalYamlReplacements}</span>
                <span className="stat-label">YAML置換</span>
              </div>
              <div className="stat">
                <span className="stat-value">{state.replaceResult.totalInlineReplacements}</span>
                <span className="stat-label">本文置換</span>
              </div>
            </div>
            {state.replaceResult.backupPath && (
              <div className="backup-info">
                バックアップ: <code>{state.replaceResult.backupPath}</code>
              </div>
            )}
            {state.replaceResult.errors.length > 0 && (
              <div className="error-list">
                <h3>エラー</h3>
                <ul>
                  {state.replaceResult.errors.map((e, i) => (
                    <li key={i}>{e}</li>
                  ))}
                </ul>
              </div>
            )}
            <button className="btn btn-secondary" onClick={actions.reset}>
              新しい検索
            </button>
          </div>
        )}
      </main>

      <footer className="app-footer">
        <span className="status-text">
          {state.phase === 'idle' && '準備完了'}
          {state.phase === 'searching' && '検索中...'}
          {state.phase === 'searched' && `該当ファイル: ${state.fileMatches.length}件 / 該当タグ: ${totalMatches}件`}
          {state.phase === 'previewing' && 'プレビュー作成中...'}
          {state.phase === 'previewed' && '変更内容を確認し、「置換」を押してください'}
          {state.phase === 'replacing' && '置換中...'}
          {state.phase === 'done' && '置換完了'}
          {state.phase === 'error' && 'エラーが発生しました'}
        </span>
        {version && <span className="version-text">v{version}</span>}
      </footer>
    </div>
  );
}
