use std::ffi::c_void;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, LocalFree, ERROR_INSUFFICIENT_BUFFER, GENERIC_ALL, HANDLE, HLOCAL};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW,
    ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SE_FILE_OBJECT,
    SE_KERNEL_OBJECT, SE_OBJECT_TYPE,
};
use windows_sys::Win32::Security::Cryptography::{
    BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
};
use windows_sys::Win32::Security::{
    GetAce, GetAclInformation, GetSecurityDescriptorControl, GetTokenInformation, IsValidSid,
    TokenUser, AclSizeInformation, ACCESS_ALLOWED_ACE, ACL, ACL_SIZE_INFORMATION,
    DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    SE_DACL_PROTECTED, TOKEN_QUERY, TOKEN_USER, SECURITY_ATTRIBUTES,
};
use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub struct PipeSecurity {
    pub attributes: SECURITY_ATTRIBUTES,
    descriptor: *mut c_void,
    pub sid: String,
}

unsafe impl Send for PipeSecurity {}
unsafe impl Sync for PipeSecurity {}

impl Drop for PipeSecurity {
    fn drop(&mut self) {
        if !self.descriptor.is_null() {
            unsafe {
                LocalFree(self.descriptor as HLOCAL);
            }
        }
    }
}

fn wide_string(ptr: *const u16) -> String {
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

pub fn current_user_sid_string() -> Result<String, String> {
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(format!("OpenProcessToken failed: {}", GetLastError()));
        }

        let mut needed = 0u32;
        let first = GetTokenInformation(token, TokenUser, null_mut(), 0, &mut needed);
        if first != 0 || GetLastError() != ERROR_INSUFFICIENT_BUFFER || needed == 0 {
            CloseHandle(token);
            return Err(format!("GetTokenInformation(size) failed: {}", GetLastError()));
        }

        let units = (needed as usize + std::mem::size_of::<usize>() - 1) / std::mem::size_of::<usize>();
        let mut buffer = vec![0usize; units];
        if GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr() as *mut c_void,
            needed,
            &mut needed,
        ) == 0
        {
            let error = GetLastError();
            CloseHandle(token);
            return Err(format!("GetTokenInformation(data) failed: {error}"));
        }

        let token_user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let mut sid_ptr: *mut u16 = null_mut();
        if ConvertSidToStringSidW(token_user.User.Sid, &mut sid_ptr) == 0 {
            let error = GetLastError();
            CloseHandle(token);
            return Err(format!("ConvertSidToStringSidW failed: {error}"));
        }

        let sid = wide_string(sid_ptr);
        LocalFree(sid_ptr as HLOCAL);
        CloseHandle(token);
        Ok(sid)
    }
}

pub fn current_user_pipe_security() -> Result<PipeSecurity, String> {
    let sid = current_user_sid_string()?;
    // An elevated process may otherwise give the pipe the Administrators group
    // as its owner. The host requires the account SID to own the pipe.
    let sddl = format!("O:{sid}D:P(A;;GA;;;{sid})");
    let mut wide: Vec<u16> = sddl.encode_utf16().collect();
    wide.push(0);

    unsafe {
        let mut descriptor: *mut c_void = null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide.as_ptr(),
            1,
            &mut descriptor,
            null_mut(),
        ) == 0
        {
            return Err(format!(
                "ConvertStringSecurityDescriptorToSecurityDescriptorW failed: {}",
                GetLastError()
            ));
        }

        Ok(PipeSecurity {
            attributes: SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor,
                bInheritHandle: 0,
            },
            descriptor,
            sid,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AclVerification {
    pub query_ok: bool,
    pub protected_dacl: bool,
    pub owner_is_current_user: bool,
    pub current_user_only: bool,
    pub current_user_full_control: bool,
    pub ace_count: u32,
}

pub fn verify_pipe_security(handle: HANDLE, expected_sid: &str) -> Result<AclVerification, String> {
    verify_object_security(handle, SE_KERNEL_OBJECT, expected_sid)
}

pub fn verify_file_security(handle: HANDLE, expected_sid: &str) -> Result<AclVerification, String> {
    verify_object_security(handle, SE_FILE_OBJECT, expected_sid)
}

fn verify_object_security(
    handle: HANDLE,
    object_type: SE_OBJECT_TYPE,
    expected_sid: &str,
) -> Result<AclVerification, String> {
    unsafe {
        let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
        let mut owner = null_mut();
        let mut dacl: *mut ACL = null_mut();
        let security_info = OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
        let status = GetSecurityInfo(
            handle,
            object_type,
            security_info,
            &mut owner,
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        );
        if status != 0 || descriptor.is_null() {
            return Err(format!("GetSecurityInfo failed: {status}"));
        }
        let checked = (|| -> Result<AclVerification, String> {
            let mut control = 0u16;
            let mut revision = 0u32;
            if GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) == 0 {
                return Err(format!("GetSecurityDescriptorControl failed: {}", GetLastError()));
            }
            let protected_dacl = control & SE_DACL_PROTECTED != 0;
            let owner_is_current_user = sid_matches(owner, expected_sid)?;
            if dacl.is_null() {
                return Ok(AclVerification {
                    query_ok: true,
                    protected_dacl,
                    owner_is_current_user,
                    current_user_only: false,
                    current_user_full_control: false,
                    ace_count: 0,
                });
            }
            let mut acl_info = ACL_SIZE_INFORMATION::default();
            if GetAclInformation(
                dacl,
                &mut acl_info as *mut _ as *mut c_void,
                std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            ) == 0 {
                return Err(format!("GetAclInformation failed: {}", GetLastError()));
            }
            let ace_count = acl_info.AceCount;
            let mut full_control = false;
            if ace_count == 1 {
                let mut ace_ptr: *mut c_void = null_mut();
                if GetAce(dacl, 0, &mut ace_ptr) == 0 {
                    return Err(format!("GetAce failed: {}", GetLastError()));
                }
                let ace = &*(ace_ptr as *const ACCESS_ALLOWED_ACE);
                if ace.Header.AceType == 0 &&
                    usize::from(ace.Header.AceSize) >= std::mem::size_of::<ACCESS_ALLOWED_ACE>() + 4 {
                    let ace_sid = &ace.SidStart as *const u32 as *mut c_void;
                    let sid_in_dacl = sid_matches(ace_sid, expected_sid)?;
                    full_control = sid_in_dacl &&
                        (ace.Mask & GENERIC_ALL != 0 || ace.Mask & FILE_ALL_ACCESS == FILE_ALL_ACCESS);
                }
            }
            Ok(AclVerification {
                query_ok: true,
                protected_dacl,
                owner_is_current_user,
                current_user_only: protected_dacl && ace_count == 1 && full_control,
                current_user_full_control: full_control,
                ace_count,
            })
        })();
        LocalFree(descriptor as HLOCAL);
        checked
    }
}

fn sid_matches(sid: *mut c_void, expected: &str) -> Result<bool, String> {
    if sid.is_null() || unsafe { IsValidSid(sid) } == 0 {
        return Ok(false);
    }
    let mut text_ptr: *mut u16 = null_mut();
    if unsafe { ConvertSidToStringSidW(sid, &mut text_ptr) } == 0 {
        return Err(format!("ConvertSidToStringSidW failed: {}", unsafe { GetLastError() }));
    }
    let actual = wide_string(text_ptr);
    unsafe { LocalFree(text_ptr as HLOCAL) };
    Ok(actual == expected)
}

pub fn random_hex(bytes: usize) -> Result<String, String> {
    let mut buffer = vec![0u8; bytes];
    let status = unsafe {
        BCryptGenRandom(
            null_mut(),
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status != 0 {
        return Err(format!("BCryptGenRandom failed: 0x{status:08x}"));
    }

    let mut output = String::with_capacity(bytes * 2);
    for byte in buffer {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_user_security_descriptor_builds() {
        let security = current_user_pipe_security().expect("security descriptor");
        assert!(security.sid.starts_with("S-1-"));
        assert!(!security.attributes.lpSecurityDescriptor.is_null());
    }

    #[test]
    fn random_token_is_hex() {
        let token = random_hex(32).expect("random token");
        assert_eq!(token.len(), 64);
        assert!(token.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn kernel_verifies_created_pipe_as_current_user_only() {
        let security = current_user_pipe_security().expect("security descriptor");
        let pipe_name = format!(
            r"\\.\pipe\relay-rust-security-test-{}-{}",
            std::process::id(),
            random_hex(4).expect("suffix")
        );
        let server = crate::pipe::create_server(&pipe_name, &security).expect("create pipe");
        let verification =
            verify_pipe_security(server.raw(), &security.sid).expect("kernel security query");
        assert!(verification.query_ok);
        assert!(verification.protected_dacl);
        assert!(verification.owner_is_current_user);
        assert!(verification.current_user_only);
        assert!(verification.current_user_full_control);
        assert_eq!(verification.ace_count, 1);
    }

    #[test]
    fn second_daemon_cannot_claim_the_same_pipe() {
        let security = current_user_pipe_security().expect("security descriptor");
        let pipe_name = format!(
            r"\\.\pipe\relay-rust-single-instance-test-{}-{}",
            std::process::id(),
            random_hex(4).expect("suffix")
        );
        let first = crate::pipe::create_server(&pipe_name, &security).expect("first pipe");
        assert!(crate::pipe::create_server(&pipe_name, &security).is_err());
        drop(first);
        assert!(crate::pipe::create_server(&pipe_name, &security).is_ok());
    }
}
