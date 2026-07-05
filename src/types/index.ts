export interface FileMatch {
  relativePath: string;
  yamlTagCount: number;
  inlineTagCount: number;
}

export interface FileDiff {
  relativePath: string;
  diffs: SingleDiff[];
}

export interface SingleDiff {
  lineNumber: number;
  tagType: 'yaml' | 'inline';
  oldLine: string;
  newLine: string;
}

export interface ReplaceResult {
  filesModified: number;
  totalYamlReplacements: number;
  totalInlineReplacements: number;
  backupPath: string;
  errors: string[];
}
