use crate::config::UnreadProvider;
use regex::Regex;
use std::{
    collections::HashMap,
    process::Command,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

static COUNT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:\(|\b)(\d{1,4})(?:\)|\s+(?:new|unread|não\s+lidas?|novas?))")
        .expect("valid unread pattern")
});

#[derive(Default)]
pub struct UnreadCache {
    inner: Mutex<Option<(Instant, HashMap<UnreadProvider, Option<u32>>)>>,
}

impl UnreadCache {
    pub fn counts(&self) -> HashMap<UnreadProvider, Option<u32>> {
        if let Ok(cache) = self.inner.lock() {
            if let Some((updated_at, counts)) = cache.as_ref() {
                if updated_at.elapsed() < Duration::from_secs(5) {
                    return counts.clone();
                }
            }
        }

        let counts = read_process_titles();
        if let Ok(mut cache) = self.inner.lock() {
            *cache = Some((Instant::now(), counts.clone()));
        }
        counts
    }
}

fn read_process_titles() -> HashMap<UnreadProvider, Option<u32>> {
    let mut command = Command::new("tasklist");
    command.args(["/V", "/FO", "CSV", "/NH"]);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);

    let Ok(output) = command.output() else {
        return HashMap::new();
    };
    if !output.status.success() {
        return HashMap::new();
    }

    let mut found: HashMap<UnreadProvider, bool> = HashMap::new();
    let mut counts: HashMap<UnreadProvider, u32> = HashMap::new();
    let output = String::from_utf8_lossy(&output.stdout);
    let mut reader = csv::ReaderBuilder::new().has_headers(false).from_reader(output.as_bytes());

    for record in reader.records().flatten() {
        let process = record.get(0).unwrap_or_default().to_ascii_lowercase();
        let title = record.get(8).unwrap_or_default();
        let provider = if matches!(process.as_str(), "ms-teams.exe" | "teams.exe" | "msteams.exe") {
            Some(UnreadProvider::Teams)
        } else if process == "whatsapp.exe" {
            Some(UnreadProvider::Whatsapp)
        } else {
            None
        };

        if let Some(provider) = provider {
            found.insert(provider, true);
            let count = parse_count(title).unwrap_or(0);
            counts.entry(provider).and_modify(|current| *current = (*current).max(count)).or_insert(count);
        }
    }

    found.into_iter().map(|(provider, _)| (provider, Some(*counts.get(&provider).unwrap_or(&0)))).collect()
}

fn parse_count(title: &str) -> Option<u32> {
    COUNT_PATTERN
        .captures_iter(title)
        .filter_map(|capture| capture.get(1)?.as_str().parse::<u32>().ok())
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_parenthesized_badges() {
        assert_eq!(parse_count("(12) Chat | Microsoft Teams"), Some(12));
        assert_eq!(parse_count("(3) WhatsApp"), Some(3));
    }

    #[test]
    fn ignores_titles_without_a_counter() {
        assert_eq!(parse_count("Microsoft Teams"), None);
    }
}
