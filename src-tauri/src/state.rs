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
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
    sync::{Arc, RwLock},
    thread,
    time::Duration,
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
    pub icon: Option<String>,
    pub unread_provider: Option<UnreadProvider>,
}

impl AppState {
    pub fn new() -> Self {
        let config_path = ProjectDirs::from("org", "OpenProductivity", "Open Productivity Deck")
            .map(|directories| directories.config_dir().join("deck.json"))
            .unwrap_or_else(|| PathBuf::from("deck.json"));
        let config = config::load(&config_path);
        let local_address = discover_local_ipv4().to_string();
        let unread = Arc::new(UnreadCache::default());
        let unread_worker = Arc::clone(&unread);
        let _ = thread::Builder::new()
            .name("unread-counter".into())
            .spawn(move || loop {
                unread_worker.refresh();
                thread::sleep(Duration::from_secs(5));
            });

        Self {
            config: Arc::new(RwLock::new(config)),
            token: Arc::new(RwLock::new(Uuid::new_v4().to_string())),
            local_address,
            port: configured_port(),
            config_path: Arc::new(config_path),
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

fn configured_port() -> u16 {
    std::env::var("OPEN_PRODUCTIVITY_DECK_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|port| *port >= 1024)
        .unwrap_or(DEFAULT_PORT)
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
}
