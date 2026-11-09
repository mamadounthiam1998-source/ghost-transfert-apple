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

#[cfg(test)]
mod tests {
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
