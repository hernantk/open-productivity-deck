mod audio;
mod config;
mod launcher;
mod server;
mod state;
mod unread;

use audio::AudioState;
use config::DeckConfig;
use state::{AppState, DashboardState};

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
fn regenerate_pairing(state: tauri::State<'_, AppState>) -> String {
    state.regenerate_pairing()
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
            regenerate_pairing
        ])
        .setup(move |_app| {
            tauri::async_runtime::spawn(async move {
                if let Err(error) = server::run(server_state).await {
                    eprintln!("{error}");
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erro ao executar o Open Productivity Deck");
}
