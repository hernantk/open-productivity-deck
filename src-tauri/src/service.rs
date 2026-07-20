use crate::state::{DEFAULT_PORT, DEFAULT_SECURE_PORT};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSettings {
    pub port: u16,
    pub secure_port: u16,
}

impl Default for ServiceSettings {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            secure_port: DEFAULT_SECURE_PORT,
        }
    }
}

impl ServiceSettings {
    pub fn validate(self) -> Result<Self, String> {
        if self.port < 1024 {
            return Err("A porta HTTP precisa ser 1024 ou superior".into());
        }
        if self.secure_port < 1024 {
            return Err("A porta HTTPS precisa ser 1024 ou superior".into());
        }
        if self.port == self.secure_port {
            return Err("As portas HTTP e HTTPS precisam ser diferentes".into());
        }
        Ok(self)
    }
}

pub fn load(path: &Path) -> ServiceSettings {
    fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .and_then(|settings: ServiceSettings| settings.validate().ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, settings: &ServiceSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("Não foi possível criar a pasta de serviço: {error}"))?;
    }
    let temporary = path.with_extension("json.tmp");
    let contents = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(&temporary, contents).map_err(|error| format!("Não foi possível gravar as portas: {error}"))?;
    fs::rename(&temporary, path).map_err(|error| format!("Não foi possível publicar as portas: {error}"))
}

pub fn resolve(path: &Path) -> ServiceSettings {
    let stored = load(path);
    let port = std::env::var("OPEN_PRODUCTIVITY_DECK_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|port| *port >= 1024)
        .unwrap_or(stored.port);
    let secure_port = std::env::var("OPEN_PRODUCTIVITY_DECK_HTTPS_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|secure_port| *secure_port >= 1024 && *secure_port != port)
        .unwrap_or_else(|| {
            if port == stored.port {
                stored.secure_port
            } else if port == DEFAULT_PORT {
                DEFAULT_SECURE_PORT
            } else {
                port.saturating_add(1)
            }
        });
    ServiceSettings { port, secure_port }.validate().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_identical_ports() {
        let settings = ServiceSettings {
            port: 4000,
            secure_port: 4000,
        };
        assert!(settings.validate().is_err());
    }
}
