use crate::security::redact;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::Mutex,
};

/// Journal local. Chaque ligne est caviardee avant ecriture (aucune donnee personnelle).
pub struct Logger {
    path: PathBuf,
    lock: Mutex<()>,
}

impl Logger {
    pub fn new(path: PathBuf) -> Self {
        Self { path, lock: Mutex::new(()) }
    }

    pub fn log(&self, level: &str, msg: &str) {
        let _g = self.lock.lock();
        if let Some(p) = self.path.parent() {
            let _ = fs::create_dir_all(p);
        }
        // rotation simple a 2 Mio
        if fs::metadata(&self.path).map(|m| m.len() > 2_000_000).unwrap_or(false) {
            let _ = fs::rename(&self.path, self.path.with_extension("old"));
        }
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.path) {
            let _ = writeln!(f, "{} {:5} {}", chrono::Utc::now().to_rfc3339(), level, redact(msg));
        }
    }
    pub fn info(&self, m: &str) {
        self.log("INFO", m)
    }
    pub fn error(&self, m: &str) {
        self.log("ERROR", m)
    }

    pub fn tail(&self, lines: usize) -> Vec<String> {
        let t = fs::read_to_string(&self.path).unwrap_or_default();
        let v: Vec<String> = t.lines().map(String::from).collect();
        v[v.len().saturating_sub(lines)..].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logs_are_redacted() {
        let d = tempfile::tempdir().unwrap();
        let l = Logger::new(d.path().join("g.log"));
        l.info("appareil 00008110-000A1B2C3D4E5F01 de marie@exemple.fr");
        let t = l.tail(5).join("\n");
        assert!(t.contains("INFO") && !t.contains("00008110") && !t.contains("marie@"));
    }
}
