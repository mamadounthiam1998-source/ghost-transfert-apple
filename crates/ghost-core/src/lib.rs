//! Coeur de GHOST TRANSFERT APPLE.
//! Toute la logique critique (appareils, transfert, sauvegarde, cryptographie,
//! securite) vit ici, independamment de l'interface graphique.

pub mod analysis;
pub mod backup;
pub mod crypto;
pub mod devices;
pub mod diagnostics;
pub mod error;
pub mod hashing;
pub mod history;
pub mod logging;
pub mod progress;
pub mod security;
pub mod tools;
pub mod transfer;
pub mod updates;

pub use error::{GhostError, Result};
