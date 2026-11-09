use crate::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub kind: String,
    pub started: String,
    pub finished: String,
    /// "success" | "partial" | "failed" | "cancelled"
    pub status: String,
    pub summary: String,
    pub simulated: bool,
}

impl HistoryEntry {
    pub fn now(kind: &str, status: &str, summary: String, simulated: bool, started: String) -> Self {
        let finished = chrono::Utc::now().to_rfc3339();
        HistoryEntry {
            id: format!("{}-{}", chrono::Utc::now().timestamp_millis(), kind),
            kind: kind.into(),
            started,
            finished,
            status: status.into(),
            summary: crate::security::redact(&summary),
            simulated,
        }
    }
}

pub struct HistoryStore {
    path: PathBuf,
}

impl HistoryStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn append(&self, e: &HistoryEntry) -> Result<()> {
        if let Some(p) = self.path.parent() {
            fs::create_dir_all(p)?;
        }
        let mut f = OpenOptions::new().create(true).append(true).open(&self.path)?;
        writeln!(f, "{}", serde_json::to_string(e)?)?;
        Ok(())
    }

    /// Plus recents d'abord. Les lignes corrompues sont ignorees.
    pub fn list(&self, limit: usize) -> Vec<HistoryEntry> {
        let Ok(text) = fs::read_to_string(&self.path) else { return vec![] };
        let mut v: Vec<HistoryEntry> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
        v.reverse();
        v.truncate(limit);
        v
    }

    pub fn clear(&self) -> Result<()> {
        if self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn append_list_clear_and_corruption() {
        let d = tempfile::tempdir().unwrap();
        let h = HistoryStore::new(d.path().join("h.jsonl"));
        let t = chrono::Utc::now().to_rfc3339();
        h.append(&HistoryEntry::now("backup", "success", "ok pour 00008110-000A1B2C3D4E5F01".into(), false, t.clone())).unwrap();
        std::fs::OpenOptions::new().append(true).open(d.path().join("h.jsonl")).unwrap().write_all(b"{corrompu\n").unwrap();
        h.append(&HistoryEntry::now("clone", "partial", "x".into(), true, t)).unwrap();
        let l = h.list(10);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].kind, "clone");
        assert!(!l[1].summary.contains("00008110"), "UDID non caviardé");
        h.clear().unwrap();
        assert!(h.list(10).is_empty());
    }
}
