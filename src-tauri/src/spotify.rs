use serde::Serialize;
use windows::{
    core::{Result as WindowsResult},
    Media::Control::{
        GlobalSystemMediaTransportControlsSession,
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
    },
    Win32::{
        Foundation::RPC_E_CHANGED_MODE,
        System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
    },
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyState {
    pub available: bool,
    pub playing: bool,
    pub title: String,
    pub artist: String,
}

impl SpotifyState {
    fn unavailable() -> Self {
        Self {
            available: false,
            playing: false,
            title: "Spotify não está reproduzindo".into(),
            artist: "Abra o Spotify no computador".into(),
        }
    }
}

#[derive(Clone, Copy)]
pub enum SpotifyAction {
    Toggle,
    Next,
    Previous,
}

struct WinRtGuard {
    uninitialize: bool,
}

impl WinRtGuard {
    fn initialize() -> WindowsResult<Self> {
        match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
            Ok(()) => Ok(Self { uninitialize: true }),
            Err(error) if error.code() == RPC_E_CHANGED_MODE => Ok(Self { uninitialize: false }),
            Err(error) => Err(error),
        }
    }
}

impl Drop for WinRtGuard {
    fn drop(&mut self) {
        if self.uninitialize {
            unsafe { RoUninitialize() };
        }
    }
}

struct SpotifySession {
    session: GlobalSystemMediaTransportControlsSession,
    _guard: WinRtGuard,
}

impl SpotifySession {
    fn find() -> WindowsResult<Option<Self>> {
        let guard = WinRtGuard::initialize()?;
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.join()?;
        let sessions = manager.GetSessions()?;

        for index in 0..sessions.Size()? {
            let session = sessions.GetAt(index)?;
            let Ok(source_id) = session.SourceAppUserModelId() else {
                continue;
            };
            if is_spotify_source(&source_id.to_string()) {
                return Ok(Some(Self { session, _guard: guard }));
            }
        }
        Ok(None)
    }
}

pub fn state() -> SpotifyState {
    read_state().unwrap_or_else(|_| SpotifyState::unavailable())
}

pub fn control(action: SpotifyAction) -> Result<SpotifyState, String> {
    let Some(spotify) = SpotifySession::find().map_err(format_error)? else {
        return Err("Abra o Spotify e inicie uma música primeiro".into());
    };
    let accepted = match action {
        SpotifyAction::Toggle => spotify.session.TryTogglePlayPauseAsync(),
        SpotifyAction::Next => spotify.session.TrySkipNextAsync(),
        SpotifyAction::Previous => spotify.session.TrySkipPreviousAsync(),
    }
    .map_err(format_error)?
    .join()
    .map_err(format_error)?;

    if !accepted {
        return Err("O Spotify recusou este comando".into());
    }
    read_session_state(&spotify.session).map_err(format_error)
}

fn read_state() -> WindowsResult<SpotifyState> {
    let Some(spotify) = SpotifySession::find()? else {
        return Ok(SpotifyState::unavailable());
    };
    read_session_state(&spotify.session)
}

fn read_session_state(session: &GlobalSystemMediaTransportControlsSession) -> WindowsResult<SpotifyState> {
    let properties = session.TryGetMediaPropertiesAsync()?.join()?;
    let status = session.GetPlaybackInfo()?.PlaybackStatus()?;
    Ok(SpotifyState {
        available: true,
        playing: status == PlaybackStatus::Playing,
        title: properties.Title()?.to_string(),
        artist: properties.Artist()?.to_string(),
    })
}

fn is_spotify_source(source_id: &str) -> bool {
    source_id.to_ascii_lowercase().contains("spotify")
}

fn format_error(error: windows::core::Error) -> String {
    format!("Falha no controle de mídia do Windows ({}): {error}", error.code())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_store_and_desktop_spotify_sessions() {
        assert!(is_spotify_source("Spotify.exe"));
        assert!(is_spotify_source("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"));
        assert!(!is_spotify_source("Google.Chrome"));
    }
}
