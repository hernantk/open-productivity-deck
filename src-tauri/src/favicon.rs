use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use std::{io::Read, time::Duration};

const MAX_BYTES: usize = 256 * 1024;

pub fn fetch_site_icon(target: &str) -> Result<String, String> {
    let url = normalize_url(target)?;
    let host = url.host_str().ok_or_else(|| "URL sem host".to_string())?;
    let origin = origin_of(&url);

    let candidates = [
        format!("https://www.google.com/s2/favicons?domain={host}&sz=128"),
        format!("https://icons.duckduckgo.com/ip3/{host}.ico"),
        format!("{origin}/apple-touch-icon.png"),
        format!("{origin}/favicon.ico"),
        format!("{origin}/favicon.png"),
    ];

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(6))
        .timeout_read(Duration::from_secs(8))
        .user_agent("OpenProductivityDeck/0.6")
        .build();

    let mut last_error = "Nenhum ícone encontrado para este site".to_string();
    for candidate in candidates {
        match download_icon(&agent, &candidate) {
            Ok(data_url) => return Ok(data_url),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

fn normalize_url(target: &str) -> Result<url::Url, String> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return Err("Informe o endereço do site".into());
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let parsed = url::Url::parse(&with_scheme).map_err(|_| "Endereço inválido".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Use um endereço http:// ou https://".into());
    }
    if parsed.host_str().is_none() {
        return Err("URL sem host".into());
    }
    Ok(parsed)
}

fn origin_of(url: &url::Url) -> String {
    let host = url.host_str().unwrap_or_default();
    match url.port() {
        Some(port) => format!("{}://{host}:{port}", url.scheme()),
        None => format!("{}://{host}", url.scheme()),
    }
}

fn download_icon(agent: &ureq::Agent, url: &str) -> Result<String, String> {
    let response = agent
        .get(url)
        .call()
        .map_err(|error| format!("Falha ao baixar ícone: {error}"))?;
    let content_type = response
        .header("content-type")
        .unwrap_or("application/octet-stream")
        .to_ascii_lowercase();
    let mime = mime_from_content_type(&content_type, url).ok_or_else(|| "Resposta não é uma imagem".to_string())?;

    let mut buffer = Vec::new();
    response
        .into_reader()
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut buffer)
        .map_err(|error| format!("Falha ao ler ícone: {error}"))?;
    if buffer.is_empty() || buffer.len() > MAX_BYTES {
        return Err("Ícone vazio ou grande demais".into());
    }
    if looks_like_html(&buffer) {
        return Err("O servidor devolveu HTML em vez de ícone".into());
    }
    Ok(format!("data:{mime};base64,{}", BASE64.encode(buffer)))
}

fn mime_from_content_type(content_type: &str, url: &str) -> Option<&'static str> {
    if content_type.contains("image/png") || content_type.contains("application/png") {
        return Some("image/png");
    }
    if content_type.contains("image/jpeg") || content_type.contains("image/jpg") {
        return Some("image/jpeg");
    }
    if content_type.contains("image/webp") {
        return Some("image/webp");
    }
    if content_type.contains("image/svg") {
        return Some("image/svg+xml");
    }
    if content_type.contains("image/x-icon")
        || content_type.contains("image/vnd.microsoft.icon")
        || content_type.contains("image/ico")
        || url.ends_with(".ico")
    {
        return Some("image/x-icon");
    }
    if content_type.starts_with("image/") {
        return Some("image/png");
    }
    let lower = url.to_ascii_lowercase();
    if lower.contains(".png") || lower.contains("favicons") {
        return Some("image/png");
    }
    if lower.contains(".jpg") || lower.contains(".jpeg") {
        return Some("image/jpeg");
    }
    if lower.contains(".webp") {
        return Some("image/webp");
    }
    if lower.contains(".svg") {
        return Some("image/svg+xml");
    }
    if lower.contains(".ico") {
        return Some("image/x-icon");
    }
    None
}

fn looks_like_html(bytes: &[u8]) -> bool {
    let sample = String::from_utf8_lossy(&bytes[..bytes.len().min(64)]).to_ascii_lowercase();
    sample.contains("<html") || sample.contains("<!doctype")
}
