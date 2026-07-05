import { useState, useRef, useEffect } from 'react';

interface Props {
  phase: string;
  canSearch: boolean;
  canExecute: boolean;
  onSearch: () => void;
  onReplace: () => void;
  onReplaceWithBackup: () => void;
}

export function ActionBar({ phase, canSearch, canExecute, onSearch, onReplace, onReplaceWithBackup }: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, []);

  const handleSelect = (withBackup: boolean) => {
    setOpen(false);
    if (withBackup) {
      onReplaceWithBackup();
    } else {
      onReplace();
    }
  };

  return (
    <div className="action-bar">
      <button
        className="btn btn-primary"
        disabled={!canSearch}
        onClick={onSearch}
      >
        {phase === 'searching' ? '検索中...' : '検索'}
      </button>
      <div className="split-btn-group" ref={ref}>
        <button
          className="btn btn-danger split-btn-main"
          disabled={!canExecute}
          onClick={onReplace}
        >
          {phase === 'replacing' ? '置換中...' : '置換'}
        </button>
        <button
          className="btn btn-danger split-btn-toggle"
          disabled={!canExecute}
          onClick={(e) => { e.stopPropagation(); setOpen(!open); }}
        >
          ▼
        </button>
        {open && (
          <div className="split-dropdown">
            <button
              className="split-dropdown-item"
              onClick={() => handleSelect(false)}
            >
              置換
            </button>
            <button
              className="split-dropdown-item"
              onClick={() => handleSelect(true)}
            >
              バックアップして置換
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
