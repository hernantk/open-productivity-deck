use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckConfig {
    pub version: u32,
    pub title: String,
    pub buttons: Vec<DeckButton>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckButton {
    pub id: Uuid,
    pub label: String,
    pub target: String,
    pub kind: LaunchKind,
    pub color: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub unread_provider: Option<UnreadProvider>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LaunchKind {
    Application,
    Url,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnreadProvider {
    Teams,
    Whatsapp,
}

impl Default for DeckConfig {
    fn default() -> Self {
        Self {
            version: 1,
            title: "Meu deck".into(),
            buttons: vec![
                DeckButton {
                    id: Uuid::new_v4(),
                    label: "Microsoft Teams".into(),
                    target: "msteams:".into(),
                    kind: LaunchKind::Url,
                    color: "#675a9e".into(),
                    icon: None,
                    unread_provider: Some(UnreadProvider::Teams),
                },
                DeckButton {
                    id: Uuid::new_v4(),
                    label: "WhatsApp".into(),
                    target: "whatsapp:".into(),
                    kind: LaunchKind::Url,
                    color: "#286c64".into(),
                    icon: None,
                    unread_provider: Some(UnreadProvider::Whatsapp),
                },
            ],
        }
    }
}

impl DeckConfig {
    pub fn validate(mut self) -> Result<Self, String> {
        if self.buttons.len() > 48 {
            return Err("O deck aceita no máximo 48 botões".into());
        }

        self.title = self.title.trim().chars().take(64).collect();
        if self.title.is_empty() {
            self.title = "Meu deck".into();
        }

        for button in &mut self.buttons {
            button.label = button.label.trim().chars().take(32).collect();
            button.target = button.target.trim().chars().take(2048).collect();

            if button.label.is_empty() {
                return Err("Todos os botões precisam de um nome".into());
            }
            if button.target.is_empty() {
                return Err(format!("Defina o destino de ‘{}’", button.label));
            }
            if !is_hex_color(&button.color) {
                return Err(format!("A cor de ‘{}’ é inválida", button.label));
            }
            if let Some(icon) = &button.icon {
                if icon.len() > 350_000 || !is_supported_icon(icon) {
                    return Err(format!("O ícone de ‘{}’ é inválido ou muito grande", button.label));
                }
            }
        }

        if self.version == 0 {
            self.version = 1;
        }
        Ok(self)
    }
}

fn is_supported_icon(value: &str) -> bool {
    [
        "data:image/png;base64,",
        "data:image/jpeg;base64,",
        "data:image/webp;base64,",
        "data:image/svg+xml;base64,",
    ]
    .iter()
    .any(|prefix| value.starts_with(prefix))
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..].chars().all(|character| character.is_ascii_hexdigit())
}

pub fn load(path: &Path) -> DeckConfig {
    fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .and_then(|config: DeckConfig| config.validate().ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, config: &DeckConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("Não foi possível criar a pasta de configuração: {error}"))?;
    }

    let temporary = path.with_extension("json.tmp");
    let contents = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
    fs::write(&temporary, contents).map_err(|error| format!("Não foi possível gravar a configuração: {error}"))?;
    fs::rename(&temporary, path).map_err(|error| format!("Não foi possível publicar a configuração: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_targets() {
        let mut config = DeckConfig::default();
        config.buttons[0].target.clear();
        assert!(config.validate().is_err());
    }

    #[test]
    fn accepts_six_digit_colors() {
        assert!(is_hex_color("#e9592f"));
        assert!(!is_hex_color("orange"));
    }
}
