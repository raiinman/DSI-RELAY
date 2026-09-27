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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HintPlan {
    pub file_count: u64,
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
    verify_content: bool,
) -> Result<IndexPlan, IndexError> {
    reconcile_with_guard(root, previous, hints, verify_content, &|| true)
}

pub fn reconcile_with_guard(
    root: &Path,
    previous: &[IndexedFileSnapshot],
    hints: &[String],
    verify_content: bool,
    should_continue: &impl Fn() -> bool,
) -> Result<IndexPlan, IndexError> {
    let started = Instant::now();
    let normalized_hints = validate_hints(hints)?;
    let previous_map: BTreeMap<String, IndexedFileSnapshot> = previous
        .iter()
        .cloned()
        .map(|file| (file.relative_path.clone(), file))
        .collect();
    let (metadata, symlinks_skipped) = collect_metadata_with_guard(root, should_continue)?;
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
        ensure_continue(should_continue)?;
        total_bytes = total_bytes.saturating_add(item.size_bytes);
        match previous_map.get(&item.relative_path) {
            Some(old)
                if old.size_bytes == item.size_bytes
                    && old.modified_unix_ns == item.modified_unix_ns
                    && !verify_content
                    && !normalized_hints.contains(&item.relative_path) =>
            {
                files_unchanged += 1;
                files.push(old.clone());
            }
            Some(old) => {
                let content_sha256 = hash_file_with_guard(&item.absolute_path, should_continue)?;
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
                let content_sha256 = hash_file_with_guard(&item.absolute_path, should_continue)?;
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

    ensure_continue(should_continue)?;
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

pub fn apply_hints(
    root: &Path,
    previous_hinted: &[IndexedFileSnapshot],
    hints: &[String],
    base_file_count: u64,
    base_total_bytes: u64,
) -> Result<HintPlan, IndexError> {
    let started = Instant::now();
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        IndexError::new(
            "PROJECT_PATH_INVALID",
            format!("canonicalize project root: {error}"),
        )
    })?;
    let normalized_hints = validate_hints(hints)?;
    if normalized_hints.is_empty() {
        return Err(IndexError::new(
            "INDEX_HINT_INVALID",
            "hint-only updates require at least one project-relative path",
        ));
    }
    let mut current: BTreeMap<String, IndexedFileSnapshot> = previous_hinted
        .iter()
        .cloned()
        .map(|file| (file.relative_path.clone(), file))
        .collect();
    let mut touched_files = Vec::new();
    let mut changes = Vec::new();
    let mut added_paths = Vec::new();
    let mut deleted = Vec::new();
    let mut files_hashed = 0usize;
    let mut files_unchanged = 0usize;
    let mut file_count = base_file_count;
    let mut total_bytes = base_total_bytes;

    for relative_path in &normalized_hints {
        let absolute_path = canonical_root.join(relative_path);
        reject_symlink_components(&canonical_root, relative_path)?;
        let metadata = match fs::metadata(&absolute_path) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(IndexError::new(
                    "PROJECT_FILE_UNREADABLE",
                    format!("read hinted file metadata: {error}"),
                ));
            }
        };
        match metadata {
            Some(metadata) if metadata.is_dir() => {
                return Err(IndexError::new(
                    "INDEX_HINT_DIRECTORY",
                    "directory hints require authoritative reconciliation",
                ));
            }
            Some(metadata) if metadata.is_file() => {
                let canonical = fs::canonicalize(&absolute_path).map_err(|error| {
                    IndexError::new(
                        "PROJECT_FILE_UNREADABLE",
                        format!("canonicalize hinted file: {error}"),
                    )
                })?;
                if !canonical.starts_with(&canonical_root) {
                    return Err(IndexError::new(
                        "PROJECT_PATH_ESCAPE",
                        "hinted file escaped the canonical project root",
                    ));
                }
                let content_sha256 = hash_file(&canonical)?;
                files_hashed += 1;
                let modified_unix_ns = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_nanos().min(u64::MAX as u128) as u64)
                    .unwrap_or(0);
                let snapshot = IndexedFileSnapshot {
                    relative_path: relative_path.clone(),
                    size_bytes: metadata.len(),
                    modified_unix_ns,
                    content_sha256: content_sha256.clone(),
                };
                match current.remove(relative_path) {
                    Some(old) => {
                        total_bytes = total_bytes
                            .checked_sub(old.size_bytes)
                            .and_then(|bytes| bytes.checked_add(snapshot.size_bytes))
                            .ok_or_else(|| {
                                IndexError::new(
                                    "INDEX_METADATA_OVERFLOW",
                                    "hinted update byte count overflow",
                                )
                            })?;
                        if old.content_sha256 == content_sha256 {
                            files_unchanged += 1;
                        } else {
                            changes.push(IndexChange {
                                change_kind: "modified".to_string(),
                                relative_path: relative_path.clone(),
                                previous_path: None,
                                before_sha256: Some(old.content_sha256),
                                after_sha256: Some(content_sha256),
                            });
                        }
                    }
                    None => {
                        file_count = file_count.checked_add(1).ok_or_else(|| {
                            IndexError::new("INDEX_METADATA_OVERFLOW", "file count overflow")
                        })?;
                        total_bytes =
                            total_bytes
                                .checked_add(snapshot.size_bytes)
                                .ok_or_else(|| {
                                    IndexError::new(
                                        "INDEX_METADATA_OVERFLOW",
                                        "byte count overflow",
                                    )
                                })?;
                        added_paths.push((relative_path.clone(), content_sha256));
                    }
                }
                touched_files.push(snapshot);
            }
            Some(_) => {
                return Err(IndexError::new(
                    "INDEX_HINT_INVALID",
                    "hinted path is not a regular file",
                ));
            }
            None => {
                if let Some(old) = current.remove(relative_path) {
                    file_count = file_count.checked_sub(1).ok_or_else(|| {
                        IndexError::new("INDEX_METADATA_OVERFLOW", "file count underflow")
                    })?;
                    total_bytes = total_bytes.checked_sub(old.size_bytes).ok_or_else(|| {
                        IndexError::new("INDEX_METADATA_OVERFLOW", "byte count underflow")
                    })?;
                    deleted.push((relative_path.clone(), old.content_sha256));
                }
            }
        }
    }

    for (from, to, sha) in match_renames(&mut added_paths, &mut deleted) {
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
    changes.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(HintPlan {
        file_count,
        stats: IndexStats {
            files_seen: normalized_hints.len(),
            files_hashed,
            files_unchanged,
            symlinks_skipped: 0,
            total_bytes,
            hint_count: normalized_hints.len(),
            hint_hits: changes.len(),
            elapsed_ms: elapsed_ms(started),
        },
        touched_files,
        changes,
    })
}

fn reject_symlink_components(root: &Path, relative_path: &str) -> Result<(), IndexError> {
    let mut current = root.to_path_buf();
    for component in Path::new(relative_path).components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(IndexError::new(
                    "INDEX_HINT_INVALID",
                    "hinted path crosses a symlink",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(IndexError::new(
                    "PROJECT_FILE_UNREADABLE",
                    format!("inspect hinted path: {error}"),
                ));
            }
        }
    }
    Ok(())
}

fn collect_metadata(root: &Path) -> Result<(Vec<FileMeta>, usize), IndexError> {
    collect_metadata_with_guard(root, &|| true)
}

fn collect_metadata_with_guard(
    root: &Path,
    should_continue: &impl Fn() -> bool,
) -> Result<(Vec<FileMeta>, usize), IndexError> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut symlinks_skipped = 0usize;

    while let Some(directory) = stack.pop() {
        ensure_continue(should_continue)?;
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
            ensure_continue(should_continue)?;
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

pub fn validate_hints(hints: &[String]) -> Result<Vec<String>, IndexError> {
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
    hash_file_with_guard(path, &|| true)
}

fn hash_file_with_guard(
    path: &Path,
    should_continue: &impl Fn() -> bool,
) -> Result<String, IndexError> {
    let mut file = File::open(path).map_err(|error| {
        IndexError::new(
            "PROJECT_FILE_UNREADABLE",
            format!("open file for hashing: {error}"),
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        ensure_continue(should_continue)?;
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

fn ensure_continue(should_continue: &impl Fn() -> bool) -> Result<(), IndexError> {
    if should_continue() {
        Ok(())
    } else {
        Err(IndexError::new(
            "INDEX_RECOVERY_DEFERRED",
            "background index recovery was deferred",
        ))
    }
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

    fn committed_hints(
        previous: &[IndexedFileSnapshot],
        plan: &HintPlan,
    ) -> Vec<IndexedFileSnapshot> {
        let mut files: BTreeMap<String, IndexedFileSnapshot> = previous
            .iter()
            .cloned()
            .map(|file| (file.relative_path.clone(), file))
            .collect();
        for change in &plan.changes {
            if change.change_kind == "deleted" {
                files.remove(&change.relative_path);
            }
            if let Some(previous_path) = &change.previous_path {
                files.remove(previous_path);
            }
        }
        for file in &plan.touched_files {
            files.insert(file.relative_path.clone(), file.clone());
        }
        files.into_values().collect()
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
        let plan = reconcile(&dir, &baseline.files, &["nested/b.txt".to_string()], false).unwrap();

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
    fn guarded_reconciliation_defers_before_returning_a_partial_plan() {
        let dir = temp_dir("recovery-defer");
        fs::create_dir_all(&dir).unwrap();
        write(&dir.join("one.txt"), "content");
        let baseline = build_baseline(&dir).unwrap();
        let calls = std::cell::Cell::new(0usize);
        let error = reconcile_with_guard(&dir, &baseline.files, &[], true, &|| {
            calls.set(calls.get() + 1);
            calls.get() < 3
        })
        .unwrap_err();
        assert_eq!(error.code, "INDEX_RECOVERY_DEFERRED");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn hinted_update_is_provisional_and_reconciliation_finds_unhinted_change() {
        let dir = temp_dir("hint-only");
        fs::create_dir_all(&dir).unwrap();
        write(&dir.join("a.txt"), "alpha");
        write(&dir.join("b.txt"), "bravo");
        write(&dir.join("c.txt"), "charlie");
        let baseline = build_baseline(&dir).unwrap();

        write(&dir.join("a.txt"), "alpha changed and longer");
        write(&dir.join("b.txt"), "bravo changed and longer");
        let previous_hinted = vec![baseline.files[0].clone()];
        let hinted = apply_hints(
            &dir,
            &previous_hinted,
            &["a.txt".to_string()],
            baseline.files.len() as u64,
            baseline.stats.total_bytes,
        )
        .unwrap();
        assert_eq!(hinted.stats.files_seen, 1);
        assert_eq!(hinted.stats.files_hashed, 1);
        assert_eq!(hinted.changes.len(), 1);
        assert_eq!(hinted.changes[0].relative_path, "a.txt");

        let reconciled =
            reconcile(&dir, &committed_hints(&baseline.files, &hinted), &[], false).unwrap();
        assert_eq!(reconciled.stats.files_hashed, 1);
        assert_eq!(reconciled.changes.len(), 1);
        assert_eq!(reconciled.changes[0].relative_path, "b.txt");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn hinted_rename_requires_both_paths_and_directory_hint_fails() {
        let dir = temp_dir("hint-rename");
        fs::create_dir_all(dir.join("nested")).unwrap();
        write(&dir.join("old.txt"), "same content");
        let baseline = build_baseline(&dir).unwrap();
        fs::rename(dir.join("old.txt"), dir.join("new.txt")).unwrap();
        let hinted = apply_hints(
            &dir,
            &baseline.files,
            &["old.txt".to_string(), "new.txt".to_string()],
            baseline.files.len() as u64,
            baseline.stats.total_bytes,
        )
        .unwrap();
        assert_eq!(hinted.changes.len(), 1);
        assert_eq!(hinted.changes[0].change_kind, "renamed");
        let error = apply_hints(
            &dir,
            &[],
            &["nested".to_string()],
            hinted.file_count,
            hinted.stats.total_bytes,
        )
        .unwrap_err();
        assert_eq!(error.code, "INDEX_HINT_DIRECTORY");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn large_fixture_hint_update_avoids_filesystem_tree_walk() {
        let dir = temp_dir("hint-scale");
        fs::create_dir_all(&dir).unwrap();
        for index in 0..1_000 {
            let relative = format!("group-{:02}/file-{index:04}.txt", index / 100);
            write(&dir.join(relative), "small deterministic fixture");
        }
        let baseline = build_baseline(&dir).unwrap();
        assert_eq!(baseline.stats.files_hashed, 1_000);
        let changed_path = "group-07/file-0750.txt";
        write(&dir.join(changed_path), "one changed file with more bytes");
        let previous_hinted = baseline
            .files
            .iter()
            .filter(|file| file.relative_path == changed_path)
            .cloned()
            .collect::<Vec<_>>();
        let hinted = apply_hints(
            &dir,
            &previous_hinted,
            &[changed_path.to_string()],
            baseline.files.len() as u64,
            baseline.stats.total_bytes,
        )
        .unwrap();
        let confirmed =
            reconcile(&dir, &committed_hints(&baseline.files, &hinted), &[], false).unwrap();
        let content_verified =
            reconcile(&dir, &committed_hints(&baseline.files, &hinted), &[], true).unwrap();
        assert_eq!(hinted.stats.files_seen, 1);
        assert_eq!(hinted.stats.files_hashed, 1);
        assert_eq!(confirmed.stats.files_seen, 1_000);
        assert_eq!(confirmed.stats.files_hashed, 0);
        assert!(confirmed.changes.is_empty());
        assert_eq!(content_verified.stats.files_hashed, 1_000);
        assert!(content_verified.changes.is_empty());
        println!(
            "PHASE3_HINT_METRICS={}",
            serde_json::json!({
                "fixture_files": 1_000,
                "baseline_ms": baseline.stats.elapsed_ms,
                "hint_update_ms": hinted.stats.elapsed_ms,
                "hint_paths_stat_checked": hinted.stats.files_seen,
                "hint_files_hashed": hinted.stats.files_hashed,
                "full_metadata_reconcile_ms": confirmed.stats.elapsed_ms,
                "full_metadata_files_seen": confirmed.stats.files_seen,
                "full_reconcile_files_hashed": confirmed.stats.files_hashed,
                "content_verify_ms": content_verified.stats.elapsed_ms,
                "content_verify_files_hashed": content_verified.stats.files_hashed
            })
        );
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
        let plan = reconcile(&dir, &baseline.files, &[], false).unwrap();

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

        let plan = reconcile(&dir, &baseline.files, &[], false).unwrap();
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

        let error = reconcile(
            &dir,
            &baseline.files,
            &["../outside.txt".to_string()],
            false,
        )
        .expect_err("parent traversal watcher hint must fail");
        assert_eq!(error.code, "PROJECT_PATH_ESCAPE");

        let absolute = dir.join("a.txt").to_string_lossy().to_string();
        let error = reconcile(&dir, &baseline.files, &[absolute], false)
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
