//! Classification et analyse d'un dossier (sauvegarde, dossier d'export…).

use crate::{progress::OpCtl, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
use walkdir::WalkDir;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Photos,
    Videos,
    Audio,
    Documents,
    Contacts,
    Calendar,
    Messages,
    Settings,
    Other,
}

impl Category {
    pub const ALL: [Category; 9] = [
        Category::Photos,
        Category::Videos,
        Category::Audio,
        Category::Documents,
        Category::Contacts,
        Category::Calendar,
        Category::Messages,
        Category::Settings,
        Category::Other,
    ];
}

pub fn classify(rel: &str) -> Category {
    let lower = rel.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "jpg" | "jpeg" | "png" | "heic" | "heif" | "gif" | "dng" | "webp" | "tiff" => Category::Photos,
        "mov" | "mp4" | "m4v" | "avi" | "mkv" => Category::Videos,
        "mp3" | "m4a" | "aac" | "wav" | "flac" | "aiff" => Category::Audio,
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "rtf" | "pages" | "numbers" | "key"
        | "zip" | "csv" | "md" => Category::Documents,
        "vcf" | "abbu" | "abcddb" => Category::Contacts,
        "ics" | "calendar" | "caldav" => Category::Calendar,
        "sms" | "mms" | "imessage" => Category::Messages,
        "plist" | "mobileconfig" => Category::Settings,
        _ => {
            if lower.contains("addressbook") {
                Category::Contacts
            } else if lower.contains("calendar") {
                Category::Calendar
            } else if lower.contains("sms") {
                Category::Messages
            } else {
                Category::Other
            }
        }
    }
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CategoryStat {
    pub files: u64,
    pub bytes: u64,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisReport {
    pub root: String,
    pub total_files: u64,
    pub total_bytes: u64,
    pub categories: BTreeMap<String, CategoryStat>,
    /// Fichiers multimedias vides ou illisibles.
    pub suspicious: Vec<String>,
    pub symlinks_ignored: u64,
}

pub fn analyze_dir(root: &Path, ctl: &OpCtl) -> Result<AnalysisReport> {
    let mut rep = AnalysisReport { root: root.display().to_string(), ..Default::default() };
    for e in WalkDir::new(root).follow_links(false) {
        ctl.check()?;
        let e = match e {
            Ok(e) => e,
            Err(_) => continue,
        };
        if e.file_type().is_symlink() {
            rep.symlinks_ignored += 1;
            continue;
        }
        if !e.file_type().is_file() {
            continue;
        }
        let rel = e.path().strip_prefix(root).unwrap_or(e.path()).to_string_lossy().replace('\\', "/");
        let size = match e.metadata() {
            Ok(m) => m.len(),
            Err(_) => {
                rep.suspicious.push(rel);
                continue;
            }
        };
        let cat = classify(&rel);
        if size == 0 && matches!(cat, Category::Photos | Category::Videos | Category::Audio) {
            rep.suspicious.push(rel.clone());
        }
        let s = rep.categories.entry(format!("{cat:?}").to_lowercase()).or_default();
        s.files += 1;
        s.bytes += size;
        rep.total_files += 1;
        rep.total_bytes += size;
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classification() {
        assert_eq!(classify("DCIM/100APPLE/IMG_1.HEIC"), Category::Photos);
        assert_eq!(classify("a/b.MOV"), Category::Videos);
        assert_eq!(classify("Contacts/all.vcf"), Category::Contacts);
        assert_eq!(classify("x/y.bin"), Category::Other);
    }
    #[test]
    fn analysis_counts_and_flags_empty_media() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.jpg"), b"123").unwrap();
        std::fs::write(d.path().join("b.mov"), b"").unwrap();
        std::fs::write(d.path().join("c.pdf"), b"12345").unwrap();
        let r = analyze_dir(d.path(), &OpCtl::new()).unwrap();
        assert_eq!(r.total_files, 3);
        assert_eq!(r.total_bytes, 8);
        assert_eq!(r.categories["photos"].files, 1);
        assert_eq!(r.suspicious, vec!["b.mov"]);
    }
}
