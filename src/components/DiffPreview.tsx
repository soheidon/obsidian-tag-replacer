import { useState } from 'react';
import type { FileDiff } from '../types';

interface Props {
  diffs: FileDiff[];
}

export function DiffPreview({ diffs }: Props) {
  const [collapsed, setCollapsed] = useState(false);

  if (diffs.length === 0) return null;

  return (
    <div className="diff-section">
      <div className="diff-section-header">
        <h2 className="section-title">変更プレビュー</h2>
        <button
          className="btn btn-secondary btn-sm"
          onClick={() => setCollapsed(!collapsed)}
        >
          {collapsed ? '表示' : '閉じる'}
        </button>
      </div>
      {!collapsed && (
        <>
          {diffs.map((fd) => (
            <details key={fd.relativePath} className="diff-file" open>
              <summary className="diff-file-header">
                <span className="diff-file-path">{fd.relativePath}</span>
                <span className="diff-file-count">{fd.diffs.length} 件の変更</span>
              </summary>
              <div className="diff-table-wrapper">
                <table className="diff-table">
                  <thead>
                    <tr>
                      <th className="col-line">行</th>
                      <th className="col-type">種別</th>
                      <th className="col-old">変更前</th>
                      <th className="col-new">変更後</th>
                    </tr>
                  </thead>
                  <tbody>
                    {fd.diffs.map((d, i) => (
                      <tr key={i} className="diff-row">
                        <td className="col-line">{d.lineNumber}</td>
                        <td className="col-type">
                          <span className={`type-badge ${d.tagType}`}>{d.tagType === 'yaml' ? 'YAML' : '本文'}</span>
                        </td>
                        <td className="col-old">
                          <code>{d.oldLine}</code>
                        </td>
                        <td className="col-new">
                          <code>{d.newLine}</code>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </details>
          ))}
        </>
      )}
    </div>
  );
}
