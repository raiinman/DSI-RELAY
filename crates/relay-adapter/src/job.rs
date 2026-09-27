use crate::AdapterError;
use serde::Serialize;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, QueryInformationJobObject,
    SetInformationJobObject, JobObjectExtendedLimitInformation,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PROCESS_MEMORY,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct JobLimitEvidence {
    pub active_process_limit: u32,
    pub process_memory_limit_bytes: usize,
    pub kill_on_close: bool,
}

pub struct JobGuard {
    handle: HANDLE,
    evidence: JobLimitEvidence,
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { CloseHandle(self.handle) };
        }
    }
}

impl JobGuard {
    pub fn create_for_handle(
        process_handle: HANDLE,
        max_process_memory_bytes: usize,
    ) -> Result<Self, AdapterError> {
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(AdapterError::new(
                    "ADAPTER_RESOURCE_LIMIT_ERROR",
                    format!("CreateJobObjectW failed: {}", GetLastError()),
                ));
            }

            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags =
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                    | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
                    | JOB_OBJECT_LIMIT_PROCESS_MEMORY;
            limits.BasicLimitInformation.ActiveProcessLimit = 1;
            limits.ProcessMemoryLimit = max_process_memory_bytes;

            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) == 0 {
                let error = GetLastError();
                CloseHandle(job);
                return Err(AdapterError::new(
                    "ADAPTER_RESOURCE_LIMIT_ERROR",
                    format!("SetInformationJobObject failed: {error}"),
                ));
            }

            if AssignProcessToJobObject(job, process_handle) == 0 {
                let error = GetLastError();
                CloseHandle(job);
                return Err(AdapterError::new(
                    "ADAPTER_RESOURCE_LIMIT_ERROR",
                    format!("AssignProcessToJobObject failed: {error}"),
                ));
            }

            let mut actual = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            if QueryInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &mut actual as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                std::ptr::null_mut(),
            ) == 0 {
                let error = GetLastError();
                CloseHandle(job);
                return Err(AdapterError::new(
                    "ADAPTER_RESOURCE_LIMIT_ERROR",
                    format!("QueryInformationJobObject failed: {error}"),
                ));
            }

            let flags = actual.BasicLimitInformation.LimitFlags;
            Ok(Self {
                handle: job,
                evidence: JobLimitEvidence {
                    active_process_limit:
                        actual.BasicLimitInformation.ActiveProcessLimit,
                    process_memory_limit_bytes: actual.ProcessMemoryLimit,
                    kill_on_close:
                        flags & JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE != 0,
                },
            })
        }
    }

    pub fn evidence(&self) -> JobLimitEvidence {
        self.evidence.clone()
    }
}
