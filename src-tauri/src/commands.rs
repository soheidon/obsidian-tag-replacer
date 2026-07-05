use chrono::Local;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Manager;
use walkdir::WalkDir;

// ── Data structures ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMatch {
    pub relative_path: String,
    pub yaml_tag_count: usize,
    pub inline_tag_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub relative_path: String,
    pub diffs: Vec<SingleDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SingleDiff {
    pub line_number: usize,
    pub tag_type: String, // "yaml" or "inline"
    pub old_line: String,
    pub new_line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceResult {
    pub files_modified: usize,
    pub total_yaml_replacements: usize,
    pub total_inline_replacements: usize,
    pub backup_path: String,
    pub errors: Vec<String>,
}

// ── Helper: tag boundary detection ──

/// Characters that can terminate an inline #tag in body text.
/// Rust `regex` lacks lookahead, so we check the char AFTER the match.
fn is_tag_boundary(ch: char) -> bool {
    matches!(
        ch,
        '\0' // sentinel for end-of-string
            | ' '
            | '\t'
            | '\n'
            | '\u{3000}' // full-width space
            | '.'
            | ','
            | '。'
            | '、'
            | ';'
            | '；'
            | ':'
            | '：'
            | '!'
            | '！'
            | '?'
            | '？'
            | ')'
            | '）'
            | ']'
            | '】'
            | '」'
            | '』'
    )
}

/// Check if the character just *before* a `#tag` match is `/` (URL anchor exclusion).
fn preceded_by_slash(text: &str, match_start: usize) -> bool {
    if match_start == 0 {
        return false;
    }
    text.as_bytes()[match_start - 1] == b'/'
}

// ── Helper: frontmatter extraction ──

/// Returns (frontmatter_start_offset, frontmatter_end_offset) in the original content,
/// or None if the file doesn't start with `---`.
fn extract_frontmatter(content: &str) -> Option<(usize, usize)> {
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
        return None;
    }
    // Find closing `---` on its own line
    let after_first = &content[3..]; // skip the opening "---"
    if let Some(rest) = after_first.strip_prefix('\n') {
        // Empty frontmatter: "---\n---\n..."
        if rest.starts_with("---\n") || rest.starts_with("---\r\n") {
            return Some((0, 3 + 1 + 3 + 1)); // "---" + \n + "---" + \n
        }
        if rest == "---" || rest == "---\n" || rest == "---\r\n" {
            return Some((0, content.len()));
        }
        if let Some(idx) = rest.find("\n---\n") {
            return Some((0, 3 + 1 + idx + 4)); // end = opening "---\n" + content + "\n---\n"
        }
        if let Some(idx) = rest.find("\n---\r\n") {
            return Some((0, 3 + 1 + idx + 5));
        }
    }
    None
}

// ── Helper: code block masking ──

/// Returns a Vec of (start, end) byte ranges of fenced code blocks in `content`.
fn find_code_blocks(content: &str) -> Vec<(usize, usize)> {
    let re = Regex::new(r"(?m)^```").unwrap();
    let mut ranges = Vec::new();
    let mut open: Option<usize> = None;
    for m in re.find_iter(content) {
        match open {
            None => open = Some(m.start()),
            Some(start) => {
                ranges.push((start, m.end()));
                open = None;
            }
        }
    }
    ranges
}

/// Returns true if `pos` falls inside any code block range.
fn inside_code_block(pos: usize, code_blocks: &[(usize, usize)]) -> bool {
    code_blocks.iter().any(|(start, end)| pos >= *start && pos < *end)
}

// ── YAML tag counting and replacing ──

/// Count YAML frontmatter `tags:` list items matching `old_tag`.
fn count_yaml_tags(frontmatter: &str, old_tag: &str) -> usize {
    let escaped = regex::escape(old_tag);
    let patterns = [
        format!(r"^(\s*-\s+){}\s*$", escaped),          // unquoted
        format!(r#"^(\s*-\s+)"{}"\s*$"#, escaped),       // double-quoted
        format!(r"^(\s*-\s+)'{}'\s*$", escaped),          // single-quoted
    ];

    frontmatter
        .lines()
        .filter(|line| patterns.iter().any(|p| Regex::new(p).is_ok_and(|r| r.is_match(line))))
        .count()
}

/// Generate diffs for YAML frontmatter tag replacements.
fn diff_yaml_tags(frontmatter: &str, old_tag: &str, new_tag: &str, line_offset: usize) -> Vec<SingleDiff> {
    let escaped = regex::escape(old_tag);
    let patterns: Vec<(Regex, String)> = [
        // unquoted
        (
            Regex::new(&format!(r"^(\s*-\s+){}\s*$", escaped)).unwrap(),
            format!("${{1}}{}", new_tag),
        ),
        // double-quoted
        (
            Regex::new(&format!(r#"^(\s*-\s+)"{}"\s*$"#, escaped)).unwrap(),
            format!("${{1}}\"{}\"", new_tag),
        ),
        // single-quoted
        (
            Regex::new(&format!(r"^(\s*-\s+)'{}'\s*$", escaped)).unwrap(),
            format!("${{1}}'{}'", new_tag),
        ),
    ]
    .into();

    let mut diffs = Vec::new();
    for (line_num, line) in frontmatter.lines().enumerate() {
        for (re, replacement_tmpl) in &patterns {
            if re.is_match(line) {
                let new_line = re.replace(line, replacement_tmpl.as_str()).to_string();
                diffs.push(SingleDiff {
                    line_number: line_offset + line_num + 1, // 1-indexed
                    tag_type: "yaml".to_string(),
                    old_line: line.to_string(),
                    new_line,
                });
                break;
            }
        }
    }
    diffs
}

/// Replace YAML tags in frontmatter text. Returns (new_frontmatter, count).
fn replace_yaml_tags(frontmatter: &str, old_tag: &str, new_tag: &str) -> (String, usize) {
    let escaped = regex::escape(old_tag);
    let patterns: Vec<(Regex, String)> = [
        (
            Regex::new(&format!(r"^(\s*-\s+){}\s*$", escaped)).unwrap(),
            format!("${{1}}{}", new_tag),
        ),
        (
            Regex::new(&format!(r#"^(\s*-\s+)"{}"\s*$"#, escaped)).unwrap(),
            format!("${{1}}\"{}\"", new_tag),
        ),
        (
            Regex::new(&format!(r"^(\s*-\s+)'{}'\s*$", escaped)).unwrap(),
            format!("${{1}}'{}'", new_tag),
        ),
    ]
    .into();

    let mut count = 0;
    let new_fm: String = frontmatter
        .lines()
        .map(|line| {
            for (re, replacement_tmpl) in &patterns {
                if re.is_match(line) {
                    count += 1;
                    return re.replace(line, replacement_tmpl.as_str()).to_string();
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Preserve trailing newline
    if frontmatter.ends_with('\n') {
        (new_fm + "\n", count)
    } else {
        (new_fm, count)
    }
}

// ── Inline tag counting and replacing ──

/// Count inline `#old_tag` occurrences in body text (outside code blocks, not after `/`).
fn count_inline_tags(body: &str, old_tag: &str) -> usize {
    let escaped = regex::escape(old_tag);
    let pattern = format!(r"#{}", escaped);
    let re = Regex::new(&pattern).unwrap();
    let code_blocks = find_code_blocks(body);

    re.find_iter(body)
        .filter(|m| {
            !preceded_by_slash(body, m.start())
                && !inside_code_block(m.start(), &code_blocks)
                && is_tag_boundary(after_match_char(body, m.end()))
        })
        .count()
}

/// Generate diffs for inline tag replacements in body text.
fn diff_inline_tags(body: &str, old_tag: &str, new_tag: &str, line_offset: usize) -> Vec<SingleDiff> {
    let escaped = regex::escape(old_tag);
    let pattern = format!(r"#{}", escaped);
    let re = Regex::new(&pattern).unwrap();
    let code_blocks = find_code_blocks(body);

    let mut diffs = Vec::new();
    // Build line index so we can report line numbers
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(
            body.match_indices('\n')
                .map(|(pos, _)| pos + 1),
        )
        .collect();

    for m in re.find_iter(body) {
        if preceded_by_slash(body, m.start()) || inside_code_block(m.start(), &code_blocks) {
            continue;
        }
        let after = after_match_char(body, m.end());
        if !is_tag_boundary(after) {
            continue;
        }

        let replacement = format!("#{}", new_tag);
        // Find which line this match is on
        let line_num = line_starts
            .iter()
            .enumerate()
            .rev()
            .find(|(_, &start)| start <= m.start())
            .map(|(i, _)| i + 1)
            .unwrap_or(1);

        // Extract the full line for display
        let line_start = line_starts.get(line_num - 1).copied().unwrap_or(0);
        let line_end = body[line_start..]
            .find('\n')
            .map(|p| line_start + p)
            .unwrap_or(body.len());
        let old_line = body[line_start..line_end].to_string();
        let new_line = old_line.replacen(&format!("#{}", old_tag), &replacement, 1);

        diffs.push(SingleDiff {
            line_number: line_offset + line_num,
            tag_type: "inline".to_string(),
            old_line,
            new_line,
        });
    }
    diffs
}

/// Replace inline `#old_tag` with `#new_tag` in body text. Returns (new_body, count).
fn replace_inline_tags(body: &str, old_tag: &str, new_tag: &str) -> (String, usize) {
    let escaped = regex::escape(old_tag);
    let pattern = format!(r"#{}", escaped);
    let re = Regex::new(&pattern).unwrap();
    let code_blocks = find_code_blocks(body);
    let mut count = 0;

    // Since Rust regex doesn't support lookaround and we need to
    // check the character after the match, we collect matches first,
    // then rebuild the string with replacements from right to left.
    let matches: Vec<_> = re
        .find_iter(body)
        .filter(|m| {
            !preceded_by_slash(body, m.start())
                && !inside_code_block(m.start(), &code_blocks)
                && is_tag_boundary(after_match_char(body, m.end()))
        })
        .collect();

    let mut result = body.to_string();
    // Replace from right to left to preserve byte offsets
    for m in matches.iter().rev() {
        let replacement = format!("#{}", new_tag);
        result.replace_range(m.start()..m.end(), &replacement);
        count += 1;
    }

    (result, count)
}

/// Returns the character at `pos` in `text`, or '\0' if `pos` is at/past the end.
fn after_match_char(text: &str, pos: usize) -> char {
    text[pos..].chars().next().unwrap_or('\0')
}

// ── Helper: build relative path ──

fn make_relative(vault_path: &Path, file_path: &Path) -> String {
    file_path
        .strip_prefix(vault_path)
        .unwrap_or(file_path)
        .to_string_lossy()
        .to_string()
}

// ── Helper: skip directories ──

fn should_skip_dir(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir()
        && entry
            .file_name()
            .to_str()
            .is_some_and(|name| {
                name == ".obsidian"
                    || name == ".trash"
                    || name == ".git"
                    || name == "node_modules"
                    || name.starts_with(".tag-replace-backup")
            })
}

// ── Tauri commands ──

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub vault_path: String,
}

#[tauri::command]
pub fn load_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data dir: {}", e))?
        .join("settings.json");

    if !path.exists() {
        return Ok(AppSettings {
            vault_path: String::new(),
        });
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("Cannot read settings: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Cannot parse settings: {}", e))
}

#[tauri::command]
pub fn save_settings(app: tauri::AppHandle, vault_path: String) -> Result<(), String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data dir: {}", e))?;

    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create app data dir: {}", e))?;

    let path = dir.join("settings.json");
    let settings = AppSettings { vault_path };
    let content =
        serde_json::to_string_pretty(&settings).map_err(|e| format!("Cannot serialize: {}", e))?;
    fs::write(&path, content).map_err(|e| format!("Cannot write settings: {}", e))
}

#[tauri::command]
pub fn search_files(vault_path: String, old_tag: String) -> Result<Vec<FileMatch>, String> {
    let vault = PathBuf::from(&vault_path);
    if !vault.is_dir() {
        return Err(format!("Vault path does not exist: {}", vault_path));
    }

    let mut results = Vec::new();

    for entry in WalkDir::new(&vault)
        .into_iter()
        .filter_entry(|e| !should_skip_dir(e))
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "md") {
            continue;
        }

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let yaml_count = if let Some((start, end)) = extract_frontmatter(&content) {
            let fm = &content[start..end];
            // remove the opening/closing "---" lines for tag matching
            let fm_inner = fm
                .strip_prefix("---\n")
                .or_else(|| fm.strip_prefix("---\r\n"))
                .and_then(|s| s.strip_suffix("\n---\n").or_else(|| s.strip_suffix("\n---")))
                .or_else(|| {
                    if fm == "---\n" || fm == "---\r\n" || fm == "---" {
                        Some("")
                    } else {
                        None
                    }
                })
                .unwrap_or(fm);
            count_yaml_tags(fm_inner, &old_tag)
        } else {
            0
        };

        // Body = content after frontmatter
        let body = if let Some((_, end)) = extract_frontmatter(&content) {
            &content[end..]
        } else {
            &content
        };

        let inline_count = count_inline_tags(body, &old_tag);

        if yaml_count > 0 || inline_count > 0 {
            results.push(FileMatch {
                relative_path: make_relative(&vault, path),
                yaml_tag_count: yaml_count,
                inline_tag_count: inline_count,
            });
        }
    }

    // Sort by relative path for stable output
    results.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(results)
}

#[tauri::command]
pub fn preview_changes(
    vault_path: String,
    old_tag: String,
    new_tag: String,
    relative_paths: Vec<String>,
) -> Result<Vec<FileDiff>, String> {
    let vault = PathBuf::from(&vault_path);
    if !vault.is_dir() {
        return Err(format!("Vault path does not exist: {}", vault_path));
    }

    let mut results = Vec::new();

    for rel_path in &relative_paths {
        let full_path = vault.join(rel_path);
        let content = match fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Cannot read {}: {}", rel_path, e);
                continue;
            }
        };

        let mut file_diffs = Vec::new();

        if let Some((start, end)) = extract_frontmatter(&content) {
            let fm = &content[start..end];
            let fm_inner = fm
                .strip_prefix("---\n")
                .or_else(|| fm.strip_prefix("---\r\n"))
                .and_then(|s| s.strip_suffix("\n---\n").or_else(|| s.strip_suffix("\n---")))
                .or_else(|| {
                    if fm == "---\n" || fm == "---\r\n" || fm == "---" {
                        Some("")
                    } else {
                        None
                    }
                })
                .unwrap_or(fm);

            // line_offset: how many lines before the inner frontmatter
            let fm_start_line = content[..start].lines().count();
            file_diffs.extend(diff_yaml_tags(fm_inner, &old_tag, &new_tag, fm_start_line + 1));
        }

        let body_start = if let Some((_, end)) = extract_frontmatter(&content) {
            &content[end..]
        } else {
            &content
        };
        let body_offset = content[..content.len() - body_start.len()]
            .lines()
            .count();

        file_diffs.extend(diff_inline_tags(body_start, &old_tag, &new_tag, body_offset));

        if !file_diffs.is_empty() {
            results.push(FileDiff {
                relative_path: rel_path.clone(),
                diffs: file_diffs,
            });
        }
    }

    Ok(results)
}

#[tauri::command]
pub fn execute_replace(
    vault_path: String,
    old_tag: String,
    new_tag: String,
    relative_paths: Vec<String>,
    create_backup: bool,
) -> Result<ReplaceResult, String> {
    let vault = PathBuf::from(&vault_path);
    if !vault.is_dir() {
        return Err(format!("Vault path does not exist: {}", vault_path));
    }

    // Create backup directory (only if requested)
    let backup_dir = if create_backup {
        let timestamp = Local::now().format("%Y-%m-%d-%H%M").to_string();
        vault.join(".tag-replace-backup").join(&timestamp)
    } else {
        PathBuf::new()
    };

    let mut files_modified = 0usize;
    let mut total_yaml = 0usize;
    let mut total_inline = 0usize;
    let mut errors = Vec::new();

    for rel_path in &relative_paths {
        let full_path = vault.join(rel_path);

        // Read original
        let original = match fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!("Cannot read {}: {}", rel_path, e));
                continue;
            }
        };

        // Do the replacement
        let mut modified = original.clone();
        let mut yaml_count = 0usize;
        let mut inline_count = 0usize;

        // YAML frontmatter replacement
        if let Some((start, end)) = extract_frontmatter(&original) {
            let fm = &original[start..end];
            let (new_fm, count) = replace_yaml_tags(fm, &old_tag, &new_tag);
            if count > 0 {
                yaml_count = count;
                modified.replace_range(start..end, &new_fm);
            }
        }

        // Inline tag replacement (on the already-YAML-replaced content)
        let body_for_inline = if let Some((_, end)) = extract_frontmatter(&original) {
            let fm = &original[..end];
            let (new_fm, _) = replace_yaml_tags(fm, &old_tag, &new_tag);
            let new_fm_end = new_fm.len();
            &modified[new_fm_end..]
        } else {
            &modified
        };

        let (new_body, count) = replace_inline_tags(body_for_inline, &old_tag, &new_tag);
        if count > 0 {
            inline_count = count;
            let body_start = modified.len() - body_for_inline.len();
            modified.replace_range(body_start.., &new_body);
        }

        if yaml_count == 0 && inline_count == 0 {
            continue; // no changes
        }

        // Create backup BEFORE writing (only if requested)
        if create_backup {
            let backup_file = backup_dir.join(rel_path);
            if let Some(parent) = backup_file.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    errors.push(format!("Cannot create backup dir for {}: {}", rel_path, e));
                    continue;
                }
            }
            if let Err(e) = fs::copy(&full_path, &backup_file) {
                errors.push(format!("Backup failed for {}: {}", rel_path, e));
                continue;
            }
        }

        // Write modified content
        if let Err(e) = fs::write(&full_path, &modified) {
            errors.push(format!("Write failed for {}: {}", rel_path, e));
            continue;
        }

        files_modified += 1;
        total_yaml += yaml_count;
        total_inline += inline_count;
    }

    Ok(ReplaceResult {
        files_modified,
        total_yaml_replacements: total_yaml,
        total_inline_replacements: total_inline,
        backup_path: backup_dir.to_string_lossy().to_string(),
        errors,
    })
}

// ── Unit tests ──

#[cfg(test)]
mod tests {
    use super::*;

    // ── frontmatter extraction ──

    #[test]
    fn test_extract_frontmatter_present() {
        let content = "---\ntags:\n  - foo\n---\n\n# Body text\n";
        let (start, end) = extract_frontmatter(content).unwrap();
        assert_eq!(start, 0);
        let fm = &content[start..end];
        assert!(fm.starts_with("---\n"));
        assert!(fm.ends_with("\n---\n") || fm.ends_with("\n---"));
    }

    #[test]
    fn test_extract_frontmatter_empty() {
        let content = "---\n---\n\nBody\n";
        let result = extract_frontmatter(content);
        assert!(result.is_some());
    }

    #[test]
    fn test_extract_frontmatter_absent() {
        let content = "# Just a heading\n\nBody text\n";
        assert!(extract_frontmatter(content).is_none());
    }

    // ── YAML tag counting ──

    #[test]
    fn test_count_yaml_exact_match() {
        let fm = "tags:\n  - foo\n  - bar\n  - foo-extra\n";
        assert_eq!(count_yaml_tags(fm, "foo"), 1);
    }

    #[test]
    fn test_count_yaml_quoted() {
        let fm = "tags:\n  - \"foo\"\n  - 'foo'\n  - foo\n";
        assert_eq!(count_yaml_tags(fm, "foo"), 3);
    }

    #[test]
    fn test_count_yaml_japanese_tag() {
        let fm = "tags:\n  - ▲〇原稿・ブログ\n  - メモ\n";
        assert_eq!(count_yaml_tags(fm, "▲〇原稿・ブログ"), 1);
    }

    // ── YAML tag replacement ──

    #[test]
    fn test_replace_yaml_preserves_quotes() {
        let fm = "tags:\n  - \"foo\"\n  - 'foo'\n";
        let (new_fm, count) = replace_yaml_tags(fm, "foo", "bar");
        assert_eq!(count, 2);
        assert!(new_fm.contains("\"bar\""));
        assert!(new_fm.contains("'bar'"));
    }

    #[test]
    fn test_replace_yaml_not_partial() {
        let fm = "tags:\n  - foo\n  - foo-extra\n";
        let (new_fm, count) = replace_yaml_tags(fm, "foo", "bar");
        assert_eq!(count, 1);
        assert!(new_fm.contains("  - bar\n"));
        assert!(new_fm.contains("  - foo-extra\n"));
    }

    // ── Inline tag detection ──

    #[test]
    fn test_inline_basic() {
        let body = "Here is a #test-tag in text.\n";
        assert_eq!(count_inline_tags(body, "test-tag"), 1);
    }

    #[test]
    fn test_inline_japanese_tag() {
        let body = "これは #▲〇原稿・ブログ です。\n";
        assert_eq!(count_inline_tags(body, "▲〇原稿・ブログ"), 1);
    }

    #[test]
    fn test_inline_japanese_period_end() {
        let body = "#▲〇原稿・ブログ。\n";
        assert_eq!(count_inline_tags(body, "▲〇原稿・ブログ"), 1);
    }

    #[test]
    fn test_inline_fullwidth_space_end() {
        let body = "#▲〇原稿・ブログ　続き\n";
        assert_eq!(count_inline_tags(body, "▲〇原稿・ブログ"), 1);
    }

    #[test]
    fn test_inline_url_excluded() {
        let body = "https://example.com/#test-tag\n";
        assert_eq!(count_inline_tags(body, "test-tag"), 0);
    }

    #[test]
    fn test_inline_url_excluded_japanese() {
        let body = "https://example.com/#▲〇原稿・ブログ\n";
        assert_eq!(count_inline_tags(body, "▲〇原稿・ブログ"), 0);
    }

    #[test]
    fn test_inline_code_block_excluded() {
        let body = "```\n#test-tag\n```\nOutside #test-tag.\n";
        assert_eq!(count_inline_tags(body, "test-tag"), 1); // only outside
    }

    #[test]
    fn test_inline_multiple_occurrences() {
        let body = "#test-tag and #test-tag again.\n";
        assert_eq!(count_inline_tags(body, "test-tag"), 2);
    }

    #[test]
    fn test_inline_tag_at_end_of_string() {
        let body = "Tag: #test-tag";
        assert_eq!(count_inline_tags(body, "test-tag"), 1);
    }

    #[test]
    fn test_inline_tag_with_close_paren() {
        let body = "(#test-tag)";
        assert_eq!(count_inline_tags(body, "test-tag"), 1);
    }

    // ── Inline tag replacement ──

    #[test]
    fn test_replace_inline_basic() {
        let body = "See #foo and #foo.\n";
        let (new_body, count) = replace_inline_tags(body, "foo", "bar");
        assert_eq!(count, 2);
        assert!(!new_body.contains("#foo"));
        assert!(new_body.contains("#bar"));
    }

    #[test]
    fn test_replace_inline_skips_url() {
        let body = "https://x.com/#foo still #foo.\n";
        let (new_body, count) = replace_inline_tags(body, "foo", "bar");
        assert_eq!(count, 1);
        assert!(new_body.contains("https://x.com/#foo")); // unchanged
    }

    // ── Code block detection ──

    #[test]
    fn test_find_code_blocks_single() {
        let content = "```\ncode here\n```\n";
        let blocks = find_code_blocks(content);
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn test_inside_code_block() {
        let blocks = vec![(4, 19)];
        assert!(inside_code_block(10, &blocks));
        assert!(!inside_code_block(0, &blocks));
        assert!(!inside_code_block(20, &blocks));
    }

    // ── is_tag_boundary ──

    #[test]
    fn test_boundary_chars() {
        assert!(is_tag_boundary(' '));
        assert!(is_tag_boundary('\0'));
        assert!(is_tag_boundary('。'));
        assert!(is_tag_boundary('、'));
        assert!(is_tag_boundary('）'));
        assert!(is_tag_boundary('\n'));
        assert!(!is_tag_boundary('a'));
        assert!(!is_tag_boundary('▲'));
    }

    // ── Integration test with fixture files ──

    #[test]
    fn test_integration_with_fixture() {
        let vault_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../test-fixture");

        // Search
        let matches = search_files(vault_path.to_string(), "▲〇原稿・ブログ".to_string()).unwrap();
        assert_eq!(matches.len(), 2, "should match 2 files (note1, note2); note3 has -extra suffix (no exact match), note4 has no tag");

        let note1 = matches.iter().find(|m| m.relative_path == "note1.md").unwrap();
        assert_eq!(note1.yaml_tag_count, 1, "note1: 1 YAML tag");
        assert_eq!(note1.inline_tag_count, 1, "note1: 1 inline tag (URL + code block excluded)");

        let note2 = matches.iter().find(|m| m.relative_path.contains("note2.md")).unwrap();
        assert_eq!(note2.yaml_tag_count, 1, "note2: 1 YAML tag (quoted)");
        assert_eq!(note2.inline_tag_count, 2, "note2: 2 inline tags");

        // note3 is not in matches because -extra is not exact match
        assert!(matches.iter().all(|m| m.relative_path != "note3.md"), "note3 should not appear");

        // Preview
        let paths: Vec<String> = matches.iter().map(|m| m.relative_path.clone()).collect();
        let diffs = preview_changes(
            vault_path.to_string(),
            "▲〇原稿・ブログ".to_string(),
            "原稿".to_string(),
            paths.clone(),
        )
        .unwrap();

        let note1_diff = diffs.iter().find(|d| d.relative_path == "note1.md").unwrap();
        assert_eq!(note1_diff.diffs.len(), 2, "note1: 2 diffs (1 YAML + 1 inline)");

        let yaml_diff = note1_diff.diffs.iter().find(|d| d.tag_type == "yaml").unwrap();
        assert!(yaml_diff.old_line.contains("▲〇原稿・ブログ"));
        assert!(yaml_diff.new_line.contains("原稿"));
        assert!(!yaml_diff.new_line.contains("▲〇原稿・ブログ"));

        // Execute (in a temp copy to not modify fixtures)
        use std::fs;
        let tmp_dir = std::env::temp_dir().join("test-fixture-copy");
        let _ = fs::remove_dir_all(&tmp_dir);
        // Copy fixture to temp
        fn copy_dir(src: &Path, dst: &Path) {
            fs::create_dir_all(dst).unwrap();
            for entry in WalkDir::new(src).min_depth(1) {
                let entry = entry.unwrap();
                let rel = entry.path().strip_prefix(src).unwrap();
                let target = dst.join(rel);
                if entry.file_type().is_dir() {
                    fs::create_dir_all(&target).unwrap();
                } else {
                    fs::copy(entry.path(), &target).unwrap();
                }
            }
        }
        copy_dir(Path::new(vault_path), &tmp_dir);

        let result = execute_replace(
            tmp_dir.to_string_lossy().to_string(),
            "▲〇原稿・ブログ".to_string(),
            "原稿".to_string(),
            paths.clone(),
            true,
        )
        .unwrap();

        assert_eq!(result.files_modified, 2, "note1 and note2 modified; note3 has no exact match; note4 has no match");
        assert_eq!(result.total_yaml_replacements, 2);
        assert_eq!(result.total_inline_replacements, 3);
        assert!(result.errors.is_empty());

        // Verify modifications
        let modified_note1 = fs::read_to_string(tmp_dir.join("note1.md")).unwrap();
        assert!(modified_note1.contains("  - 原稿"), "YAML tag should be replaced");
        // Body inline tag replaced
        assert!(modified_note1.contains("これはテストです。#原稿 のタグを"), "inline tag in body should be replaced");

        // URL should NOT be modified
        assert!(modified_note1.contains("https://example.com/#▲〇原稿・ブログ"), "URL should be unchanged");

        // Code block should NOT be modified
        assert!(modified_note1.contains("#▲〇原稿・ブログ はコードブロック内"), "codeblock should be unchanged");

        // note3 should NOT be modified (partial match prevention)
        let note3_content = fs::read_to_string(tmp_dir.join("note3.md")).unwrap();
        assert!(note3_content.contains("▲〇原稿・ブログ-extra"), "partial match should not be replaced");

        // Backup should exist
        let backup_base = tmp_dir.join(".tag-replace-backup");
        assert!(backup_base.is_dir(), "backup directory should exist");

        // Cleanup
        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
