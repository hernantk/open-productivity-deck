use crate::{
    app_icon,
    audio::{self, AudioState},
    config::{self, DeckConfig, UnreadProvider},
    launcher,
    service::{self, ServiceSettings},
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
    pub device_id: String,
    pub device_name: String,
    pub local_address: String,
    pub port: u16,
    pub secure_port: u16,
    pub tls_dir: Arc<PathBuf>,
    config_path: Arc<PathBuf>,
    service_path: Arc<PathBuf>,
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
    pub service_settings: ServiceSettings,
    pub device_id: String,
    pub device_name: String,
    pub unread: HashMap<String, Option<u32>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteState {
    pub device_id: String,
    pub device_name: String,
    pub title: String,
    pub theme: crate::config::ThemeMode,
    pub grid_size: u8,
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
    pub show_label: bool,
    pub transparent_background: bool,
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
        let service_path = config_dir.join("service.json");
        let token_path = config_dir.join("auth-token");
        let device_id_path = config_dir.join("device-id");
        let mut config = config::load(&config_path);
        migrate_provider_logos(&mut config, &config_path);
        let local_address = discover_local_ipv4().to_string();
        let token = load_or_create_token(&token_path);
        let device_id = load_or_create_token(&device_id_path);
        let device_name = std::env::var("COMPUTERNAME").ok().filter(|name| !name.trim().is_empty()).unwrap_or_else(|| "Computador Windows".into());
        let ports = service::resolve(&service_path);
        let unread = Arc::new(UnreadCache::default());
        Arc::clone(&unread).start();

        Self {
            config: Arc::new(RwLock::new(config)),
            token: Arc::new(RwLock::new(token)),
            device_id,
            device_name,
            local_address,
            port: ports.port,
            secure_port: ports.secure_port,
            tls_dir: Arc::new(tls_dir),
            config_path: Arc::new(config_path),
            service_path: Arc::new(service_path),
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
            service_settings: service::load(&self.service_path),
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
            unread: self.unread_counts(),
        }
    }

    pub fn remote(&self) -> RemoteState {
        let config = self.config.read().expect("config lock poisoned");
        RemoteState {
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
            title: config.title.clone(),
            theme: config.theme,
            grid_size: config.grid_size,
            buttons: config
                .buttons
                .iter()
                .map(|button| RemoteButton {
                    id: button.id,
                    label: button.label.clone(),
                    color: button.color.clone(),
                    icon: button.icon.clone(),
                    show_label: button.show_label,
                    transparent_background: button.transparent_background,
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

    pub fn save_service_settings(&self, settings: ServiceSettings) -> Result<ServiceSettings, String> {
        let settings = settings.validate()?;
        service::save(&self.service_path, &settings)?;
        Ok(settings)
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

fn migrate_provider_logos(config: &mut DeckConfig, config_path: &Path) {
    const LOGO_MIGRATION_VERSION: u32 = 2;
    if config.version >= LOGO_MIGRATION_VERSION {
        return;
    }

    let needs_teams = config.buttons.iter().any(|button| button.icon.is_none() && button.unread_provider == Some(UnreadProvider::Teams));
    let needs_whatsapp = config.buttons.iter().any(|button| button.icon.is_none() && button.unread_provider == Some(UnreadProvider::Whatsapp));
    let teams = needs_teams
        .then(|| app_icon::extract_packaged_logo("MSTeams", r"Images\TeamsForWorkNewAppList.targetsize-256_altform-unplated.png").ok())
        .flatten();
    let whatsapp = needs_whatsapp
        .then(|| app_icon::extract_packaged_logo("5319275A.WhatsAppDesktop", r"Assets\AppList.targetsize-256_altform-unplated.png").ok())
        .flatten();

    for button in &mut config.buttons {
        if button.icon.is_some() {
            continue;
        }
        button.icon = match button.unread_provider {
            Some(UnreadProvider::Teams) => teams.clone(),
            Some(UnreadProvider::Whatsapp) => whatsapp.clone(),
            None => None,
        };
    }

    let pending = config.buttons.iter().any(|button| button.icon.is_none() && button.unread_provider.is_some());
    if !pending {
        config.version = LOGO_MIGRATION_VERSION;
    }
    let _ = config::save(config_path, config);
}

fn discover_local_ipv4() -> Ipv4Addr {
    #[cfg(windows)]
    if let Some(address) = select_network_interface(&windows_network_interfaces()) {
        return address;
    }

    local_ip_address::list_afinet_netifas()
        .ok()
        .and_then(|interfaces| select_local_ipv4(&interfaces))
        .or_else(|| match local_ip_address::local_ip().ok() {
            Some(IpAddr::V4(address)) if is_usable(address) => Some(address),
            _ => None,
        })
        .unwrap_or(Ipv4Addr::LOCALHOST)
}

#[derive(Debug)]
struct NetworkInterface {
    name: String,
    description: String,
    address: Ipv4Addr,
    has_gateway: bool,
    metric: u32,
    is_physical_lan: bool,
}

fn select_network_interface(interfaces: &[NetworkInterface]) -> Option<Ipv4Addr> {
    interfaces
        .iter()
        .filter(|interface| {
            is_usable(interface.address)
                && interface.is_physical_lan
                && !is_virtual_adapter(&interface.name)
                && !is_virtual_adapter(&interface.description)
        })
        .min_by_key(|interface| (!interface.has_gateway, interface.metric))
        .map(|interface| interface.address)
}

#[cfg(windows)]
fn windows_network_interfaces() -> Vec<NetworkInterface> {
    use std::mem::size_of;
    use windows::Win32::{
        Foundation::ERROR_BUFFER_OVERFLOW,
        NetworkManagement::{
            IpHelper::{
                GetAdaptersAddresses, GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST,
                GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH,
                IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211,
            },
            Ndis::IfOperStatusUp,
        },
        Networking::WinSock::{AF_INET, SOCKADDR_IN, SOCKET_ADDRESS},
    };

    const INITIAL_BUFFER_SIZE: u32 = 15_000;
    let flags = GAA_FLAG_INCLUDE_GATEWAYS
        | GAA_FLAG_SKIP_ANYCAST
        | GAA_FLAG_SKIP_DNS_SERVER
        | GAA_FLAG_SKIP_MULTICAST;
    let mut size = INITIAL_BUFFER_SIZE;

    loop {
        let word_count = (size as usize + size_of::<usize>() - 1) / size_of::<usize>();
        let mut buffer = vec![0usize; word_count];
        let first = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        let result = unsafe { GetAdaptersAddresses(AF_INET.0.into(), flags, None, Some(first), &mut size) };
        if result == ERROR_BUFFER_OVERFLOW.0 {
            continue;
        }
        if result != 0 {
            return Vec::new();
        }

        let mut interfaces = Vec::new();
        let mut current = first;
        while let Some(adapter) = unsafe { current.as_ref() } {
            if adapter.OperStatus == IfOperStatusUp {
                let name = unsafe { adapter.FriendlyName.to_string().unwrap_or_default() };
                let description = unsafe { adapter.Description.to_string().unwrap_or_default() };
                let mut unicast = adapter.FirstUnicastAddress;
                while let Some(address) = unsafe { unicast.as_ref() } {
                    if let Some(address) = unsafe { socket_ipv4(&address.Address) } {
                        interfaces.push(NetworkInterface {
                            name: name.clone(),
                            description: description.clone(),
                            address,
                            has_gateway: !adapter.FirstGatewayAddress.is_null(),
                            metric: adapter.Ipv4Metric,
                            is_physical_lan: matches!(adapter.IfType, IF_TYPE_ETHERNET_CSMACD | IF_TYPE_IEEE80211),
                        });
                    }
                    unicast = address.Next;
                }
            }
            current = adapter.Next;
        }
        return interfaces;
    }

    unsafe fn socket_ipv4(address: &SOCKET_ADDRESS) -> Option<Ipv4Addr> {
        if address.lpSockaddr.is_null() || address.iSockaddrLength < size_of::<SOCKADDR_IN>() as i32 {
            return None;
        }
        let address = unsafe { &*address.lpSockaddr.cast::<SOCKADDR_IN>() };
        if address.sin_family != AF_INET {
            return None;
        }
        Some(Ipv4Addr::from(unsafe { address.sin_addr.S_un.S_addr }.to_ne_bytes()))
    }
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
        "globalprotect",
        "hamachi",
        "hyper-v",
        "loopback",
        "mullvad",
        "nordlynx",
        "openvpn",
        "protonvpn",
        "radmin",
        "surfshark",
        "tap-",
        "tailscale",
        "tunnel",
        "vethernet",
        "virtual",
        "vmware",
        "vpn",
        "wireguard",
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
    fn prefers_physical_internet_adapter_over_generic_vpn_alias() {
        let interfaces = vec![
            NetworkInterface {
                name: "Ethernet 3".into(),
                description: "Fortinet SSL VPN Virtual Ethernet Adapter".into(),
                address: Ipv4Addr::new(172, 23, 23, 1),
                has_gateway: false,
                metric: 1,
                is_physical_lan: true,
            },
            NetworkInterface {
                name: "Radmin VPN".into(),
                description: "Famatech Radmin VPN Ethernet Adapter".into(),
                address: Ipv4Addr::new(26, 207, 153, 147),
                has_gateway: true,
                metric: 1,
                is_physical_lan: true,
            },
            NetworkInterface {
                name: "Ethernet".into(),
                description: "Realtek Gaming 2.5GbE Family Controller".into(),
                address: Ipv4Addr::new(192, 168, 3, 8),
                has_gateway: true,
                metric: 25,
                is_physical_lan: true,
            },
        ];

        assert_eq!(select_network_interface(&interfaces), Some(Ipv4Addr::new(192, 168, 3, 8)));
    }

    #[test]
    fn prioritizes_gateway_then_interface_metric() {
        let interfaces = vec![
            NetworkInterface {
                name: "Ethernet".into(),
                description: "Physical adapter".into(),
                address: Ipv4Addr::new(192, 168, 10, 20),
                has_gateway: false,
                metric: 1,
                is_physical_lan: true,
            },
            NetworkInterface {
                name: "Wi-Fi".into(),
                description: "Wireless adapter".into(),
                address: Ipv4Addr::new(192, 168, 3, 9),
                has_gateway: true,
                metric: 35,
                is_physical_lan: true,
            },
            NetworkInterface {
                name: "Ethernet 2".into(),
                description: "USB Ethernet adapter".into(),
                address: Ipv4Addr::new(192, 168, 2, 9),
                has_gateway: true,
                metric: 25,
                is_physical_lan: true,
            },
        ];

        assert_eq!(select_network_interface(&interfaces), Some(Ipv4Addr::new(192, 168, 2, 9)));
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
