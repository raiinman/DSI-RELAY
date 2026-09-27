use crate::adapter::{AdapterError, JobGuard, JobLimitEvidence};
use flatbuffers::{FlatBufferBuilder, WIPOffset};
use serde::{Deserialize, Serialize};

use std::ffi::{c_void, CString};
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::thread;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{
    CloseHandle, FreeLibrary, GetLastError, FARPROC, HMODULE,
};
use windows_sys::Win32::Security::Isolation::DeleteAppContainerProfile;
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
};
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, ResumeThread, TerminateProcess, PROCESS_INFORMATION,
    STARTUPINFOW, CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT,
};

pub const SANDBOX_SPEC_VERSION: &str = "0.1.0";

#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    pub identity: String,
    pub spec_version: String,
    pub read_write_paths: Vec<PathBuf>,
    pub read_only_paths: Vec<PathBuf>,
    pub capabilities: Vec<String>,
    pub network_default_allow: bool,
    pub disallow_win32k: bool,
    pub max_process_memory_bytes: usize,
    pub timeout_ms: u64,
    pub environment: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxProbeResult {
    pub pid: u32,
    pub is_app_container: bool,
    pub capability_count: u32,
    pub allowed_read_ok: bool,
    pub allowed_write_ok: bool,
    pub read_only_write_ok: bool,
    pub blocked_read_ok: bool,
    pub blocked_write_ok: bool,
    pub network_connect_ok: bool,
    pub network_error_code: Option<i32>,
    pub parent_secret_visible: bool,
    pub user_profile_visible: bool,
    pub child_process_created: bool,
    pub child_process_error: u32,
    pub environment_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SandboxRunEvidence {
    pub api_available: bool,
    pub job_limits: JobLimitEvidence,
    pub launch_ms: f64,
    pub total_ms: f64,
    pub process_exit_code: u32,
    pub result: SandboxProbeResult,
}

type CreateProcessInSandboxFn = unsafe extern "system" fn(
    *const u16,
    *mut u16,
    *const c_void,
    *const c_void,
    i32,
    u32,
    *const c_void,
    *const u16,
    *const STARTUPINFOW,
    *const u16,
    *const c_void,
    u32,
    *mut PROCESS_INFORMATION,
) -> i32;


struct ModuleHandle(HMODULE);

impl Drop for ModuleHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                FreeLibrary(self.0);
            }
        }
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn environment_block(values: &[(String, String)]) -> Vec<u16> {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()));
    let mut output = Vec::new();
    for (key, value) in sorted {
        output.extend(format!("{key}={value}").encode_utf16());
        output.push(0);
    }
    output.push(0);
    output
}

fn build_string_vector<'a>(
    builder: &mut FlatBufferBuilder<'a>,
    values: &[PathBuf],
) -> Option<WIPOffset<flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<&'a str>>>> {
    if values.is_empty() {
        return None;
    }
    let strings: Vec<_> = values
        .iter()
        .map(|path| builder.create_string(&path.to_string_lossy()))
        .collect();
    Some(builder.create_vector(&strings))
}

fn build_sandbox_spec(policy: &SandboxPolicy) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let version = builder.create_string(&policy.spec_version);
    let capabilities = if policy.capabilities.is_empty() {
        None
    } else {
        Some(builder.create_string(&policy.capabilities.join(",")))
    };
    let read_write = build_string_vector(&mut builder, &policy.read_write_paths);
    let read_only = build_string_vector(&mut builder, &policy.read_only_paths);

    let network_policy = if policy.network_default_allow {
        let egress_table = builder.start_table();
        builder.push_slot::<u8>(4, 1, 0);
        let egress = builder.end_table(egress_table);

        let network_table = builder.start_table();
        builder.push_slot_always(6, egress);
        Some(builder.end_table(network_table))
    } else {
        None
    };

    let table = builder.start_table();
    builder.push_slot_always(4, version);
    builder.push_slot(6, true, false);
    builder.push_slot(10, policy.disallow_win32k, false);
    if let Some(value) = capabilities {
        builder.push_slot_always(16, value);
    }
    if let Some(value) = read_write {
        builder.push_slot_always(18, value);
    }
    if let Some(value) = read_only {
        builder.push_slot_always(20, value);
    }
    if let Some(value) = network_policy {
        builder.push_slot_always(22, value);
    }
    let root = builder.end_table(table);
    builder.finish(root, Some("SBOX"));
    builder.finished_data().to_vec()
}


fn load_sandbox_api() -> Result<(ModuleHandle, CreateProcessInSandboxFn), AdapterError> {
    let dll = wide("processmodel.dll");
    let module = unsafe {
        LoadLibraryExW(
            dll.as_ptr(),
            null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    };
    if module.is_null() {
        return Err(AdapterError::new(
            "SANDBOX_API_UNAVAILABLE",
            format!("LoadLibraryExW(processmodel.dll) failed: {}", unsafe {
                GetLastError()
            }),
        ));
    }
    let module = ModuleHandle(module);
    let name = CString::new("Experimental_CreateProcessInSandbox").unwrap();
    let proc: FARPROC = unsafe { GetProcAddress(module.0, name.as_ptr() as *const u8) };
    let Some(raw) = proc else {
        return Err(AdapterError::new(
            "SANDBOX_API_UNAVAILABLE",
            "Experimental_CreateProcessInSandbox export is missing",
        ));
    };
    let function: CreateProcessInSandboxFn =
        unsafe { std::mem::transmute(raw) };
    Ok((module, function))
}

pub fn sandbox_api_available() -> bool {
    load_sandbox_api().is_ok()
}

fn cleanup_profile(identity: &str) {
    let name = wide(identity);
    unsafe {
        let _ = DeleteAppContainerProfile(name.as_ptr());
    }
}


fn quote_argument(value: &Path) -> String {
    format!("\"{}\"", value.to_string_lossy().replace('"', "\\\""))
}


fn validate_policy(policy: &SandboxPolicy) -> Result<(), AdapterError> {
    if policy.spec_version != SANDBOX_SPEC_VERSION {
        return Err(AdapterError::new(
            "SANDBOX_SPEC_INCOMPATIBLE",
            format!(
                "sandbox spec {} is unsupported; expected {}",
                policy.spec_version, SANDBOX_SPEC_VERSION
            ),
        ));
    }

    const ALLOWED_CAPABILITIES: &[&str] = &[
        "internetClient",
    ];
    for capability in &policy.capabilities {
        if !ALLOWED_CAPABILITIES.contains(&capability.as_str()) {
            return Err(AdapterError::new(
                "SANDBOX_CAPABILITY_UNSUPPORTED",
                format!("sandbox capability {capability} is not allowed by RELAY"),
            ));
        }
    }

    if policy.network_default_allow
        && !policy.capabilities.iter().any(|value| value == "internetClient")
    {
        return Err(AdapterError::new(
            "SANDBOX_POLICY_INVALID",
            "network default-allow requires the internetClient capability",
        ));
    }

    Ok(())
}

pub fn run_probe(
    worker_path: &Path,
    mailbox: &Path,
    policy: &SandboxPolicy,
) -> Result<SandboxRunEvidence, AdapterError> {
    validate_policy(policy)?;
    if !mailbox.is_absolute() || !worker_path.is_absolute() {
        return Err(AdapterError::new(
            "SANDBOX_CONFIGURATION_INVALID",
            "worker and mailbox paths must be absolute",
        ));
    }

    let response_path = mailbox.join("response.json");
    let _ = fs::remove_file(&response_path);

    let spec = build_sandbox_spec(policy);
    let (_module, create_process) = load_sandbox_api()?;
    let application = wide(&worker_path.to_string_lossy());
    let mut command_line = wide(&format!(
        "{} {}",
        quote_argument(worker_path),
        quote_argument(mailbox)
    ));
    let current_directory = wide(&mailbox.to_string_lossy());
    let identity = wide(&policy.identity);
    let environment = environment_block(&policy.environment);

    let mut startup = STARTUPINFOW::default();
    startup.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    let mut process = PROCESS_INFORMATION::default();

    let started = Instant::now();
    let ok = unsafe {
        create_process(
            application.as_ptr(),
            command_line.as_mut_ptr(),
            null(),
            null(),
            0,
            CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW | CREATE_SUSPENDED,
            environment.as_ptr() as *const c_void,
            current_directory.as_ptr(),
            &startup,
            identity.as_ptr(),
            spec.as_ptr() as *const c_void,
            spec.len() as u32,
            &mut process,
        )
    };
    if ok == 0 {
        let error = unsafe { GetLastError() };
        cleanup_profile(&policy.identity);
        return Err(AdapterError::new(
            "SANDBOX_LAUNCH_FAILED",
            format!(
                "Experimental_CreateProcessInSandbox failed: {error}"
            ),
        ));
    }
    let launch_ms = started.elapsed().as_secs_f64() * 1000.0;

    let job = match JobGuard::create_for_handle(
        process.hProcess,
        policy.max_process_memory_bytes,
    ) {
        Ok(job) => job,
        Err(error) => {
            unsafe {
                TerminateProcess(process.hProcess, 1);
                CloseHandle(process.hThread);
                CloseHandle(process.hProcess);
            }
            cleanup_profile(&policy.identity);
            return Err(error);
        }
    };
    let job_limits = job.evidence();

    let resume = unsafe { ResumeThread(process.hThread) };
    if resume == u32::MAX {
        unsafe {
            TerminateProcess(process.hProcess, 1);
            CloseHandle(process.hThread);
            CloseHandle(process.hProcess);
        }
        cleanup_profile(&policy.identity);
        return Err(AdapterError::new(
            "SANDBOX_LAUNCH_FAILED",
            format!("ResumeThread failed: {}", unsafe { GetLastError() }),
        ));
    }


    unsafe {
        CloseHandle(process.hThread);
    }

    let deadline = Instant::now() + Duration::from_millis(policy.timeout_ms);
    let mut exit_code = 259u32;
    let result = loop {
        if response_path.exists() {
            let bytes = fs::read(&response_path).map_err(|error| {
                AdapterError::new(
                    "SANDBOX_RESPONSE_ERROR",
                    format!("read sandbox response: {error}"),
                )
            })?;
            let parsed: SandboxProbeResult =
                serde_json::from_slice(&bytes).map_err(|error| {
                    AdapterError::new(
                        "SANDBOX_RESPONSE_ERROR",
                        format!("parse sandbox response: {error}"),
                    )
                })?;
            break Ok(parsed);
        }

        let exit_ok = unsafe {
            GetExitCodeProcess(process.hProcess, &mut exit_code)
        };
        if exit_ok == 0 {
            break Err(AdapterError::new(
                "SANDBOX_PROCESS_ERROR",
                format!("GetExitCodeProcess failed: {}", unsafe {
                    GetLastError()
                }),
            ));
        }
        if exit_code != 259 {
            break Err(AdapterError::new(
                "SANDBOX_WORKER_EXITED",
                format!(
                    "sandbox worker exited with code {exit_code} before writing a response"
                ),
            ));
        }
        if Instant::now() >= deadline {
            unsafe {
                TerminateProcess(process.hProcess, 1);
            }
            break Err(AdapterError::new(
                "SANDBOX_TIMEOUT",
                "sandbox worker did not produce a response before timeout",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    };

    if exit_code == 259 {
        for _ in 0..100 {
            unsafe {
                GetExitCodeProcess(process.hProcess, &mut exit_code);
            }
            if exit_code != 259 {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    if exit_code == 259 {
        unsafe {
            TerminateProcess(process.hProcess, 0);
            GetExitCodeProcess(process.hProcess, &mut exit_code);
        }
    }

    unsafe {
        CloseHandle(process.hProcess);
    }
    drop(job);
    cleanup_profile(&policy.identity);

    let result = result?;
    Ok(SandboxRunEvidence {
        api_available: true,
        job_limits,
        launch_ms,
        total_ms: started.elapsed().as_secs_f64() * 1000.0,
        process_exit_code: exit_code,
        result,
    })
}
