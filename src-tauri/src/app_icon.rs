#[cfg(target_os = "windows")]
use std::{os::windows::process::CommandExt, process::Command};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(target_os = "windows")]
const EXTRACT_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$original = $env:OPD_ICON_PATH
$source = $original

if ([IO.Path]::GetExtension($original) -ieq '.lnk') {
    try {
        $shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($original)
        if ($shortcut.IconLocation) {
            $candidate = $shortcut.IconLocation -replace ',\s*-?\d+\s*$', ''
            $candidate = [Environment]::ExpandEnvironmentVariables($candidate)
            if (Test-Path -LiteralPath $candidate) { $source = $candidate }
        }
        if ($source -eq $original -and $shortcut.TargetPath -and (Test-Path -LiteralPath $shortcut.TargetPath)) {
            $source = $shortcut.TargetPath
        }
    } catch {}
}

$icon = [Drawing.Icon]::ExtractAssociatedIcon($source)
if ($null -eq $icon) { throw 'Ícone não encontrado' }
$bitmap = $icon.ToBitmap()
$stream = New-Object IO.MemoryStream
try {
    $bitmap.Save($stream, [Drawing.Imaging.ImageFormat]::Png)
    [Console]::Out.Write([Convert]::ToBase64String($stream.ToArray()))
} finally {
    $stream.Dispose()
    $bitmap.Dispose()
    $icon.Dispose()
}
"#;

#[cfg(target_os = "windows")]
const PACKAGE_LOGO_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$package = Get-AppxPackage -Name $env:OPD_PACKAGE_NAME | Sort-Object Version -Descending | Select-Object -First 1
if ($null -eq $package) { throw 'Pacote não instalado' }
$asset = Join-Path $package.InstallLocation $env:OPD_LOGO_PATH
if (-not (Test-Path -LiteralPath $asset)) { throw 'Logo não encontrado no pacote' }
[Console]::Out.Write([Convert]::ToBase64String([IO.File]::ReadAllBytes($asset)))
"#;

#[cfg(target_os = "windows")]
pub fn extract(path: &str) -> Result<String, String> {
    if !std::path::Path::new(path).exists() {
        return Err("O aplicativo selecionado não foi encontrado".into());
    }

    let output = Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", EXTRACT_SCRIPT])
        .env("OPD_ICON_PATH", path)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("Não foi possível iniciar a extração do ícone: {error}"))?;

    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim().chars().take(240).collect::<String>();
        return Err(if detail.is_empty() { "O Windows não forneceu um ícone para este aplicativo".into() } else { detail });
    }

    let encoded = String::from_utf8(output.stdout).map_err(|_| "O ícone retornado pelo Windows é inválido".to_string())?;
    let encoded = encoded.trim();
    if encoded.is_empty() || encoded.len() > 350_000 {
        return Err("O ícone extraído é inválido ou muito grande".into());
    }
    Ok(format!("data:image/png;base64,{encoded}"))
}

#[cfg(target_os = "windows")]
pub fn extract_packaged_logo(package_name: &str, relative_path: &str) -> Result<String, String> {
    let output = Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", PACKAGE_LOGO_SCRIPT])
        .env("OPD_PACKAGE_NAME", package_name)
        .env("OPD_LOGO_PATH", relative_path)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("Não foi possível localizar o logo do pacote: {error}"))?;

    if !output.status.success() {
        return Err("O logo do aplicativo não foi encontrado".into());
    }
    let encoded = String::from_utf8(output.stdout).map_err(|_| "O logo retornado pelo Windows é inválido".to_string())?;
    let encoded = encoded.trim();
    if encoded.is_empty() || encoded.len() > 350_000 {
        return Err("O logo importado é inválido ou muito grande".into());
    }
    Ok(format!("data:image/png;base64,{encoded}"))
}

#[cfg(not(target_os = "windows"))]
pub fn extract(_path: &str) -> Result<String, String> {
    Err("A extração automática de ícones está disponível somente no Windows".into())
}

#[cfg(not(target_os = "windows"))]
pub fn extract_packaged_logo(_package_name: &str, _relative_path: &str) -> Result<String, String> {
    Err("A importação de logos está disponível somente no Windows".into())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn extracts_an_icon_from_a_windows_executable() {
        let executable = std::path::Path::new(r"C:\Windows\System32\notepad.exe");
        if executable.exists() {
            let icon = extract(executable.to_str().unwrap()).unwrap();
            assert!(icon.starts_with("data:image/png;base64,"));
            assert!(icon.len() > 100);
        }
    }

    #[test]
    fn extracts_collaboration_app_logos_when_installed() {
        let packages = [
            ("MSTeams", r"Images\TeamsForWorkNewAppList.targetsize-256_altform-unplated.png"),
            ("5319275A.WhatsAppDesktop", r"Assets\AppList.targetsize-256_altform-unplated.png"),
        ];
        for (package, asset) in packages {
            if let Ok(icon) = extract_packaged_logo(package, asset) {
                assert!(icon.starts_with("data:image/png;base64,"));
                assert!(icon.len() > 100);
            }
        }
    }
}
