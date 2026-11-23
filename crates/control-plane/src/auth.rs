use anyhow::{anyhow, Result};
use base64::Engine;
use rand::RngCore;

use crate::db::DbPool;
use crate::{crypto, repos};

pub fn generate_token_plaintext() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    // URL-safe-ish token
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub async fn resolve_active_token(
    pool: &DbPool,
    token_plaintext: &str,
) -> Result<Option<crate::models::AuthTokenRow>> {
    let hash = crypto::hash_token(token_plaintext);
    let row = repos::find_active_token_by_hash(pool, &hash).await?;
    if let Some(t) = &row {
        let _ = repos::touch_token(pool, t.id).await;
    }
    Ok(row)
}

pub async fn create_admin_token(
    pool: &DbPool,
    token_plaintext: &str,
) -> Result<crate::models::AuthTokenRow> {
    if repos::count_admin_tokens(pool).await? > 0 {
        return Err(anyhow!(
            "admin token already bootstrapped; only one admin is allowed"
        ));
    }
    let hash = crypto::hash_token(token_plaintext);
    Ok(repos::insert_auth_token(pool, "admin", None, &hash).await?)
}

pub async fn create_tenant_with_token(
    pool: &DbPool,
    name: &str,
) -> Result<(crate::models::Tenant, String)> {
    let tenant = repos::create_tenant(pool, name).await?;
    let token = generate_token_plaintext();
    let hash = crypto::hash_token(&token);
    repos::insert_auth_token(pool, "tenant", Some(tenant.id), &hash).await?;
    Ok((tenant, token))
}

pub async fn rotate_tenant_token_by_name_or_id(
    pool: &DbPool,
    tenant_ref: &str,
) -> Result<(crate::models::Tenant, String), anyhow::Error> {
    let tenant = resolve_tenant_ref(pool, tenant_ref).await?;
    let token = generate_token_plaintext();
    let hash = crypto::hash_token(&token);
    repos::rotate_tenant_token(pool, tenant.id, &hash).await?;
    Ok((tenant, token))
}

pub async fn resolve_tenant_ref(pool: &DbPool, tenant_ref: &str) -> Result<crate::models::Tenant> {
    if let Ok(id) = uuid::Uuid::parse_str(tenant_ref) {
        if let Some(t) = repos::get_tenant_by_id(pool, id).await? {
            return Ok(t);
        }
    }
    repos::get_tenant_by_name(pool, tenant_ref)
        .await?
        .ok_or_else(|| anyhow!("tenant not found: {tenant_ref}"))
}
