use thiserror::Error;

#[derive(Debug, Error)]
pub enum GhostError {
    #[error("Entrée/sortie : {0}")]
    Io(#[from] std::io::Error),
    #[error("Cryptographie : {0}")]
    Crypto(String),
    #[error("Sécurité : {0}")]
    Security(String),
    #[error("Outil externe : {0}")]
    Tool(String),
    #[error("Appareil : {0}")]
    Device(String),
    #[error("Intégrité : {0}")]
    Integrity(String),
    #[error("Données : {0}")]
    Data(String),
    #[error("Espace disque insuffisant : {needed} octets nécessaires, {available} disponibles")]
    NoSpace { needed: u64, available: u64 },
    #[error("Opération annulée")]
    Cancelled,
}

impl From<serde_json::Error> for GhostError {
    fn from(e: serde_json::Error) -> Self {
        GhostError::Data(e.to_string())
    }
}

impl serde::Serialize for GhostError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, GhostError>;
