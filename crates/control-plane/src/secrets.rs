use anyhow::{anyhow, Context, Result};
use uuid::Uuid;

use crate::db::DbPool;
use crate::{crypto, repos};

pub async fn ensure_tenant_dek(pool: &DbPool, tenant_id: Uuid) -> Result<([u8; 32], i32)> {
    let kek = crypto::load_kek_from_env()?;
    if let Some(row) = repos::get_tenant_key(pool, tenant_id).await? {
        let dek = crypto::unwrap_dek(&kek, &row.wrapped_dek)?;
        return Ok((dek, row.key_version));
    }
    let (wrapped, version) = crypto::generate_wrapped_dek(&kek)?;
    // handle race: try insert, if conflict read again
    match repos::insert_tenant_key(pool, tenant_id, &wrapped, version).await {
        Ok(row) => {
            let dek = crypto::unwrap_dek(&kek, &row.wrapped_dek)?;
            Ok((dek, row.key_version))
        }
        Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
            let row = repos::get_tenant_key(pool, tenant_id)
                .await?
                .ok_or_else(|| anyhow!("tenant key missing after conflict"))?;
            let dek = crypto::unwrap_dek(&kek, &row.wrapped_dek)?;
            Ok((dek, row.key_version))
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn set_secret_from_env(
    pool: &DbPool,
    tenant_id: Uuid,
    name: &str,
) -> Result<crate::models::SecretRow> {
    if name.trim().is_empty() {
        return Err(anyhow!("secret name must not be empty"));
    }
    let env_name = crypto::normalize_secret_name_to_env(name);
    let value = std::env::var(&env_name).with_context(|| {
        format!("environment variable {env_name} is not set (required to set secret '{name}')")
    })?;
    let (dek, key_version) = ensure_tenant_dek(pool, tenant_id).await?;
    let (nonce, ciphertext) = crypto::encrypt_secret(&dek, value.as_bytes())?;
    Ok(repos::upsert_secret(pool, tenant_id, name, &nonce, &ciphertext, key_version).await?)
}

pub async fn check_secret_refs_exist(
    pool: &DbPool,
    tenant_id: Uuid,
    refs: &std::collections::BTreeSet<String>,
) -> Result<Vec<String>> {
    let mut missing = Vec::new();
    for r in refs {
        if !repos::secret_exists(pool, tenant_id, r).await? {
            missing.push(r.clone());
        }
    }
    Ok(missing)
}
