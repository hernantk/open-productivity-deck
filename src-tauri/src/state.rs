use crate::{
    audio::{self, AudioState},
    config::{self, DeckConfig, UnreadProvider},
    launcher,
    unread::UnreadCache,
};
use directories::ProjectDirs;
use serde::Serialize;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock},
};
use uuid::Uuid;

pub const DEFAULT_PORT: u16 = 37_621;

#[derive(Clone)]
pub struct AppState {
    config: Arc<RwLock<DeckConfig>>,
    token: Arc<RwLock<String>>,
    pub local_address: String,
    pub port: u16,
    config_path: Arc<PathBuf>,
    unread: Arc<UnreadCache>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardState {
    pub config: DeckConfig,
    pub audio: Option<AudioState>,
    pub pairing_url: String,
    pub local_address: String,
    pub port: u16,
    pub unread: HashMap<String, Option<u32>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteState {
    pub title: String,
    pub buttons: Vec<RemoteButton>,
    pub audio: Option<AudioState>,
    pub unread: HashMap<String, Option<u32>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteButton {
    pub id: Uuid,
    pub label: String,
    pub color: String,
    pub unread_provider: Option<UnreadProvider>,
}

impl AppState {
    pub fn new() -> Self {
        let config_path = ProjectDirs::from("org", "OpenProductivity", "Open Productivity Deck")
            .map(|directories| directories.config_dir().join("deck.json"))
            .unwrap_or_else(|| PathBuf::from("deck.json"));
        let config = config::load(&config_path);
        let local_address = local_ip_address::local_ip()
            .map(|address| address.to_string())
            .unwrap_or_else(|_| "127.0.0.1".into());

        Self {
            config: Arc::new(RwLock::new(config)),
            token: Arc::new(RwLock::new(Uuid::new_v4().to_string())),
            local_address,
            port: DEFAULT_PORT,
            config_path: Arc::new(config_path),
            unread: Arc::new(UnreadCache::default()),
        }
    }

    pub fn dashboard(&self) -> DashboardState {
        DashboardState {
            config: self.config.read().expect("config lock poisoned").clone(),
            audio: audio::state().ok(),
            pairing_url: self.pairing_url(),
            local_address: self.local_address.clone(),
            port: self.port,
            unread: self.unread_counts(),
        }
    }

    pub fn remote(&self) -> RemoteState {
        let config = self.config.read().expect("config lock poisoned");
        RemoteState {
            title: config.title.clone(),
            buttons: config
                .buttons
                .iter()
                .map(|button| RemoteButton {
                    id: button.id,
                    label: button.label.clone(),
                    color: button.color.clone(),
                    unread_provider: button.unread_provider,
                })
                .collect(),
            audio: audio::state().ok(),
            unread: self.unread_counts(),
        }
    }

    pub fn save_config(&self, config: DeckConfig) -> Result<DeckConfig, String> {
        let config = config.validate()?;
        config::save(&self.config_path, &config)?;
        *self.config.write().map_err(|_| "A configuração está bloqueada".to_string())? = config.clone();
        Ok(config)
    }

    pub fn launch(&self, id: Uuid) -> Result<(), String> {
        let config = self.config.read().map_err(|_| "A configuração está bloqueada".to_string())?;
        let button = config.buttons.iter().find(|button| button.id == id).ok_or_else(|| "Ação desconhecida".to_string())?;
        launcher::launch(button)
    }

    pub fn authorize(&self, provided: &str) -> bool {
        self.token.read().map(|token| token.as_str() == provided).unwrap_or(false)
    }

    pub fn regenerate_pairing(&self) -> String {
        if let Ok(mut token) = self.token.write() {
            *token = Uuid::new_v4().to_string();
        }
        self.pairing_url()
    }

    fn pairing_url(&self) -> String {
        let host = if self.local_address.contains(':') { format!("[{}]", self.local_address) } else { self.local_address.clone() };
        let token = self.token.read().map(|token| token.clone()).unwrap_or_default();
        format!("http://{host}:{}?token={token}", self.port)
    }

    fn unread_counts(&self) -> HashMap<String, Option<u32>> {
        self.unread
            .counts()
            .into_iter()
            .map(|(provider, count)| {
                let name = match provider {
                    UnreadProvider::Teams => "teams",
                    UnreadProvider::Whatsapp => "whatsapp",
                };
                (name.into(), count)
            })
            .collect()
    }
}
