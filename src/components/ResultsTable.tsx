import type { FileMatch } from '../types';

interface Props {
  matches: FileMatch[];
}

export function ResultsTable({ matches }: Props) {
  if (matches.length === 0) return null;

  return (
    <div className="results-section">
      <table className="results-table">
        <thead>
          <tr>
            <th>ファイル</th>
            <th className="col-num">YAMLタグ</th>
            <th className="col-num">本文タグ</th>
          </tr>
        </thead>
        <tbody>
          {matches.map((f) => (
            <tr key={f.relativePath}>
              <td className="col-path">{f.relativePath}</td>
              <td className={`col-num ${f.yamlTagCount === 0 ? 'zero' : ''}`}>
                {f.yamlTagCount}
              </td>
              <td className={`col-num ${f.inlineTagCount === 0 ? 'zero' : ''}`}>
                {f.inlineTagCount}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
