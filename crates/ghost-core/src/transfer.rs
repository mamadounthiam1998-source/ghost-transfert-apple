//! Moteur de transfert : copie atomique, verification SHA-256, reprise,
//! deduplication, gestion des conflits. La progression est calculee a partir
//! des octets reellement ecrits.

use crate::{
    analysis::{classify, Category},
    hashing::sha256_file,
    progress::{OpCtl, Progress, Tracker},
    security::safe_join,
    GhostError, Result,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use walkdir::WalkDir;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConflictPolicy {
    /// Ne touche pas au fichier destination different (il est signale comme non transfere).
    Skip,
    /// Copie sous un nouveau nom « fichier (1).ext ».
    Rename,
    /// Ecrase : exige `confirm_overwrite`.
    Overwrite,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransferOptions {
    pub categories: Option<Vec<Category>>,
    pub dedupe: bool,
    pub conflict: ConflictPolicy,
    pub confirm_overwrite: bool,
    pub verify: bool,
    #[serde(default)]
    pub simulated: bool,
}

impl Default for TransferOptions {
    fn default() -> Self {
        TransferOptions {
            categories: None,
            dedupe: true,
            conflict: ConflictPolicy::Skip,
            confirm_overwrite: false,
            verify: true,
            simulated: false,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub rel: String,
    pub size: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub root: PathBuf,
    pub items: Vec<PlanItem>,
    pub total_bytes: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ItemIssue {
    pub rel: String,
    pub reason: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct TransferReport {
    pub planned_files: u64,
    pub copied: u64,
    pub already_present: u64,
    pub duplicates_skipped: u64,
    pub renamed: u64,
    pub verified_ok: u64,
    pub verified_failed: u64,
    pub bytes_copied: u64,
    pub not_transferred: Vec<ItemIssue>,
    pub errors: Vec<ItemIssue>,
    pub duration_ms: u64,
    /// Vrai uniquement si rien n'a echoue et rien n'est reste non transfere.
    pub success: bool,
}

pub fn build_plan(root: &Path, categories: &Option<Vec<Category>>, ctl: &OpCtl) -> Result<Plan> {
    if !root.is_dir() {
        return Err(GhostError::Data(format!("dossier source introuvable : {}", root.display())));
    }
    let mut items = vec![];
    let mut total = 0;
    for e in WalkDir::new(root).follow_links(false).sort_by_file_name() {
        ctl.check()?;
        let Ok(e) = e else { continue };
        if !e.file_type().is_file() {
            continue; // dossiers et liens symboliques ignores
        }
        let rel = e.path().strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
        if let Some(cats) = categories {
            if !cats.contains(&classify(&rel)) {
                continue;
            }
        }
        let size = e.metadata().map(|m| m.len()).unwrap_or(0);
        total += size;
        items.push(PlanItem { rel, size });
    }
    Ok(Plan { root: root.to_path_buf(), items, total_bytes: total })
}

pub fn free_space(path: &Path) -> Result<u64> {
    let mut p = path.to_path_buf();
    while !p.exists() {
        if !p.pop() {
            break;
        }
    }
    fs4::available_space(&p).map_err(GhostError::from)
}

pub fn check_space(dest: &Path, needed: u64) -> Result<()> {
    let available = free_space(dest)?;
    // marge de 1 % pour les metadonnees
    let needed_margin = needed + needed / 100;
    if available < needed_margin {
        Err(GhostError::NoSpace { needed: needed_margin, available })
    } else {
        Ok(())
    }
}

fn unique_name(target: &Path) -> PathBuf {
    let stem = target.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = target.extension().map(|s| format!(".{}", s.to_string_lossy())).unwrap_or_default();
    for n in 1.. {
        let cand = target.with_file_name(format!("{stem} ({n}){ext}"));
        if !cand.exists() {
            return cand;
        }
    }
    unreachable!()
}

/// Copie src -> dst via un fichier temporaire, retourne le SHA-256 des octets copies.
fn copy_atomic(
    src: &Path,
    dst: &Path,
    ctl: &OpCtl,
    mut on_bytes: impl FnMut(u64),
) -> Result<String> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = dst.with_file_name(format!(
        "{}.ghostpart",
        dst.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    ));
    let result = (|| -> Result<String> {
        let mut r = File::open(src)?;
        let mut w = File::create(&tmp)?;
        let mut h = Sha256::new();
        let mut buf = vec![0u8; 1 << 20];
        loop {
            ctl.check()?;
            let n = r.read(&mut buf)?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
            w.write_all(&buf[..n])?;
            on_bytes(n as u64);
        }
        w.sync_all()?;
        if let Ok(m) = fs::metadata(src).and_then(|m| m.modified()) {
            let _ = w.set_modified(m); // conserve la date de modification
        }
        Ok(hex::encode(h.finalize()))
    })();
    match result {
        Ok(hash) => {
            fs::rename(&tmp, dst)?;
            Ok(hash)
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

pub fn run(
    plan: &Plan,
    dest: &Path,
    opts: &TransferOptions,
    ctl: &OpCtl,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<TransferReport> {
    if opts.conflict == ConflictPolicy::Overwrite && !opts.confirm_overwrite {
        return Err(GhostError::Security(
            "l'écrasement de fichiers existants exige une confirmation explicite".into(),
        ));
    }
    fs::create_dir_all(dest)?;
    check_space(dest, plan.total_bytes)?;

    let start = Instant::now();
    let mut rep = TransferReport { planned_files: plan.items.len() as u64, ..Default::default() };
    let mut tr = Tracker::new("Transfert", plan.items.len() as u64, plan.total_bytes, opts.simulated);
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    on_progress(tr.snapshot());

    for item in &plan.items {
        ctl.check()?;
        tr.p.current = Some(item.rel.clone());
        let issue = |reason: String| ItemIssue { rel: item.rel.clone(), reason };

        let (src, mut target) = match (safe_join(&plan.root, &item.rel), safe_join(dest, &item.rel)) {
            (Ok(s), Ok(t)) => (s, t),
            (Err(e), _) | (_, Err(e)) => {
                rep.errors.push(issue(e.to_string()));
                tr.p.errors += 1;
                tr.p.files_done += 1;
                tr.p.bytes_done += item.size;
                continue;
            }
        };

        let src_hash = match sha256_file(&src) {
            Ok(h) => h,
            Err(e) => {
                rep.errors.push(issue(format!("lecture impossible : {e}")));
                tr.p.errors += 1;
                tr.p.files_done += 1;
                tr.p.bytes_done += item.size;
                continue;
            }
        };

        // Doublon dans la meme operation (contenu identique).
        if opts.dedupe {
            if let Some(first) = seen.get(&src_hash) {
                if first != &item.rel {
                    rep.duplicates_skipped += 1;
                    tr.p.files_done += 1;
                    tr.p.bytes_done += item.size;
                    continue;
                }
            }
            seen.insert(src_hash.clone(), item.rel.clone());
        }

        // Reprise / conflit.
        if target.exists() {
            let same = fs::metadata(&target).map(|m| m.len() == item.size).unwrap_or(false)
                && sha256_file(&target).map(|h| h == src_hash).unwrap_or(false);
            if same {
                rep.already_present += 1;
                rep.verified_ok += 1;
                tr.p.verified += 1;
                tr.p.files_done += 1;
                tr.p.bytes_done += item.size;
                continue;
            }
            match opts.conflict {
                ConflictPolicy::Skip => {
                    rep.not_transferred.push(issue("un fichier différent existe déjà à la destination".into()));
                    tr.p.files_done += 1;
                    tr.p.bytes_done += item.size;
                    continue;
                }
                ConflictPolicy::Rename => {
                    target = unique_name(&target);
                    rep.renamed += 1;
                }
                ConflictPolicy::Overwrite => {}
            }
        }

        let before = tr.p.bytes_done;
        let mut copied_here = 0u64;
        let res = copy_atomic(&src, &target, ctl, |n| {
            copied_here += n;
            tr.p.bytes_done = before + copied_here;
            if last_emit.elapsed() > Duration::from_millis(100) {
                last_emit = Instant::now();
                on_progress(tr.snapshot());
            }
        });
        tr.p.bytes_done = before + item.size;
        tr.p.files_done += 1;
        match res {
            Ok(copy_hash) => {
                rep.copied += 1;
                rep.bytes_copied += item.size;
                if opts.verify {
                    let ok = copy_hash == src_hash && sha256_file(&target).map(|h| h == src_hash).unwrap_or(false);
                    if ok {
                        rep.verified_ok += 1;
                        tr.p.verified += 1;
                    } else {
                        rep.verified_failed += 1;
                        rep.errors.push(issue("la vérification SHA-256 a échoué".into()));
                        tr.p.errors += 1;
                        let _ = fs::remove_file(&target);
                    }
                }
            }
            Err(GhostError::Cancelled) => return Err(GhostError::Cancelled),
            Err(e) => {
                rep.errors.push(issue(e.to_string()));
                tr.p.errors += 1;
            }
        }
        if last_emit.elapsed() > Duration::from_millis(100) {
            last_emit = Instant::now();
            on_progress(tr.snapshot());
        }
    }

    tr.p.current = None;
    tr.p.phase = "Terminé".into();
    on_progress(tr.snapshot());
    rep.duration_ms = start.elapsed().as_millis() as u64;
    rep.success = rep.errors.is_empty() && rep.not_transferred.is_empty() && rep.verified_failed == 0;
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write(root: &Path, rel: &str, data: &[u8]) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, data).unwrap();
    }

    fn go(src: &Path, dst: &Path, opts: &TransferOptions) -> Result<TransferReport> {
        let ctl = OpCtl::new();
        let plan = build_plan(src, &opts.categories, &ctl)?;
        run(&plan, dst, opts, &ctl, &mut |_| {})
    }

    #[test]
    fn copies_and_verifies_with_structure_and_dates() {
        let (s, d) = (tempdir().unwrap(), tempdir().unwrap());
        write(s.path(), "DCIM/100APPLE/IMG_1.jpg", b"photo-1");
        write(s.path(), "Docs/a.pdf", b"pdf");
        let old = std::time::SystemTime::now() - Duration::from_secs(86_400 * 400);
        File::options().write(true).open(s.path().join("Docs/a.pdf")).unwrap().set_modified(old).unwrap();
        let r = go(s.path(), d.path(), &TransferOptions::default()).unwrap();
        assert!(r.success);
        assert_eq!((r.copied, r.verified_ok), (2, 2));
        assert_eq!(fs::read(d.path().join("DCIM/100APPLE/IMG_1.jpg")).unwrap(), b"photo-1");
        let m = fs::metadata(d.path().join("Docs/a.pdf")).unwrap().modified().unwrap();
        let diff = m.duration_since(old).unwrap_or_else(|e| e.duration());
        assert!(diff < Duration::from_secs(2), "date de modification non conservée");
        assert!(!d.path().join("Docs/a.pdf.ghostpart").exists());
    }

    #[test]
    fn rerun_resumes_without_recopying() {
        let (s, d) = (tempdir().unwrap(), tempdir().unwrap());
        write(s.path(), "a.jpg", b"1");
        write(s.path(), "b.jpg", b"2");
        go(s.path(), d.path(), &TransferOptions::default()).unwrap();
        let r = go(s.path(), d.path(), &TransferOptions::default()).unwrap();
        assert_eq!((r.copied, r.already_present), (0, 2));
        assert!(r.success);
    }

    #[test]
    fn partial_files_are_replaced_on_resume() {
        let (s, d) = (tempdir().unwrap(), tempdir().unwrap());
        write(s.path(), "a.bin", &vec![5u8; 3000]);
        write(d.path(), "a.bin.ghostpart", b"reste d'un transfert interrompu");
        let r = go(s.path(), d.path(), &TransferOptions::default()).unwrap();
        assert!(r.success);
        assert_eq!(fs::read(d.path().join("a.bin")).unwrap().len(), 3000);
        assert!(!d.path().join("a.bin.ghostpart").exists());
    }

    #[test]
    fn duplicates_are_skipped() {
        let (s, d) = (tempdir().unwrap(), tempdir().unwrap());
        write(s.path(), "a.jpg", b"same");
        write(s.path(), "b.jpg", b"same");
        write(s.path(), "c.jpg", b"other");
        let r = go(s.path(), d.path(), &TransferOptions::default()).unwrap();
        assert_eq!((r.copied, r.duplicates_skipped), (2, 1));
    }

    #[test]
    fn conflict_skip_is_reported_not_silent() {
        let (s, d) = (tempdir().unwrap(), tempdir().unwrap());
        write(s.path(), "a.txt", b"nouveau");
        write(d.path(), "a.txt", b"ancien");
        let r = go(s.path(), d.path(), &TransferOptions::default()).unwrap();
        assert!(!r.success, "ne doit pas annoncer un succès total");
        assert_eq!(r.not_transferred.len(), 1);
        assert_eq!(fs::read(d.path().join("a.txt")).unwrap(), b"ancien");
    }

    #[test]
    fn conflict_rename_keeps_both() {
        let (s, d) = (tempdir().unwrap(), tempdir().unwrap());
        write(s.path(), "a.txt", b"nouveau");
        write(d.path(), "a.txt", b"ancien");
        let o = TransferOptions { conflict: ConflictPolicy::Rename, ..Default::default() };
        let r = go(s.path(), d.path(), &o).unwrap();
        assert!(r.success);
        assert_eq!(fs::read(d.path().join("a (1).txt")).unwrap(), b"nouveau");
        assert_eq!(fs::read(d.path().join("a.txt")).unwrap(), b"ancien");
    }

    #[test]
    fn overwrite_requires_explicit_confirmation() {
        let (s, d) = (tempdir().unwrap(), tempdir().unwrap());
        write(s.path(), "a.txt", b"nouveau");
        write(d.path(), "a.txt", b"ancien");
        let mut o = TransferOptions { conflict: ConflictPolicy::Overwrite, ..Default::default() };
        assert!(matches!(go(s.path(), d.path(), &o), Err(GhostError::Security(_))));
        assert_eq!(fs::read(d.path().join("a.txt")).unwrap(), b"ancien");
        o.confirm_overwrite = true;
        assert!(go(s.path(), d.path(), &o).unwrap().success);
        assert_eq!(fs::read(d.path().join("a.txt")).unwrap(), b"nouveau");
    }

    #[test]
    fn category_filter() {
        let (s, d) = (tempdir().unwrap(), tempdir().unwrap());
        write(s.path(), "a.jpg", b"1");
        write(s.path(), "b.pdf", b"2");
        let o = TransferOptions { categories: Some(vec![Category::Photos]), ..Default::default() };
        let r = go(s.path(), d.path(), &o).unwrap();
        assert_eq!(r.copied, 1);
        assert!(!d.path().join("b.pdf").exists());
    }

    #[test]
    fn traversal_in_a_forged_plan_is_blocked() {
        let (s, d) = (tempdir().unwrap(), tempdir().unwrap());
        let outside = d.path().join("outside.txt");
        write(s.path(), "ok.txt", b"1");
        let dest = d.path().join("dest");
        let plan = Plan {
            root: s.path().to_path_buf(),
            items: vec![PlanItem { rel: "../../etc/passwd".into(), size: 1 }, PlanItem { rel: "ok.txt".into(), size: 1 }],
            total_bytes: 2,
        };
        let r = run(&plan, &dest, &TransferOptions::default(), &OpCtl::new(), &mut |_| {}).unwrap();
        assert_eq!(r.errors.len(), 1);
        assert_eq!(r.copied, 1);
        assert!(!outside.exists());
        assert!(!r.success);
    }

    #[test]
    fn unreadable_source_is_an_error_not_a_success() {
        let (s, d) = (tempdir().unwrap(), tempdir().unwrap());
        write(s.path(), "a.txt", b"1");
        let plan = Plan {
            root: s.path().to_path_buf(),
            items: vec![PlanItem { rel: "a.txt".into(), size: 1 }, PlanItem { rel: "disparu.txt".into(), size: 5 }],
            total_bytes: 6,
        };
        let r = run(&plan, d.path(), &TransferOptions::default(), &OpCtl::new(), &mut |_| {}).unwrap();
        assert_eq!(r.errors.len(), 1);
        assert!(!r.success);
    }

    #[test]
    fn insufficient_space_is_refused_before_copying() {
        let d = tempdir().unwrap();
        assert!(matches!(check_space(d.path(), u64::MAX / 2), Err(GhostError::NoSpace { .. })));
    }

    #[test]
    fn cancellation_stops_and_leaves_no_partial_file() {
        let (s, d) = (tempdir().unwrap(), tempdir().unwrap());
        write(s.path(), "a.bin", &vec![1u8; 4_000_000]);
        let ctl = OpCtl::new();
        let plan = build_plan(s.path(), &None, &ctl).unwrap();
        ctl.cancel();
        let r = run(&plan, d.path(), &TransferOptions::default(), &ctl, &mut |_| {});
        assert!(matches!(r, Err(GhostError::Cancelled)));
        assert!(!d.path().join("a.bin").exists() && !d.path().join("a.bin.ghostpart").exists());
    }

    #[test]
    fn progress_is_driven_by_real_bytes_and_ends_at_100() {
        let (s, d) = (tempdir().unwrap(), tempdir().unwrap());
        write(s.path(), "a.bin", &vec![1u8; 2_500_000]);
        write(s.path(), "b.bin", &vec![2u8; 100]);
        let ctl = OpCtl::new();
        let plan = build_plan(s.path(), &None, &ctl).unwrap();
        let mut last = Progress::default();
        let mut prev = 0.0;
        run(&plan, d.path(), &TransferOptions::default(), &ctl, &mut |p| {
            assert!(p.percent + 1e-9 >= prev, "la progression ne doit jamais reculer");
            prev = p.percent;
            last = p;
        })
        .unwrap();
        assert_eq!(last.bytes_done, plan.total_bytes);
        assert!((last.percent - 100.0).abs() < 1e-9);
        assert_eq!(last.files_done, 2);
    }
}
