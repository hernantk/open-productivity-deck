use crate::{
    audio::{self, AudioState},
    config::{self, DeckConfig, UnreadProvider},
    launcher,
    spotify::{self, SpotifyState},
    unread::UnreadCache,
};
use directories::ProjectDirs;
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    net::{IpAddr, Ipv4Addr},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};
use uuid::Uuid;

pub const DEFAULT_PORT: u16 = 37_621;
pub const DEFAULT_SECURE_PORT: u16 = 37_622;

#[derive(Clone)]
pub struct AppState {
    config: Arc<RwLock<DeckConfig>>,
    token: Arc<RwLock<String>>,
    pub local_address: String,
    pub port: u16,
    pub secure_port: u16,
    pub tls_dir: Arc<PathBuf>,
    config_path: Arc<PathBuf>,
    token_path: Arc<PathBuf>,
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
    pub secure_port: u16,
    pub unread: HashMap<String, Option<u32>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteState {
    pub title: String,
    pub buttons: Vec<RemoteButton>,
    pub audio: Option<AudioState>,
    pub spotify: SpotifyState,
    pub unread: HashMap<String, Option<u32>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteButton {
    pub id: Uuid,
    pub label: String,
    pub color: String,
    pub icon: Option<String>,
    pub unread_provider: Option<UnreadProvider>,
}

impl AppState {
    pub fn new() -> Self {
        let directories = ProjectDirs::from("org", "OpenProductivity", "Open Productivity Deck");
        let config_dir = directories
            .as_ref()
            .map(|directories| directories.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let tls_dir = directories
            .as_ref()
            .map(|directories| directories.data_local_dir().join("tls"))
            .unwrap_or_else(|| config_dir.join("tls"));
        let config_path = config_dir.join("deck.json");
        let token_path = config_dir.join("auth-token");
        let config = config::load(&config_path);
        let local_address = discover_local_ipv4().to_string();
        let token = load_or_create_token(&token_path);
        let (port, secure_port) = configured_ports();
        let unread = Arc::new(UnreadCache::default());
        Arc::clone(&unread).start();

        Self {
            config: Arc::new(RwLock::new(config)),
            token: Arc::new(RwLock::new(token)),
            local_address,
            port,
            secure_port,
            tls_dir: Arc::new(tls_dir),
            config_path: Arc::new(config_path),
            token_path: Arc::new(token_path),
            unread,
        }
    }

    pub fn dashboard(&self) -> DashboardState {
        DashboardState {
            config: self.config.read().expect("config lock poisoned").clone(),
            audio: audio::state().ok(),
            pairing_url: self.pairing_url(),
            local_address: self.local_address.clone(),
            port: self.port,
            secure_port: self.secure_port,
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
                    icon: button.icon.clone(),
                    unread_provider: button.unread_provider,
                })
                .collect(),
            audio: audio::state().ok(),
            spotify: spotify::state(),
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

    pub fn regenerate_pairing(&self) -> Result<String, String> {
        let token = Uuid::new_v4().to_string();
        persist_token(&self.token_path, &token)?;
        *self.token.write().map_err(|_| "A autenticação está bloqueada".to_string())? = token;
        Ok(self.pairing_url())
    }

    fn pairing_url(&self) -> String {
        let host = if self.local_address.contains(':') { format!("[{}]", self.local_address) } else { self.local_address.clone() };
        let token = self.token.read().map(|token| token.clone()).unwrap_or_default();
        format!("http://{host}:{}/setup?token={token}", self.port)
    }

    pub fn unread_counts(&self) -> HashMap<String, Option<u32>> {
        self.unread
            .snapshot()
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

    pub fn subscribe_unread(&self) -> tokio::sync::broadcast::Receiver<()> {
        self.unread.subscribe()
    }
}

fn discover_local_ipv4() -> Ipv4Addr {
    local_ip_address::list_afinet_netifas()
        .ok()
        .and_then(|interfaces| select_local_ipv4(&interfaces))
        .or_else(|| match local_ip_address::local_ip().ok() {
            Some(IpAddr::V4(address)) if is_usable(address) => Some(address),
            _ => None,
        })
        .unwrap_or(Ipv4Addr::LOCALHOST)
}

fn configured_ports() -> (u16, u16) {
    let port = std::env::var("OPEN_PRODUCTIVITY_DECK_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|port| *port >= 1024)
        .unwrap_or(DEFAULT_PORT);
    let secure_port = std::env::var("OPEN_PRODUCTIVITY_DECK_HTTPS_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|secure_port| *secure_port >= 1024 && *secure_port != port)
        .unwrap_or_else(|| if port == DEFAULT_PORT { DEFAULT_SECURE_PORT } else { port.saturating_add(1) });
    (port, secure_port)
}

fn load_or_create_token(path: &Path) -> String {
    if let Ok(token) = fs::read_to_string(path) {
        let token = token.trim();
        if Uuid::parse_str(token).is_ok() {
            return token.to_string();
        }
    }

    let token = Uuid::new_v4().to_string();
    let _ = persist_token(path, &token);
    token
}

fn persist_token(path: &Path, token: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("Não foi possível criar a pasta de autenticação: {error}"))?;
    }
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, token).map_err(|error| format!("Não foi possível salvar a autenticação: {error}"))?;
    fs::rename(&temporary, path).map_err(|error| format!("Não foi possível publicar a autenticação: {error}"))
}

fn select_local_ipv4(interfaces: &[(String, IpAddr)]) -> Option<Ipv4Addr> {
    let candidates: Vec<(&str, Ipv4Addr)> = interfaces
        .iter()
        .filter_map(|(name, address)| match address {
            IpAddr::V4(address) if is_usable(*address) => Some((name.as_str(), *address)),
            _ => None,
        })
        .collect();

    candidates
        .iter()
        .find(|(name, address)| address.is_private() && !is_virtual_adapter(name))
        .or_else(|| candidates.iter().find(|(_, address)| address.is_private()))
        .or_else(|| candidates.iter().find(|(name, _)| !is_virtual_adapter(name)))
        .or_else(|| candidates.first())
        .map(|(_, address)| *address)
}

fn is_usable(address: Ipv4Addr) -> bool {
    !address.is_loopback() && !address.is_link_local() && !address.is_unspecified() && !address.is_broadcast()
}

fn is_virtual_adapter(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "bluetooth",
        "docker",
        "fortinet",
        "hyper-v",
        "loopback",
        "openvpn",
        "radmin",
        "tap-",
        "tailscale",
        "tunnel",
        "vethernet",
        "virtual",
        "vmware",
        "vpn",
        "wintun",
        "wsl",
        "zerotier",
    ]
    .iter()
    .any(|marker| name.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_private_physical_network_over_vpn() {
        let interfaces = vec![
            ("Radmin VPN".into(), IpAddr::V4(Ipv4Addr::new(26, 207, 153, 147))),
            ("OpenVPN Wintun".into(), IpAddr::V4(Ipv4Addr::new(10, 8, 0, 2))),
            ("Ethernet".into(), IpAddr::V4(Ipv4Addr::new(192, 168, 3, 8))),
        ];

        assert_eq!(select_local_ipv4(&interfaces), Some(Ipv4Addr::new(192, 168, 3, 8)));
    }

    #[test]
    fn ignores_loopback_and_link_local_addresses() {
        let interfaces = vec![
            ("Loopback".into(), IpAddr::V4(Ipv4Addr::LOCALHOST)),
            ("Bluetooth".into(), IpAddr::V4(Ipv4Addr::new(169, 254, 67, 201))),
            ("Wi-Fi".into(), IpAddr::V4(Ipv4Addr::new(10, 0, 0, 24))),
        ];

        assert_eq!(select_local_ipv4(&interfaces), Some(Ipv4Addr::new(10, 0, 0, 24)));
    }

    #[test]
    fn reuses_persisted_authentication_token() {
        let directory = std::env::temp_dir().join(format!("opd-token-test-{}", Uuid::new_v4()));
        let path = directory.join("auth-token");
        let first = load_or_create_token(&path);
        let second = load_or_create_token(&path);

        assert_eq!(first, second);
        assert!(Uuid::parse_str(&first).is_ok());
        let _ = fs::remove_dir_all(directory);
    }
}
