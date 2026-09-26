use relay_adapter::{AdapterBroker, AdapterManifest, BrokerPolicy};
use relay_contracts::{CommandRequest, RequestContext};
use relay_core::indexing;
use relay_core::policy::ExecutionAuthority;
use relay_core::service::{ParserProjectSnapshot, RelayCore, RuntimeContext};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

const MAX_SOURCE_BYTES: u64 = 1024 * 1024;
const SCAN_INTERVAL: Duration = Duration::from_secs(1);
const RETRY_DELAY: Duration = Duration::from_secs(5);
const NORMAL_IDLE: Duration = Duration::from_secs(30);
static PARSE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InstallationFile {
    format_version: u32,
    installations: Vec<InstallationGrant>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InstallationGrant {
    project_id: String,
    manifest_path: PathBuf,
    worker_path: PathBuf,
    adapter_id: String,
    adapter_version: String,
    worker_sha256: String,
    target_tool: String,
    target_version: String,
    allow_source_delivery: bool,
    source_extensions: Vec<String>,
}

pub struct Installation {
    adapter_id: String,
    adapter_version: String,
    source_extensions: BTreeSet<String>,
    broker: AdapterBroker,
}

/// Local-user managed installation grants are read at startup. A declaration in project
/// configuration is never enough to send source content to a worker.
pub fn load_installations(state_dir: &Path) -> Result<BTreeMap<String, Installation>, String> {
    let path = state_dir.join("parser-installations.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) if bytes.len() <= 1024 * 1024 => bytes,
        Ok(_) => return Err("parser installation file exceeds 1 MiB".to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(format!("read parser installations: {error}")),
    };
    let file: InstallationFile = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse parser installations: {error}"))?;
    if file.format_version != 1 || file.installations.len() > 32 {
        return Err("parser installation format or count is unsupported".to_string());
    }
    let mut installed = BTreeMap::new();
    for grant in file.installations {
        if !grant.allow_source_delivery {
            continue;
        }
        if grant.project_id.is_empty()
            || grant.project_id.len() > 128
            || grant.source_extensions.is_empty()
            || grant.source_extensions.len() > 32
        {
            return Err("parser grant has an invalid project or source selection".to_string());
        }
        let mut extensions = BTreeSet::new();
        for extension in &grant.source_extensions {
            if extension.is_empty()
                || extension.len() > 16
                || !extension
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                || !extensions.insert(extension.clone())
            {
                return Err(
                    "parser source extensions must be distinct lowercase tokens".to_string()
                );
            }
        }
        let manifest_path = fs::canonicalize(&grant.manifest_path)
            .map_err(|error| format!("resolve parser manifest: {error}"))?;
        let worker_path = fs::canonicalize(&grant.worker_path)
            .map_err(|error| format!("resolve parser worker: {error}"))?;
        let manifest_bytes =
            fs::read(&manifest_path).map_err(|error| format!("read parser manifest: {error}"))?;
        if manifest_bytes.len() > 1024 * 1024 {
            return Err("parser manifest exceeds 1 MiB".to_string());
        }
        let manifest: AdapterManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| format!("parse parser manifest: {error}"))?;
        if manifest_path.parent() != worker_path.parent()
            || manifest.adapter.id != grant.adapter_id
            || manifest.adapter.version != grant.adapter_version
            || !manifest
                .artifact
                .sha256
                .eq_ignore_ascii_case(&grant.worker_sha256)
            || manifest.target.tool != grant.target_tool
            || manifest.target.version != grant.target_version
            || manifest.permissions.project_read != [grant.project_id.clone()]
            || manifest.permissions.network
            || manifest.permissions.subprocess
            || !manifest.permissions.project_write.is_empty()
            || !manifest.permissions.credentials.is_empty()
            || !manifest.permissions.external_apps.is_empty()
            || !manifest.relay.command_bindings.iter().any(|binding| {
                binding.command == "adapter.dependencies.parse" && binding.command_version == 1
            })
        {
            return Err("parser manifest does not request exactly the granted read scope and parser operation".to_string());
        }
        let mut allowed_project_read = BTreeSet::new();
        allowed_project_read.insert(grant.project_id.clone());
        let policy = BrokerPolicy {
            allowed_project_read,
            allowed_project_write: BTreeSet::new(),
            allow_network: false,
            allowed_credentials: BTreeSet::new(),
            allow_subprocess: false,
            allowed_external_apps: BTreeSet::new(),
            target_tool: manifest.target.tool.clone(),
            target_version: manifest.target.version.clone(),
            max_process_memory_bytes: 64 * 1024 * 1024,
            request_timeout_ms: 3_000,
            max_failures_before_quarantine: 2,
            backoff_ms: 500,
        };
        let broker = AdapterBroker::new(
            policy,
            state_dir.join("parser-mailboxes").join(&grant.project_id),
        )
        .map_err(|error| format!("initialize parser broker: {error}"))?;
        broker
            .install(manifest.clone(), &worker_path)
            .map_err(|error| format!("install parser package: {error}"))?;
        if installed
            .insert(
                grant.project_id,
                Installation {
                    adapter_id: manifest.adapter.id,
                    adapter_version: manifest.adapter.version,
                    source_extensions: extensions,
                    broker,
                },
            )
            .is_some()
        {
            return Err("multiple parser installations target one project".to_string());
        }
    }
    Ok(installed)
}

struct ParsedSource {
    source_sha256: String,
    configuration_revision: i64,
    adapter_id: String,
    adapter_version: String,
    target_sha256: BTreeMap<String, String>,
}

struct CachedProject {
    snapshot: ParserProjectSnapshot,
    indexed: BTreeMap<String, String>,
}

pub struct ParserRuntime {
    stop_requested: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl ParserRuntime {
    pub fn start(
        core: Arc<RelayCore>,
        runtime: RuntimeContext,
        foreground_active: Arc<AtomicBool>,
        installations: BTreeMap<String, Installation>,
    ) -> Option<Self> {
        if installations.is_empty() {
            return None;
        }
        let stop_requested = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&stop_requested);
        let join = thread::spawn(move || {
            let mut parsed: BTreeMap<(String, String), ParsedSource> = BTreeMap::new();
            let mut attempted = BTreeMap::new();
            let mut snapshots: BTreeMap<String, CachedProject> = BTreeMap::new();
            let mut complete = BTreeSet::new();
            while !stop.load(Ordering::SeqCst) {
                if !foreground_active.load(Ordering::SeqCst)
                    && user_idle_duration().is_some_and(|idle| idle >= idle_threshold())
                {
                    if let Ok(headers) = core.parser_ready_projects() {
                        let ready_ids: BTreeSet<_> =
                            headers.iter().map(|item| item.project_id.clone()).collect();
                        snapshots.retain(|project_id, _| ready_ids.contains(project_id));
                        complete.retain(|project_id| ready_ids.contains(project_id));
                        for mut header in headers {
                            if !installations.contains_key(&header.project_id) {
                                continue;
                            }
                            let needs_refresh =
                                snapshots.get(&header.project_id).is_none_or(|old| {
                                    old.snapshot.generation != header.generation
                                        || old.snapshot.configuration.revision
                                            != header.configuration.revision
                                });
                            if needs_refresh
                                && let Ok(Some(files)) =
                                    core.parser_ready_files(&header.project_id, header.generation)
                            {
                                if let Some(old) = snapshots.get(&header.project_id) {
                                    let changed = core
                                        .parser_changed_paths(
                                            &header.project_id,
                                            old.snapshot.generation,
                                            header.generation,
                                        )
                                        .ok()
                                        .flatten();
                                    parsed.retain(|(id, source), observation| {
                                        if id != &header.project_id {
                                            return true;
                                        }
                                        changed.as_ref().is_some_and(|paths| {
                                            !paths.contains(source)
                                                && !observation
                                                    .target_sha256
                                                    .keys()
                                                    .any(|target| paths.contains(target))
                                        })
                                    });
                                }
                                header.files = files;
                                let indexed = header
                                    .files
                                    .iter()
                                    .map(|file| {
                                        (file.relative_path.clone(), file.content_sha256.clone())
                                    })
                                    .collect();
                                complete.remove(&header.project_id);
                                snapshots.insert(
                                    header.project_id.clone(),
                                    CachedProject {
                                        snapshot: header,
                                        indexed,
                                    },
                                );
                            }
                        }
                        process_one(
                            &core,
                            &runtime,
                            &installations,
                            snapshots.values(),
                            &mut parsed,
                            &mut attempted,
                            &mut complete,
                        );
                    }
                }
                thread::sleep(SCAN_INTERVAL);
            }
        });
        Some(Self {
            stop_requested,
            join: Some(join),
        })
    }

    pub fn stop(&mut self) {
        self.stop_requested.store(true, Ordering::SeqCst);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for ParserRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}

fn idle_threshold() -> Duration {
    if cfg!(debug_assertions)
        && let Ok(value) = std::env::var("RELAY_TEST_PARSER_IDLE_MS")
        && let Ok(milliseconds) = value.parse::<u64>()
    {
        return Duration::from_millis(milliseconds.max(1));
    }
    NORMAL_IDLE
}

fn user_idle_duration() -> Option<Duration> {
    let mut last = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    if unsafe { GetLastInputInfo(&mut last) } == 0 {
        return None;
    }
    let elapsed = unsafe { GetTickCount() }.wrapping_sub(last.dwTime);
    (elapsed <= 7 * 24 * 60 * 60 * 1_000).then(|| Duration::from_millis(elapsed as u64))
}

fn process_one<'a>(
    core: &RelayCore,
    runtime: &RuntimeContext,
    installations: &BTreeMap<String, Installation>,
    projects: impl Iterator<Item = &'a CachedProject>,
    parsed: &mut BTreeMap<(String, String), ParsedSource>,
    attempted: &mut BTreeMap<(String, String), Instant>,
    complete: &mut BTreeSet<String>,
) {
    for cached in projects {
        let project = &cached.snapshot;
        if complete.contains(&project.project_id) {
            continue;
        }
        let Some(installation) = installations.get(&project.project_id) else {
            continue;
        };
        if project.configuration.adapter_id.as_deref() != Some(&installation.adapter_id)
            || project.configuration.adapter_version.as_deref()
                != Some(&installation.adapter_version)
        {
            complete.insert(project.project_id.clone());
            continue;
        }
        let indexed = &cached.indexed;
        parsed.retain(|(id, path), _| {
            id != &project.project_id || indexed.contains_key(path.as_str())
        });
        let mut retry_pending = false;
        for file in &project.files {
            let path = &file.relative_path;
            let extension = Path::new(path).extension().and_then(|value| value.to_str());
            if !extension.is_some_and(|value| installation.source_extensions.contains(value)) {
                continue;
            }
            let key = (project.project_id.clone(), path.clone());
            if parsed.get(&key).is_some_and(|old| {
                old.source_sha256 == file.content_sha256
                    && old.configuration_revision == project.configuration.revision
                    && old.adapter_id == installation.adapter_id
                    && old.adapter_version == installation.adapter_version
                    && old.target_sha256.iter().all(|(target, sha)| {
                        indexed.get(target).is_some_and(|current| current == sha)
                    })
            }) {
                continue;
            }
            if attempted
                .get(&key)
                .is_some_and(|at| at.elapsed() < RETRY_DELAY)
            {
                retry_pending = true;
                continue;
            }
            attempted.insert(key.clone(), Instant::now());
            if let Some(observation) = parse_and_replace(
                core,
                runtime,
                installation,
                project,
                path,
                &file.content_sha256,
                indexed,
            ) {
                parsed.insert(key, observation);
            }
            return;
        }
        if !retry_pending {
            complete.insert(project.project_id.clone());
        }
    }
}

fn parse_and_replace(
    core: &RelayCore,
    runtime: &RuntimeContext,
    installation: &Installation,
    project: &ParserProjectSnapshot,
    source_path: &str,
    source_sha256: &str,
    indexed: &BTreeMap<String, String>,
) -> Option<ParsedSource> {
    let root = indexing::canonical_project_root(&project.root_uri).ok()?;
    let path = fs::canonicalize(root.join(source_path)).ok()?;
    if !path.starts_with(&root) || !path.is_file() {
        return None;
    }
    let mut reader = fs::File::open(&path).ok()?.take(MAX_SOURCE_BYTES + 1);
    let mut content = Vec::new();
    reader.read_to_end(&mut content).ok()?;
    if content.len() as u64 > MAX_SOURCE_BYTES || digest(&content) != source_sha256 {
        return None;
    }
    if !core.parser_source_selected(
        &project.project_id,
        project.generation,
        project.configuration.revision,
        &installation.adapter_id,
        &installation.adapter_version,
        source_path,
        source_sha256,
    ) {
        return None;
    }
    let content_utf8 = String::from_utf8(content).ok()?;
    let outcome = installation
        .broker
        .invoke_dependency_parser(
            &installation.adapter_id,
            source_path,
            source_sha256,
            &project.configuration.project_type,
            &content_utf8,
        )
        .ok()?;
    // Recheck source bytes after sandbox work; Core also checks ready state, generation,
    // indexed digest, and indexed targets atomically when replacement commits.
    let mut reader = fs::File::open(&path).ok()?.take(MAX_SOURCE_BYTES + 1);
    let mut current = Vec::new();
    reader.read_to_end(&mut current).ok()?;
    if current.len() as u64 > MAX_SOURCE_BYTES || digest(&current) != source_sha256 {
        return None;
    }
    let targets = outcome.response["result"]["targets"].as_array()?;
    let mut target_sha256 = BTreeMap::new();
    for target in targets {
        let path = target.as_str()?;
        let sha = indexed.get(path)?;
        target_sha256.insert(path.to_string(), sha.clone());
    }
    let sequence = PARSE_ID.fetch_add(1, Ordering::Relaxed);
    let request_id = format!("PARSER-{}-{sequence}", runtime.pid);
    let response = core.execute_authorized(
        CommandRequest {
            request_id: request_id.clone(),
            command: "project.dependencies.replace".to_string(),
            command_version: Some(1),
            arguments: json!({
                "project_id": project.project_id,
                "expected_generation": project.generation,
                "source_path": source_path,
                "source_sha256": source_sha256,
                "producer_id": outcome.provenance.adapter_id,
                "producer_version": outcome.provenance.adapter_version,
                "expected_configuration_revision": project.configuration.revision,
                "expected_adapter_id": installation.adapter_id,
                "expected_adapter_version": installation.adapter_version,
                "targets": targets,
            }),
            idempotency_key: Some(request_id),
            context: RequestContext::default(),
        },
        runtime,
        &ExecutionAuthority::local_user("relayd-parser".to_string()),
    );
    response.ok.then(|| ParsedSource {
        source_sha256: source_sha256.to_string(),
        configuration_revision: project.configuration.revision,
        adapter_id: installation.adapter_id.clone(),
        adapter_version: installation.adapter_version.clone(),
        target_sha256,
    })
}

fn digest(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut text = String::with_capacity(64);
    for byte in hash {
        use std::fmt::Write as _;
        write!(&mut text, "{byte:02x}").expect("write digest");
    }
    text
}
