import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { FileMatch, FileDiff, ReplaceResult } from '../types';

type Phase =
  | 'idle'
  | 'searching'
  | 'searched'
  | 'previewing'
  | 'previewed'
  | 'replacing'
  | 'done'
  | 'error';

interface TagReplacerState {
  vaultPath: string;
  oldTag: string;
  newTag: string;
  phase: Phase;
  fileMatches: FileMatch[];
  fileDiffs: FileDiff[];
  replaceResult: ReplaceResult | null;
  error: string | null;
}

export function useTagReplacer() {
  const [state, setState] = useState<TagReplacerState>({
    vaultPath: '',
    oldTag: '',
    newTag: '',
    phase: 'idle',
    fileMatches: [],
    fileDiffs: [],
    replaceResult: null,
    error: null,
  });

  const [version, setVersion] = useState('');

  // Load version + persisted settings on mount
  useEffect(() => {
    invoke<string>('get_app_version').then(setVersion).catch(() => {});
    invoke<{ vaultPath: string }>('load_settings').then((s) => {
      if (s.vaultPath) {
        setState((prev) => ({ ...prev, vaultPath: s.vaultPath }));
      }
    }).catch(() => {});
  }, []);

  const setVaultPath = useCallback((path: string) => {
    setState((s) => ({ ...s, vaultPath: path, error: null }));
    // Persist to disk (fire-and-forget)
    invoke('save_settings', { vaultPath: path }).catch(() => {});
  }, []);

  const setOldTag = useCallback((tag: string) => {
    // auto-strip leading #
    const cleaned = tag.startsWith('#') ? tag.slice(1) : tag;
    setState((s) => ({ ...s, oldTag: cleaned, error: null }));
  }, []);

  const setNewTag = useCallback((tag: string) => {
    const cleaned = tag.startsWith('#') ? tag.slice(1) : tag;
    setState((s) => ({ ...s, newTag: cleaned, error: null }));
  }, []);

  const search = useCallback(async () => {
    setState((s) => ({
      ...s,
      phase: 'searching',
      fileMatches: [],
      fileDiffs: [],
      replaceResult: null,
      error: null,
    }));
    try {
      const matches: FileMatch[] = await invoke('search_files', {
        vaultPath: state.vaultPath,
        oldTag: state.oldTag,
      });
      setState((s) => ({
        ...s,
        phase: 'searched',
        fileMatches: matches,
        fileDiffs: [],
        replaceResult: null,
        error: null,
      }));
    } catch (e) {
      setState((s) => ({ ...s, phase: 'error', error: String(e) }));
    }
  }, [state.vaultPath, state.oldTag]);

  const preview = useCallback(async () => {
    const relativePaths = state.fileMatches.map((f) => f.relativePath);
    setState((s) => ({ ...s, phase: 'previewing', error: null }));
    try {
      const diffs: FileDiff[] = await invoke('preview_changes', {
        vaultPath: state.vaultPath,
        oldTag: state.oldTag,
        newTag: state.newTag,
        relativePaths,
      });
      setState((s) => ({ ...s, phase: 'previewed', fileDiffs: diffs }));
    } catch (e) {
      setState((s) => ({ ...s, phase: 'searched', error: String(e) }));
    }
  }, [state.vaultPath, state.oldTag, state.newTag, state.fileMatches]);

  const executeReplace = useCallback(async (createBackup: boolean) => {
    if (createBackup) {
      if (!window.confirm('バックアップを作成して置換します。\n実行しますか？')) return;
    } else {
      if (!window.confirm('バックアップなしで置換します。\n元に戻せません。実行しますか？')) return;
    }

    const relativePaths = state.fileMatches.map((f) => f.relativePath);
    setState((s) => ({ ...s, phase: 'replacing', error: null }));
    try {
      const result: ReplaceResult = await invoke('execute_replace', {
        vaultPath: state.vaultPath,
        oldTag: state.oldTag,
        newTag: state.newTag,
        relativePaths,
        createBackup,
      });
      setState((s) => ({ ...s, phase: 'done', replaceResult: result }));
    } catch (e) {
      setState((s) => ({ ...s, phase: 'previewed', error: String(e) }));
    }
  }, [state.vaultPath, state.oldTag, state.newTag, state.fileMatches]);

  const replace = useCallback(() => executeReplace(false), [executeReplace]);
  const replaceWithBackup = useCallback(() => executeReplace(true), [executeReplace]);

  const reset = useCallback(() => {
    setState({
      vaultPath: '',
      oldTag: '',
      newTag: '',
      phase: 'idle',
      fileMatches: [],
      fileDiffs: [],
      replaceResult: null,
      error: null,
    });
  }, []);

  const busy = (['searching', 'previewing', 'replacing'] as Phase[]).includes(state.phase);

  const canSearch =
    state.vaultPath.trim().length > 0 &&
    state.oldTag.trim().length > 0 &&
    !busy;

  const canPreview =
    state.fileMatches.length > 0 &&
    state.oldTag.trim().length > 0 &&
    state.newTag.trim().length > 0 &&
    state.oldTag.trim() !== state.newTag.trim() &&
    !busy;

  const canExecute = state.phase === 'previewed';

  const totalMatches = state.fileMatches.reduce(
    (sum, f) => sum + f.yamlTagCount + f.inlineTagCount,
    0,
  );

  return {
    state,
    actions: { setVaultPath, setOldTag, setNewTag, search, preview, replace, replaceWithBackup, reset },
    version,
    canSearch,
    canPreview,
    canExecute,
    totalMatches,
  };
}
