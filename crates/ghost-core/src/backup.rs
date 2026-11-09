//! Ghost Backup : sauvegardes locales via les mecanismes officiels Apple
//! (idevicebackup2 / protocole de sauvegarde), manifestes SHA-256, historique des versions,
//! verification d'integrite, export chiffre AES-256-GCM, suppression securisee.
//!
//! Disposition : `<racine>/<hid>/data/<UDID>/…` (donnees Apple) et
//! `<racine>/<hid>/versions/<id>.json` (un manifeste d'integrite par sauvegarde).

use crate::{
    crypto::{decrypt_stream, encrypt_stream, KdfParams},
    devices::{DeviceBackend, DeviceInfo},
    hashing::{sha256_bytes, sha256_file},
    progress::{OpCtl, Progress, Tracker},
    security::{safe_join, secure_delete_dir, secure_delete_file, validate_udid},
    transfer::check_space,
    GhostError, Result,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ManifestFile {
    pub rel: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VersionRecord {
    pub id: String,
    pub hid: String,
    pub source_udid: String,
    pub created: String,
    pub device_name: String,
    pub os_version: String,
    pub encrypted: bool,
    pub simulated: bool,
    pub files: u64,
    pub bytes: u64,
    pub new_or_changed: u64,
    pub manifest: Vec<ManifestFile>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub hid: String,
    pub version: String,
    pub source_udid: String,
    pub created: String,
    pub device_name: String,
    pub os_version: String,
    pub encrypted: bool,
    pub simulated: bool,
    pub files: u64,
    pub bytes: u64,
    pub new_or_changed: u64,
    pub data_dir: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct VerifyReport {
    pub checked: u64,
    pub ok: u64,
    pub missing: Vec<String>,
    pub modified: Vec<String>,
    pub intact: bool,
}

pub struct BackupManager {
    pub root: PathBuf,
}

fn hid_of(udid: &str) -> String {
    sha256_bytes(udid.as_bytes())[..12].to_string()
}

fn check_component(s: &str, what: &str) -> Result<()> {
    if s.is_empty() || s.len() > 64 || !s.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(GhostError::Security(format!("{what} invalide")));
    }
    Ok(())
}

impl BackupManager {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn dir(&self, hid: &str) -> Result<PathBuf> {
        check_component(hid, "identifiant de sauvegarde")?;
        safe_join(&self.root, hid)
    }

    fn info_of(&self, r: &VersionRecord) -> BackupInfo {
        BackupInfo {
            hid: r.hid.clone(),
            version: r.id.clone(),
            source_udid: r.source_udid.clone(),
            created: r.created.clone(),
            device_name: r.device_name.clone(),
            os_version: r.os_version.clone(),
            encrypted: r.encrypted,
            simulated: r.simulated,
            files: r.files,
            bytes: r.bytes,
            new_or_changed: r.new_or_changed,
            data_dir: self.root.join(&r.hid).join("data").display().to_string(),
        }
    }

    fn read_record(&self, hid: &str, version: &str) -> Result<VersionRecord> {
        check_component(version, "version")?;
        let p = self.dir(hid)?.join("versions").join(format!("{version}.json"));
        Ok(serde_json::from_reader(BufReader::new(File::open(&p).map_err(|_| {
            GhostError::Data("version de sauvegarde introuvable".into())
        })?))?)
    }

    fn records(&self, hid: &str) -> Vec<VersionRecord> {
        let Ok(dir) = self.dir(hid).map(|d| d.join("versions")) else { return vec![] };
        let mut v: Vec<VersionRecord> = fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| File::open(e.path()).ok())
            .filter_map(|f| serde_json::from_reader(BufReader::new(f)).ok())
            .collect();
        v.sort_by(|a, b| a.created.cmp(&b.created));
        v
    }

    pub fn list(&self) -> Vec<BackupInfo> {
        let mut all = vec![];
        for e in fs::read_dir(&self.root).into_iter().flatten().flatten() {
            if let Some(hid) = e.file_name().to_str() {
                all.extend(self.records(hid).iter().map(|r| self.info_of(r)));
            }
        }
        all.sort_by(|a, b| b.created.cmp(&a.created));
        all
    }

    pub fn data_root(&self, hid: &str) -> Result<PathBuf> {
        Ok(self.dir(hid)?.join("data"))
    }

    fn build_manifest(
        &self,
        root: &Path,
        ctl: &OpCtl,
        simulated: bool,
        on: &mut dyn FnMut(Progress),
    ) -> Result<Vec<ManifestFile>> {
        let paths: Vec<PathBuf> = WalkDir::new(root)
            .follow_links(false)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .collect();
        let total: u64 = paths.iter().filter_map(|p| fs::metadata(p).ok()).map(|m| m.len()).sum();
        let mut tr = Tracker::new("Calcul des empreintes SHA-256", paths.len() as u64, total, simulated);
        let mut out = vec![];
        for p in paths {
            ctl.check()?;
            let size = fs::metadata(&p)?.len();
            out.push(ManifestFile {
                rel: p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"),
                size,
                sha256: sha256_file(&p)?,
            });
            tr.p.files_done += 1;
            tr.p.bytes_done += size;
            on(tr.snapshot());
        }
        Ok(out)
    }

    pub fn create(
        &self,
        backend: &dyn DeviceBackend,
        dev: &DeviceInfo,
        password: Option<&str>,
        ctl: &OpCtl,
        on: &mut dyn FnMut(Progress),
    ) -> Result<BackupInfo> {
        validate_udid(&dev.udid)?;
        if !dev.connected || !dev.trusted {
            return Err(GhostError::Device("l'appareil n'est pas connecté et approuvé".into()));
        }
        let hid = hid_of(&dev.udid);
        let dir = self.dir(&hid)?;
        let data = dir.join("data");
        fs::create_dir_all(&data)?;
        fs::create_dir_all(dir.join("versions"))?;
        // Un appareil simulé n'écrit pas réellement les Go qu'il annonce : pas de contrôle.
        if let (Some(used), false) = (dev.used_bytes, dev.simulated) {
            if !data.join(&dev.udid).exists() {
                check_space(&self.root, used)?;
            }
        }
        let encrypted = password.map(|p| !p.is_empty()).unwrap_or(false);
        if encrypted {
            backend.set_backup_encryption(&dev.udid, password.unwrap())?;
        }
        backend.backup(&dev.udid, &data, ctl, on)?;

        let manifest = self.build_manifest(&data.join(&dev.udid), ctl, dev.simulated, on)?;
        let previous: HashMap<String, String> = self
            .records(&hid)
            .last()
            .map(|r| r.manifest.iter().map(|f| (f.rel.clone(), f.sha256.clone())).collect())
            .unwrap_or_default();
        let new_or_changed = manifest.iter().filter(|f| previous.get(&f.rel) != Some(&f.sha256)).count() as u64;
        let now = chrono::Utc::now();
        let rec = VersionRecord {
            id: now.format("%Y%m%dT%H%M%S%3fZ").to_string(),
            hid: hid.clone(),
            source_udid: dev.udid.clone(),
            created: now.to_rfc3339(),
            device_name: dev.name.clone(),
            os_version: dev.os_version.clone(),
            encrypted,
            simulated: dev.simulated,
            files: manifest.len() as u64,
            bytes: manifest.iter().map(|f| f.size).sum(),
            new_or_changed,
            manifest,
        };
        let path = dir.join("versions").join(format!("{}.json", rec.id));
        serde_json::to_writer(BufWriter::new(File::create(&path)?), &rec)?;
        Ok(self.info_of(&rec))
    }

    pub fn verify(&self, hid: &str, version: &str, ctl: &OpCtl, on: &mut dyn FnMut(Progress)) -> Result<VerifyReport> {
        let rec = self.read_record(hid, version)?;
        let root = self.data_root(hid)?.join({
            validate_udid(&rec.source_udid)?;
            &rec.source_udid
        });
        let total: u64 = rec.manifest.iter().map(|f| f.size).sum();
        let mut tr = Tracker::new("Vérification d'intégrité", rec.manifest.len() as u64, total, rec.simulated);
        let mut rep = VerifyReport::default();
        for f in &rec.manifest {
            ctl.check()?;
            rep.checked += 1;
            match safe_join(&root, &f.rel) {
                Ok(p) if p.is_file() => match sha256_file(&p) {
                    Ok(h) if h == f.sha256 => {
                        rep.ok += 1;
                        tr.p.verified += 1;
                    }
                    _ => rep.modified.push(f.rel.clone()),
                },
                _ => rep.missing.push(f.rel.clone()),
            }
            tr.p.files_done += 1;
            tr.p.bytes_done += f.size;
            tr.p.errors = (rep.missing.len() + rep.modified.len()) as u64;
            on(tr.snapshot());
        }
        rep.intact = rep.missing.is_empty() && rep.modified.is_empty();
        Ok(rep)
    }

    pub fn delete(&self, hid: &str) -> Result<()> {
        secure_delete_dir(&self.dir(hid)?)
    }

    /// Archive chiffree (tar + AES-256-GCM / Argon2id) de TOUTE la sauvegarde `hid`.
    pub fn export_encrypted(&self, hid: &str, out: &Path, password: &str, params: KdfParams, ctl: &OpCtl) -> Result<()> {
        let dir = self.dir(hid)?;
        if !dir.is_dir() {
            return Err(GhostError::Data("sauvegarde introuvable".into()));
        }
        if password.chars().count() < 8 {
            return Err(GhostError::Security("le mot de passe d'export doit contenir au moins 8 caractères".into()));
        }
        let tmp_tar = out.with_extension("ghosttmp");
        let result = (|| -> Result<()> {
            {
                let mut b = tar::Builder::new(BufWriter::new(File::create(&tmp_tar)?));
                b.follow_symlinks(false);
                b.append_dir_all(hid, &dir)?;
                b.into_inner()?.into_inner().map_err(|e| e.into_error())?.sync_all()?;
            }
            ctl.check()?;
            let part = out.with_extension("ghostpart");
            encrypt_stream(BufReader::new(File::open(&tmp_tar)?), BufWriter::new(File::create(&part)?), password, params)?;
            fs::rename(&part, out)?;
            Ok(())
        })();
        let _ = secure_delete_file(&tmp_tar);
        if result.is_err() {
            let _ = fs::remove_file(out.with_extension("ghostpart"));
        }
        result
    }

    /// Importe une archive chiffree. Refuse chemins dangereux, liens et ecrasement non confirme.
    pub fn import_encrypted(&self, file: &Path, password: &str, overwrite: bool) -> Result<String> {
        fs::create_dir_all(&self.root)?;
        let tmp_tar = file.with_extension("ghostdec");
        let result = (|| -> Result<String> {
            decrypt_stream(BufReader::new(File::open(file)?), BufWriter::new(File::create(&tmp_tar)?), password)?;
            let mut hid_seen: Option<String> = None;
            let mut ar = tar::Archive::new(BufReader::new(File::open(&tmp_tar)?));
            // Premier passage : validation complete AVANT toute ecriture.
            for e in ar.entries()? {
                let e = e?;
                let t = e.header().entry_type();
                if !(t.is_file() || t.is_dir()) {
                    return Err(GhostError::Security("l'archive contient un élément non autorisé (lien ou fichier spécial)".into()));
                }
                let path = e.path()?.to_string_lossy().replace('\\', "/");
                let first = path.split('/').next().unwrap_or("");
                check_component(first, "dossier racine de l'archive")?;
                safe_join(&self.root, &path)?;
                match &hid_seen {
                    None => hid_seen = Some(first.to_string()),
                    Some(h) if h != first => return Err(GhostError::Security("l'archive contient plusieurs racines".into())),
                    _ => {}
                }
            }
            let hid = hid_seen.ok_or_else(|| GhostError::Data("archive vide".into()))?;
            let target = self.dir(&hid)?;
            if target.exists() && !overwrite {
                return Err(GhostError::Security(
                    "cette sauvegarde existe déjà ; l'écrasement exige une confirmation explicite".into(),
                ));
            }
            let mut ar = tar::Archive::new(BufReader::new(File::open(&tmp_tar)?));
            for e in ar.entries()? {
                let mut e = e?;
                let path = e.path()?.to_string_lossy().replace('\\', "/");
                let dest = safe_join(&self.root, &path)?;
                if e.header().entry_type().is_dir() {
                    fs::create_dir_all(&dest)?;
                } else {
                    if let Some(p) = dest.parent() {
                        fs::create_dir_all(p)?;
                    }
                    e.unpack(&dest)?;
                }
            }
            Ok(hid)
        })();
        let _ = secure_delete_file(&tmp_tar);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{SimBackend, SIM_SOURCE};

    fn setup() -> (tempfile::TempDir, BackupManager, SimBackend, DeviceInfo) {
        let d = tempfile::tempdir().unwrap();
        let m = BackupManager::new(d.path().join("backups"));
        let sim = SimBackend::new(0);
        let dev = sim.list().unwrap().remove(0);
        (d, m, sim, dev)
    }

    #[test]
    fn create_list_verify() {
        let (_d, m, sim, dev) = setup();
        let info = m.create(&sim, &dev, None, &OpCtl::new(), &mut |_| {}).unwrap();
        assert!(info.files > 30 && info.new_or_changed == info.files);
        assert_eq!(m.list().len(), 1);
        let rep = m.verify(&info.hid, &info.version, &OpCtl::new(), &mut |_| {}).unwrap();
        assert!(rep.intact && rep.ok == rep.checked);
    }

    #[test]
    fn second_backup_is_incremental_and_versions_accumulate() {
        let (_d, m, sim, dev) = setup();
        let a = m.create(&sim, &dev, None, &OpCtl::new(), &mut |_| {}).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let b = m.create(&sim, &dev, None, &OpCtl::new(), &mut |_| {}).unwrap();
        assert_eq!(b.new_or_changed, 0, "rien n'a changé depuis la première sauvegarde");
        assert_ne!(a.version, b.version);
        assert_eq!(m.list().len(), 2);
    }

    #[test]
    fn corruption_and_deletion_are_detected() {
        let (_d, m, sim, dev) = setup();
        let info = m.create(&sim, &dev, None, &OpCtl::new(), &mut |_| {}).unwrap();
        let root = m.data_root(&info.hid).unwrap().join(SIM_SOURCE);
        fs::write(root.join("Contacts/AddressBook.vcf"), b"altere").unwrap();
        fs::remove_file(root.join("Calendar/events.ics")).unwrap();
        let rep = m.verify(&info.hid, &info.version, &OpCtl::new(), &mut |_| {}).unwrap();
        assert!(!rep.intact);
        assert_eq!(rep.modified, vec!["Contacts/AddressBook.vcf"]);
        assert_eq!(rep.missing, vec!["Calendar/events.ics"]);
    }

    #[test]
    fn untrusted_device_cannot_be_backed_up() {
        let (_d, m, sim, mut dev) = setup();
        dev.trusted = false;
        assert!(m.create(&sim, &dev, None, &OpCtl::new(), &mut |_| {}).is_err());
    }

    #[test]
    fn hostile_identifiers_are_rejected() {
        let (_d, m, ..) = setup();
        assert!(m.verify("../../etc", "x", &OpCtl::new(), &mut |_| {}).is_err());
        assert!(m.delete("..").is_err());
        assert!(m.data_root("a/b").is_err());
    }

    #[test]
    fn encrypted_export_import_roundtrip() {
        let (d, m, sim, dev) = setup();
        let info = m.create(&sim, &dev, Some("mot-de-passe-solide"), &OpCtl::new(), &mut |_| {}).unwrap();
        assert!(info.encrypted);
        let out = d.path().join("export.ghost");
        m.export_encrypted(&info.hid, &out, "export-secret-1", KdfParams::light(), &OpCtl::new()).unwrap();
        assert!(!out.with_extension("ghosttmp").exists(), "le tar temporaire doit être supprimé");
        let raw = fs::read(&out).unwrap();
        assert!(raw.starts_with(b"GHOSTENC1"));
        assert!(!raw.windows(8).any(|w| w == b"ustar\0\x30"), "aucun contenu tar en clair");

        let m2 = BackupManager::new(d.path().join("autre"));
        assert!(m2.import_encrypted(&out, "mauvais-mdp-xx", false).is_err());
        let hid = m2.import_encrypted(&out, "export-secret-1", false).unwrap();
        assert_eq!(hid, info.hid);
        let rep = m2.verify(&hid, &info.version, &OpCtl::new(), &mut |_| {}).unwrap();
        assert!(rep.intact, "{rep:?}");
        // pas d'ecrasement silencieux
        assert!(m2.import_encrypted(&out, "export-secret-1", false).is_err());
        assert!(m2.import_encrypted(&out, "export-secret-1", true).is_ok());
    }

    #[test]
    fn weak_export_password_is_refused() {
        let (d, m, sim, dev) = setup();
        let info = m.create(&sim, &dev, None, &OpCtl::new(), &mut |_| {}).unwrap();
        assert!(m.export_encrypted(&info.hid, &d.path().join("e"), "court", KdfParams::light(), &OpCtl::new()).is_err());
    }

    #[test]
    fn malicious_archive_with_path_traversal_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        // tar forge a la main avec un nom « abc/../../evil.txt »
        let mut tarbytes = vec![];
        {
            let mut b = tar::Builder::new(&mut tarbytes);
            let mut h = tar::Header::new_gnu();
            let name = b"abc/../../evil.txt";
            h.as_old_mut().name[..name.len()].copy_from_slice(name);
            h.set_size(4);
            h.set_mode(0o644);
            h.set_entry_type(tar::EntryType::Regular);
            h.set_cksum();
            b.append(&h, &b"evil"[..]).unwrap();
            b.finish().unwrap();
        }
        let enc = crate::crypto::encrypt_bytes(&tarbytes, "motdepasse-ok", KdfParams::light()).unwrap();
        let f = d.path().join("evil.ghost");
        fs::write(&f, enc).unwrap();
        let m = BackupManager::new(d.path().join("root"));
        assert!(matches!(m.import_encrypted(&f, "motdepasse-ok", true), Err(GhostError::Security(_))));
        assert!(!d.path().join("evil.txt").exists());
    }

    #[test]
    fn secure_delete_removes_backup() {
        let (_d, m, sim, dev) = setup();
        let info = m.create(&sim, &dev, None, &OpCtl::new(), &mut |_| {}).unwrap();
        m.delete(&info.hid).unwrap();
        assert!(m.list().is_empty());
    }
}
