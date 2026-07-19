use crate::config::{DeckButton, LaunchKind};
use regex::Regex;
use std::{path::Path, sync::LazyLock};

static URI_SCHEME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-zA-Z][a-zA-Z0-9+.-]*:").expect("valid URI regex"));

pub fn launch(button: &DeckButton) -> Result<(), String> {
    match button.kind {
        LaunchKind::Application => {
            let path = Path::new(&button.target);
            if !path.is_absolute() || !path.exists() {
                return Err(format!("O destino de ‘{}’ não foi encontrado", button.label));
            }
        }
        LaunchKind::Url => {
            if !URI_SCHEME.is_match(&button.target) {
                return Err(format!("O endereço de ‘{}’ não possui um protocolo válido", button.label));
            }
        }
    }

    open::that_detached(&button.target).map_err(|error| format!("Não foi possível abrir ‘{}’: {error}", button.label))
}
