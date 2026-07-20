use crate::config::{DeckButton, LaunchKind};
use regex::Regex;
use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

static URI_SCHEME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-zA-Z][a-zA-Z0-9+.-]*:").expect("valid URI regex"));

pub fn launch(button: &DeckButton) -> Result<(), String> {
    match button.kind {
        LaunchKind::Application => {
            let path = Path::new(&button.target);
            if !path.is_absolute() || !path.exists() {
                return Err(format!("O destino de ‘{}’ não foi encontrado", button.label));
            }
            let resolved = resolve_launch_path(path)?;
            if focus_existing(&resolved) {
                return Ok(());
            }
            open::that_detached(&button.target).map_err(|error| format!("Não foi possível abrir ‘{}’: {error}", button.label))
        }
        LaunchKind::Url => {
            if !URI_SCHEME.is_match(&button.target) {
                return Err(format!("O endereço de ‘{}’ não possui um protocolo válido", button.label));
            }
            // Protocolos de apps (msteams:, whatsapp:, etc.) costumam focar a instância existente.
            // URLs http(s) são entregues ao navegador, que decide entre aba nova e foco.
            open::that_detached(&button.target).map_err(|error| format!("Não foi possível abrir ‘{}’: {error}", button.label))
        }
    }
}

fn resolve_launch_path(path: &Path) -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        if path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("lnk")) {
            return resolve_shortcut(path);
        }
    }
    Ok(path.to_path_buf())
}

#[cfg(windows)]
fn resolve_shortcut(path: &Path) -> Result<PathBuf, String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let script = r#"
$ErrorActionPreference = 'Stop'
$shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($env:OPD_LNK_PATH)
if (-not $shortcut.TargetPath) { throw 'Atalho sem destino' }
[Console]::Out.Write($shortcut.TargetPath)
"#;
    let output = Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script])
        .env("OPD_LNK_PATH", path.as_os_str())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("Não foi possível ler o atalho: {error}"))?;
    if !output.status.success() {
        return Ok(path.to_path_buf());
    }
    let target = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if target.is_empty() {
        return Ok(path.to_path_buf());
    }
    Ok(PathBuf::from(target))
}

#[cfg(windows)]
fn focus_existing(path: &Path) -> bool {
    focus::try_focus(path)
}

#[cfg(not(windows))]
fn focus_existing(_path: &Path) -> bool {
    false
}

#[cfg(windows)]
mod focus {
    use std::{
        ffi::OsString,
        os::windows::ffi::OsStringExt,
        path::{Path, PathBuf},
    };
    use windows::{
        core::BOOL,
        Win32::{
            Foundation::{CloseHandle, HWND, LPARAM, MAX_PATH},
            System::Threading::{
                AttachThreadInput, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
            UI::WindowsAndMessaging::{
                BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindow, GetWindowThreadProcessId, IsIconic,
                IsWindowVisible, SetForegroundWindow, ShowWindow, GW_OWNER, SW_RESTORE,
            },
        },
    };

    struct Search<'a> {
        target: &'a Path,
        target_name: &'a std::ffi::OsStr,
        found: Option<HWND>,
    }

    pub fn try_focus(path: &Path) -> bool {
        let Some(name) = path.file_name() else {
            return false;
        };
        let target = normalize(path);
        let mut search = Search {
            target: &target,
            target_name: name,
            found: None,
        };

        unsafe {
            let _ = EnumWindows(Some(enum_callback), LPARAM(std::ptr::from_mut(&mut search) as isize));
        }

        search.found.is_some_and(activate)
    }

    unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = unsafe { &mut *(lparam.0 as *mut Search<'_>) };
        unsafe {
            if !IsWindowVisible(hwnd).as_bool() {
                return BOOL(1);
            }
            if GetWindow(hwnd, GW_OWNER).is_ok_and(|owner| !owner.0.is_null()) {
                return BOOL(1);
            }

            let mut process_id = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut process_id));
            if process_id == 0 {
                return BOOL(1);
            }

            let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) else {
                return BOOL(1);
            };
            let mut buffer = [0u16; MAX_PATH as usize];
            let mut size = buffer.len() as u32;
            let query = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, windows::core::PWSTR(buffer.as_mut_ptr()), &mut size);
            let _ = CloseHandle(process);
            if query.is_err() || size == 0 {
                return BOOL(1);
            }

            let image = PathBuf::from(OsString::from_wide(&buffer[..size as usize]));
            if paths_match(search.target, search.target_name, &image) {
                search.found = Some(hwnd);
                return BOOL(0);
            }
            BOOL(1)
        }
    }

    fn paths_match(target: &Path, target_name: &std::ffi::OsStr, image: &Path) -> bool {
        let image = normalize(image);
        let target = normalize(target);
        if image.as_os_str().eq_ignore_ascii_case(target.as_os_str()) {
            return true;
        }
        image.file_name().is_some_and(|name| name.eq_ignore_ascii_case(target_name))
    }

    fn normalize(path: &Path) -> PathBuf {
        path.components().collect::<PathBuf>()
    }

    fn activate(hwnd: HWND) -> bool {
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }

            let foreground = GetForegroundWindow();
            let current_thread = GetCurrentThreadId();
            let mut foreground_pid = 0u32;
            let foreground_thread = GetWindowThreadProcessId(foreground, Some(&mut foreground_pid));
            let mut target_pid = 0u32;
            let target_thread = GetWindowThreadProcessId(hwnd, Some(&mut target_pid));

            let attached_fg = foreground_thread != 0
                && foreground_thread != current_thread
                && AttachThreadInput(current_thread, foreground_thread, true).as_bool();
            let attached_tg =
                target_thread != 0 && target_thread != current_thread && AttachThreadInput(current_thread, target_thread, true).as_bool();

            let focused = SetForegroundWindow(hwnd).as_bool();
            let _ = BringWindowToTop(hwnd);

            if attached_tg {
                let _ = AttachThreadInput(current_thread, target_thread, false);
            }
            if attached_fg {
                let _ = AttachThreadInput(current_thread, foreground_thread, false);
            }

            focused || GetForegroundWindow() == hwnd
        }
    }
}
