//! Commandes exposees a l'interface. Aucune logique critique ici : tout est delegue a ghost-core.

use ghost_core::{
    analysis::{analyze_dir, AnalysisReport, Category},
    backup::{BackupInfo, BackupManager, VerifyReport},
    crypto::KdfParams,
    devices::{
        check_compatibility, Compatibility, DeviceBackend, DeviceInfo, LibimobiledeviceBackend, RestoreOptions,
        SimBackend,
    },
    diagnostics::{run_diagnostics, security_selftest as core_selftest, Check, REQUIRED_TOOLS},
    history::{HistoryEntry, HistoryStore},
    logging::Logger,
    progress::{OpCtl, Progress},
    security::redact,
    tools::Tools,
    transfer::{self, ConflictPolicy, TransferOptions, TransferReport},
    GhostError, Result,
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{AppHandle, Emitter, State};

type St<'a> = State<'a, Arc<AppState>>;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub backups_dir: Option<String>,
    pub theme: String,
    pub reduce_motion: bool,
    pub simulation: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { backups_dir: None, theme: "dark".into(), reduce_motion: false, simulation: false }
    }
}

pub struct AppState {
    data_dir: PathBuf,
    pub tools: Tools,
    pub history: HistoryStore,
    pub logger: Logger,
    settings: Mutex<Settings>,
    ctl: OpCtl,
    busy: Arc<AtomicBool>,
}

impl AppState {
    pub fn new(data_dir: PathBuf, tools: Tools, history: HistoryStore, logger: Logger) -> Self {
        let settings = std::fs::read_to_string(data_dir.join("settings.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        AppState { data_dir, tools, history, logger, settings: Mutex::new(settings), ctl: OpCtl::new(), busy: Arc::new(AtomicBool::new(false)) }
    }

    fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    fn save(&self, s: Settings) -> Result<()> {
        std::fs::write(self.data_dir.join("settings.json"), serde_json::to_string_pretty(&s)?)?;
        *self.settings.lock().unwrap() = s;
        Ok(())
    }

    fn backend(&self) -> Box<dyn DeviceBackend> {
        if self.settings().simulation {
            Box::new(SimBackend::new(30))
        } else {
            Box::new(LibimobiledeviceBackend::new(self.tools.clone()))
        }
    }

    fn manager(&self) -> BackupManager {
        let root = self
            .settings()
            .backups_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| self.data_dir.join("backups"));
        BackupManager::new(root)
    }
}

struct BusyGuard(Arc<AtomicBool>);
impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn begin(st: &AppState) -> Result<BusyGuard> {
    if st.busy.swap(true, Ordering::SeqCst) {
        return Err(GhostError::Data("une opération est déjà en cours".into()));
    }
    st.ctl.reset();
    Ok(BusyGuard(st.busy.clone()))
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| GhostError::Data(e.to_string()))?
}

fn emitter(app: &AppHandle) -> impl FnMut(Progress) + '_ {
    move |p| {
        let _ = app.emit("ghost-progress", p);
    }
}

/// Enregistre l'issue d'une operation (journal + historique) puis la renvoie telle quelle.
fn record<T>(
    st: &AppState,
    kind: &str,
    started: String,
    simulated: bool,
    res: Result<T>,
    ok_summary: impl FnOnce(&T) -> (String, String),
) -> Result<T> {
    let (status, summary) = match &res {
        Ok(v) => ok_summary(v),
        Err(GhostError::Cancelled) => ("cancelled".into(), "Opération annulée par l'utilisateur".into()),
        Err(e) => ("failed".into(), e.to_string()),
    };
    st.logger.log(if status == "failed" { "ERROR" } else { "INFO" }, &format!("{kind} : {status} — {summary}"));
    let _ = st.history.append(&HistoryEntry::now(kind, &status, summary, simulated, started));
    res
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn find_device(devices: &[DeviceInfo], udid: &str) -> Result<DeviceInfo> {
    devices
        .iter()
        .find(|d| d.udid == udid)
        .cloned()
        .ok_or_else(|| GhostError::Device("appareil introuvable : est-il toujours connecté ?".into()))
}

fn require_dir(p: &str) -> Result<PathBuf> {
    let path = PathBuf::from(p);
    if !path.is_dir() {
        return Err(GhostError::Data(format!("dossier introuvable : {p}")));
    }
    Ok(path)
}

// ---------------------------------------------------------------------------
// Etat & reglages
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    simulation: bool,
    tools_ok: bool,
    missing_tools: Vec<String>,
    version: String,
    platform: String,
    backups_dir: String,
    busy: bool,
}

#[tauri::command]
pub fn app_status(st: St<'_>) -> AppStatus {
    let missing: Vec<String> =
        REQUIRED_TOOLS.iter().filter(|t| st.tools.find(t).is_none()).map(|t| t.to_string()).collect();
    AppStatus {
        simulation: st.settings().simulation,
        tools_ok: missing.is_empty(),
        missing_tools: missing,
        version: env!("CARGO_PKG_VERSION").into(),
        platform: std::env::consts::OS.into(),
        backups_dir: st.manager().root.display().to_string(),
        busy: st.busy.load(Ordering::SeqCst),
    }
}

#[tauri::command]
pub fn set_simulation(st: St<'_>, enabled: bool) -> Result<()> {
    let mut s = st.settings();
    s.simulation = enabled;
    st.logger.info(&format!("mode simulation : {enabled}"));
    st.save(s)
}

#[tauri::command]
pub fn get_settings(st: St<'_>) -> Settings {
    st.settings()
}

#[tauri::command]
pub fn save_settings(st: St<'_>, settings: Settings) -> Result<()> {
    if let Some(d) = &settings.backups_dir {
        std::fs::create_dir_all(d)?;
    }
    st.save(settings)
}

// ---------------------------------------------------------------------------
// Appareils
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_devices(st: St<'_>) -> Result<Vec<DeviceInfo>> {
    let st = st.inner().clone();
    blocking(move || st.backend().list()).await
}

#[tauri::command]
pub async fn check_compat(st: St<'_>, source_udid: String, dest_udid: String) -> Result<Compatibility> {
    let st = st.inner().clone();
    blocking(move || {
        let devs = st.backend().list()?;
        Ok(check_compatibility(&find_device(&devs, &source_udid)?, &find_device(&devs, &dest_udid)?))
    })
    .await
}

// ---------------------------------------------------------------------------
// Analyse
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn analyze_folder(st: St<'_>, path: String) -> Result<AnalysisReport> {
    let st = st.inner().clone();
    blocking(move || {
        let _g = begin(&st)?;
        analyze_dir(&require_dir(&path)?, &st.ctl)
    })
    .await
}

#[tauri::command]
pub async fn analyze_backup(st: St<'_>, hid: String) -> Result<AnalysisReport> {
    let st = st.inner().clone();
    blocking(move || {
        let _g = begin(&st)?;
        analyze_dir(&st.manager().data_root(&hid)?, &st.ctl)
    })
    .await
}

// ---------------------------------------------------------------------------
// Sauvegardes
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn backup_create(app: AppHandle, st: St<'_>, udid: String, password: Option<String>) -> Result<BackupInfo> {
    let st = st.inner().clone();
    blocking(move || {
        let _g = begin(&st)?;
        let started = now();
        let backend = st.backend();
        let sim = backend.is_simulation();
        let res = (|| {
            let dev = find_device(&backend.list()?, &udid)?;
            st.manager().create(backend.as_ref(), &dev, password.as_deref(), &st.ctl, &mut emitter(&app))
        })();
        record(&st, "backup", started, sim, res, |b| {
            ("success".into(), format!("Sauvegarde de « {} » : {} fichiers, {} octets ({} nouveaux ou modifiés)", b.device_name, b.files, b.bytes, b.new_or_changed))
        })
    })
    .await
}

#[tauri::command]
pub fn backup_list(st: St<'_>) -> Vec<BackupInfo> {
    st.manager().list()
}

#[tauri::command]
pub async fn backup_verify(app: AppHandle, st: St<'_>, hid: String, version: String) -> Result<VerifyReport> {
    let st = st.inner().clone();
    blocking(move || {
        let _g = begin(&st)?;
        let started = now();
        let res = st.manager().verify(&hid, &version, &st.ctl, &mut emitter(&app));
        record(&st, "verify", started, false, res, |r| {
            (if r.intact { "success" } else { "failed" }.into(), format!("{}/{} fichiers intacts, {} manquants, {} modifiés", r.ok, r.checked, r.missing.len(), r.modified.len()))
        })
    })
    .await
}

#[tauri::command]
pub async fn backup_delete(st: St<'_>, hid: String, confirm: bool) -> Result<()> {
    if !confirm {
        return Err(GhostError::Security("la suppression exige une confirmation explicite".into()));
    }
    let st = st.inner().clone();
    blocking(move || {
        let _g = begin(&st)?;
        let started = now();
        let res = st.manager().delete(&hid);
        record(&st, "delete", started, false, res, |_| ("success".into(), "Sauvegarde supprimée (écrasement puis effacement)".into()))
    })
    .await
}

#[tauri::command]
pub async fn backup_export(st: St<'_>, hid: String, out_path: String, password: String) -> Result<()> {
    let st = st.inner().clone();
    blocking(move || {
        let _g = begin(&st)?;
        let started = now();
        let res = st.manager().export_encrypted(&hid, Path::new(&out_path), &password, KdfParams::default(), &st.ctl);
        record(&st, "export", started, false, res, |_| ("success".into(), "Export chiffré AES-256-GCM créé".into()))
    })
    .await
}

#[tauri::command]
pub async fn backup_import(st: St<'_>, path: String, password: String, overwrite: bool) -> Result<String> {
    let st = st.inner().clone();
    blocking(move || {
        let _g = begin(&st)?;
        let started = now();
        let res = st.manager().import_encrypted(Path::new(&path), &password, overwrite);
        record(&st, "import", started, false, res, |_| ("success".into(), "Archive chiffrée importée".into()))
    })
    .await
}

// ---------------------------------------------------------------------------
// Restauration / clonage
// ---------------------------------------------------------------------------

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RestoreRequest {
    pub udid: String,
    pub hid: String,
    pub system: bool,
    pub settings: bool,
    pub reboot: bool,
    pub password: Option<String>,
    /// L'utilisateur confirme que les donnees de l'appareil de destination seront remplacees.
    pub confirm_erase: bool,
}

#[tauri::command]
pub async fn restore_backup(app: AppHandle, st: St<'_>, req: RestoreRequest) -> Result<()> {
    if !req.confirm_erase {
        return Err(GhostError::Security(
            "la restauration remplace les données de l'appareil : confirmation explicite requise".into(),
        ));
    }
    let st = st.inner().clone();
    blocking(move || {
        let _g = begin(&st)?;
        let started = now();
        let backend = st.backend();
        let sim = backend.is_simulation();
        let res = (|| {
            let mgr = st.manager();
            let info = mgr.list().into_iter().find(|b| b.hid == req.hid).ok_or_else(|| GhostError::Data("sauvegarde introuvable".into()))?;
            let dev = find_device(&backend.list()?, &req.udid)?;
            if !dev.trusted {
                return Err(GhostError::Device("l'appareil n'a pas approuvé cet ordinateur".into()));
            }
            let opts = RestoreOptions {
                system: req.system,
                settings: req.settings,
                reboot: req.reboot,
                source_udid: Some(info.source_udid.clone()),
                password: req.password.clone(),
            };
            backend.restore(&req.udid, &mgr.data_root(&req.hid)?, &opts, &st.ctl, &mut emitter(&app))
        })();
        record(&st, "restore", started, sim, res, |_| ("success".into(), "Restauration lancée sur l'appareil".into()))
    })
    .await
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CloneRequest {
    pub source_udid: String,
    pub dest_udid: String,
    /// "quick" | "full" | "custom"
    pub mode: String,
    pub system: bool,
    pub settings: bool,
    pub password: Option<String>,
    pub confirm_erase: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CloneReport {
    pub backup: BackupInfo,
    pub integrity: VerifyReport,
    pub restored: bool,
    pub notes: Vec<String>,
    pub success: bool,
    pub simulated: bool,
}

#[tauri::command]
pub async fn clone_start(app: AppHandle, st: St<'_>, req: CloneRequest) -> Result<CloneReport> {
    if !req.confirm_erase {
        return Err(GhostError::Security(
            "Ghost Clone remplace les données de l'appareil de destination : confirmation explicite requise".into(),
        ));
    }
    let st = st.inner().clone();
    blocking(move || {
        let _g = begin(&st)?;
        let started = now();
        let backend = st.backend();
        let sim = backend.is_simulation();
        let res = (|| -> Result<CloneReport> {
            let devs = backend.list()?;
            let (src, dst) = (find_device(&devs, &req.source_udid)?, find_device(&devs, &req.dest_udid)?);
            let compat = check_compatibility(&src, &dst);
            if !compat.ok {
                return Err(GhostError::Device(compat.blockers.join(" ")));
            }
            let (system, settings) = match req.mode.as_str() {
                "quick" => (false, true),
                "full" => (true, true),
                "custom" => (req.system, req.settings),
                _ => return Err(GhostError::Data("mode inconnu".into())),
            };
            let mgr = st.manager();
            // 1. Sauvegarde officielle de la source + empreintes SHA-256
            let info = mgr.create(backend.as_ref(), &src, req.password.as_deref(), &st.ctl, &mut emitter(&app))?;
            // 2. Verification de la sauvegarde avant de toucher a la destination
            let integrity = mgr.verify(&info.hid, &info.version, &st.ctl, &mut emitter(&app))?;
            if !integrity.intact {
                return Err(GhostError::Integrity("la sauvegarde est altérée : la destination n'a pas été modifiée".into()));
            }
            // 3. Restauration officielle sur la destination
            let opts = RestoreOptions {
                system,
                settings,
                reboot: true,
                source_udid: Some(src.udid.clone()),
                password: req.password.clone(),
            };
            backend.restore(&dst.udid, &mgr.data_root(&info.hid)?, &opts, &st.ctl, &mut emitter(&app))?;
            let mut notes = compat.warnings.clone();
            notes.push("La vérification porte sur la sauvegarde source ; Apple ne permet pas à un logiciel tiers de relire le contenu de l'appareil de destination. Contrôlez-le après son redémarrage.".into());
            notes.push("Non transférés par conception Apple : Face ID/Touch ID, cartes Apple Pay, eSIM, applications elles-mêmes (retéléchargées), éléments uniquement dans iCloud.".into());
            Ok(CloneReport { success: true, restored: true, simulated: sim, backup: info, integrity, notes })
        })();
        record(&st, "clone", started, sim, res, |r| ("success".into(), format!("Clone de « {} » : {} fichiers sauvegardés et restaurés", r.backup.device_name, r.backup.files)))
    })
    .await
}

// ---------------------------------------------------------------------------
// Transfert intelligent (dossier -> dossier, verifie)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSummary {
    files: u64,
    bytes: u64,
    free_bytes: u64,
    enough_space: bool,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SmartRequest {
    pub source: String,
    pub dest: String,
    pub categories: Option<Vec<Category>>,
    pub dedupe: bool,
    pub conflict: ConflictPolicy,
    pub confirm_overwrite: bool,
    pub verify: bool,
}

fn check_not_nested(src: &Path, dst: &Path) -> Result<()> {
    let s = src.canonicalize()?;
    let d = if dst.exists() { dst.canonicalize()? } else { dst.parent().and_then(|p| p.canonicalize().ok()).unwrap_or_else(|| dst.to_path_buf()) };
    if d.starts_with(&s) || s.starts_with(&d) {
        return Err(GhostError::Security("la source et la destination ne doivent pas être imbriquées".into()));
    }
    Ok(())
}

#[tauri::command]
pub async fn smart_plan(st: St<'_>, source: String, dest: String, categories: Option<Vec<Category>>) -> Result<PlanSummary> {
    let st = st.inner().clone();
    blocking(move || {
        let _g = begin(&st)?;
        let plan = transfer::build_plan(&require_dir(&source)?, &categories, &st.ctl)?;
        let free = transfer::free_space(Path::new(&dest))?;
        Ok(PlanSummary { files: plan.items.len() as u64, bytes: plan.total_bytes, free_bytes: free, enough_space: transfer::check_space(Path::new(&dest), plan.total_bytes).is_ok() })
    })
    .await
}

#[tauri::command]
pub async fn smart_transfer(app: AppHandle, st: St<'_>, req: SmartRequest) -> Result<TransferReport> {
    let st = st.inner().clone();
    blocking(move || {
        let _g = begin(&st)?;
        let started = now();
        let sim = st.settings().simulation;
        let res = (|| {
            let src = require_dir(&req.source)?;
            let dst = PathBuf::from(&req.dest);
            std::fs::create_dir_all(&dst)?;
            check_not_nested(&src, &dst)?;
            let plan = transfer::build_plan(&src, &req.categories, &st.ctl)?;
            let opts = TransferOptions {
                categories: req.categories.clone(),
                dedupe: req.dedupe,
                conflict: req.conflict,
                confirm_overwrite: req.confirm_overwrite,
                verify: req.verify,
                simulated: false,
            };
            transfer::run(&plan, &dst, &opts, &st.ctl, &mut emitter(&app))
        })();
        record(&st, "transfer", started, sim && false, res, |r| {
            (
                if r.success { "success" } else { "partial" }.into(),
                format!("{} copiés, {} déjà présents, {} doublons, {} non transférés, {} erreurs", r.copied, r.already_present, r.duplicates_skipped, r.not_transferred.len(), r.errors.len()),
            )
        })
    })
    .await
}

#[tauri::command]
pub fn cancel_operation(st: St<'_>) {
    st.ctl.cancel();
}

// ---------------------------------------------------------------------------
// Historique, diagnostic, securite, mises a jour
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn history_list(st: St<'_>, limit: Option<usize>) -> Vec<HistoryEntry> {
    st.history.list(limit.unwrap_or(200))
}

#[tauri::command]
pub fn history_clear(st: St<'_>, confirm: bool) -> Result<()> {
    if !confirm {
        return Err(GhostError::Security("confirmation requise".into()));
    }
    st.history.clear()
}

#[tauri::command]
pub async fn diagnostics_run(st: St<'_>) -> Result<Vec<Check>> {
    let st = st.inner().clone();
    blocking(move || Ok(run_diagnostics(&st.tools))).await
}

#[tauri::command]
pub fn security_selftest() -> Vec<Check> {
    core_selftest()
}

#[tauri::command]
pub async fn diagnostics_export(st: St<'_>, out_path: String) -> Result<()> {
    let st = st.inner().clone();
    blocking(move || {
        let report = serde_json::json!({
            "generatedAt": now(),
            "app": env!("CARGO_PKG_VERSION"),
            "os": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            "checks": run_diagnostics(&st.tools),
            "securitySelfTest": core_selftest(),
            "recentLogs": st.logger.tail(200).iter().map(|l| redact(l)).collect::<Vec<_>>(),
            "note": "Rapport anonymisé : aucun identifiant d'appareil, e-mail ni nom de profil.",
        });
        std::fs::write(out_path, serde_json::to_string_pretty(&report)?)?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn verify_update(path: String, sha256: String, signature: String, public_key: String) -> Result<()> {
    blocking(move || ghost_core::updates::verify_package(Path::new(&path), &sha256, &signature, &public_key)).await
}
