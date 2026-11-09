//! Validation des entrees, protection contre les traversees de repertoires,
//! caviardage des journaux et suppression securisee (au mieux des capacites du systeme).

use crate::{GhostError, Result};
use regex::Regex;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::OnceLock,
};
use walkdir::WalkDir;

/// Joint un chemin relatif non fiable a `base` sans jamais pouvoir en sortir.
pub fn safe_join(base: &Path, rel: &str) -> Result<PathBuf> {
    if rel.contains('\0') {
        return Err(GhostError::Security("octet nul dans le chemin".into()));
    }
    if rel.starts_with('/') || rel.starts_with('\\') {
        return Err(GhostError::Security(format!("chemin absolu refusé : {rel}")));
    }
    let mut out = base.to_path_buf();
    for part in rel.split(['/', '\\']) {
        match part {
            "" | "." => continue,
            ".." => return Err(GhostError::Security(format!("traversée de répertoire refusée : {rel}"))),
            p if p.contains(':') => {
                return Err(GhostError::Security(format!("composant de chemin refusé : {p}")))
            }
            p => out.push(p),
        }
    }
    // Defense en profondeur : seuls des composants normaux doivent s'ajouter a la base.
    let suffix = out.strip_prefix(base).map_err(|_| GhostError::Security("chemin hors de la base".into()))?;
    if suffix.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(GhostError::Security("composant de chemin non autorisé".into()));
    }
    Ok(out)
}

/// UDID Apple : 40 hexa (anciens) ou 8-16 hexa (modernes), tirets autorises.
pub fn validate_udid(udid: &str) -> Result<()> {
    let ok = (8..=64).contains(&udid.len()) && udid.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    if ok {
        Ok(())
    } else {
        Err(GhostError::Security("identifiant d'appareil invalide".into()))
    }
}

pub fn sanitize_filename(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c })
        .collect();
    let s = s.trim().trim_matches('.').to_string();
    if s.is_empty() {
        "sans_nom".into()
    } else {
        s
    }
}

fn re(cell: &'static OnceLock<Vec<(Regex, &'static str)>>) -> &'static Vec<(Regex, &'static str)> {
    cell.get_or_init(|| {
        vec![
            (Regex::new(r"(?i)\b[0-9a-f]{8}-[0-9a-f]{16}\b").unwrap(), "<udid>"),
            (Regex::new(r"(?i)\b[0-9a-f]{40}\b").unwrap(), "<udid>"),
            (Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap(), "<email>"),
            (Regex::new(r"(?i)[A-Z]:\\Users\\[^\\\s]+").unwrap(), r"<profil>"),
            (Regex::new(r"/(?:Users|home)/[^/\s]+").unwrap(), "<profil>"),
            (Regex::new(r"\+?\d[\d .-]{8,}\d").unwrap(), "<nombre>"),
        ]
    })
}

/// Retire des textes de journaux les donnees personnelles (UDID, e-mails, profils, numeros).
pub fn redact(text: &str) -> String {
    static CELL: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let mut s = text.to_string();
    for (r, rep) in re(&CELL) {
        s = r.replace_all(&s, *rep).into_owned();
    }
    s
}

/// Ecrase puis supprime. Sur SSD/APFS/journalise, l'effacement physique n'est pas garanti.
pub fn secure_delete_file(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        fs::remove_file(path)?;
        return Ok(());
    }
    {
        let mut f = OpenOptions::new().write(true).open(path)?;
        let zeros = vec![0u8; 1 << 20];
        let mut left = meta.len();
        while left > 0 {
            let n = left.min(zeros.len() as u64) as usize;
            f.write_all(&zeros[..n])?;
            left -= n as u64;
        }
        f.sync_all()?;
    }
    fs::remove_file(path)?;
    Ok(())
}

pub fn secure_delete_dir(dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for e in WalkDir::new(dir).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        if e.file_type().is_file() {
            secure_delete_file(e.path())?;
        }
    }
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_accepts_normal_paths() {
        let b = Path::new("/base");
        assert_eq!(safe_join(b, "a/b/c.txt").unwrap(), Path::new("/base/a/b/c.txt"));
        assert_eq!(safe_join(b, "./a//b").unwrap(), Path::new("/base/a/b"));
    }

    #[test]
    fn safe_join_rejects_attacks() {
        let b = Path::new("/base");
        for bad in ["../x", "a/../../x", "/etc/passwd", "\\windows", "C:\\x", "a/b:stream", "a\0b", "..\\x"] {
            assert!(safe_join(b, bad).is_err(), "{bad:?} aurait dû être refusé");
        }
    }

    #[test]
    fn udid_validation() {
        assert!(validate_udid("00008030-001A2B3C4D5E6F70").is_ok());
        assert!(validate_udid("a".repeat(40).as_str()).is_ok());
        for bad in ["", "abc", "x; rm -rf /", "00008030-001A2B3C4D5E6F70 --evil", "$(id)"] {
            assert!(validate_udid(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn redaction_removes_personal_data() {
        let s = redact("udid 00008030-001A2B3C4D5E6F70 mail jean@ex.fr dossier C:\\Users\\Jean\\x tel +33 6 12 34 56 78");
        assert!(!s.contains("00008030") && !s.contains("jean@") && !s.contains("Jean") && !s.contains("12 34"));
    }

    #[test]
    fn filename_sanitizing() {
        assert_eq!(sanitize_filename("a/b:c?.jpg"), "a_b_c_.jpg");
        assert_eq!(sanitize_filename("..."), "sans_nom");
    }

    #[test]
    fn secure_delete_removes_files() {
        let d = tempfile::tempdir().unwrap();
        let sub = d.path().join("s");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("a"), vec![9u8; 3000]).unwrap();
        secure_delete_dir(&sub).unwrap();
        assert!(!sub.exists());
    }
}
