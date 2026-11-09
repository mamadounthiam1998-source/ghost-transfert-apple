//! Detection et operations sur les appareils Apple.
//!
//! Deux implementations de `DeviceBackend` :
//! * `LibimobiledeviceBackend` : appareils reels (idevice_id, ideviceinfo, idevicebackup2).
//! * `SimBackend` : mode simulation EXPLICITE (appareils fictifs, marques `simulated`).
//!   Il n'est jamais active automatiquement.

use crate::{
    hashing::sha256_file,
    progress::{OpCtl, Progress, Tracker},
    security::validate_udid,
    tools::Tools,
    GhostError, Result,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};
use walkdir::WalkDir;

/// Volume de données par catégorie, tel que l'appareil le déclare lui-même.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageCategory {
    pub key: String,
    pub label: String,
    pub bytes: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub udid: String,
    pub name: String,
    pub model: String,
    pub product_type: String,
    pub os_version: String,
    /// Vrai seulement si l'appareil a ete reellement vu par le service USB.
    pub connected: bool,
    /// Vrai seulement si ideviceinfo a reussi (appairage approuve).
    pub trusted: bool,
    pub capacity_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
    /// Volume par catégorie lu sur l'appareil (vide si l'appareil ne l'expose pas).
    pub usage: Vec<UsageCategory>,
    pub simulated: bool,
    pub operations: Vec<String>,
    pub note: Option<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Compatibility {
    pub ok: bool,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RestoreOptions {
    pub system: bool,
    pub settings: bool,
    pub reboot: bool,
    pub source_udid: Option<String>,
    pub password: Option<String>,
}

pub trait DeviceBackend: Send + Sync {
    fn is_simulation(&self) -> bool;
    fn list(&self) -> Result<Vec<DeviceInfo>>;
    /// Cree/actualise la sauvegarde dans `dest/<UDID>/` (incremental si elle existe).
    fn backup(&self, udid: &str, dest: &Path, ctl: &OpCtl, on: &mut dyn FnMut(Progress)) -> Result<()>;
    /// Restaure `src/<UDID source>/` sur l'appareil `udid`. DESTRUCTIF pour la destination.
    fn restore(
        &self,
        udid: &str,
        src: &Path,
        opts: &RestoreOptions,
        ctl: &OpCtl,
        on: &mut dyn FnMut(Progress),
    ) -> Result<()>;
    fn set_backup_encryption(&self, udid: &str, password: &str) -> Result<()>;
    /// Demande l'approbation « Se fier à cet ordinateur » sur l'appareil (appairage).
    fn pair(&self, udid: &str) -> Result<String>;
}

pub fn friendly_model(product_type: &str) -> String {
    let n = match product_type {
        "iPhone12,1" => "iPhone 11",
        "iPhone12,3" => "iPhone 11 Pro",
        "iPhone12,5" => "iPhone 11 Pro Max",
        "iPhone13,1" => "iPhone 12 mini",
        "iPhone13,2" => "iPhone 12",
        "iPhone13,3" => "iPhone 12 Pro",
        "iPhone13,4" => "iPhone 12 Pro Max",
        "iPhone14,4" => "iPhone 13 mini",
        "iPhone14,5" => "iPhone 13",
        "iPhone14,2" => "iPhone 13 Pro",
        "iPhone14,3" => "iPhone 13 Pro Max",
        "iPhone14,7" => "iPhone 14",
        "iPhone14,8" => "iPhone 14 Plus",
        "iPhone15,2" => "iPhone 14 Pro",
        "iPhone15,3" => "iPhone 14 Pro Max",
        "iPhone15,4" => "iPhone 15",
        "iPhone15,5" => "iPhone 15 Plus",
        "iPhone16,1" => "iPhone 15 Pro",
        "iPhone16,2" => "iPhone 15 Pro Max",
        other => return other.to_string(),
    };
    n.to_string()
}

fn major(v: &str) -> u32 {
    v.split('.').next().and_then(|s| s.parse().ok()).unwrap_or(0)
}

/// Verifie qu'une migration source -> destination est raisonnable AVANT de commencer.
pub fn check_compatibility(src: &DeviceInfo, dst: &DeviceInfo) -> Compatibility {
    let mut c = Compatibility::default();
    if src.udid == dst.udid {
        c.blockers.push("La source et la destination sont le même appareil.".into());
    }
    for (d, role) in [(src, "source"), (dst, "destination")] {
        if !d.connected {
            c.blockers.push(format!("L'appareil {role} n'est pas connecté."));
        } else if !d.trusted {
            c.blockers.push(format!(
                "L'appareil {role} n'a pas approuvé cet ordinateur (déverrouillez-le et touchez « Se fier »)."
            ));
        }
    }
    if major(&dst.os_version) > 0 && major(&src.os_version) > major(&dst.os_version) {
        c.blockers.push(format!(
            "iOS {} (source) est plus récent qu'iOS {} (destination) : Apple n'autorise pas cette restauration. Mettez à jour la destination.",
            src.os_version, dst.os_version
        ));
    }
    if let (Some(used), Some(cap)) = (src.used_bytes, dst.capacity_bytes) {
        if used > cap {
            c.blockers.push("La capacité de la destination est inférieure aux données de la source.".into());
        } else if used as f64 > cap as f64 * 0.9 {
            c.warnings.push("La destination sera presque pleine après la migration.".into());
        }
    }
    if src.product_type.starts_with("iPhone") != dst.product_type.starts_with("iPhone") {
        c.warnings.push(
            "Migration iPhone ↔ iPad : certaines données propres à un type d'appareil ne seront pas restaurées.".into(),
        );
    }
    if src.simulated || dst.simulated {
        c.warnings.push("Mode simulation : aucun appareil réel n'est concerné.".into());
    }
    c.ok = c.blockers.is_empty();
    c
}

pub fn parse_key_values(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

fn usage_label(key: &str) -> String {
    match key {
        "CameraUsage" => "Photos et vidéos (appareil photo)".into(),
        "PhotoUsage" => "Photos (photothèque)".into(),
        "MobileApplicationUsage" => "Applications".into(),
        "CalendarUsage" => "Calendrier".into(),
        "VoicemailUsage" => "Messagerie vocale".into(),
        "WebAppCacheUsage" => "Cache des apps web".into(),
        other => other.strip_suffix("Usage").unwrap_or(other).to_string(),
    }
}

/// Construit le volume par categorie a partir du domaine `com.apple.disk_usage`.
/// Toute cle `*Usage` numerique est reprise ; le reste de l'espace utilise est
/// presente comme « Autres données » (systeme, messages, documents…), jamais invente.
pub fn usage_from_disk_usage(kv: &HashMap<String, String>, used: Option<u64>) -> Vec<UsageCategory> {
    let mut v: Vec<UsageCategory> = kv
        .iter()
        .filter(|(k, _)| k.ends_with("Usage"))
        .filter_map(|(k, val)| val.parse::<u64>().ok().map(|b| (k, b)))
        .filter(|(_, b)| *b > 0)
        .map(|(k, b)| UsageCategory { key: k.clone(), label: usage_label(k), bytes: b })
        .collect();
    v.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.key.cmp(&b.key)));
    if let Some(used) = used {
        let known: u64 = v.iter().map(|c| c.bytes).sum();
        if used > known {
            v.push(UsageCategory {
                key: "Other".into(),
                label: "Autres données (système, messages, documents…)".into(),
                bytes: used - known,
            });
        }
    }
    v
}

fn parse_percent(line: &str) -> Option<f64> {
    let idx = line.find('%')?;
    let digits: String = line[..idx].chars().rev().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let digits: String = digits.chars().rev().collect();
    digits.parse::<f64>().ok().filter(|p| (0.0..=100.0).contains(p))
}

// ---------------------------------------------------------------------------
// Appareils reels
// ---------------------------------------------------------------------------

pub struct LibimobiledeviceBackend {
    pub tools: Tools,
}

impl LibimobiledeviceBackend {
    pub fn new(tools: Tools) -> Self {
        Self { tools }
    }

    fn info_for(&self, udid: &str) -> DeviceInfo {
        let mut d = DeviceInfo {
            udid: udid.into(),
            name: "Appareil Apple".into(),
            model: "Inconnu".into(),
            product_type: String::new(),
            os_version: String::new(),
            connected: true, // vu par idevice_id -l
            trusted: false,
            capacity_bytes: None,
            used_bytes: None,
            free_bytes: None,
            usage: vec![],
            simulated: false,
            operations: vec![],
            note: None,
        };
        match self.tools.run("ideviceinfo", &["-u", udid], Duration::from_secs(10)) {
            Ok(o) if o.code == 0 => {
                let kv = parse_key_values(&o.stdout);
                d.trusted = true;
                d.name = kv.get("DeviceName").cloned().unwrap_or(d.name);
                d.product_type = kv.get("ProductType").cloned().unwrap_or_default();
                d.model = friendly_model(&d.product_type);
                d.os_version = kv.get("ProductVersion").cloned().unwrap_or_default();
                d.operations = vec!["backup".into(), "restore".into(), "clone".into()];
                if let Ok(o) = self.tools.run("ideviceinfo", &["-u", udid, "-q", "com.apple.disk_usage"], Duration::from_secs(10)) {
                    let kv = parse_key_values(&o.stdout);
                    let num = |k: &str| kv.get(k).and_then(|v| v.parse::<u64>().ok());
                    d.capacity_bytes = num("TotalDiskCapacity");
                    d.free_bytes = num("TotalDataAvailable");
                    if let (Some(t), Some(f)) = (num("TotalDataCapacity"), d.free_bytes) {
                        d.used_bytes = Some(t.saturating_sub(f));
                    }
                    d.usage = usage_from_disk_usage(&kv, d.used_bytes);
                }
            }
            Ok(o) => {
                let msg = format!("{}{}", o.stderr, o.stdout).to_lowercase();
                d.note = Some(if msg.contains("trust") || msg.contains("pair") || msg.contains("password") {
                    "Déverrouillez l'appareil puis touchez « Se fier à cet ordinateur ».".into()
                } else {
                    "L'appareil est détecté mais ses informations sont inaccessibles.".into()
                });
            }
            Err(e) => d.note = Some(e.to_string()),
        }
        d
    }
}

impl DeviceBackend for LibimobiledeviceBackend {
    fn is_simulation(&self) -> bool {
        false
    }

    fn list(&self) -> Result<Vec<DeviceInfo>> {
        let o = self.tools.run("idevice_id", &["-l"], Duration::from_secs(8))?;
        if o.code != 0 {
            let detail = crate::security::redact(format!("{}{}", o.stderr, o.stdout).trim());
            return Err(GhostError::Device(format!(
                "le service USB Apple ne répond pas : installez « Appareils Apple » (Windows) ou vérifiez le service USB, puis ouvrez Diagnostic. {detail}"
            )));
        }
        Ok(o.stdout
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && validate_udid(l).is_ok())
            .map(|u| self.info_for(u))
            .collect())
    }

    fn backup(&self, udid: &str, dest: &Path, ctl: &OpCtl, on: &mut dyn FnMut(Progress)) -> Result<()> {
        validate_udid(udid)?;
        fs::create_dir_all(dest)?;
        let d = dest.to_string_lossy().into_owned();
        let mut tr = Tracker::new("Sauvegarde de l'appareil", 0, 0, false);
        on(tr.snapshot());
        let mut last = String::new();
        let (code, stderr) = self.tools.run_streaming(
            "idevicebackup2",
            &["-u", udid, "backup", &d],
            ctl,
            &mut |line| {
                last = line.to_string();
                if let Some(p) = parse_percent(line) {
                    tr.p.percent = p;
                    tr.p.message = Some(line.trim().to_string());
                    let mut s = tr.p.clone();
                    s.percent = p;
                    on(s);
                }
            },
        )?;
        if code != 0 {
            return Err(GhostError::Device(format!(
                "idevicebackup2 a échoué (code {code}) : {}",
                if stderr.trim().is_empty() { last } else { stderr.trim().to_string() }
            )));
        }
        tr.p.percent = 100.0;
        tr.p.phase = "Sauvegarde terminée".into();
        on(tr.p.clone());
        Ok(())
    }

    fn restore(
        &self,
        udid: &str,
        src: &Path,
        opts: &RestoreOptions,
        ctl: &OpCtl,
        on: &mut dyn FnMut(Progress),
    ) -> Result<()> {
        validate_udid(udid)?;
        let s = src.to_string_lossy().into_owned();
        let mut args: Vec<String> = vec!["-u".into(), udid.into()];
        if let Some(source) = &opts.source_udid {
            validate_udid(source)?;
            args.extend(["-s".into(), source.clone()]);
        }
        args.push("restore".into());
        if opts.system {
            args.push("--system".into());
        }
        if opts.settings {
            args.push("--settings".into());
        }
        if !opts.reboot {
            args.push("--no-reboot".into());
        }
        if let Some(pw) = &opts.password {
            args.extend(["--password".into(), pw.clone()]);
        }
        args.push(s);
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut tr = Tracker::new("Restauration sur l'appareil", 0, 0, false);
        on(tr.snapshot());
        let mut last = String::new();
        let (code, stderr) = self.tools.run_streaming("idevicebackup2", &argv, ctl, &mut |line| {
            last = line.to_string();
            if let Some(p) = parse_percent(line) {
                tr.p.message = Some(line.trim().to_string());
                let mut s = tr.p.clone();
                s.percent = p;
                on(s);
            }
        })?;
        if code != 0 {
            return Err(GhostError::Device(format!(
                "la restauration a échoué (code {code}) : {}",
                if stderr.trim().is_empty() { last } else { stderr.trim().to_string() }
            )));
        }
        Ok(())
    }

    fn pair(&self, udid: &str) -> Result<String> {
        validate_udid(udid)?;
        let o = self.tools.run("idevicepair", &["-u", udid, "pair"], Duration::from_secs(30))?;
        let msg = crate::security::redact(format!("{}{}", o.stdout, o.stderr).trim());
        if o.code == 0 {
            Ok(if msg.is_empty() { "Appairage effectué.".into() } else { msg })
        } else {
            Err(GhostError::Device(format!(
                "Touchez « Se fier » sur l'appareil déverrouillé (saisissez son code si demandé), puis recommencez. Détail : {msg}"
            )))
        }
    }

    fn set_backup_encryption(&self, udid: &str, password: &str) -> Result<()> {
        validate_udid(udid)?;
        let o = self.tools.run("idevicebackup2", &["-u", udid, "encryption", "on", password], Duration::from_secs(30))?;
        // Code 0 egalement quand le chiffrement est deja actif ; on lit la sortie en cas d'echec.
        if o.code != 0 {
            return Err(GhostError::Device(format!(
                "activation du chiffrement impossible : {}",
                format!("{}{}", o.stderr, o.stdout).trim()
            )));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Mode simulation (explicite)
// ---------------------------------------------------------------------------

pub struct SimBackend {
    pub delay_ms: u64,
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn fill(&mut self, n: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(n + 8);
        while v.len() < n {
            v.extend_from_slice(&self.next().to_le_bytes());
        }
        v.truncate(n);
        v
    }
}

pub const SIM_SOURCE: &str = "00008110-000A1B2C3D4E5F01";
pub const SIM_DEST: &str = "00008120-000F9E8D7C6B5A02";

impl SimBackend {
    pub fn new(delay_ms: u64) -> Self {
        Self { delay_ms }
    }

    fn device(udid: &str, name: &str, pt: &str, os: &str, cap_gb: u64, used_gb: u64) -> DeviceInfo {
        let gb = 1_000_000_000u64;
        DeviceInfo {
            udid: udid.into(),
            name: name.into(),
            model: friendly_model(pt),
            product_type: pt.into(),
            os_version: os.into(),
            connected: true,
            trusted: true,
            capacity_bytes: Some(cap_gb * gb),
            used_bytes: Some(used_gb * gb),
            free_bytes: Some((cap_gb - used_gb) * gb),
            usage: usage_from_disk_usage(
                &[
                    ("CameraUsage", used_gb * gb * 42 / 100),
                    ("PhotoUsage", used_gb * gb * 6 / 100),
                    ("MobileApplicationUsage", used_gb * gb * 30 / 100),
                    ("CalendarUsage", 12_000_000),
                ]
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
                Some(used_gb * gb),
            ),
            simulated: true,
            operations: vec!["backup".into(), "restore".into(), "clone".into()],
            note: Some("Appareil FICTIF (mode simulation).".into()),
        }
    }

    fn files(seed: u64) -> Vec<(String, usize)> {
        let mut rng = Rng(seed | 1);
        let mut v = vec![];
        for i in 0..24 {
            let ext = if i % 3 == 0 { "HEIC" } else { "JPG" };
            v.push((format!("Media/DCIM/100APPLE/IMG_{:04}.{ext}", 1000 + i), 60_000 + (rng.next() % 90_000) as usize));
        }
        for i in 0..3 {
            v.push((format!("Media/DCIM/100APPLE/MOV_{:04}.MOV", 2000 + i), 600_000 + (rng.next() % 300_000) as usize));
        }
        for i in 0..8 {
            v.push((format!("Documents/doc_{i}.pdf"), 5_000 + (rng.next() % 20_000) as usize));
        }
        v.push(("Contacts/AddressBook.vcf".into(), 12_000));
        v.push(("Calendar/events.ics".into(), 6_000));
        v.push(("Library/SMS/sms.db".into(), 30_000));
        v.push(("Library/Preferences/settings.plist".into(), 2_000));
        v
    }
}

impl DeviceBackend for SimBackend {
    fn is_simulation(&self) -> bool {
        true
    }

    fn list(&self) -> Result<Vec<DeviceInfo>> {
        Ok(vec![
            Self::device(SIM_SOURCE, "iPhone de Camille (simulé)", "iPhone15,2", "17.5.1", 256, 118),
            Self::device(SIM_DEST, "iPhone neuf (simulé)", "iPhone16,1", "18.0", 512, 4),
        ])
    }

    fn backup(&self, udid: &str, dest: &Path, ctl: &OpCtl, on: &mut dyn FnMut(Progress)) -> Result<()> {
        validate_udid(udid)?;
        let seed = u64::from_str_radix(&udid.replace('-', "")[..15.min(udid.replace('-', "").len())], 16).unwrap_or(7);
        let files = Self::files(seed);
        let total: u64 = files.iter().map(|f| f.1 as u64).sum();
        let mut tr = Tracker::new("Sauvegarde (simulation)", files.len() as u64, total, true);
        let root = dest.join(udid);
        let mut rng = Rng(seed | 1);
        for (rel, size) in files {
            ctl.check()?;
            tr.p.current = Some(rel.clone());
            let p = crate::security::safe_join(&root, &rel)?;
            fs::create_dir_all(p.parent().unwrap())?;
            let data = rng.fill(size);
            let mut f = fs::File::create(&p)?;
            f.write_all(&data)?;
            f.sync_all()?;
            tr.p.bytes_done += size as u64;
            tr.p.files_done += 1;
            on(tr.snapshot());
            if self.delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(self.delay_ms));
            }
        }
        tr.p.current = None;
        tr.p.phase = "Sauvegarde terminée (simulation)".into();
        on(tr.snapshot());
        Ok(())
    }

    fn restore(
        &self,
        udid: &str,
        src: &Path,
        opts: &RestoreOptions,
        ctl: &OpCtl,
        on: &mut dyn FnMut(Progress),
    ) -> Result<()> {
        validate_udid(udid)?;
        let source = opts.source_udid.clone().unwrap_or_else(|| udid.to_string());
        validate_udid(&source)?;
        let root = src.join(&source);
        if !root.is_dir() {
            return Err(GhostError::Device(format!("sauvegarde introuvable : {}", root.display())));
        }
        let items: Vec<PathBuf> = WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .collect();
        let total: u64 = items.iter().filter_map(|p| fs::metadata(p).ok()).map(|m| m.len()).sum();
        let mut tr = Tracker::new("Restauration (simulation)", items.len() as u64, total, true);
        for p in items {
            ctl.check()?;
            sha256_file(&p)?; // relecture reelle de chaque fichier
            tr.p.bytes_done += fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            tr.p.files_done += 1;
            on(tr.snapshot());
            if self.delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(self.delay_ms));
            }
        }
        Ok(())
    }

    fn set_backup_encryption(&self, _udid: &str, _password: &str) -> Result<()> {
        Ok(())
    }

    fn pair(&self, _udid: &str) -> Result<String> {
        Ok("Simulation : appareil déjà approuvé.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ideviceinfo_output() {
        let kv = parse_key_values("DeviceName: iPhone de Léa\nProductType: iPhone14,5\nProductVersion: 17.2\nBad line\n");
        assert_eq!(kv["DeviceName"], "iPhone de Léa");
        assert_eq!(kv["ProductVersion"], "17.2");
    }

    #[test]
    fn usage_categories_from_disk_usage() {
        let kv = parse_key_values(
            "CameraUsage: 40000000000\nMobileApplicationUsage: 30000000000\nCalendarUsage: 5000000\nVoicemailUsage: 0\nTotalDiskCapacity: 256000000000\nFooUsage: abc\nBarUsage: 7\n",
        );
        let u = usage_from_disk_usage(&kv, Some(100_000_000_000));
        assert_eq!(u[0].label, "Photos et vidéos (appareil photo)");
        assert_eq!(u[1].label, "Applications");
        assert!(u.iter().all(|c| c.key != "VoicemailUsage"), "les catégories vides sont omises");
        assert!(u.iter().all(|c| c.key != "FooUsage"), "les valeurs non numériques sont ignorées");
        assert!(u.iter().any(|c| c.label == "Bar"), "une clé inconnue est reprise telle quelle");
        let other = u.last().unwrap();
        assert_eq!(other.key, "Other");
        assert_eq!(u.iter().map(|c| c.bytes).sum::<u64>(), 100_000_000_000, "la somme doit égaler l'espace utilisé");
    }

    #[test]
    fn usage_without_used_bytes_has_no_remainder_and_empty_when_absent() {
        assert!(usage_from_disk_usage(&HashMap::new(), None).is_empty());
        let kv = parse_key_values("CalendarUsage: 10\n");
        assert_eq!(usage_from_disk_usage(&kv, None).len(), 1);
        assert_eq!(usage_from_disk_usage(&kv, Some(10)).len(), 1, "pas de reste si tout est expliqué");
    }

    #[test]
    fn sim_devices_expose_usage() {
        let d = SimBackend::new(0).list().unwrap();
        assert!(!d[0].usage.is_empty());
        assert_eq!(d[0].usage.iter().map(|c| c.bytes).sum::<u64>(), d[0].used_bytes.unwrap());
    }

    #[test]
    fn percent_parsing() {
        assert_eq!(parse_percent("[=====     ] 45% Finished"), Some(45.0));
        assert_eq!(parse_percent("Receiving files"), None);
        assert_eq!(parse_percent("abc 250%"), None);
    }

    #[test]
    fn friendly_names() {
        assert_eq!(friendly_model("iPhone16,1"), "iPhone 15 Pro");
        assert_eq!(friendly_model("iPad99,9"), "iPad99,9");
    }

    fn dev(udid: &str, os: &str, cap: u64, used: u64, trusted: bool) -> DeviceInfo {
        let mut d = SimBackend::device(udid, "x", "iPhone14,5", os, cap, used);
        d.trusted = trusted;
        d.simulated = false;
        d
    }

    #[test]
    fn compatibility_rules() {
        let a = dev("aaaaaaaa-1111111111111111", "17.0", 128, 100, true);
        let b = dev("bbbbbbbb-2222222222222222", "17.5", 256, 5, true);
        assert!(check_compatibility(&a, &b).ok);
        // destination plus ancienne
        let old = dev("cccccccc-3333333333333333", "16.0", 256, 5, true);
        assert!(!check_compatibility(&a, &old).ok);
        // capacite insuffisante
        let small = dev("dddddddd-4444444444444444", "17.5", 64, 5, true);
        assert!(!check_compatibility(&a, &small).ok);
        // non approuve
        let untrusted = dev("eeeeeeee-5555555555555555", "17.5", 256, 5, false);
        assert!(!check_compatibility(&a, &untrusted).ok);
        // meme appareil
        assert!(!check_compatibility(&a, &a).ok);
    }

    #[test]
    fn sim_backup_then_restore_roundtrip_is_real() {
        let d = tempfile::tempdir().unwrap();
        let sim = SimBackend::new(0);
        let ctl = OpCtl::new();
        let mut events = 0;
        sim.backup(SIM_SOURCE, d.path(), &ctl, &mut |_| events += 1).unwrap();
        assert!(events > 10);
        let root = d.path().join(SIM_SOURCE);
        assert!(root.join("Contacts/AddressBook.vcf").is_file());
        let opts = RestoreOptions { source_udid: Some(SIM_SOURCE.into()), ..Default::default() };
        let mut last = Progress::default();
        sim.restore(SIM_DEST, d.path(), &opts, &ctl, &mut |p| last = p).unwrap();
        assert!((last.percent - 100.0).abs() < 1e-9);
        assert!(last.simulated);
    }

    #[test]
    fn sim_backup_is_deterministic() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let sim = SimBackend::new(0);
        sim.backup(SIM_SOURCE, a.path(), &OpCtl::new(), &mut |_| {}).unwrap();
        sim.backup(SIM_SOURCE, b.path(), &OpCtl::new(), &mut |_| {}).unwrap();
        let f = "Documents/doc_3.pdf";
        assert_eq!(
            sha256_file(&a.path().join(SIM_SOURCE).join(f)).unwrap(),
            sha256_file(&b.path().join(SIM_SOURCE).join(f)).unwrap()
        );
    }

    #[test]
    fn restore_of_missing_backup_fails() {
        let d = tempfile::tempdir().unwrap();
        let r = SimBackend::new(0).restore(SIM_DEST, d.path(), &RestoreOptions::default(), &OpCtl::new(), &mut |_| {});
        assert!(r.is_err());
    }

    #[test]
    fn hostile_udid_is_rejected_before_any_command() {
        let b = LibimobiledeviceBackend::new(Tools::default());
        let d = tempfile::tempdir().unwrap();
        let r = b.backup("x; rm -rf ~", d.path(), &OpCtl::new(), &mut |_| {});
        assert!(matches!(r, Err(GhostError::Security(_))));
    }
}
