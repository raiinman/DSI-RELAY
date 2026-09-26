use notify::event::{CreateKind, ModifyKind, RemoveKind};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use relay_contracts::{CommandRequest, RequestContext};
use relay_core::indexing;
use relay_core::policy::ExecutionAuthority;
use relay_core::service::{RelayCore, RuntimeContext};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

const EVENT_QUEUE_CAPACITY: usize = 1_024;
const MAX_HINTS_PER_BATCH: usize = 32;
const MAX_BACKGROUND_FILE_BYTES: u64 = 64 * 1024 * 1024;
const BATCH_INTERVAL: Duration = Duration::from_millis(250);
const RECOVERY_QUIET_PERIOD: Duration = Duration::from_secs(2);
const RECOVERY_POLL_INTERVAL: Duration = Duration::from_secs(1);
const RECOVERY_RETRY_DELAY: Duration = Duration::from_secs(30);
const RECOVERY_ATTEMPT_LIMIT: Duration = Duration::from_secs(15);
const RECOVERY_IDLE: Duration = Duration::from_secs(30);
static EVENT_ID: AtomicU64 = AtomicU64::new(1);

enum Message {
    Event(notify::Result<Event>),
    Stop,
}

pub struct WatcherRuntime {
    sender: SyncSender<Message>,
    refresh_pending: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl WatcherRuntime {
    pub fn start(
        core: Arc<RelayCore>,
        runtime: RuntimeContext,
        foreground_active: Arc<AtomicBool>,
        started_at_unix_ms: u64,
    ) -> Self {
        let (sender, receiver) = mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
        let worker_sender = sender.clone();
        let refresh_pending = Arc::new(AtomicBool::new(false));
        let worker_refresh = Arc::clone(&refresh_pending);
        let join = thread::spawn(move || {
            run_worker(
                core,
                runtime,
                foreground_active,
                started_at_unix_ms,
                worker_refresh,
                worker_sender,
                receiver,
            );
        });
        Self {
            sender,
            refresh_pending,
            join: Some(join),
        }
    }

    pub fn refresh(&self) {
        self.refresh_pending.store(true, Ordering::SeqCst);
    }

    pub fn stop(&mut self) {
        let _ = self.sender.send(Message::Stop);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for WatcherRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}

fn mark_all_stale(core: &RelayCore, roots: &BTreeMap<String, PathBuf>) {
    for project_id in roots.keys() {
        let _ = core.mark_index_stale(project_id);
    }
}

fn refresh_roots(
    core: &RelayCore,
    watcher: &mut RecommendedWatcher,
    roots: &mut BTreeMap<String, PathBuf>,
    watched_paths: &mut BTreeSet<PathBuf>,
) {
    let Ok(targets) = core.index_watch_targets() else {
        mark_all_stale(core, roots);
        return;
    };
    let mut desired = BTreeMap::new();
    for (project_id, root_uri) in targets {
        match indexing::canonical_project_root(&root_uri) {
            Ok(path) => {
                desired.insert(project_id, path);
            }
            Err(_) => {
                let _ = core.mark_index_stale(&project_id);
            }
        }
    }
    let desired_paths: BTreeSet<PathBuf> = desired.values().cloned().collect();
    for old_root in watched_paths.difference(&desired_paths) {
        let _ = watcher.unwatch(old_root);
    }
    let mut next_watched: BTreeSet<PathBuf> = watched_paths
        .intersection(&desired_paths)
        .cloned()
        .collect();
    let new_paths: Vec<PathBuf> = desired_paths.difference(&next_watched).cloned().collect();
    for new_root in new_paths {
        if watcher.watch(&new_root, RecursiveMode::Recursive).is_ok() {
            next_watched.insert(new_root);
        }
    }
    for (project_id, root) in &desired {
        if roots.get(project_id) != Some(root) || !next_watched.contains(root) {
            let _ = core.mark_index_stale(project_id);
        }
    }
    *watched_paths = next_watched;
    *roots = desired;
}

fn run_worker(
    core: Arc<RelayCore>,
    runtime: RuntimeContext,
    foreground_active: Arc<AtomicBool>,
    started_at_unix_ms: u64,
    refresh_pending: Arc<AtomicBool>,
    sender: SyncSender<Message>,
    receiver: Receiver<Message>,
) {
    let overflow = Arc::new(AtomicBool::new(false));
    let callback_overflow = Arc::clone(&overflow);
    let mut watcher = match notify::recommended_watcher(move |result| {
        if let Err(TrySendError::Full(_)) = sender.try_send(Message::Event(result)) {
            callback_overflow.store(true, Ordering::SeqCst);
        }
    }) {
        Ok(watcher) => watcher,
        Err(_) => {
            if let Ok(targets) = core.index_watch_targets() {
                for (project_id, _) in targets {
                    let _ = core.mark_index_stale(&project_id);
                }
            }
            return;
        }
    };
    let mut roots = BTreeMap::new();
    let mut watched_paths = BTreeSet::new();
    refresh_roots(&core, &mut watcher, &mut roots, &mut watched_paths);
    let mut last_refresh = Instant::now();
    let mut pending: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut last_flush = Instant::now();
    let mut last_event = Instant::now();
    let mut last_recovery_poll = Instant::now();
    let mut last_recovery_attempt = BTreeMap::new();

    loop {
        if refresh_pending.swap(false, Ordering::SeqCst)
            || last_refresh.elapsed() >= Duration::from_secs(30)
        {
            refresh_roots(&core, &mut watcher, &mut roots, &mut watched_paths);
            last_refresh = Instant::now();
        }
        if overflow.swap(false, Ordering::SeqCst) {
            mark_all_stale(&core, &roots);
            pending.clear();
            last_event = Instant::now();
        }
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(Message::Stop) => break,
            Ok(Message::Event(Err(_))) => {
                mark_all_stale(&core, &roots);
                pending.clear();
                last_event = Instant::now();
            }
            Ok(Message::Event(Ok(event))) => {
                last_event = Instant::now();
                collect_event(&core, &roots, event, &mut pending);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if last_flush.elapsed() >= BATCH_INTERVAL && !foreground_active.load(Ordering::SeqCst) {
            flush_hints(&core, &runtime, started_at_unix_ms, &mut pending);
            last_flush = Instant::now();
        }
        if last_recovery_poll.elapsed() >= RECOVERY_POLL_INTERVAL {
            recover_one_stale_project(
                &core,
                &roots,
                &watched_paths,
                &pending,
                &foreground_active,
                last_event,
                &mut last_recovery_attempt,
            );
            last_recovery_poll = Instant::now();
        }
    }
}

fn recovery_idle_threshold() -> Duration {
    if cfg!(debug_assertions)
        && let Ok(value) = std::env::var("RELAY_TEST_RECOVERY_IDLE_MS")
        && let Ok(milliseconds) = value.parse::<u64>()
    {
        return Duration::from_millis(milliseconds.max(1));
    }
    RECOVERY_IDLE
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
    // A distant or future input tick is ambiguous after wrap or injected input.
    (elapsed <= 7 * 24 * 60 * 60 * 1_000).then(|| Duration::from_millis(elapsed as u64))
}

fn recover_one_stale_project(
    core: &RelayCore,
    roots: &BTreeMap<String, PathBuf>,
    watched_paths: &BTreeSet<PathBuf>,
    pending: &BTreeMap<String, BTreeSet<String>>,
    foreground_active: &AtomicBool,
    last_event: Instant,
    last_attempt: &mut BTreeMap<String, Instant>,
) {
    let idle_threshold = recovery_idle_threshold();
    let safe_to_scan = || {
        !foreground_active.load(Ordering::SeqCst)
            && last_event.elapsed() >= RECOVERY_QUIET_PERIOD
            && user_idle_duration().is_some_and(|idle| idle >= idle_threshold)
    };
    if !pending.is_empty() || !safe_to_scan() {
        return;
    }
    for (project_id, root) in roots {
        if !watched_paths.contains(root) {
            continue;
        }
        if last_attempt
            .get(project_id)
            .is_some_and(|at| at.elapsed() < RECOVERY_RETRY_DELAY)
        {
            continue;
        }
        let Ok(Some(state)) = core.index_recovery_state(project_id) else {
            continue;
        };
        if state.status != "stale" {
            continue;
        }
        let started = Instant::now();
        last_attempt.insert(project_id.clone(), started);
        let should_continue = || started.elapsed() < RECOVERY_ATTEMPT_LIMIT && safe_to_scan();
        let _ = core.reconcile_index_background(
            project_id,
            state.content_verification_required,
            &should_continue,
        );
        break;
    }
}

fn collect_event(
    core: &RelayCore,
    roots: &BTreeMap<String, PathBuf>,
    event: Event,
    pending: &mut BTreeMap<String, BTreeSet<String>>,
) {
    if event.kind.is_access() {
        return;
    }
    if event.need_rescan() || event.paths.is_empty() {
        mark_all_stale(core, roots);
        pending.clear();
        return;
    }
    let uncertain_kind = matches!(
        event.kind,
        EventKind::Create(CreateKind::Folder)
            | EventKind::Remove(RemoveKind::Folder)
            | EventKind::Modify(ModifyKind::Name(_))
            | EventKind::Any
            | EventKind::Other
    );
    for path in event.paths {
        for (project_id, root) in roots {
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            if uncertain_kind || relative.as_os_str().is_empty() || path.is_dir() {
                let _ = core.mark_index_stale(project_id);
                pending.remove(project_id);
                continue;
            }
            if path
                .metadata()
                .map(|metadata| metadata.len() > MAX_BACKGROUND_FILE_BYTES)
                .unwrap_or(false)
            {
                let _ = core.mark_index_stale(project_id);
                pending.remove(project_id);
                continue;
            }
            let relative = relative.to_string_lossy().replace('\\', "/");
            if indexing::validate_hints(std::slice::from_ref(&relative)).is_err() {
                let _ = core.mark_index_stale(project_id);
                pending.remove(project_id);
                continue;
            }
            let paths = pending.entry(project_id.clone()).or_default();
            paths.insert(relative);
            if paths.len() > MAX_HINTS_PER_BATCH {
                let _ = core.mark_index_stale(project_id);
                pending.remove(project_id);
            }
        }
    }
}

fn flush_hints(
    core: &RelayCore,
    runtime: &RuntimeContext,
    started_at_unix_ms: u64,
    pending: &mut BTreeMap<String, BTreeSet<String>>,
) {
    let authority = ExecutionAuthority::local_user("relayd-watcher".to_string());
    for (project_id, paths) in std::mem::take(pending) {
        if paths.is_empty() {
            continue;
        }
        let sequence = EVENT_ID.fetch_add(1, Ordering::Relaxed);
        let id = format!("WATCH-{}-{started_at_unix_ms}-{sequence}", runtime.pid);
        let request = CommandRequest {
            request_id: id.clone(),
            command: "project.index.apply_hints".to_string(),
            command_version: Some(1),
            arguments: json!({ "project_id": project_id, "hints": paths }),
            idempotency_key: Some(id),
            context: RequestContext::default(),
        };
        let response = core.execute_authorized(request, runtime, &authority);
        if !response.ok {
            let _ = core.mark_index_stale(&project_id);
        }
    }
}
