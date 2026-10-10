//! Local, per-user confirmation channel between the MCP process and the GUI.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalRequest {
    pub id: String,
    pub tool: String,
    pub message: String,
    pub client: Option<String>,
    pub expires_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignment: Option<AssignmentRequest>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssignmentRequest {
    pub repository_name: String,
    pub repository_path: String,
    pub profiles: Vec<AssignmentProfile>,
    pub profile_id: Option<String>,
    pub apply_defaults: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssignmentProfile {
    pub id: String,
    pub label: String,
    pub apply_defaults: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalResponse {
    pub id: String,
    pub approved: bool,
    #[serde(default, rename = "profileId", skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    #[serde(
        default,
        rename = "applyDefaults",
        skip_serializing_if = "Option::is_none"
    )]
    pub apply_defaults: Option<bool>,
}

impl ApprovalRequest {
    pub fn new(tool: &str, message: String, client: Option<String>, timeout: Duration) -> Self {
        let expiry = Utc::now() + chrono::Duration::from_std(timeout).unwrap_or_default();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            tool: tool.into(),
            message,
            client,
            expires_at: expiry.to_rfc3339(),
            kind: None,
            assignment: None,
        }
    }

    pub fn assignment(mut self, details: AssignmentRequest) -> Self {
        self.kind = Some("assignment".into());
        self.assignment = Some(details);
        self
    }

    fn valid(&self) -> bool {
        uuid::Uuid::parse_str(&self.id).is_ok()
            && !self.tool.is_empty()
            && !self.message.is_empty()
            && DateTime::parse_from_rfc3339(&self.expires_at).is_ok()
            && (self.kind.as_deref() != Some("assignment") || self.assignment.is_some())
    }

    pub fn remaining(&self) -> Duration {
        self.expires_at
            .parse::<DateTime<Utc>>()
            .ok()
            .and_then(|time| (time - Utc::now()).to_std().ok())
            .unwrap_or_default()
    }
}

fn parse_request(line: &[u8]) -> Option<ApprovalRequest> {
    let request: ApprovalRequest = serde_json::from_slice(line).ok()?;
    request.valid().then_some(request)
}

fn parse_response(line: &[u8], id: &str) -> ApprovalResponse {
    serde_json::from_slice::<ApprovalResponse>(line)
        .ok()
        .filter(|response| response.id == id)
        .unwrap_or_else(|| ApprovalResponse {
            id: id.into(),
            approved: false,
            profile_id: None,
            apply_defaults: None,
        })
}

pub fn expected_gui_path(mcp_exe: &Path) -> PathBuf {
    let parent = mcp_exe.parent().unwrap_or(Path::new("."));
    if cfg!(debug_assertions) {
        parent
            .ancestors()
            .find(|p| p.join("Cargo.toml").exists() && p.join("crates").exists())
            .unwrap_or(parent)
            .join("target")
            .join("debug")
            .join("git-context.exe")
    } else {
        parent.join("git-context.exe")
    }
}

pub fn image_matches(actual: &Path, expected: &Path) -> bool {
    // Compare component by component so "/" and "\\" and letter case do not matter.
    let normalize = |path: &Path| {
        path.components()
            .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
            .collect::<Vec<_>>()
    };
    normalize(actual) == normalize(expected)
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{os::windows::ffi::OsStrExt, ptr, sync::Arc, thread, time::Instant};
    use windows_sys::Win32::{
        Foundation::{
            CloseHandle, GetLastError, LocalFree, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED,
            GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
        },
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            },
            GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
        },
        Storage::FileSystem::{
            CreateFileW, FlushFileBuffers, ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE,
            OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
        },
        System::{
            Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe,
                GetNamedPipeServerProcessId, PeekNamedPipe, WaitNamedPipeW, PIPE_READMODE_BYTE,
                PIPE_TYPE_BYTE, PIPE_WAIT,
            },
            Threading::{
                GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    };

    // CloseHandle and LocalFree are paired with every successful allocation below.
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    fn wide(value: &str) -> Vec<u16> {
        std::ffi::OsStr::new(value)
            .encode_wide()
            .chain([0])
            .collect()
    }
    fn last_error(context: &str) -> String {
        format!("{context}: {}", std::io::Error::last_os_error())
    }

    pub fn pipe_name() -> Result<String, String> {
        let mut token = ptr::null_mut();
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(last_error("OpenProcessToken"));
        }
        let token = Handle(token);
        let mut length = 0;
        unsafe {
            GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut length);
        }
        if length == 0 {
            return Err(last_error("GetTokenInformation size"));
        }
        let mut buffer = vec![0u8; length as usize];
        if unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                length,
                &mut length,
            )
        } == 0
        {
            return Err(last_error("GetTokenInformation"));
        }
        let user = unsafe { buffer.as_ptr().cast::<TOKEN_USER>().read_unaligned() };
        let mut sid = ptr::null_mut();
        if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid) } == 0 {
            return Err(last_error("ConvertSidToStringSidW"));
        }
        let sid_text = unsafe {
            let mut len = 0;
            while *sid.add(len) != 0 {
                len += 1;
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(sid, len))
        };
        unsafe {
            LocalFree(sid.cast());
        }
        let prefix = if cfg!(debug_assertions) {
            "gitcontext-dev-approval"
        } else {
            "gitcontext-approval"
        };
        Ok(format!(r"\\.\pipe\{prefix}-{sid_text}"))
    }

    fn create_pipe(name: &str, first: bool) -> Result<Handle, String> {
        let sid = current_sid()?;
        let sddl = wide(&format!("O:{sid}D:P(A;;GA;;;{sid})"));
        let mut descriptor = ptr::null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut descriptor,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(last_error("Pipe security descriptor"));
        }
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let handle = unsafe {
            CreateNamedPipeW(
                wide(name).as_ptr(),
                PIPE_ACCESS_DUPLEX
                    | if first {
                        FILE_FLAG_FIRST_PIPE_INSTANCE
                    } else {
                        0
                    },
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                2,
                8192,
                8192,
                0,
                &attributes,
            )
        };
        unsafe {
            LocalFree(descriptor.cast());
        }
        if handle == INVALID_HANDLE_VALUE {
            Err(last_error("CreateNamedPipeW"))
        } else {
            Ok(Handle(handle))
        }
    }

    fn current_sid() -> Result<String, String> {
        pipe_name().map(|name| {
            name.split_once("-S-")
                .map(|(_, rest)| format!("S-{rest}"))
                .unwrap_or_default()
        })
    }

    fn write_line(handle: HANDLE, line: &[u8]) -> Result<(), String> {
        let mut written = 0;
        if unsafe {
            WriteFile(
                handle,
                line.as_ptr(),
                line.len() as u32,
                &mut written,
                ptr::null_mut(),
            )
        } == 0
            || written as usize != line.len()
        {
            return Err(last_error("Pipe write"));
        }
        Ok(())
    }

    fn read_line(handle: HANDLE, deadline: Instant) -> Result<Vec<u8>, String> {
        let mut output = Vec::new();
        while Instant::now() < deadline && output.len() < 8192 {
            let mut available = 0;
            if unsafe {
                PeekNamedPipe(
                    handle,
                    ptr::null_mut(),
                    0,
                    ptr::null_mut(),
                    &mut available,
                    ptr::null_mut(),
                )
            } == 0
            {
                return Err(last_error("Pipe disconnected"));
            }
            if available == 0 {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            let mut byte = 0;
            let mut read = 0;
            if unsafe { ReadFile(handle, &mut byte, 1, &mut read, ptr::null_mut()) } == 0
                || read != 1
            {
                return Err(last_error("Pipe read"));
            }
            if byte == b'\n' {
                return Ok(output);
            }
            output.push(byte);
        }
        Err("Approval response timed out or exceeded the limit.".into())
    }

    pub fn start_server_with_selection(
        name: String,
        callback: impl Fn(ApprovalRequest) -> ApprovalResponse + Send + Sync + 'static,
    ) -> Result<(), String> {
        let first = create_pipe(&name, true)?;
        let first_raw = first.0 as usize;
        std::mem::forget(first);
        let callback = Arc::new(callback);
        let spawned = thread::Builder::new()
            .name("gitcontext-approval".into())
            .spawn(move || {
                let mut pipe = Handle(first_raw as HANDLE);
                loop {
                    let connected = unsafe { ConnectNamedPipe(pipe.0, ptr::null_mut()) } != 0
                        || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
                    if connected {
                        if let Ok(line) = read_line(pipe.0, Instant::now() + Duration::from_secs(5))
                        {
                            if let Some(request) = parse_request(&line) {
                                let response = if request.remaining().is_zero() {
                                    ApprovalResponse {
                                        id: request.id.clone(),
                                        approved: false,
                                        profile_id: None,
                                        apply_defaults: None,
                                    }
                                } else {
                                    callback(request.clone())
                                };
                                if let Ok(mut bytes) = serde_json::to_vec(&response) {
                                    bytes.push(b'\n');
                                    if write_line(pipe.0, &bytes).is_ok() {
                                        // Wait until the client has read the reply; DisconnectNamedPipe
                                        // discards unread data.
                                        unsafe {
                                            FlushFileBuffers(pipe.0);
                                        }
                                    }
                                }
                            }
                        }
                        unsafe {
                            DisconnectNamedPipe(pipe.0);
                        }
                    }
                    match create_pipe(&name, false) {
                        Ok(next) => pipe = next,
                        Err(_) => break,
                    }
                }
            });
        if let Err(error) = spawned {
            unsafe {
                CloseHandle(first_raw as HANDLE);
            }
            return Err(error.to_string());
        }
        Ok(())
    }

    fn server_image(handle: HANDLE) -> Result<PathBuf, String> {
        let mut pid = 0;
        if unsafe { GetNamedPipeServerProcessId(handle, &mut pid) } == 0 {
            return Err(last_error("Pipe server PID"));
        }
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return Err(last_error("Open pipe server process"));
        }
        let process = Handle(process);
        let mut buffer = vec![0u16; 32768];
        let mut length = buffer.len() as u32;
        if unsafe { QueryFullProcessImageNameW(process.0, 0, buffer.as_mut_ptr(), &mut length) }
            == 0
        {
            return Err(last_error("Pipe server image"));
        }
        Ok(PathBuf::from(String::from_utf16_lossy(
            &buffer[..length as usize],
        )))
    }

    pub fn request_selection(
        name: &str,
        expected: &Path,
        approval: &ApprovalRequest,
        timeout: Duration,
    ) -> Result<ApprovalResponse, String> {
        let name_wide = wide(name);
        let deadline = Instant::now() + timeout;
        let handle = loop {
            let handle = unsafe {
                CreateFileW(
                    name_wide.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    0,
                    ptr::null(),
                    OPEN_EXISTING,
                    0,
                    ptr::null_mut(),
                )
            };
            if handle != INVALID_HANDLE_VALUE {
                break Handle(handle);
            }
            if unsafe { GetLastError() } != ERROR_PIPE_BUSY || Instant::now() >= deadline {
                return Err(last_error("GUI unavailable"));
            }
            unsafe {
                WaitNamedPipeW(name_wide.as_ptr(), 100);
            }
        };
        if !image_matches(&server_image(handle.0)?, expected) {
            return Err("GUI server image did not match GitContext.".into());
        }
        let mut bytes = serde_json::to_vec(approval).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        if write_line(handle.0, &bytes).is_err() {
            return Ok(ApprovalResponse {
                id: approval.id.clone(),
                approved: false,
                profile_id: None,
                apply_defaults: None,
            });
        }
        let response = match read_line(handle.0, deadline) {
            Ok(response) => response,
            Err(_) => {
                return Ok(ApprovalResponse {
                    id: approval.id.clone(),
                    approved: false,
                    profile_id: None,
                    apply_defaults: None,
                })
            }
        };
        Ok(parse_response(&response, &approval.id))
    }
}

#[cfg(windows)]
pub use windows::{pipe_name, request_selection, start_server_with_selection};

pub fn start_server(
    name: String,
    callback: impl Fn(ApprovalRequest) -> bool + Send + Sync + 'static,
) -> Result<(), String> {
    start_server_with_selection(name, move |request| ApprovalResponse {
        id: request.id.clone(),
        approved: callback(request),
        profile_id: None,
        apply_defaults: None,
    })
}

pub fn request(
    name: &str,
    expected: &Path,
    approval: &ApprovalRequest,
    timeout: Duration,
) -> Result<bool, String> {
    request_selection(name, expected, approval, timeout).map(|response| response.approved)
}

#[cfg(not(windows))]
pub fn pipe_name() -> Result<String, String> {
    Err("GUI confirmation is unavailable on this platform.".into())
}
#[cfg(not(windows))]
pub fn start_server_with_selection(
    _: String,
    _: impl Fn(ApprovalRequest) -> ApprovalResponse + Send + Sync + 'static,
) -> Result<(), String> {
    Err("GUI confirmation is unavailable on this platform.".into())
}
#[cfg(not(windows))]
pub fn request_selection(
    _: &str,
    _: &Path,
    _: &ApprovalRequest,
    _: Duration,
) -> Result<ApprovalResponse, String> {
    Err("GUI confirmation is unavailable on this platform.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn messages_reject_bad_ids_and_json() {
        let request = ApprovalRequest::new("push", "Confirm".into(), None, Duration::from_secs(2));
        assert!(parse_request(&serde_json::to_vec(&request).unwrap()).is_some());
        assert!(parse_request(b"{broken").is_none());
        assert!(!parse_response(br#"{"id":"wrong","approved":true}"#, &request.id).approved);
    }
    #[test]
    fn assignment_messages_keep_remote_wire_compatibility() {
        let remote = ApprovalRequest::new("push", "Confirm".into(), None, Duration::from_secs(2));
        let old_request: serde_json::Value = serde_json::from_str(&format!(
            r#"{{"id":"{}","tool":"push","message":"Confirm","client":null,"expiresAt":"{}"}}"#,
            remote.id, remote.expires_at
        ))
        .unwrap();
        assert!(parse_request(&serde_json::to_vec(&old_request).unwrap()).is_some());
        assert!(serde_json::to_value(&remote).unwrap().get("kind").is_none());
        assert!(
            parse_response(
                format!(r#"{{"id":"{}","approved":true}}"#, remote.id).as_bytes(),
                &remote.id
            )
            .approved
        );
        let assignment = remote.assignment(AssignmentRequest {
            repository_name: "sample".into(),
            repository_path: r"C:\sample".into(),
            profiles: vec![AssignmentProfile {
                id: "one".into(),
                label: "One (@example)".into(),
                apply_defaults: true,
            }],
            profile_id: Some("one".into()),
            apply_defaults: true,
        });
        let parsed = parse_request(&serde_json::to_vec(&assignment).unwrap()).unwrap();
        assert_eq!(parsed.kind.as_deref(), Some("assignment"));
        let response = parse_response(
            format!(
                r#"{{"id":"{}","approved":true,"profileId":"one","applyDefaults":false}}"#,
                assignment.id
            )
            .as_bytes(),
            &assignment.id,
        );
        assert_eq!(response.profile_id.as_deref(), Some("one"));
        assert_eq!(response.apply_defaults, Some(false));
    }
    #[test]
    fn image_comparison_ignores_case() {
        assert!(image_matches(
            Path::new(r"C:\App\Git-Context.exe"),
            Path::new(r"c:\app\git-context.EXE")
        ));
        assert!(!image_matches(
            Path::new(r"C:\Other\git-context.exe"),
            Path::new(r"c:\app\git-context.exe")
        ));
        // The development path is built with joins; mixed separators must still match.
        assert!(image_matches(
            Path::new(r"C:\Work\src-tauri\target\debug\git-context.exe"),
            Path::new("C:\\Work\\src-tauri\\target/debug/git-context.exe")
        ));
    }
    #[cfg(debug_assertions)]
    #[test]
    fn development_gui_path_uses_workspace_target() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let mcp = root.join("target/review/debug/gitcontext-mcp.exe");
        let expected = expected_gui_path(&mcp);
        assert!(expected.ends_with(Path::new("target/debug/git-context.exe")));
    }
    #[cfg(windows)]
    #[test]
    fn real_pipe_round_trip_spoof_and_timeout() {
        let pipe = format!(r"\\.\pipe\gitcontext-test-{}", uuid::Uuid::new_v4());
        start_server(pipe.clone(), |request| {
            if request.tool == "push" {
                std::thread::sleep(Duration::from_millis(1100));
            }
            request.tool == "push"
        })
        .unwrap();
        let exe = std::env::current_exe().unwrap();
        let wrong = exe.with_file_name("not-gitcontext.exe");
        let request_one =
            ApprovalRequest::new("push", "Check".into(), None, Duration::from_secs(3));
        let error = request(&pipe, &wrong, &request_one, Duration::from_secs(3)).unwrap_err();
        assert!(error.contains("image did not match"), "{error}");
        let request_two =
            ApprovalRequest::new("push", "Check".into(), None, Duration::from_secs(3));
        assert!(request(&pipe, &exe, &request_two, Duration::from_secs(3)).unwrap());
        let denied = ApprovalRequest::new("deny", "Check".into(), None, Duration::from_secs(3));
        assert!(!request(&pipe, &exe, &denied, Duration::from_secs(3)).unwrap());
        let request_three =
            ApprovalRequest::new("push", "Check".into(), None, Duration::from_millis(50));
        assert!(!request(&pipe, &exe, &request_three, Duration::from_millis(50)).unwrap());
    }
}
