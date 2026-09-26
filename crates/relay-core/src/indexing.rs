use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexedFileSnapshot {
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified_unix_ns: u64,
    pub content_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexChange {
    pub change_kind: String,
    pub relative_path: String,
    pub previous_path: Option<String>,
    pub before_sha256: Option<String>,
    pub after_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub files_seen: usize,
    pub files_hashed: usize,
    pub files_unchanged: usize,
    pub symlinks_skipped: usize,
    pub total_bytes: u64,
    pub hint_count: usize,
    pub hint_hits: usize,
    pub elapsed_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexPlan {
    pub files: Vec<IndexedFileSnapshot>,
    pub touched_files: Vec<IndexedFileSnapshot>,
    pub changes: Vec<IndexChange>,
    pub stats: IndexStats,
}

#[derive(Debug, Clone)]
pub struct IndexError {
    pub code: &'static str,
    pub message: String,
}

impl IndexError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for IndexError {}

#[derive(Debug, Clone)]
struct FileMeta {
    relative_path: String,
    absolute_path: PathBuf,
    size_bytes: u64,
    modified_unix_ns: u64,
}

pub fn canonical_project_root(input: &str) -> Result<PathBuf, IndexError> {
    if input.trim().is_empty() {
        return Err(IndexError::new(
            "PROJECT_PATH_INVALID",
            "project root path is empty",
        ));
    }
    let raw = file_uri_to_path(input)?;
    let canonical = fs::canonicalize(&raw).map_err(|error| {
        IndexError::new(
            "PROJECT_PATH_INVALID",
            format!("canonicalize project root: {error}"),
        )
    })?;
    let metadata = fs::metadata(&canonical).map_err(|error| {
        IndexError::new(
            "PROJECT_PATH_INVALID",
            format!("read project root metadata: {error}"),
        )
    })?;
    if !metadata.is_dir() {
        return Err(IndexError::new(
            "PROJECT_PATH_NOT_DIRECTORY",
            "project root is not a directory",
        ));
    }
    Ok(canonical)
}
fn file_uri_to_path(input: &str) -> Result<PathBuf, IndexError> {
    if let Some(rest) = input.strip_prefix("file:///") {
        if rest.contains('%') {
            return Err(IndexError::new(
                "PROJECT_PATH_INVALID",
                "percent-encoded file URIs are not supported by the Phase 3 local importer",
            ));
        }
        #[cfg(windows)]
        {
            return Ok(PathBuf::from(rest.replace('/', "\\")));
        }
        #[cfg(not(windows))]
        {
            return Ok(PathBuf::from(format!("/{rest}")));
        }
    }
    if input.starts_with("file://") {
        return Err(IndexError::new(
            "PROJECT_PATH_INVALID",
            "UNC/authority file URIs are not supported by this slice",
        ));
    }
    Ok(PathBuf::from(input))
}

pub fn build_baseline(root: &Path) -> Result<IndexPlan, IndexError> {
    let started = Instant::now();
    let (metadata, symlinks_skipped) = collect_metadata(root)?;
    let mut files = Vec::with_capacity(metadata.len());
    let mut total_bytes = 0u64;
    for item in metadata {
        let content_sha256 = hash_file(&item.absolute_path)?;
        total_bytes = total_bytes.saturating_add(item.size_bytes);
        files.push(IndexedFileSnapshot {
            relative_path: item.relative_path,
            size_bytes: item.size_bytes,
            modified_unix_ns: item.modified_unix_ns,
            content_sha256,
        });
    }
    let file_count = files.len();
    Ok(IndexPlan {
        touched_files: files.clone(),
        files,
        changes: Vec::new(),
        stats: IndexStats {
            files_seen: file_count,
            files_hashed: file_count,
            files_unchanged: 0,
            symlinks_skipped,
            total_bytes,
            hint_count: 0,
            hint_hits: 0,
            elapsed_ms: elapsed_ms(started),
        },
    })
}
pub fn reconcile(
    root: &Path,
    previous: &[IndexedFileSnapshot],
    hints: &[String],
) -> Result<IndexPlan, IndexError> {
    let started = Instant::now();
    let normalized_hints = validate_hints(hints)?;
    let previous_map: BTreeMap<String, IndexedFileSnapshot> = previous
        .iter()
        .cloned()
        .map(|file| (file.relative_path.clone(), file))
        .collect();
    let (metadata, symlinks_skipped) = collect_metadata(root)?;
    let current_paths: BTreeSet<String> = metadata
        .iter()
        .map(|item| item.relative_path.clone())
        .collect();

    let mut files = Vec::with_capacity(metadata.len());
    let mut touched_files = Vec::new();
    let mut changes = Vec::new();
    let mut added_paths = Vec::new();
    let mut deleted = Vec::new();
    let mut files_hashed = 0usize;
    let mut files_unchanged = 0usize;
    let mut total_bytes = 0u64;

    for item in metadata {
        total_bytes = total_bytes.saturating_add(item.size_bytes);
        match previous_map.get(&item.relative_path) {
            Some(old)
                if old.size_bytes == item.size_bytes
                    && old.modified_unix_ns == item.modified_unix_ns
                    && !normalized_hints.contains(&item.relative_path) =>
            {
                files_unchanged += 1;
                files.push(old.clone());
            }
            Some(old) => {
                let content_sha256 = hash_file(&item.absolute_path)?;
                files_hashed += 1;
                let snapshot = IndexedFileSnapshot {
                    relative_path: item.relative_path.clone(),
                    size_bytes: item.size_bytes,
                    modified_unix_ns: item.modified_unix_ns,
                    content_sha256: content_sha256.clone(),
                };
                touched_files.push(snapshot.clone());
                if old.content_sha256 == content_sha256 {
                    files_unchanged += 1;
                } else {
                    changes.push(IndexChange {
                        change_kind: "modified".to_string(),
                        relative_path: item.relative_path,
                        previous_path: None,
                        before_sha256: Some(old.content_sha256.clone()),
                        after_sha256: Some(content_sha256),
                    });
                }
                files.push(snapshot);
            }
            None => {
                let content_sha256 = hash_file(&item.absolute_path)?;
                files_hashed += 1;
                added_paths.push((item.relative_path.clone(), content_sha256.clone()));
                let snapshot = IndexedFileSnapshot {
                    relative_path: item.relative_path,
                    size_bytes: item.size_bytes,
                    modified_unix_ns: item.modified_unix_ns,
                    content_sha256,
                };
                touched_files.push(snapshot.clone());
                files.push(snapshot);
            }
        }
    }
    for (path, old) in &previous_map {
        if !current_paths.contains(path) {
            deleted.push((path.clone(), old.content_sha256.clone()));
        }
    }

    let renamed = match_renames(&mut added_paths, &mut deleted);
    for (from, to, sha) in renamed {
        changes.push(IndexChange {
            change_kind: "renamed".to_string(),
            relative_path: to,
            previous_path: Some(from),
            before_sha256: Some(sha.clone()),
            after_sha256: Some(sha),
        });
    }
    for (path, sha) in added_paths {
        changes.push(IndexChange {
            change_kind: "added".to_string(),
            relative_path: path,
            previous_path: None,
            before_sha256: None,
            after_sha256: Some(sha),
        });
    }
    for (path, sha) in deleted {
        changes.push(IndexChange {
            change_kind: "deleted".to_string(),
            relative_path: path,
            previous_path: None,
            before_sha256: Some(sha),
            after_sha256: None,
        });
    }
    changes.sort_by(|a, b| {
        a.relative_path
            .cmp(&b.relative_path)
            .then(a.change_kind.cmp(&b.change_kind))
    });
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    touched_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    let changed_paths: BTreeSet<&str> = changes
        .iter()
        .flat_map(|change| {
            std::iter::once(change.relative_path.as_str()).chain(change.previous_path.as_deref())
        })
        .collect();
    let hint_hits = normalized_hints
        .iter()
        .filter(|hint| changed_paths.contains(hint.as_str()))
        .count();

    Ok(IndexPlan {
        stats: IndexStats {
            files_seen: files.len(),
            files_hashed,
            files_unchanged,
            symlinks_skipped,
            total_bytes,
            hint_count: normalized_hints.len(),
            hint_hits,
            elapsed_ms: elapsed_ms(started),
        },
        files,
        touched_files,
        changes,
    })
}
fn collect_metadata(root: &Path) -> Result<(Vec<FileMeta>, usize), IndexError> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut symlinks_skipped = 0usize;

    while let Some(directory) = stack.pop() {
        let entries = fs::read_dir(&directory).map_err(|error| {
            IndexError::new(
                "PROJECT_FILE_UNREADABLE",
                format!("read directory: {error}"),
            )
        })?;
        let mut entries = entries.collect::<Result<Vec<_>, _>>().map_err(|error| {
            IndexError::new(
                "PROJECT_FILE_UNREADABLE",
                format!("enumerate directory: {error}"),
            )
        })?;
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let file_type = entry.file_type().map_err(|error| {
                IndexError::new(
                    "PROJECT_FILE_UNREADABLE",
                    format!("read file type: {error}"),
                )
            })?;
            if file_type.is_symlink() {
                symlinks_skipped += 1;
                continue;
            }
            if file_type.is_dir() {
                stack.push(entry.path());
                continue;
            }
            if !file_type.is_file() {
                continue;
            }

            let absolute_path = entry.path();
            let metadata = entry.metadata().map_err(|error| {
                IndexError::new(
                    "PROJECT_FILE_UNREADABLE",
                    format!("read file metadata: {error}"),
                )
            })?;
            let relative = absolute_path.strip_prefix(root).map_err(|_| {
                IndexError::new(
                    "PROJECT_PATH_ESCAPE",
                    "indexed file escaped the canonical project root",
                )
            })?;
            let relative_path = normalize_relative(relative)?;
            let modified_unix_ns = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_nanos().min(u64::MAX as u128) as u64)
                .unwrap_or(0);

            files.push(FileMeta {
                relative_path,
                absolute_path,
                size_bytes: metadata.len(),
                modified_unix_ns,
            });
        }
    }

    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok((files, symlinks_skipped))
}
fn normalize_relative(path: &Path) -> Result<String, IndexError> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                parts.push(part.to_string_lossy().to_string());
            }
            _ => {
                return Err(IndexError::new(
                    "PROJECT_PATH_ESCAPE",
                    "relative project path contains non-normal components",
                ));
            }
        }
    }
    if parts.is_empty() {
        return Err(IndexError::new(
            "PROJECT_PATH_INVALID",
            "empty relative file path",
        ));
    }
    Ok(parts.join("/"))
}

fn validate_hints(hints: &[String]) -> Result<Vec<String>, IndexError> {
    let mut normalized = BTreeSet::new();
    for hint in hints {
        if Path::new(hint).is_absolute() {
            return Err(IndexError::new(
                "INDEX_HINT_INVALID",
                "watcher hints must be project-relative paths",
            ));
        }
        normalized.insert(normalize_project_relative_path(hint)?);
    }
    Ok(normalized.into_iter().collect())
}

pub fn normalize_project_relative_path(input: &str) -> Result<String, IndexError> {
    let path = Path::new(input);
    if path.is_absolute() {
        return Err(IndexError::new(
            "PROJECT_PATH_INVALID",
            "path must be project-relative",
        ));
    }
    normalize_relative(path)
}

fn hash_file(path: &Path) -> Result<String, IndexError> {
    let mut file = File::open(path).map_err(|error| {
        IndexError::new(
            "PROJECT_FILE_UNREADABLE",
            format!("open file for hashing: {error}"),
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            IndexError::new(
                "PROJECT_FILE_UNREADABLE",
                format!("read file for hashing: {error}"),
            )
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(output)
}
fn match_renames(
    added: &mut Vec<(String, String)>,
    deleted: &mut Vec<(String, String)>,
) -> Vec<(String, String, String)> {
    let mut renamed = Vec::new();
    let mut used_added = BTreeSet::new();
    let mut used_deleted = BTreeSet::new();

    for (deleted_index, (old_path, old_sha)) in deleted.iter().enumerate() {
        if let Some((added_index, (new_path, _))) = added
            .iter()
            .enumerate()
            .find(|(index, (_, sha))| !used_added.contains(index) && sha == old_sha)
        {
            used_added.insert(added_index);
            used_deleted.insert(deleted_index);
            renamed.push((old_path.clone(), new_path.clone(), old_sha.clone()));
        }
    }

    *added = added
        .iter()
        .enumerate()
        .filter(|(index, _)| !used_added.contains(index))
        .map(|(_, value)| value.clone())
        .collect();
    *deleted = deleted
        .iter()
        .enumerate()
        .filter(|(index, _)| !used_deleted.contains(index))
        .map(|(_, value)| value.clone())
        .collect();
    renamed
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_micros().saturating_add(999) as u64 / 1000
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "relay-indexing-{label}-{}-{suffix}",
            std::process::id()
        ))
    }

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content.as_bytes()).unwrap();
    }

    #[test]
    fn baseline_and_one_file_change_hash_only_the_touched_file() {
        let dir = temp_dir("changed-only");
        fs::create_dir_all(&dir).unwrap();
        write(&dir.join("a.txt"), "alpha");
        write(&dir.join("nested/b.txt"), "bravo");
        write(&dir.join("nested/c.src"), "charlie");

        let baseline = build_baseline(&dir).unwrap();
        assert_eq!(baseline.files.len(), 3);
        assert_eq!(baseline.stats.files_hashed, 3);

        write(&dir.join("nested/b.txt"), "bravo changed and longer");
        let plan = reconcile(&dir, &baseline.files, &["nested/b.txt".to_string()]).unwrap();

        assert_eq!(plan.stats.files_hashed, 1);
        assert_eq!(plan.stats.files_unchanged, 2);
        assert_eq!(plan.stats.hint_count, 1);
        assert_eq!(plan.stats.hint_hits, 1);
        assert_eq!(plan.changes.len(), 1);
        assert_eq!(plan.changes[0].change_kind, "modified");
        assert_eq!(plan.changes[0].relative_path, "nested/b.txt");
        assert_eq!(plan.touched_files.len(), 1);

        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn reconciliation_recovers_changes_without_watcher_hints() {
        let dir = temp_dir("missed-hint");
        fs::create_dir_all(&dir).unwrap();
        write(&dir.join("a.txt"), "alpha");
        write(&dir.join("b.txt"), "bravo");
        let baseline = build_baseline(&dir).unwrap();

        write(&dir.join("a.txt"), "alpha changed with more bytes");
        let plan = reconcile(&dir, &baseline.files, &[]).unwrap();

        assert_eq!(plan.stats.hint_count, 0);
        assert_eq!(plan.stats.files_hashed, 1);
        assert_eq!(plan.changes.len(), 1);
        assert_eq!(plan.changes[0].relative_path, "a.txt");

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rename_delete_and_add_are_explicit() {
        let dir = temp_dir("delta-kinds");
        fs::create_dir_all(&dir).unwrap();
        write(&dir.join("rename-me.txt"), "same-content");
        write(&dir.join("delete-me.txt"), "delete-content");
        write(&dir.join("keep.txt"), "keep");
        let baseline = build_baseline(&dir).unwrap();

        fs::rename(dir.join("rename-me.txt"), dir.join("renamed.txt")).unwrap();
        fs::remove_file(dir.join("delete-me.txt")).unwrap();
        write(&dir.join("added.txt"), "new-content");

        let plan = reconcile(&dir, &baseline.files, &[]).unwrap();
        let kinds: BTreeSet<_> = plan
            .changes
            .iter()
            .map(|change| change.change_kind.as_str())
            .collect();
        assert!(kinds.contains("renamed"));
        assert!(kinds.contains("deleted"));
        assert!(kinds.contains("added"));
        assert!(plan.changes.iter().any(|change| {
            change.change_kind == "renamed"
                && change.previous_path.as_deref() == Some("rename-me.txt")
                && change.relative_path == "renamed.txt"
        }));
        assert_eq!(plan.stats.files_unchanged, 1);

        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn watcher_hints_cannot_escape_project_root() {
        let dir = temp_dir("bad-hint");
        fs::create_dir_all(&dir).unwrap();
        write(&dir.join("a.txt"), "alpha");
        let baseline = build_baseline(&dir).unwrap();

        let error = reconcile(&dir, &baseline.files, &["../outside.txt".to_string()])
            .expect_err("parent traversal watcher hint must fail");
        assert_eq!(error.code, "PROJECT_PATH_ESCAPE");

        let absolute = dir.join("a.txt").to_string_lossy().to_string();
        let error = reconcile(&dir, &baseline.files, &[absolute])
            .expect_err("absolute watcher hint must fail");
        assert_eq!(error.code, "INDEX_HINT_INVALID");

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn canonical_root_rejects_missing_and_non_directory_paths() {
        let dir = temp_dir("root-validation");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("file.txt");
        write(&file, "fixture");

        let missing = canonical_project_root(&dir.join("missing").to_string_lossy())
            .expect_err("missing project root must fail");
        assert_eq!(missing.code, "PROJECT_PATH_INVALID");

        let not_directory = canonical_project_root(&file.to_string_lossy())
            .expect_err("file project root must fail");
        assert_eq!(not_directory.code, "PROJECT_PATH_NOT_DIRECTORY");

        fs::remove_dir_all(dir).unwrap();
    }
}
