interface Props {
  oldTag: string;
  newTag: string;
  onOldChange: (tag: string) => void;
  onNewChange: (tag: string) => void;
  disabled: boolean;
}

export function TagInput({ oldTag, newTag, onOldChange, onNewChange, disabled }: Props) {
  return (
    <div className="tag-replace-fields">
      <div className="tag-field">
        <label className="tag-field-label" htmlFor="old-tag">変更前タグ</label>
        <input
          id="old-tag"
          type="text"
          className="input tag-replace-input"
          value={oldTag}
          onChange={(e) => onOldChange(e.target.value)}
          placeholder="▲鶏むね肉"
          disabled={disabled}
        />
      </div>
      <span className="tag-arrow">→</span>
      <div className="tag-field">
        <label className="tag-field-label" htmlFor="new-tag">変更後タグ</label>
        <input
          id="new-tag"
          type="text"
          className="input tag-replace-input"
          value={newTag}
          onChange={(e) => onNewChange(e.target.value)}
          placeholder="料理"
          disabled={disabled}
        />
      </div>
    </div>
  );
}
