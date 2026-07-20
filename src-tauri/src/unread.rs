use crate::config::UnreadProvider;
use regex::Regex;
use rusqlite::{Connection, OpenFlags};
use std::{
    collections::HashMap,
    path::PathBuf,
    process::Command,
    sync::{mpsc, Arc, LazyLock, Mutex},
    thread,
    time::Duration,
};
use tokio::sync::broadcast;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

static COUNT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:\(|\b)(\d{1,4})(?:\)|\s+(?:new|unread|não\s+lidas?|novas?))")
        .expect("valid unread pattern")
});
static BADGE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<badge\s+[^>]*value\s*=\s*["'](\d{1,6})["']"#)
        .expect("valid badge pattern")
});

pub struct UnreadCache {
    inner: Mutex<HashMap<UnreadProvider, Option<u32>>>,
    updates: broadcast::Sender<()>,
}

impl Default for UnreadCache {
    fn default() -> Self {
        let (updates, _) = broadcast::channel(16);
        Self {
            inner: Mutex::new(HashMap::new()),
            updates,
        }
    }
}

impl UnreadCache {
    pub fn start(self: Arc<Self>) {
        let _ = thread::Builder::new()
            .name("unread-counter".into())
            .spawn(move || self.monitor());
    }

    pub fn refresh(&self) {
        let mut counts = read_notification_badges().unwrap_or_else(|| self.snapshot());
        for (provider, count) in read_process_titles() {
            counts.entry(provider).or_insert(count);
        }
        if let Ok(mut cache) = self.inner.lock() {
            if *cache != counts {
                *cache = counts;
                let _ = self.updates.send(());
            }
        }
    }

    pub fn snapshot(&self) -> HashMap<UnreadProvider, Option<u32>> {
        self.inner.lock().map(|counts| counts.clone()).unwrap_or_default()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.updates.subscribe()
    }

    fn monitor(&self) {
        self.refresh();
        let Some(database) = notification_database_path() else {
            return self.poll_fallback();
        };
        let Some(directory) = database.parent() else {
            return self.poll_fallback();
        };
        let (events_tx, events_rx) = mpsc::channel();
        let Ok(mut watcher) = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let _ = events_tx.send(event);
        }) else {
            return self.poll_fallback();
        };
        if notify::Watcher::watch(&mut watcher, directory, notify::RecursiveMode::NonRecursive).is_err() {
            return self.poll_fallback();
        }

        loop {
            match events_rx.recv_timeout(Duration::from_secs(30)) {
                Ok(Ok(event)) if event.paths.iter().any(|path| is_notification_database_file(path)) => {
                    thread::sleep(Duration::from_millis(120));
                    while events_rx.try_recv().is_ok() {}
                    self.refresh();
                }
                Ok(_) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => self.refresh(),
                Err(mpsc::RecvTimeoutError::Disconnected) => return self.poll_fallback(),
            }
        }
    }

    fn poll_fallback(&self) {
        loop {
            thread::sleep(Duration::from_secs(2));
            self.refresh();
        }
    }
}

fn is_notification_database_file(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with("wpndatabase.db"))
        .unwrap_or(false)
}

fn read_process_titles() -> HashMap<UnreadProvider, Option<u32>> {
    let mut found: HashMap<UnreadProvider, bool> = HashMap::new();
    let mut counts: HashMap<UnreadProvider, u32> = HashMap::new();
    let processes = ["ms-teams.exe", "teams.exe", "msteams.exe", "WhatsApp.exe", "WhatsApp.Root.exe"];

    for process_filter in processes {
        let mut command = Command::new("tasklist");
        command.args(["/V", "/FO", "CSV", "/NH", "/FI", &format!("IMAGENAME eq {process_filter}")]);
        #[cfg(windows)]
        command.creation_flags(0x0800_0000);

        let Ok(output) = command.output() else {
            continue;
        };
        if !output.status.success() {
            continue;
        }

        let output = String::from_utf8_lossy(&output.stdout);
        let mut reader = csv::ReaderBuilder::new().has_headers(false).from_reader(output.as_bytes());
        for record in reader.records().flatten() {
            let process = record.get(0).unwrap_or_default().to_ascii_lowercase();
            let title = record.get(8).unwrap_or_default();
            let provider = if matches!(process.as_str(), "ms-teams.exe" | "teams.exe" | "msteams.exe") {
                Some(UnreadProvider::Teams)
            } else if matches!(process.as_str(), "whatsapp.exe" | "whatsapp.root.exe") {
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
    }

    found.into_iter().map(|(provider, _)| (provider, Some(*counts.get(&provider).unwrap_or(&0)))).collect()
}

fn read_notification_badges() -> Option<HashMap<UnreadProvider, Option<u32>>> {
    let Some(database_path) = notification_database_path() else {
        return None;
    };
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let Ok(connection) = Connection::open_with_flags(database_path, flags) else {
        return None;
    };
    let Ok(mut statement) = connection.prepare(
        "SELECT h.PrimaryId, n.Payload
         FROM Notification n
         INNER JOIN NotificationHandler h ON h.RecordId = n.HandlerId
         WHERE n.Type = 'badge'
           AND (LOWER(h.PrimaryId) LIKE '%teams%' OR LOWER(h.PrimaryId) LIKE '%whatsapp%')
         ORDER BY n.ArrivalTime DESC",
    ) else {
        return None;
    };
    let Ok(rows) = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
    }) else {
        return None;
    };

    let mut counts = HashMap::new();
    for row in rows.flatten() {
        let Some(provider) = provider_from_identifier(&row.0) else {
            continue;
        };
        if counts.contains_key(&provider) {
            continue;
        }
        let payload = String::from_utf8_lossy(&row.1);
        if let Some(count) = parse_badge(&payload) {
            counts.insert(provider, Some(count));
        }
    }
    Some(counts)
}

fn notification_database_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("Microsoft").join("Windows").join("Notifications").join("wpndatabase.db"))
        .filter(|path| path.exists())
}

fn provider_from_identifier(identifier: &str) -> Option<UnreadProvider> {
    let identifier = identifier.to_ascii_lowercase();
    if identifier.contains("teams") {
        Some(UnreadProvider::Teams)
    } else if identifier.contains("whatsapp") {
        Some(UnreadProvider::Whatsapp)
    } else {
        None
    }
}

fn parse_badge(payload: &str) -> Option<u32> {
    BADGE_PATTERN
        .captures(payload)
        .and_then(|capture| capture.get(1))
        .and_then(|value| value.as_str().parse().ok())
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

    #[test]
    fn reads_numeric_windows_badges() {
        assert_eq!(parse_badge(r#"<badge value="17"/>"#), Some(17));
        assert_eq!(parse_badge(r#"<badge value="0"/>"#), Some(0));
        assert_eq!(parse_badge(r#"<badge value="activity"/>"#), None);
    }

    #[test]
    fn recognizes_current_store_app_identifiers() {
        assert_eq!(provider_from_identifier("MSTeams_8wekyb3d8bbwe!MSTeams"), Some(UnreadProvider::Teams));
        assert_eq!(provider_from_identifier("5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App"), Some(UnreadProvider::Whatsapp));
    }

    #[test]
    fn recognizes_notification_database_wal_files() {
        assert!(is_notification_database_file(std::path::Path::new("wpndatabase.db")));
        assert!(is_notification_database_file(std::path::Path::new("wpndatabase.db-wal")));
        assert!(!is_notification_database_file(std::path::Path::new("unrelated.db")));
    }
}
