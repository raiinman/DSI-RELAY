//! Explicit same-user Windows dialogs and allowlisted application launches.
//! No path or executable is accepted from the browser for launching.

use relay_contracts::CommandRequest;
use relay_core::service::{ExtensionError, RelayCore};
use serde_json::{Value, json};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::MetadataExt;
use std::os::windows::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use windows_sys::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows_sys::Win32::System::Threading::CREATE_NEW_CONSOLE;
use windows_sys::Win32::UI::Controls::Dialogs::{
    CommDlgExtendedError, GetOpenFileNameW, GetSaveFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST,
    OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows_sys::Win32::UI::Shell::{
    BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS, BROWSEINFOW, SHBrowseForFolderW, SHGetPathFromIDListW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

const REPARSE_POINT: u32 = 0x400;

pub fn execute(
    core: &RelayCore,
    request: &CommandRequest,
) -> Option<Result<Value, ExtensionError>> {
    match request.command.as_str() {
        "local.path.pick" => Some(pick(core, &request.arguments)),
        "local.app.launch" => Some(launch(&request.arguments)),
        _ => None,
    }
}

fn pick(core: &RelayCore, args: &Value) -> Result<Value, ExtensionError> {
    let kind = args["kind"].as_str().unwrap_or("");
    let file_type = match args["file_type"].as_str().unwrap_or("any") {
        "blend" => "blend",
        "kra" => "kra",
        "png" => "png",
        "json" => "json",
        _ => "any",
    };
    if kind == "project_folder" {
        if args.get("project_id").is_some() || args.get("file_type").is_some() {
            return Err(rejected());
        }
        return match run_dialog(DialogKind::Folder, None)? {
            Some(path) if path.is_dir() && path.is_absolute() && !is_reparse(&path) => {
                selected(&path)
            }
            Some(_) => Err(rejected()),
            None => Ok(cancelled()),
        };
    }

    let project_id = args["project_id"].as_str().ok_or_else(rejected)?;
    let root = core.trusted_project_root(project_id)?;
    let dialog = match kind {
        "project_file" => DialogKind::Open(file_type),
        "project_output_png" if args.get("file_type").is_none() => DialogKind::SavePng,
        _ => return Err(rejected()),
    };
    let Some(path) = run_dialog(dialog, Some(root.clone()))? else {
        return Ok(cancelled());
    };
    let relative = project_relative(&root, &path, kind == "project_output_png")?;
    if kind == "project_file" && !valid_extension(&path, file_type) {
        return Err(rejected());
    }
    if kind == "project_output_png" && !valid_extension(&path, "png") {
        return Err(rejected());
    }
    selected(&relative)
}

fn selected(path: &Path) -> Result<Value, ExtensionError> {
    let text = path.to_string_lossy();
    if text.is_empty() || text.len() > 2048 {
        return Err(rejected());
    }
    Ok(json!({"status":"selected","path": text}))
}

fn cancelled() -> Value {
    json!({"status":"cancelled","path":null})
}

fn rejected() -> ExtensionError {
    ExtensionError::new(
        "PICKER_PATH_REJECTED",
        "The selected item is outside the chosen project or is not a supported path",
    )
}

fn unavailable() -> ExtensionError {
    ExtensionError::new(
        "PICKER_UNAVAILABLE",
        "The Windows file picker could not open",
    )
}

fn valid_extension(path: &Path, expected: &str) -> bool {
    expected == "any"
        || path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case(expected))
}

fn is_reparse(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.file_attributes() & REPARSE_POINT != 0)
}

fn project_relative(root: &Path, selected: &Path, output: bool) -> Result<PathBuf, ExtensionError> {
    if !selected.is_absolute() || selected.as_os_str().len() > 2048 {
        return Err(rejected());
    }
    if output {
        match fs::symlink_metadata(selected) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(rejected()),
        }
    }
    if !output && !selected.is_file() {
        return Err(rejected());
    }
    let comparison = if output {
        selected.parent().ok_or_else(rejected)?
    } else {
        selected
    };
    let canonical_root = root.canonicalize().map_err(|_| rejected())?;
    let canonical_target = comparison.canonicalize().map_err(|_| rejected())?;
    let canonical_selected = if output {
        canonical_target.join(selected.file_name().ok_or_else(rejected)?)
    } else {
        canonical_target
    };
    let relative = canonical_selected
        .strip_prefix(&canonical_root)
        .map_err(|_| rejected())?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(rejected());
    }
    let mut path = canonical_root;
    for component in relative.components() {
        path.push(component);
        if path.exists() && is_reparse(&path) {
            return Err(rejected());
        }
    }
    Ok(relative.to_path_buf())
}

#[derive(Clone, Copy)]
enum DialogKind<'a> {
    Folder,
    Open(&'a str),
    SavePng,
}

fn run_dialog(
    kind: DialogKind<'static>,
    initial: Option<PathBuf>,
) -> Result<Option<PathBuf>, ExtensionError> {
    // The Windows shell's modern folder dialog requires an STA thread.
    std::thread::spawn(move || unsafe {
        let hr = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        if hr < 0 {
            return Err(unavailable());
        }
        let result = match kind {
            DialogKind::Folder => folder_dialog(),
            DialogKind::Open(filter) => file_dialog(filter, initial.as_deref(), false),
            DialogKind::SavePng => file_dialog("png", initial.as_deref(), true),
        };
        CoUninitialize();
        result
    })
    .join()
    .map_err(|_| unavailable())?
}

unsafe fn folder_dialog() -> Result<Option<PathBuf>, ExtensionError> {
    let mut display = [0u16; 260];
    let title: Vec<u16> = "Choose a project folder\0".encode_utf16().collect();
    let info = BROWSEINFOW {
        hwndOwner: unsafe { GetForegroundWindow() },
        pszDisplayName: display.as_mut_ptr(),
        lpszTitle: title.as_ptr(),
        ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
        ..Default::default()
    };
    let pidl = unsafe { SHBrowseForFolderW(&info) };
    if pidl.is_null() {
        return Ok(None);
    }
    let mut buffer = [0u16; 32768];
    let ok = unsafe { SHGetPathFromIDListW(pidl, buffer.as_mut_ptr()) != 0 };
    unsafe { CoTaskMemFree(pidl.cast()) };
    if !ok {
        return Err(unavailable());
    }
    Ok(Some(path_from_wide(&buffer)))
}

unsafe fn file_dialog(
    filter_type: &str,
    initial: Option<&Path>,
    save: bool,
) -> Result<Option<PathBuf>, ExtensionError> {
    let filter_text = match filter_type {
        "blend" => "Blender files (*.blend)\0*.blend\0\0",
        "kra" => "Krita files (*.kra)\0*.kra\0\0",
        "png" => "PNG images (*.png)\0*.png\0\0",
        "json" => "JSON files (*.json)\0*.json\0\0",
        _ => "All files (*.*)\0*.*\0\0",
    };
    let filter: Vec<u16> = filter_text.encode_utf16().collect();
    let initial_wide: Option<Vec<u16>> =
        initial.map(|path| path.as_os_str().encode_wide().chain(Some(0)).collect());
    let mut file = [0u16; 32768];
    let mut options = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: unsafe { GetForegroundWindow() },
        lpstrFilter: filter.as_ptr(),
        lpstrFile: file.as_mut_ptr(),
        nMaxFile: file.len() as u32,
        lpstrInitialDir: initial_wide
            .as_ref()
            .map_or(std::ptr::null(), |s| s.as_ptr()),
        Flags: OFN_EXPLORER | OFN_PATHMUSTEXIST | if save { 0 } else { OFN_FILEMUSTEXIST },
        ..Default::default()
    };
    let ok = if save {
        unsafe { GetSaveFileNameW(&mut options) }
    } else {
        unsafe { GetOpenFileNameW(&mut options) }
    };
    if ok == 0 {
        return if unsafe { CommDlgExtendedError() } == 0 {
            Ok(None)
        } else {
            Err(unavailable())
        };
    }
    Ok(Some(path_from_wide(&file)))
}

fn path_from_wide(chars: &[u16]) -> PathBuf {
    let end = chars.iter().position(|c| *c == 0).unwrap_or(chars.len());
    PathBuf::from(OsString::from_wide(&chars[..end]))
}

fn launch(args: &Value) -> Result<Value, ExtensionError> {
    let app = args["app"].as_str().unwrap_or("");
    let status = if app == "relay_terminal" {
        launch_terminal()?
    } else {
        let binary = match app {
            "uefn" => find_uefn(),
            "blender" => find_blender(),
            "krita" => find_krita(),
            _ => None,
        };
        if let Some(binary) = binary {
            let working_dir = binary.parent().ok_or_else(|| {
                ExtensionError::new("APP_LAUNCH_FAILED", "The installed app could not start")
            })?;
            Command::new(&binary)
                .current_dir(working_dir)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|_| {
                    ExtensionError::new("APP_LAUNCH_FAILED", "The installed app could not start")
                })?;
            "started"
        } else {
            "unavailable"
        }
    };
    Ok(json!({"app":app,"status":status}))
}

fn launch_terminal() -> Result<&'static str, ExtensionError> {
    let relay_cli = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.join("relay.exe")));
    let Some(relay_cli) = relay_cli.and_then(plain_executable) else {
        return Ok("unavailable");
    };
    let Some(system_root) = env::var_os("SystemRoot") else {
        return Ok("unavailable");
    };
    let powershell =
        PathBuf::from(system_root).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let Some(powershell) = plain_executable(powershell) else {
        return Ok("unavailable");
    };
    Command::new(powershell)
        .args(["-NoLogo", "-NoProfile", "-NoExit", "-Command", "function relay { & $env:RELAY_CLI @args }; Write-Host 'RELAY CLI is ready. Try: relay doctor, relay commands'; relay --help"])
        .env("RELAY_CLI", relay_cli)
        .creation_flags(CREATE_NEW_CONSOLE)
        .spawn()
        .map_err(|_| ExtensionError::new("APP_LAUNCH_FAILED", "The RELAY terminal could not start"))?;
    Ok("started")
}

fn common_roots() -> Vec<PathBuf> {
    [
        env::var_os("ProgramFiles"),
        env::var_os("ProgramFiles(x86)"),
    ]
    .into_iter()
    .flatten()
    .map(PathBuf::from)
    .collect()
}

fn plain_executable(path: PathBuf) -> Option<PathBuf> {
    (path.is_absolute() && path.is_file() && !path.ancestors().any(is_reparse)).then_some(path)
}

fn find_blender() -> Option<PathBuf> {
    for base in common_roots() {
        let root = base.join("Blender Foundation");
        if let Some(path) = plain_executable(root.join("Blender/blender.exe")) {
            return Some(path);
        }
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.take(32).flatten() {
                if entry.file_name().to_string_lossy().starts_with("Blender") {
                    if let Some(path) = plain_executable(entry.path().join("blender.exe")) {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

fn find_krita() -> Option<PathBuf> {
    for base in common_roots() {
        for relative in ["Krita (x64)/bin/krita.exe", "Krita/bin/krita.exe"] {
            if let Some(path) = plain_executable(base.join(relative)) {
                return Some(path);
            }
        }
    }
    None
}

fn find_uefn() -> Option<PathBuf> {
    for base in common_roots() {
        if let Some(path) = plain_executable(base.join("Epic Games/Fortnite/FortniteGame/Binaries/Win64/UnrealEditorFortnite-Win64-Shipping.exe")) {
            return Some(path);
        }
    }
    let manifest_dir =
        PathBuf::from(env::var_os("ProgramData")?).join("Epic/EpicGamesLauncher/Data/Manifests");
    let entries = fs::read_dir(manifest_dir).ok()?;
    for entry in entries.take(64).flatten() {
        if entry.path().extension().and_then(|e| e.to_str()) != Some("item") {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if !metadata.is_file() || metadata.len() > 1024 * 1024 || is_reparse(&entry.path()) {
            continue;
        }
        let Ok(bytes) = fs::read(entry.path()) else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        let named_uefn = value["DisplayName"].as_str() == Some("Unreal Editor for Fortnite")
            || value["AppName"]
                .as_str()
                .is_some_and(|name| name.to_ascii_lowercase().contains("uefn"));
        if !named_uefn {
            continue;
        }
        let Some(location) = value["InstallLocation"].as_str() else {
            continue;
        };
        if let Some(path) = plain_executable(
            PathBuf::from(location)
                .join("FortniteGame/Binaries/Win64/UnrealEditorFortnite-Win64-Shipping.exe"),
        ) {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_schemas_are_valid() {
        for (name, result) in [
            ("local.path.pick", json!({"status":"cancelled","path":null})),
            (
                "local.app.launch",
                json!({"app":"krita","status":"unavailable"}),
            ),
        ] {
            let spec = relay_contracts::registry::resolve_command(name, Some(1)).unwrap();
            relay_contracts::registry::validate_value(&spec.result_schema, &result).unwrap();
        }
    }

    #[test]
    fn extension_check_is_exact() {
        assert!(valid_extension(Path::new("x.KRA"), "kra"));
        assert!(!valid_extension(Path::new("x.exe"), "kra"));
    }

    #[test]
    fn picked_files_remain_inside_the_selected_project() {
        let root = env::temp_dir().join(format!("relay-picker-test-{}", std::process::id()));
        let project = root.join("project");
        let elsewhere = root.join("elsewhere");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&elsewhere).unwrap();
        fs::write(project.join("asset.kra"), b"fixture").unwrap();
        fs::write(elsewhere.join("asset.kra"), b"fixture").unwrap();
        assert_eq!(
            project_relative(&project, &project.join("asset.kra"), false).ok(),
            Some(PathBuf::from("asset.kra"))
        );
        assert!(project_relative(&project, &elsewhere.join("asset.kra"), false).is_err());
        assert_eq!(
            project_relative(&project, &project.join("new.png"), true).ok(),
            Some(PathBuf::from("new.png"))
        );
        assert!(project_relative(&project, &project.join("asset.kra"), true).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
