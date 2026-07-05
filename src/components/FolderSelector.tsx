import { open } from '@tauri-apps/plugin-dialog';

interface Props {
  value: string;
  onChange: (path: string) => void;
  disabled: boolean;
}

export function FolderSelector({ value, onChange, disabled }: Props) {
  const handleBrowse = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (selected) {
      onChange(selected as string);
    }
  };

  return (
    <div className="folder-selector">
      <label className="input-label" htmlFor="vault-path">Vaultフォルダ</label>
      <div className="input-row">
        <input
          id="vault-path"
          type="text"
          className="input"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder="C:\Users\...\Obsidian Vault"
          disabled={disabled}
        />
        <button className="btn btn-secondary browse-btn" onClick={handleBrowse} disabled={disabled}>
          参照...
        </button>
      </div>
    </div>
  );
}
