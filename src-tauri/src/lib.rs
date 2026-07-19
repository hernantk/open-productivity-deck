mod audio;
mod config;
mod launcher;
mod server;
mod state;
mod unread;

use audio::AudioState;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use config::DeckConfig;
use std::{fs, path::Path};
use state::{AppState, DashboardState};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WindowEvent,
};

#[tauri::command]
fn get_dashboard_state(state: tauri::State<'_, AppState>) -> DashboardState {
    state.dashboard()
}

#[tauri::command]
fn save_config(config: DeckConfig, state: tauri::State<'_, AppState>) -> Result<DeckConfig, String> {
    state.save_config(config)
}

#[tauri::command]
fn set_output_volume(value: f32) -> Result<AudioState, String> {
    audio::set_volume(value)
}

#[tauri::command]
fn toggle_output_mute() -> Result<AudioState, String> {
    audio::toggle_mute()
}

#[tauri::command]
fn toggle_input_mute() -> Result<AudioState, String> {
    audio::toggle_input_mute()
}

#[tauri::command]
fn read_icon_data_url(path: String) -> Result<String, String> {
    let path = Path::new(&path);
    let mime = match path.extension().and_then(|extension| extension.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        _ => return Err("Escolha uma imagem PNG, JPEG, WebP ou SVG".into()),
    };
    let metadata = fs::metadata(path).map_err(|error| format!("Não foi possível ler o ícone: {error}"))?;
    if metadata.len() > 256 * 1024 {
        return Err("O ícone deve ter no máximo 256 KB".into());
    }
    let contents = fs::read(path).map_err(|error| format!("Não foi possível ler o ícone: {error}"))?;
    Ok(format!("data:{mime};base64,{}", BASE64.encode(contents)))
}

#[tauri::command]
fn regenerate_pairing(state: tauri::State<'_, AppState>) -> String {
    state.regenerate_pairing()
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::new();
    let server_state = state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_dashboard_state,
            save_config,
            set_output_volume,
            toggle_output_mute,
            toggle_input_mute,
            read_icon_data_url,
            regenerate_pairing
        ])
        .setup(move |app| {
            tauri::async_runtime::spawn(async move {
                if let Err(error) = server::run(server_state).await {
                    eprintln!("{error}");
                }
            });

            let open_item = MenuItem::with_id(app, "tray-open", "Abrir", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "tray-quit", "Sair", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_item, &quit_item])?;
            let icon = app.default_window_icon().expect("ícone padrão não configurado").clone();

            TrayIconBuilder::new()
                .icon(icon)
                .tooltip("Open Productivity Deck")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "tray-open" => show_main_window(app),
                    "tray-quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("erro ao executar o Open Productivity Deck");
}
