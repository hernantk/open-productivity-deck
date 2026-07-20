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
            let resolved = resolve_launch_target(path)?;
            if resolved.hosted_browser {
                if focus::try_toggle_hosted(button.id, &button.label, resolved.app_url.as_deref()) {
                    return Ok(());
                }
                let before = focus::browser_snapshot();
                open::that_detached(&button.target)
                    .map_err(|error| format!("Não foi possível abrir ‘{}’: {error}", button.label))?;
                focus::remember_website_window(
                    button.id,
                    resolved.app_url.as_deref().unwrap_or(""),
                    &button.label,
                    before,
                );
                return Ok(());
            }
            if focus::try_toggle(&resolved, &button.label) {
                return Ok(());
            }
            open::that_detached(&button.target).map_err(|error| format!("Não foi possível abrir ‘{}’: {error}", button.label))
        }
        LaunchKind::Url => {
            if !URI_SCHEME.is_match(&button.target) {
                return Err(format!("O endereço de ‘{}’ não possui um protocolo válido", button.label));
            }
            if focus::try_toggle_protocol(button.id, &button.target, &button.label) {
                return Ok(());
            }
            let lower = button.target.to_ascii_lowercase();
            let track_site = lower.starts_with("http://") || lower.starts_with("https://");
            let before = if track_site {
                focus::browser_snapshot()
            } else {
                Vec::new()
            };
            open::that_detached(&button.target).map_err(|error| format!("Não foi possível abrir ‘{}’: {error}", button.label))?;
            if track_site {
                focus::remember_website_window(button.id, &button.target, &button.label, before);
            }
            Ok(())
        }
    }
}

struct LaunchTarget {
    path: PathBuf,
    names: Vec<String>,
    aumid: Option<String>,
    /// PWA / app mode do Chrome/Edge (`chrome_proxy`, `--app-id`, etc.).
    hosted_browser: bool,
    app_url: Option<String>,
}

fn resolve_launch_target(path: &Path) -> Result<LaunchTarget, String> {
    let mut names = Vec::new();
    if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
        names.push(name.to_string());
    }
    if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
        names.push(format!("{stem}.exe"));
    }

    #[cfg(windows)]
    if path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("lnk")) {
        return resolve_shortcut(path, names);
    }

    Ok(LaunchTarget {
        path: path.to_path_buf(),
        names,
        aumid: None,
        hosted_browser: false,
        app_url: None,
    })
}

#[cfg(windows)]
fn resolve_shortcut(path: &Path, mut names: Vec<String>) -> Result<LaunchTarget, String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let script = r#"
$ErrorActionPreference = 'Stop'
$shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($env:OPD_LNK_PATH)
$target = [string]$shortcut.TargetPath
$args = [string]$shortcut.Arguments
$aumid = ''
$appUrl = ''
if ($args -match 'AppsFolder\\([^\\]+)') { $aumid = $Matches[1] }
if ($args -match '--app-id=([^\s]+)') { if (-not $aumid) { $aumid = $Matches[1] } }
if ($args -match '--app=([^\s]+)') { $appUrl = $Matches[1] }
elseif ($args -match '--app\s+([^\s]+)') { $appUrl = $Matches[1] }
Write-Output $target
Write-Output $args
Write-Output $aumid
Write-Output $appUrl
"#;
    let output = Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script])
        .env("OPD_LNK_PATH", path.as_os_str())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("Não foi possível ler o atalho: {error}"))?;

    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines().map(str::trim);
    let target = lines.next().unwrap_or_default();
    let args = lines.next().unwrap_or_default();
    let aumid = lines.next().unwrap_or_default();
    let app_url = lines.next().unwrap_or_default();

    let resolved = if target.is_empty() { path.to_path_buf() } else { PathBuf::from(target) };
    if let Some(name) = resolved.file_name().and_then(|name| name.to_str()) {
        push_unique(&mut names, name.to_string());
    }
    if let Some(stem) = resolved.file_stem().and_then(|stem| stem.to_str()) {
        push_unique(&mut names, format!("{stem}.exe"));
    }

    let exe_name = resolved
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let args_l = args.to_ascii_lowercase();
    let hosted_browser = exe_name.contains("chrome_proxy")
        || exe_name.contains("msedge_proxy")
        || args_l.contains("--app-id=")
        || args_l.contains("--app=")
        || args_l.contains("--app ");

    if hosted_browser {
        if exe_name.contains("edge") {
            push_unique(&mut names, "msedge.exe".into());
        } else {
            push_unique(&mut names, "chrome.exe".into());
            push_unique(&mut names, "msedge.exe".into());
        }
    }

    if !aumid.is_empty() {
        // Store apps: ms-teams.exe, WhatsApp.exe, etc. costumam aparecer no AUMID.
        for piece in aumid.split(['!', '_', '.']) {
            if piece.len() >= 3 && piece.chars().any(|c| c.is_ascii_alphabetic()) {
                push_unique(&mut names, format!("{piece}.exe"));
            }
        }
    }

    Ok(LaunchTarget {
        path: resolved,
        names,
        aumid: (!aumid.is_empty()).then(|| aumid.to_string()),
        hosted_browser,
        app_url: (!app_url.is_empty()).then(|| app_url.to_string()),
    })
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing.eq_ignore_ascii_case(&value)) {
        values.push(value);
    }
}

#[cfg(not(windows))]
mod focus {
    use super::LaunchTarget;
    use uuid::Uuid;

    pub fn try_toggle(_target: &LaunchTarget, _label: &str) -> bool {
        false
    }

    pub fn try_toggle_protocol(_id: Uuid, _url: &str, _label: &str) -> bool {
        false
    }

    pub fn try_toggle_hosted(_id: Uuid, _label: &str, _url: Option<&str>) -> bool {
        false
    }

    pub fn browser_snapshot() -> Vec<(isize, String)> {
        Vec::new()
    }

    pub fn remember_website_window(_id: Uuid, _url: &str, _label: &str, _before: Vec<(isize, String)>) {}
}

#[cfg(windows)]
mod focus {
    use super::LaunchTarget;
    use std::{
        collections::HashMap,
        ffi::OsString,
        os::windows::ffi::OsStringExt,
        path::{Path, PathBuf},
        sync::{LazyLock, Mutex},
        time::Duration,
    };
    use uuid::Uuid;
    use windows::{
        core::BOOL,
        Win32::{
            Foundation::{CloseHandle, HWND, LPARAM, MAX_PATH, RECT},
            System::Threading::{
                AttachThreadInput, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
            UI::{
                Input::KeyboardAndMouse::{
                    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
                    VK_MENU,
                },
                WindowsAndMessaging::{
                    BringWindowToTop, EnumWindows, GetClassNameW, GetForegroundWindow, GetWindow, GetWindowLongPtrW,
                    GetWindowPlacement, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
                    IsIconic, IsWindow, IsWindowVisible, SetForegroundWindow, SetWindowPlacement, ShowWindow, GWL_EXSTYLE,
                    GW_OWNER, SW_MINIMIZE, SW_RESTORE, SW_SHOW, SW_SHOWMAXIMIZED, SW_SHOWNORMAL, WINDOW_EX_STYLE,
                    WINDOWPLACEMENT, WPF_RESTORETOMAXIMIZED, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
                },
            },
        },
    };

    static SITE_HWNDS: LazyLock<Mutex<HashMap<Uuid, isize>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

    struct Candidate {
        hwnd: HWND,
        score: i32,
        iconic: bool,
    }

    struct Search<'a> {
        target: &'a Path,
        names: &'a [String],
        label: &'a str,
        aumid: Option<&'a str>,
        matches: Vec<Candidate>,
    }

    pub fn try_toggle(target: &LaunchTarget, label: &str) -> bool {
        let normalized = normalize(&target.path);
        let mut search = Search {
            target: &normalized,
            names: &target.names,
            label,
            aumid: target.aumid.as_deref(),
            matches: Vec::new(),
        };

        unsafe {
            let _ = EnumWindows(Some(enum_callback), LPARAM(std::ptr::from_mut(&mut search) as isize));
        }

        if search.matches.is_empty() {
            return false;
        }

        search.matches.sort_by(|a, b| b.score.cmp(&a.score));
        toggle_candidates(&search.matches)
    }

    pub fn try_toggle_protocol(id: Uuid, url: &str, label: &str) -> bool {
        let lower = url.to_ascii_lowercase();
        let mut names = Vec::new();
        if lower.starts_with("msteams:") || lower.starts_with("ms-teams:") {
            names.extend(["ms-teams.exe".into(), "teams.exe".into(), "msteams.exe".into()]);
        } else if lower.starts_with("whatsapp:") {
            names.push("whatsapp.exe".into());
        } else if lower.starts_with("spotify:") {
            names.push("spotify.exe".into());
        } else if lower.starts_with("discord:") {
            names.push("discord.exe".into());
        } else if lower.starts_with("slack:") {
            names.push("slack.exe".into());
        } else if lower.starts_with("http://") || lower.starts_with("https://") {
            return try_toggle_website(id, url, label);
        } else if let Some(scheme) = lower.split(':').next() {
            names.push(format!("{scheme}.exe"));
        }

        if names.is_empty() {
            return false;
        }

        try_toggle(
            &LaunchTarget {
                path: PathBuf::from(&names[0]),
                names,
                aumid: None,
                hosted_browser: false,
                app_url: None,
            },
            label,
        )
    }

    pub fn try_toggle_hosted(id: Uuid, label: &str, url: Option<&str>) -> bool {
        try_toggle_browser_window(id, label, url)
    }

    const BROWSER_EXES: &[&str] = &[
        "chrome.exe",
        "msedge.exe",
        "firefox.exe",
        "brave.exe",
        "opera.exe",
        "vivaldi.exe",
        "chromium.exe",
        "arc.exe",
        "waterfox.exe",
        "librewolf.exe",
        "thorium.exe",
        "iexplore.exe",
        "msedgewebview2.exe",
    ];

    const BROWSER_CLASSES: &[&str] = &["Chrome_WidgetWin_1", "Chrome_WidgetWin_0", "MozillaWindowClass"];

    struct WebsiteSearch<'a> {
        needles: &'a [String],
        label: &'a str,
        matches: Vec<Candidate>,
        snapshot: Option<&'a mut Vec<(isize, String)>>,
    }

    fn try_toggle_website(id: Uuid, url: &str, label: &str) -> bool {
        try_toggle_browser_window(id, label, Some(url))
    }

    fn try_toggle_browser_window(id: Uuid, label: &str, url: Option<&str>) -> bool {
        if let Some(hwnd) = cached_hwnd(id) {
            let iconic = unsafe { IsIconic(hwnd) }.as_bool();
            if toggle_candidates(&[Candidate {
                hwnd,
                score: 200,
                iconic,
            }]) {
                return true;
            }
            clear_cached_hwnd(id);
        }

        let needles = url
            .and_then(|value| url::Url::parse(value).ok())
            .and_then(|parsed| parsed.host_str().map(host_needles))
            .unwrap_or_default();
        if needles.is_empty() && label.trim().len() < 2 {
            return false;
        }

        let mut search = WebsiteSearch {
            needles: &needles,
            label,
            matches: Vec::new(),
            snapshot: None,
        };

        unsafe {
            let _ = EnumWindows(Some(website_enum_callback), LPARAM(std::ptr::from_mut(&mut search) as isize));
        }

        if search.matches.is_empty() {
            return false;
        }

        search.matches.sort_by(|a, b| b.score.cmp(&a.score));
        if toggle_candidates(&search.matches) {
            store_cached_hwnd(id, search.matches[0].hwnd);
            return true;
        }
        false
    }

    pub fn browser_snapshot() -> Vec<(isize, String)> {
        let mut windows = Vec::new();
        let mut search = WebsiteSearch {
            needles: &[],
            label: "",
            matches: Vec::new(),
            snapshot: Some(&mut windows),
        };
        unsafe {
            let _ = EnumWindows(Some(website_enum_callback), LPARAM(std::ptr::from_mut(&mut search) as isize));
        }
        windows
    }

    pub fn remember_website_window(id: Uuid, url: &str, label: &str, before: Vec<(isize, String)>) {
        let needles = url::Url::parse(url)
            .ok()
            .and_then(|parsed| parsed.host_str().map(host_needles))
            .unwrap_or_default();

        for attempt in 0..12 {
            std::thread::sleep(Duration::from_millis(100));

            if !needles.is_empty() || label.trim().len() >= 2 {
                let mut search = WebsiteSearch {
                    needles: &needles,
                    label,
                    matches: Vec::new(),
                    snapshot: None,
                };
                unsafe {
                    let _ = EnumWindows(Some(website_enum_callback), LPARAM(std::ptr::from_mut(&mut search) as isize));
                }
                if let Some(best) = search.matches.into_iter().max_by_key(|candidate| candidate.score) {
                    store_cached_hwnd(id, best.hwnd);
                    return;
                }
            }

            let after = browser_snapshot();
            if let Some(hwnd) = detect_changed_browser(&before, &after) {
                store_cached_hwnd(id, hwnd);
                return;
            }

            // Só no fim: a janela em foco pode ser o próprio Deck ou outro browser.
            if attempt >= 5 {
                let foreground = unsafe { GetForegroundWindow() };
                if !foreground.0.is_null() && is_browser_window(foreground) {
                    store_cached_hwnd(id, foreground);
                    return;
                }
            }
        }
    }

    fn detect_changed_browser(before: &[(isize, String)], after: &[(isize, String)]) -> Option<HWND> {
        for (hwnd, title) in after {
            if title.trim().is_empty() {
                continue;
            }
            match before.iter().find(|(id, _)| id == hwnd) {
                None => return Some(HWND(*hwnd as _)),
                Some((_, old_title)) if old_title != title => return Some(HWND(*hwnd as _)),
                Some(_) => {}
            }
        }
        None
    }

    fn cached_hwnd(id: Uuid) -> Option<HWND> {
        let value = SITE_HWNDS.lock().ok()?.get(&id).copied()?;
        let hwnd = HWND(value as _);
        if unsafe { IsWindow(Some(hwnd)) }.as_bool() && is_browser_window(hwnd) {
            Some(hwnd)
        } else {
            let _ = SITE_HWNDS.lock().map(|mut map| map.remove(&id));
            None
        }
    }

    fn store_cached_hwnd(id: Uuid, hwnd: HWND) {
        if hwnd.0.is_null() {
            return;
        }
        if let Ok(mut map) = SITE_HWNDS.lock() {
            map.insert(id, hwnd.0 as isize);
        }
    }

    fn clear_cached_hwnd(id: Uuid) {
        if let Ok(mut map) = SITE_HWNDS.lock() {
            map.remove(&id);
        }
    }

    fn host_needles(host: &str) -> Vec<String> {
        let bare = host.strip_prefix("www.").unwrap_or(host).to_ascii_lowercase();
        if bare.len() < 3 {
            return Vec::new();
        }

        let mut needles = vec![bare.clone()];
        let parts: Vec<&str> = bare.split('.').collect();
        if parts.len() >= 2 {
            let sld = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
            if sld != bare && sld.len() >= 3 {
                needles.push(sld);
            }
            let brand = parts[parts.len() - 2];
            if brand.len() >= 3 {
                needles.push(brand.to_string());
            }
        }
        needles
    }

    fn is_browser_window(hwnd: HWND) -> bool {
        if unsafe { !IsWindow(Some(hwnd)).as_bool() } {
            return false;
        }
        let class = window_class(hwnd);
        if BROWSER_CLASSES.iter().any(|name| class.eq_ignore_ascii_case(name)) {
            return true;
        }
        let mut process_id = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        }
        if process_id == 0 {
            return false;
        }
        process_image(process_id)
            .and_then(|image| image.file_name().and_then(|name| name.to_str().map(|s| s.to_string())))
            .is_some_and(|exe| BROWSER_EXES.iter().any(|name| name.eq_ignore_ascii_case(&exe)))
    }

    fn page_title(title: &str) -> String {
        let mut cleaned = title.to_ascii_lowercase();
        for suffix in [
            " - google chrome",
            " - microsoft edge",
            " - brave",
            " - opera",
            " - vivaldi",
            " - chromium",
            " — mozilla firefox",
            " - mozilla firefox",
            " - firefox",
        ] {
            if let Some(stripped) = cleaned.strip_suffix(suffix) {
                cleaned = stripped.to_string();
                break;
            }
        }
        cleaned
    }

    unsafe extern "system" fn website_enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = unsafe { &mut *(lparam.0 as *mut WebsiteSearch<'_>) };
        unsafe {
            let iconic = IsIconic(hwnd).as_bool();
            if !iconic && !IsWindowVisible(hwnd).as_bool() {
                return BOOL(1);
            }
            if GetWindow(hwnd, GW_OWNER).is_ok_and(|owner| !owner.0.is_null()) {
                return BOOL(1);
            }
            if is_tool_window(hwnd) {
                return BOOL(1);
            }
            if !is_browser_window(hwnd) {
                return BOOL(1);
            }

            let title = window_title(hwnd);
            if let Some(snapshot) = search.snapshot.as_mut() {
                snapshot.push((hwnd.0 as isize, title.clone()));
                return BOOL(1);
            }

            let title_l = page_title(&title);
            if title_l.trim().is_empty() {
                return BOOL(1);
            }

            let mut score = 0;
            let mut host_hit = false;
            for needle in search.needles {
                if title_l.contains(needle) {
                    host_hit = true;
                    score += if needle.contains('.') { 110 } else { 90 };
                    break;
                }
            }

            let label = search.label.trim();
            let mut label_hit = false;
            if label.len() >= 2 {
                let label_l = label.to_ascii_lowercase();
                if title_l == label_l {
                    label_hit = true;
                    score += 120;
                } else if title_l.starts_with(&label_l) || title_l.contains(&label_l) {
                    label_hit = true;
                    score += if title_l.starts_with(&label_l) { 95 } else { 80 };
                } else {
                    for word in label_l.split_whitespace().filter(|word| word.len() >= 3) {
                        if title_l.contains(word) {
                            label_hit = true;
                            score += 55;
                            break;
                        }
                    }
                }
            }

            if !host_hit && !label_hit {
                return BOOL(1);
            }

            let area = window_area(hwnd);
            if area >= 40_000 {
                score += 20;
            } else if area >= 10_000 {
                score += 10;
            } else if area < 2_000 && !iconic {
                score -= 30;
            }
            if GetForegroundWindow() == hwnd {
                score += 25;
            }

            if score < 55 {
                return BOOL(1);
            }

            search.matches.push(Candidate { hwnd, score, iconic });
            BOOL(1)
        }
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
            if is_tool_window(hwnd) {
                return BOOL(1);
            }

            let mut process_id = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut process_id));
            if process_id == 0 {
                return BOOL(1);
            }

            let Some(image) = process_image(process_id) else {
                return BOOL(1);
            };

            let title = window_title(hwnd);
            let mut score = match_score(search.target, search.names, search.aumid, &image, &title, search.label);
            if score <= 0 {
                return BOOL(1);
            }

            let iconic = IsIconic(hwnd).as_bool();
            let area = window_area(hwnd);
            if area >= 40_000 {
                score += 20;
            } else if area >= 10_000 {
                score += 10;
            } else if area < 2_000 && !iconic {
                score -= 30;
            }
            if !title.trim().is_empty() {
                score += 12;
            }
            if GetForegroundWindow() == hwnd {
                score += 25;
            }

            search.matches.push(Candidate { hwnd, score, iconic });
            BOOL(1)
        }
    }

    fn match_score(target: &Path, names: &[String], aumid: Option<&str>, image: &Path, title: &str, label: &str) -> i32 {
        let image_norm = normalize(image);
        let mut score = 0;

        if image_norm.as_os_str().eq_ignore_ascii_case(target.as_os_str()) {
            score += 120;
        }

        if let Some(image_name) = image.file_name().and_then(|name| name.to_str()) {
            if names.iter().any(|name| name.eq_ignore_ascii_case(image_name)) {
                score += 80;
            }
            let image_stem = Path::new(image_name).file_stem().and_then(|stem| stem.to_str()).unwrap_or_default();
            for name in names {
                let stem = Path::new(name).file_stem().and_then(|stem| stem.to_str()).unwrap_or(name.as_str());
                if !stem.is_empty() && image_stem.eq_ignore_ascii_case(stem) {
                    score += 70;
                }
                if image_name.to_ascii_lowercase().contains(&stem.to_ascii_lowercase()) && stem.len() >= 4 {
                    score += 40;
                }
            }
        }

        if let Some(aumid) = aumid {
            let image_l = image.to_string_lossy().to_ascii_lowercase();
            let aumid_l = aumid.to_ascii_lowercase();
            if image_l.contains("windowsapps") {
                for piece in aumid_l.split(['!', '_', '.']) {
                    if piece.len() >= 4 && image_l.contains(piece) {
                        score += 90;
                        break;
                    }
                }
            }
        }

        let label = label.trim();
        if label.len() >= 3 {
            let title_l = title.to_ascii_lowercase();
            let label_l = label.to_ascii_lowercase();
            if title_l.contains(&label_l) {
                score += 55;
            } else {
                for word in label_l.split_whitespace().filter(|word| word.len() >= 4) {
                    if title_l.contains(word) {
                        score += 35;
                        break;
                    }
                }
            }
        }

        score
    }

    fn toggle_candidates(candidates: &[Candidate]) -> bool {
        let threshold = candidates.first().map(|best| best.score.saturating_sub(40)).unwrap_or(0);
        let group: Vec<&Candidate> = candidates
            .iter()
            .filter(|candidate| candidate.score >= threshold && candidate.score >= 40)
            .collect();
        if group.is_empty() {
            return false;
        }

        let any_visible = group.iter().any(|candidate| !candidate.iconic);
        if any_visible {
            let mut minimized = false;
            for candidate in &group {
                if !candidate.iconic {
                    minimized |= minimize(candidate.hwnd);
                }
            }
            return minimized;
        }

        let mut restored = false;
        for candidate in &group {
            restored |= restore_window(candidate.hwnd);
        }
        force_foreground(group[0].hwnd);
        restored || !unsafe { IsIconic(group[0].hwnd) }.as_bool()
    }

    fn minimize(hwnd: HWND) -> bool {
        unsafe {
            let _ = ShowWindow(hwnd, SW_MINIMIZE);
            IsIconic(hwnd).as_bool()
        }
    }

    fn restore_window(hwnd: HWND) -> bool {
        unsafe {
            let mut placement = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            let maximize = if GetWindowPlacement(hwnd, &mut placement).is_ok() {
                placement.flags.contains(WPF_RESTORETOMAXIMIZED) || placement.showCmd == SW_SHOWMAXIMIZED.0 as u32
            } else {
                false
            };

            if IsIconic(hwnd).as_bool() {
                let command = if maximize { SW_SHOWMAXIMIZED } else { SW_RESTORE };
                let _ = ShowWindow(hwnd, command);

                if IsIconic(hwnd).as_bool() {
                    placement.showCmd = if maximize {
                        SW_SHOWMAXIMIZED.0 as u32
                    } else {
                        SW_SHOWNORMAL.0 as u32
                    };
                    let _ = SetWindowPlacement(hwnd, &placement);
                }
            } else {
                let _ = ShowWindow(hwnd, if maximize { SW_SHOWMAXIMIZED } else { SW_SHOW });
            }

            !IsIconic(hwnd).as_bool()
        }
    }

    fn force_foreground(hwnd: HWND) {
        unsafe {
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

            tap_alt();
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);

            if attached_tg {
                let _ = AttachThreadInput(current_thread, target_thread, false);
            }
            if attached_fg {
                let _ = AttachThreadInput(current_thread, foreground_thread, false);
            }
        }
    }

    fn tap_alt() {
        unsafe {
            let mut inputs = [
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(VK_MENU.0),
                            wScan: 0,
                            dwFlags: KEYBD_EVENT_FLAGS(0),
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(VK_MENU.0),
                            wScan: 0,
                            dwFlags: KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
            ];
            let _ = SendInput(&mut inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }

    fn process_image(process_id: u32) -> Option<PathBuf> {
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()?;
            let mut buffer = [0u16; MAX_PATH as usize * 4];
            let mut size = buffer.len() as u32;
            let query = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, windows::core::PWSTR(buffer.as_mut_ptr()), &mut size);
            let _ = CloseHandle(process);
            if query.is_err() || size == 0 {
                return None;
            }
            Some(PathBuf::from(OsString::from_wide(&buffer[..size as usize])))
        }
    }

    fn window_title(hwnd: HWND) -> String {
        unsafe {
            let length = GetWindowTextLengthW(hwnd);
            if length <= 0 {
                return String::new();
            }
            let mut buffer = vec![0u16; length as usize + 1];
            let read = GetWindowTextW(hwnd, &mut buffer);
            if read <= 0 {
                return String::new();
            }
            String::from_utf16_lossy(&buffer[..read as usize])
        }
    }

    fn window_class(hwnd: HWND) -> String {
        unsafe {
            let mut buffer = [0u16; 256];
            let read = GetClassNameW(hwnd, &mut buffer);
            if read <= 0 {
                return String::new();
            }
            String::from_utf16_lossy(&buffer[..read as usize])
        }
    }

    fn window_area(hwnd: HWND) -> i32 {
        unsafe {
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_err() {
                return 0;
            }
            (rect.right - rect.left).saturating_mul(rect.bottom - rect.top).max(0)
        }
    }

    fn is_tool_window(hwnd: HWND) -> bool {
        unsafe {
            let style = WINDOW_EX_STYLE(GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32);
            style.contains(WS_EX_TOOLWINDOW) && !style.contains(WS_EX_APPWINDOW)
        }
    }

    fn normalize(path: &Path) -> PathBuf {
        path.components().collect::<PathBuf>()
    }
}
