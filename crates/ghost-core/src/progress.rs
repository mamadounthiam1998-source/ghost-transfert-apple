use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

use crate::{GhostError, Result};

/// Controle d'une operation : annulation cooperative.
#[derive(Clone, Default)]
pub struct OpCtl {
    pub cancel: Arc<AtomicBool>,
}

impl OpCtl {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
    pub fn reset(&self) {
        self.cancel.store(false, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(GhostError::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// Progression reelle : chaque champ provient d'operations effectivement realisees.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: String,
    pub percent: f64,
    pub files_done: u64,
    pub files_total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub speed_bps: f64,
    pub eta_secs: Option<u64>,
    pub errors: u64,
    pub verified: u64,
    pub current: Option<String>,
    pub message: Option<String>,
    pub simulated: bool,
}

/// Calcule vitesse et temps restant a partir du temps ecoule reel.
pub struct Tracker {
    start: Instant,
    pub p: Progress,
}

impl Tracker {
    pub fn new(phase: &str, files_total: u64, bytes_total: u64, simulated: bool) -> Self {
        Tracker {
            start: Instant::now(),
            p: Progress {
                phase: phase.into(),
                files_total,
                bytes_total,
                simulated,
                ..Default::default()
            },
        }
    }

    pub fn snapshot(&mut self) -> Progress {
        let secs = self.start.elapsed().as_secs_f64().max(0.001);
        self.p.speed_bps = self.p.bytes_done as f64 / secs;
        self.p.percent = if self.p.bytes_total > 0 {
            (self.p.bytes_done as f64 / self.p.bytes_total as f64 * 100.0).min(100.0)
        } else if self.p.files_total > 0 {
            (self.p.files_done as f64 / self.p.files_total as f64 * 100.0).min(100.0)
        } else {
            0.0
        };
        self.p.eta_secs = if self.p.speed_bps > 1.0 && self.p.bytes_total > self.p.bytes_done {
            Some(((self.p.bytes_total - self.p.bytes_done) as f64 / self.p.speed_bps) as u64)
        } else {
            None
        };
        self.p.clone()
    }
}

/// Transforme la progression d'une phase en progression GLOBALE d'un transfert
/// (lecture de la source 0-40 %, controle 40-52 %, ecriture sur la destination 52-100 %).
/// Le resultat ne recule jamais si l'appelant le combine avec `max` du precedent.
pub fn clone_stage(phase: &str, phase_percent: f64) -> (f64, &'static str) {
    let (lo, hi, label) = if phase.contains("empreintes") {
        (40.0, 48.0, "Étape 2/3 — Contrôle des données")
    } else if phase.starts_with("Vérification") {
        (48.0, 52.0, "Étape 2/3 — Contrôle des données")
    } else if phase.starts_with("Restauration") {
        (52.0, 100.0, "Étape 3/3 — Écriture sur l'iPhone de destination")
    } else {
        (0.0, 40.0, "Étape 1/3 — Lecture de l'iPhone source")
    };
    (lo + phase_percent.clamp(0.0, 100.0) * (hi - lo) / 100.0, label)
}

#[cfg(test)]
mod tests {
    #[test]
    fn clone_stage_is_global_and_bounded() {
        use super::clone_stage;
        assert_eq!(clone_stage("Sauvegarde de l'appareil", 0.0).0, 0.0);
        assert_eq!(clone_stage("Sauvegarde de l'appareil", 100.0).0, 40.0);
        assert_eq!(clone_stage("Calcul des empreintes SHA-256", 50.0).0, 44.0);
        assert_eq!(clone_stage("Vérification d'intégrité", 100.0).0, 52.0);
        assert_eq!(clone_stage("Restauration sur l'appareil", 0.0).0, 52.0);
        assert_eq!(clone_stage("Restauration (simulation)", 100.0).0, 100.0);
        assert_eq!(clone_stage("Restauration", 999.0).0, 100.0, "borné à 100");
        assert_eq!(clone_stage("Restauration", -5.0).0, 52.0);
    }

    use super::*;
    #[test]
    fn percent_is_derived_from_bytes() {
        let mut t = Tracker::new("x", 2, 200, false);
        t.p.bytes_done = 50;
        assert!((t.snapshot().percent - 25.0).abs() < 1e-9);
    }
    #[test]
    fn cancel_flag() {
        let c = OpCtl::new();
        assert!(c.check().is_ok());
        c.cancel();
        assert!(matches!(c.check(), Err(GhostError::Cancelled)));
    }
}
