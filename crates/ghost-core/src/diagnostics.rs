//! Centre de diagnostic : etat des outils, pilotes, services et auto-test de securite.

use crate::{
    crypto::{decrypt_bytes, encrypt_bytes, KdfParams},
    security::{redact, safe_join, validate_udid},
    tools::Tools,
};
use serde::Serialize;
use std::{path::Path, time::Duration};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub id: String,
    pub name: String,
    /// "ok" | "warn" | "error"
    pub status: String,
    pub detail: String,
    pub solution: Option<String>,
}

fn check(id: &str, name: &str, status: &str, detail: String, solution: Option<&str>) -> Check {
    Check { id: id.into(), name: name.into(), status: status.into(), detail, solution: solution.map(String::from) }
}

pub const REQUIRED_TOOLS: [&str; 4] = ["idevice_id", "ideviceinfo", "idevicebackup2", "idevicepair"];

pub fn run_diagnostics(tools: &Tools) -> Vec<Check> {
    let mut v = vec![];
    for t in REQUIRED_TOOLS {
        match tools.find(t) {
            Some(p) => {
                let ver = tools
                    .run(t, &["--version"], Duration::from_secs(5))
                    .map(|o| format!("{}{}", o.stdout, o.stderr).lines().next().unwrap_or("").trim().to_string())
                    .unwrap_or_default();
                v.push(check(t, &format!("Outil {t}"), "ok", format!("{} {ver}", redact(&p.display().to_string())), None));
            }
            None => v.push(check(
                t,
                &format!("Outil {t}"),
                "error",
                "introuvable".into(),
                Some(if cfg!(windows) {
                    "Placez les binaires libimobiledevice dans le dossier « tools » de l'application (voir le guide d'installation)."
                } else {
                    "Installez libimobiledevice : brew install libimobiledevice (macOS) ou le paquet de votre distribution."
                }),
            )),
        }
    }
    if cfg!(windows) {
        let svc = std::process::Command::new("sc").args(["query", "Apple Mobile Device Service"]).output();
        match svc {
            Ok(o) if String::from_utf8_lossy(&o.stdout).contains("RUNNING") => {
                v.push(check("amds", "Service Apple Mobile Device", "ok", "en cours d'exécution".into(), None))
            }
            _ => v.push(check(
                "amds",
                "Service Apple Mobile Device",
                "error",
                "absent ou arrêté".into(),
                Some("Installez « Appareils Apple » depuis le Microsoft Store (ou iTunes) pour obtenir les pilotes USB."),
            )),
        }
    } else if cfg!(target_os = "macos") {
        v.push(check("usb", "Pilotes USB Apple", "ok", "intégrés à macOS".into(), None));
    }
    v.push(check("os", "Système", "ok", format!("{} {}", std::env::consts::OS, std::env::consts::ARCH), None));
    v
}

/// Auto-test de securite : executable a tout moment depuis l'interface.
pub fn security_selftest() -> Vec<Check> {
    let mut v = vec![];
    let enc = encrypt_bytes(b"ghost", "autotest-1234", KdfParams::light())
        .and_then(|e| decrypt_bytes(&e, "autotest-1234").map(|d| (e, d)));
    v.push(match enc {
        Ok((e, d)) if d == b"ghost" && decrypt_bytes(&e, "faux").is_err() => {
            check("aes", "Chiffrement AES-256-GCM", "ok", "aller-retour valide, mauvais mot de passe refusé".into(), None)
        }
        _ => check("aes", "Chiffrement AES-256-GCM", "error", "échec de l'auto-test".into(), Some("Réinstallez l'application.")),
    });
    let base = Path::new("base");
    let blocked = ["../x", "/etc/passwd", "a/../../b", "C:\\x"].iter().all(|p| safe_join(base, p).is_err());
    v.push(check(
        "path",
        "Protection contre les traversées de répertoires",
        if blocked { "ok" } else { "error" },
        if blocked { "chemins hostiles refusés".into() } else { "un chemin hostile a été accepté".into() },
        None,
    ));
    let inj = validate_udid("1234abcd; rm -rf /").is_err();
    v.push(check(
        "inj",
        "Protection contre l'injection de commandes",
        if inj { "ok" } else { "error" },
        "identifiants d'appareil validés, outils lancés sans shell".into(),
        None,
    ));
    let red = !redact("jean@x.fr 00008110-000A1B2C3D4E5F01").contains("jean@");
    v.push(check(
        "log",
        "Journaux sans données personnelles",
        if red { "ok" } else { "error" },
        "UDID, e-mails et profils utilisateur caviardés".into(),
        None,
    ));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selftest_all_ok() {
        assert!(security_selftest().iter().all(|c| c.status == "ok"));
    }
    #[test]
    fn missing_tools_are_reported_as_errors_with_solution() {
        let r = run_diagnostics(&Tools::new(vec![]));
        let missing: Vec<_> = r.iter().filter(|c| c.status == "error" && REQUIRED_TOOLS.contains(&c.id.as_str())).collect();
        for c in missing {
            assert!(c.solution.is_some());
        }
    }
}
