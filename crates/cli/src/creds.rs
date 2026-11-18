use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub token: String,
    pub kind: String,              // "admin" or "tenant"
    pub tenant_id: Option<String>, // uuid string for tenant tokens
    pub tenant_name: Option<String>,
}

pub fn credentials_path() -> Result<PathBuf> {
    let base = dirs::config_dir().ok_or_else(|| anyhow!("could not determine config directory"))?;
    Ok(base.join("talaint").join("credentials.json"))
}

pub fn load_credentials() -> Result<Option<StoredCredentials>> {
    let path = credentials_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let creds: StoredCredentials =
        serde_json::from_str(&data).context("invalid credentials file")?;
    Ok(Some(creds))
}

pub fn save_credentials(creds: &StoredCredentials) -> Result<PathBuf> {
    let path = credentials_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let data = serde_json::to_string_pretty(creds)?;
    fs::write(&path, data).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

pub fn require_credentials() -> Result<StoredCredentials> {
    load_credentials()?
        .ok_or_else(|| anyhow!("not authenticated. run `talaint auth login --token <token>` first"))
}
