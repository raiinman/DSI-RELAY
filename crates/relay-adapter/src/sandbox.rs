use crate::{AdapterError, JobGuard, JobLimitEvidence};
use serde::Serialize;
use serde_json::Value;
use std::ffi::{c_void, CString};
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::thread;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{
    CloseHandle, FreeLibrary, GetLastError, LocalFree, ERROR_INSUFFICIENT_BUFFER,
    FARPROC, HLOCAL, HMODULE,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW,
    ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW,
    SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, GRANT_ACCESS,
    SDDL_REVISION_1, SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_USER,
};
use windows_sys::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeleteAppContainerProfile,
    DeriveAppContainerSidFromAppContainerName,
};
use windows_sys::Win32::Security::{
    ACL, DeriveCapabilitySidsFromName, FreeSid, GetSecurityDescriptorSacl, PSID,
    PSECURITY_DESCRIPTOR, SECURITY_CAPABILITIES, SID_AND_ATTRIBUTES,
    CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, LABEL_SECURITY_INFORMATION,
    OBJECT_INHERIT_ACE,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
};
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
};
use windows_sys::Win32::System::SystemServices::SE_GROUP_ENABLED;
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
    InitializeProcThreadAttributeList, ResumeThread, TerminateProcess,
    UpdateProcThreadAttribute, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROCESS_INFORMATION, STARTUPINFOEXW, CREATE_NO_WINDOW, CREATE_SUSPENDED,
    CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
    PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY,
    PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
};

const HRESULT_ALREADY_EXISTS: i32 = 0x8007_00B7u32 as i32;
const PROC_THREAD_ATTRIBUTE_ALL_APPLICATION_PACKAGES_POLICY: usize = 0x0002_000F;
const PROCESS_CREATION_ALL_APPLICATION_PACKAGES_OPT_OUT: u32 = 0x1;
const WIN32K_SYSTEM_CALL_DISABLE_ALWAYS_ON: u64 = 1u64 << 28;

#[derive(Debug, Clone)]
pub struct StableSandboxPolicy {
    pub identity: String,
    pub read_write_paths: Vec<PathBuf>,
    pub read_only_paths: Vec<PathBuf>,
    pub capabilities: Vec<String>,
    pub disallow_win32k: bool,
    pub max_process_memory_bytes: usize,
    pub timeout_ms: u64,
    pub environment: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StableSandboxRunEvidence {
    pub backend: String,
    pub stable_api_available: bool,
    pub low_privilege_appcontainer: bool,
    pub temporary_acl_grant_count: usize,
    pub temporary_low_il_label_count: usize,
    pub job_limits: JobLimitEvidence,
    pub prelaunch_ms: f64,
    pub launch_ms: f64,
    pub total_ms: f64,
    pub process_exit_code: u32,
    pub worker_pid: u32,
    pub result: Value,
}

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

fn quote_argument(value: &Path) -> String {
    format!("\"{}\"", value.to_string_lossy().replace('"', "\\\""))
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

fn load_userenv_for_probe() -> Result<ModuleHandle, AdapterError> {
    let dll = wide("userenv.dll");
    let module = unsafe {
        LoadLibraryExW(
            dll.as_ptr(),
            null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    };
    if module.is_null() {
        return Err(AdapterError::new(
            "STABLE_SANDBOX_UNAVAILABLE",
            format!("LoadLibraryExW(userenv.dll) failed: {}", unsafe {
                GetLastError()
            }),
        ));
    }
    Ok(ModuleHandle(module))
}

fn export_present(module: HMODULE, name: &str) -> bool {
    let Ok(name) = CString::new(name) else {
        return false;
    };
    let proc: FARPROC =
        unsafe { GetProcAddress(module, name.as_ptr() as *const u8) };
    proc.is_some()
}

pub fn stable_appcontainer_api_available() -> bool {
    let Ok(module) = load_userenv_for_probe() else {
        return false;
    };
    [
        "CreateAppContainerProfile",
        "DeriveAppContainerSidFromAppContainerName",
        "DeleteAppContainerProfile",
    ]
    .iter()
    .all(|name| export_present(module.0, name))
}

struct OwnedCapabilitySid(PSID);

impl Drop for OwnedCapabilitySid {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                LocalFree(self.0 as HLOCAL);
            }
        }
    }
}

fn derive_capability_sid(name: &str) -> Result<OwnedCapabilitySid, AdapterError> {
    let name = wide(name);
    let mut group_sids: *mut PSID = null_mut();
    let mut group_count = 0u32;
    let mut capability_sids: *mut PSID = null_mut();
    let mut capability_count = 0u32;

    let ok = unsafe {
        DeriveCapabilitySidsFromName(
            name.as_ptr(),
            &mut group_sids,
            &mut group_count,
            &mut capability_sids,
            &mut capability_count,
        )
    };
    if ok == 0 {
        return Err(AdapterError::new(
            "STABLE_SANDBOX_CAPABILITY_UNSUPPORTED",
            format!(
                "DeriveCapabilitySidsFromName failed: {}",
                unsafe { GetLastError() }
            ),
        ));
    }

    unsafe {
        for index in 0..group_count as usize {
            let sid = *group_sids.add(index);
            if !sid.is_null() {
                LocalFree(sid as HLOCAL);
            }
        }
        if !group_sids.is_null() {
            LocalFree(group_sids as HLOCAL);
        }
    }

    if capability_count != 1 || capability_sids.is_null() {
        unsafe {
            if !capability_sids.is_null() {
                for index in 0..capability_count as usize {
                    let sid = *capability_sids.add(index);
                    if !sid.is_null() {
                        LocalFree(sid as HLOCAL);
                    }
                }
                LocalFree(capability_sids as HLOCAL);
            }
        }
        return Err(AdapterError::new(
            "STABLE_SANDBOX_CAPABILITY_UNSUPPORTED",
            format!("capability resolved to {capability_count} SIDs instead of one"),
        ));
    }

    let sid = unsafe { *capability_sids };
    unsafe {
        LocalFree(capability_sids as HLOCAL);
    }
    Ok(OwnedCapabilitySid(sid))
}

struct AppContainerProfile {
    name: String,
    sid: PSID,
}

impl Drop for AppContainerProfile {
    fn drop(&mut self) {
        let name = wide(&self.name);
        unsafe {
            let _ = DeleteAppContainerProfile(name.as_ptr());
            if !self.sid.is_null() {
                FreeSid(self.sid);
            }
        }
    }
}


fn create_profile(
    identity: &str,
    capabilities: &[SID_AND_ATTRIBUTES],
) -> Result<AppContainerProfile, AdapterError> {
    let name = wide(identity);
    let display = wide("DSI RELAY adapter sandbox");
    let description = wide("DSI RELAY synthetic AppContainer fallback");
    let mut sid: PSID = null_mut();

    let hr = unsafe {
        CreateAppContainerProfile(
            name.as_ptr(),
            display.as_ptr(),
            description.as_ptr(),
            if capabilities.is_empty() {
                null()
            } else {
                capabilities.as_ptr()
            },
            capabilities.len() as u32,
            &mut sid,
        )
    };

    if hr < 0 && hr != HRESULT_ALREADY_EXISTS {
        return Err(AdapterError::new(
            "STABLE_SANDBOX_PROFILE_ERROR",
            format!("CreateAppContainerProfile failed: 0x{:08x}", hr as u32),
        ));
    }

    if hr == HRESULT_ALREADY_EXISTS {
        let derive = unsafe {
            DeriveAppContainerSidFromAppContainerName(name.as_ptr(), &mut sid)
        };
        if derive < 0 {
            return Err(AdapterError::new(
                "STABLE_SANDBOX_PROFILE_ERROR",
                format!(
                    "DeriveAppContainerSidFromAppContainerName failed: 0x{:08x}",
                    derive as u32
                ),
            ));
        }
    }

    if sid.is_null() {
        return Err(AdapterError::new(
            "STABLE_SANDBOX_PROFILE_ERROR",
            "AppContainer profile returned a null SID",
        ));
    }

    Ok(AppContainerProfile {
        name: identity.to_string(),
        sid,
    })
}


struct AclBackup {
    path: Vec<u16>,
    original_sd: PSECURITY_DESCRIPTOR,
    original_dacl: *mut ACL,
    restored: bool,
}

impl AclBackup {
    fn restore(&mut self) {
        if self.restored {
            return;
        }
        unsafe {
            let _ = SetNamedSecurityInfoW(
                self.path.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                self.original_dacl,
                null_mut(),
            );
            if !self.original_sd.is_null() {
                LocalFree(self.original_sd as HLOCAL);
            }
        }
        self.restored = true;
    }
}

impl Drop for AclBackup {
    fn drop(&mut self) {
        self.restore();
    }
}

struct AclGrantSet {
    grants: Vec<AclBackup>,
}

impl AclGrantSet {
    fn len(&self) -> usize {
        self.grants.len()
    }
}

impl Drop for AclGrantSet {
    fn drop(&mut self) {
        while let Some(mut grant) = self.grants.pop() {
            grant.restore();
        }
    }
}


fn grant_one_path(
    path: &Path,
    sid: PSID,
    rights: u32,
    inherit: bool,
) -> Result<AclBackup, AdapterError> {
    let path_wide = wide(&path.to_string_lossy());
    let mut original_dacl: *mut ACL = null_mut();
    let mut original_sd: PSECURITY_DESCRIPTOR = null_mut();

    let get = unsafe {
        GetNamedSecurityInfoW(
            path_wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut original_dacl,
            null_mut(),
            &mut original_sd,
        )
    };
    if get != 0 {
        return Err(AdapterError::new(
            "STABLE_SANDBOX_ACL_ERROR",
            format!("GetNamedSecurityInfoW failed for {}: {get}", path.display()),
        ));
    }

    let mut entry = EXPLICIT_ACCESS_W::default();
    entry.grfAccessPermissions = rights;
    entry.grfAccessMode = GRANT_ACCESS;
    entry.grfInheritance = if inherit {
        OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE
    } else {
        0
    };
    entry.Trustee.TrusteeForm = TRUSTEE_IS_SID;
    entry.Trustee.TrusteeType = TRUSTEE_IS_USER;
    entry.Trustee.ptstrName = sid as *mut u16;

    let mut new_dacl: *mut ACL = null_mut();
    let set_entries = unsafe {
        SetEntriesInAclW(
            1,
            &entry,
            original_dacl,
            &mut new_dacl,
        )
    };
    if set_entries != 0 {
        unsafe {
            LocalFree(original_sd as HLOCAL);
        }
        return Err(AdapterError::new(
            "STABLE_SANDBOX_ACL_ERROR",
            format!("SetEntriesInAclW failed for {}: {set_entries}", path.display()),
        ));
    }

    let apply = unsafe {
        SetNamedSecurityInfoW(
            path_wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            new_dacl,
            null_mut(),
        )
    };
    unsafe {
        if !new_dacl.is_null() {
            LocalFree(new_dacl as HLOCAL);
        }
    }
    if apply != 0 {
        unsafe {
            LocalFree(original_sd as HLOCAL);
        }
        return Err(AdapterError::new(
            "STABLE_SANDBOX_ACL_ERROR",
            format!("SetNamedSecurityInfoW failed for {}: {apply}", path.display()),
        ));
    }

    Ok(AclBackup {
        path: path_wide,
        original_sd,
        original_dacl,
        restored: false,
    })
}


fn collect_tree(path: &Path, output: &mut Vec<PathBuf>) -> Result<(), AdapterError> {
    output.push(path.to_path_buf());
    if !path.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(path).map_err(|error| {
        AdapterError::new(
            "STABLE_SANDBOX_ACL_ERROR",
            format!("read granted directory {}: {error}", path.display()),
        )
    })? {
        let entry = entry.map_err(|error| {
            AdapterError::new(
                "STABLE_SANDBOX_ACL_ERROR",
                format!("read granted directory entry: {error}"),
            )
        })?;
        collect_tree(&entry.path(), output)?;
    }
    Ok(())
}

fn grant_tree(
    root: &Path,
    sid: PSID,
    rights: u32,
) -> Result<AclGrantSet, AdapterError> {
    let mut paths = Vec::new();
    collect_tree(root, &mut paths)?;
    let mut grants = Vec::with_capacity(paths.len());

    for path in paths {
        let inherit = path.is_dir();
        match grant_one_path(&path, sid, rights, inherit) {
            Ok(grant) => grants.push(grant),
            Err(error) => {
                drop(AclGrantSet { grants });
                return Err(error);
            }
        }
    }

    Ok(AclGrantSet { grants })
}


struct ProcAttributes {
    storage: Vec<usize>,
    ptr: LPPROC_THREAD_ATTRIBUTE_LIST,
}

impl ProcAttributes {
    fn new(count: u32) -> Result<Self, AdapterError> {
        let mut bytes = 0usize;
        unsafe {
            let _ = InitializeProcThreadAttributeList(
                null_mut(),
                count,
                0,
                &mut bytes,
            );
        }
        if bytes == 0 || unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
            return Err(AdapterError::new(
                "STABLE_SANDBOX_ATTRIBUTE_ERROR",
                format!(
                    "InitializeProcThreadAttributeList(size) failed: {}",
                    unsafe { GetLastError() }
                ),
            ));
        }

        let words =
            (bytes + std::mem::size_of::<usize>() - 1) / std::mem::size_of::<usize>();
        let mut storage = vec![0usize; words];
        let ptr = storage.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
        let ok = unsafe {
            InitializeProcThreadAttributeList(
                ptr,
                count,
                0,
                &mut bytes,
            )
        };
        if ok == 0 {
            return Err(AdapterError::new(
                "STABLE_SANDBOX_ATTRIBUTE_ERROR",
                format!(
                    "InitializeProcThreadAttributeList(init) failed: {}",
                    unsafe { GetLastError() }
                ),
            ));
        }

        Ok(Self { storage, ptr })
    }

    fn add<T>(
        &mut self,
        attribute: usize,
        value: &T,
    ) -> Result<(), AdapterError> {
        let ok = unsafe {
            UpdateProcThreadAttribute(
                self.ptr,
                0,
                attribute,
                value as *const T as *const c_void,
                std::mem::size_of::<T>(),
                null_mut(),
                null(),
            )
        };
        if ok == 0 {
            return Err(AdapterError::new(
                "STABLE_SANDBOX_ATTRIBUTE_ERROR",
                format!(
                    "UpdateProcThreadAttribute({attribute:#x}) failed: {}",
                    unsafe { GetLastError() }
                ),
            ));
        }
        Ok(())
    }
}

impl Drop for ProcAttributes {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                DeleteProcThreadAttributeList(self.ptr);
            }
        }
        let _ = self.storage.len();
    }
}


fn validate_policy(policy: &StableSandboxPolicy) -> Result<(), AdapterError> {
    if !policy.capabilities.is_empty() {
        return Err(AdapterError::new(
            "STABLE_SANDBOX_DIRECT_CAPABILITY_UNSUPPORTED",
            "stable LPAC workers receive no direct capabilities; privileged egress must be brokered by RELAY",
        ));
    }
    if policy.identity.trim().is_empty() {
        return Err(AdapterError::new(
            "STABLE_SANDBOX_CONFIGURATION_INVALID",
            "AppContainer identity must be non-empty",
        ));
    }
    Ok(())
}

struct CapabilitySet {
    _owned: Vec<OwnedCapabilitySid>,
    attributes: Vec<SID_AND_ATTRIBUTES>,
}

fn prepare_capabilities(
    names: &[String],
) -> Result<CapabilitySet, AdapterError> {
    let mut owned = Vec::with_capacity(names.len());
    for name in names {
        owned.push(derive_capability_sid(name)?);
    }
    let attributes = owned
        .iter()
        .map(|sid| SID_AND_ATTRIBUTES {
            Sid: sid.0,
            Attributes: SE_GROUP_ENABLED as u32,
        })
        .collect();

    Ok(CapabilitySet {
        _owned: owned,
        attributes,
    })
}


pub fn run_sandboxed_worker(
    worker_path: &Path,
    mailbox: &Path,
    policy: &StableSandboxPolicy,
) -> Result<StableSandboxRunEvidence, AdapterError> {
    let overall_started = Instant::now();
    validate_policy(policy)?;
    if !stable_appcontainer_api_available() {
        return Err(AdapterError::new(
            "STABLE_SANDBOX_UNAVAILABLE",
            "required AppContainer profile APIs are unavailable",
        ));
    }
    if !worker_path.is_absolute() || !mailbox.is_absolute() {
        return Err(AdapterError::new(
            "STABLE_SANDBOX_CONFIGURATION_INVALID",
            "worker and mailbox paths must be absolute",
        ));
    }
    if policy.read_write_paths.len() != 1
        || policy.read_write_paths.first().map(PathBuf::as_path) != Some(mailbox)
    {
        return Err(AdapterError::new(
            "STABLE_SANDBOX_CONFIGURATION_INVALID",
            "stable untrusted workers receive exactly one read/write grant: the broker-owned mailbox",
        ));
    }

    let response_path = mailbox.join("response.json");
    fs::write(&response_path, b"").map_err(|error| {
        AdapterError::new(
            "STABLE_SANDBOX_CONFIGURATION_INVALID",
            format!("prepare response mailbox file: {error}"),
        )
    })?;

    let capability_set = prepare_capabilities(&policy.capabilities)?;
    let profile = create_profile(&policy.identity, &capability_set.attributes)?;

    let mut grant_sets = Vec::new();
    for path in &policy.read_write_paths {
        grant_sets.push(grant_tree(
            path,
            profile.sid,
            FILE_GENERIC_READ | FILE_GENERIC_WRITE,
        )?);
    }
    for path in &policy.read_only_paths {
        grant_sets.push(grant_tree(
            path,
            profile.sid,
            FILE_GENERIC_READ | FILE_GENERIC_EXECUTE,
        )?);
    }
    let grant_count: usize = grant_sets.iter().map(AclGrantSet::len).sum();

    let mut label_backups = Vec::new();
    for path in &policy.read_write_paths {
        label_backups.extend(apply_low_integrity_tree(path)?);
    }
    let label_count = label_backups.len();

    let security_capabilities = SECURITY_CAPABILITIES {
        AppContainerSid: profile.sid,
        Capabilities: if capability_set.attributes.is_empty() {
            null_mut()
        } else {
            capability_set.attributes.as_ptr() as *mut SID_AND_ATTRIBUTES
        },
        CapabilityCount: capability_set.attributes.len() as u32,
        Reserved: 0,
    };

    let attribute_count = if policy.disallow_win32k { 3 } else { 2 };
    let mut attributes = ProcAttributes::new(attribute_count)?;

    attributes.add(
        PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
        &security_capabilities,
    )?;
    let all_packages_policy = PROCESS_CREATION_ALL_APPLICATION_PACKAGES_OPT_OUT;
    attributes.add(
        PROC_THREAD_ATTRIBUTE_ALL_APPLICATION_PACKAGES_POLICY,
        &all_packages_policy,
    )?;
    let mitigation_policy = WIN32K_SYSTEM_CALL_DISABLE_ALWAYS_ON;
    if policy.disallow_win32k {
        attributes.add(
            PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY as usize,
            &mitigation_policy,
        )?;
    }

    let application = wide(&worker_path.to_string_lossy());
    let mut command_line = wide(&format!(
        "{} {}",
        quote_argument(worker_path),
        quote_argument(mailbox)
    ));
    let current_directory = wide(&mailbox.to_string_lossy());
    let environment = environment_block(&policy.environment);

    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    startup.lpAttributeList = attributes.ptr;
    let mut process = PROCESS_INFORMATION::default();

    let prelaunch_ms = overall_started.elapsed().as_secs_f64() * 1000.0;
    let started = Instant::now();
    let created = unsafe {
        CreateProcessW(
            application.as_ptr(),
            command_line.as_mut_ptr(),
            null(),
            null(),
            0,
            CREATE_UNICODE_ENVIRONMENT
                | CREATE_NO_WINDOW
                | CREATE_SUSPENDED
                | EXTENDED_STARTUPINFO_PRESENT,
            environment.as_ptr() as *const c_void,
            current_directory.as_ptr(),
            &startup.StartupInfo,
            &mut process,
        )
    };
    if created == 0 {
        return Err(AdapterError::new(
            "STABLE_SANDBOX_LAUNCH_FAILED",
            format!("CreateProcessW(AppContainer) failed: {}", unsafe {
                GetLastError()
            }),
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
        return Err(AdapterError::new(
            "STABLE_SANDBOX_LAUNCH_FAILED",
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
                    "STABLE_SANDBOX_RESPONSE_ERROR",
                    format!("read stable sandbox response: {error}"),
                )
            })?;
            if !bytes.is_empty() {
                match serde_json::from_slice::<Value>(&bytes) {
                    Ok(parsed) => break Ok(parsed),
                    Err(error) => {
                        break Err(AdapterError::new(
                            "STABLE_SANDBOX_RESPONSE_INVALID",
                            format!("worker response was not valid JSON: {error}"),
                        ));
                    }
                }
            }
        }

        let exit_ok = unsafe {
            GetExitCodeProcess(process.hProcess, &mut exit_code)
        };
        if exit_ok == 0 {
            break Err(AdapterError::new(
                "STABLE_SANDBOX_PROCESS_ERROR",
                format!("GetExitCodeProcess failed: {}", unsafe {
                    GetLastError()
                }),
            ));
        }
        if exit_code != 259 {
            break Err(AdapterError::new(
                "STABLE_SANDBOX_WORKER_EXITED",
                format!(
                    "stable AppContainer worker exited with code {exit_code} before response"
                ),
            ));
        }
        if Instant::now() >= deadline {
            unsafe {
                TerminateProcess(process.hProcess, 1);
            }
            break Err(AdapterError::new(
                "STABLE_SANDBOX_TIMEOUT",
                "stable AppContainer worker did not produce a response before timeout",
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

    let result = result?;
    drop(attributes);
    drop(label_backups);
    drop(grant_sets);
    drop(profile);
    drop(capability_set);
    let total_ms = overall_started.elapsed().as_secs_f64() * 1000.0;

    Ok(StableSandboxRunEvidence {
        backend: "legacy_appcontainer_lpac".to_string(),
        stable_api_available: true,
        low_privilege_appcontainer: true,
        temporary_acl_grant_count: grant_count,
        temporary_low_il_label_count: label_count,
        job_limits,
        prelaunch_ms,
        launch_ms,
        total_ms,
        process_exit_code: exit_code,
        worker_pid: process.dwProcessId,
        result,
    })
}


struct LabelBackup {
    path: Vec<u16>,
    original_sd: PSECURITY_DESCRIPTOR,
    original_sacl: *mut ACL,
    restored: bool,
}

impl LabelBackup {
    fn restore(&mut self) {
        if self.restored {
            return;
        }
        unsafe {
            let _ = SetNamedSecurityInfoW(
                self.path.as_ptr(),
                SE_FILE_OBJECT,
                LABEL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                null(),
                self.original_sacl,
            );
            if !self.original_sd.is_null() {
                LocalFree(self.original_sd as HLOCAL);
            }
        }
        self.restored = true;
    }
}

impl Drop for LabelBackup {
    fn drop(&mut self) {
        self.restore();
    }
}


fn apply_low_integrity_label(path: &Path) -> Result<LabelBackup, AdapterError> {
    let path_wide = wide(&path.to_string_lossy());
    let mut original_sacl: *mut ACL = null_mut();
    let mut original_sd: PSECURITY_DESCRIPTOR = null_mut();

    let get = unsafe {
        GetNamedSecurityInfoW(
            path_wide.as_ptr(),
            SE_FILE_OBJECT,
            LABEL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            null_mut(),
            &mut original_sacl,
            &mut original_sd,
        )
    };
    if get != 0 {
        return Err(AdapterError::new(
            "STABLE_SANDBOX_LABEL_ERROR",
            format!("read integrity label for {}: {get}", path.display()),
        ));
    }

    let sddl = wide("S:(ML;OICI;NW;;;LW)");
    let mut low_sd: PSECURITY_DESCRIPTOR = null_mut();
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut low_sd,
            null_mut(),
        )
    };
    if converted == 0 {
        unsafe {
            LocalFree(original_sd as HLOCAL);
        }
        return Err(AdapterError::new(
            "STABLE_SANDBOX_LABEL_ERROR",
            format!(
                "convert Low-IL descriptor failed: {}",
                unsafe { GetLastError() }
            ),
        ));
    }

    let mut present = 0;
    let mut defaulted = 0;
    let mut low_sacl: *mut ACL = null_mut();
    let sacl_ok = unsafe {
        GetSecurityDescriptorSacl(
            low_sd,
            &mut present,
            &mut low_sacl,
            &mut defaulted,
        )
    };
    if sacl_ok == 0 || present == 0 || low_sacl.is_null() {
        unsafe {
            LocalFree(low_sd as HLOCAL);
            LocalFree(original_sd as HLOCAL);
        }
        return Err(AdapterError::new(
            "STABLE_SANDBOX_LABEL_ERROR",
            "converted Low-IL descriptor did not contain a mandatory label",
        ));
    }

    let apply = unsafe {
        SetNamedSecurityInfoW(
            path_wide.as_ptr(),
            SE_FILE_OBJECT,
            LABEL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            null(),
            low_sacl,
        )
    };
    unsafe {
        LocalFree(low_sd as HLOCAL);
    }
    if apply != 0 {
        unsafe {
            LocalFree(original_sd as HLOCAL);
        }
        return Err(AdapterError::new(
            "STABLE_SANDBOX_LABEL_ERROR",
            format!("apply Low-IL label for {}: {apply}", path.display()),
        ));
    }

    Ok(LabelBackup {
        path: path_wide,
        original_sd,
        original_sacl,
        restored: false,
    })
}


fn apply_low_integrity_tree(root: &Path) -> Result<Vec<LabelBackup>, AdapterError> {
    let mut paths = Vec::new();
    collect_tree(root, &mut paths)?;
    let mut backups = Vec::with_capacity(paths.len());
    for path in paths {
        match apply_low_integrity_label(&path) {
            Ok(backup) => backups.push(backup),
            Err(error) => {
                drop(backups);
                return Err(error);
            }
        }
    }
    Ok(backups)
}


fn wide_ptr_to_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe {
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
    }
}

pub fn security_descriptor_sddl(path: &Path) -> Result<String, AdapterError> {
    let path_wide = wide(&path.to_string_lossy());
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    let information =
        DACL_SECURITY_INFORMATION | LABEL_SECURITY_INFORMATION;
    let status = unsafe {
        GetNamedSecurityInfoW(
            path_wide.as_ptr(),
            SE_FILE_OBJECT,
            information,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(AdapterError::new(
            "STABLE_SANDBOX_DESCRIPTOR_ERROR",
            format!("GetNamedSecurityInfoW failed for {}: {status}", path.display()),
        ));
    }

    let mut text: *mut u16 = null_mut();
    let mut length = 0u32;
    let converted = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            information,
            &mut text,
            &mut length,
        )
    };
    if converted == 0 {
        unsafe {
            LocalFree(descriptor as HLOCAL);
        }
        return Err(AdapterError::new(
            "STABLE_SANDBOX_DESCRIPTOR_ERROR",
            format!(
                "ConvertSecurityDescriptorToStringSecurityDescriptorW failed: {}",
                unsafe { GetLastError() }
            ),
        ));
    }

    let value = wide_ptr_to_string(text);
    unsafe {
        LocalFree(text as HLOCAL);
        LocalFree(descriptor as HLOCAL);
    }
    Ok(value)
}
