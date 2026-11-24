use std::sync::OnceLock;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{
    Row,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteRow},
};
use subtle::ConstantTimeEq;

use crate::proto::AgentConfig;

use super::config::config;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentInfo {
    pub version: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub docker: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AgentRecord {
    pub id: String,
    pub name: String,
    pub secret_hash: String,
    pub config: AgentConfig,
    pub last_seen: Option<i64>,
    pub info: Option<AgentInfo>,
    pub address: Option<String>,
}

impl AgentRecord {
    pub fn link_host(&self) -> String {
        self.address
            .clone()
            .filter(|a| !a.is_empty())
            .or_else(|| self.info.as_ref().and_then(|i| i.address.clone()))
            .or_else(|| self.info.as_ref().map(|i| i.hostname.clone()))
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
pub struct UserRecord {
    pub username: String,
    pub password_hash: String,
}

static POOL: OnceLock<SqlitePool> = OnceLock::new();

fn pool() -> &'static SqlitePool {
    POOL.get().expect("database not initialised")
}

pub async fn init() {
    let options = SqliteConnectOptions::new()
        .filename(config().data_dir.join("senn.db"))
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .expect("open database");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    POOL.set(pool).expect("database already initialised");
}

pub fn now() -> i64 {
    jiff::Timestamp::now().as_second()
}

pub fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("random bytes");
    URL_SAFE_NO_PAD.encode(buf)
}

pub fn hash(value: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(value.as_bytes()))
}

fn db_error(e: sqlx::Error) -> String {
    tracing::error!("database: {e}");
    "database error".into()
}

pub async fn user() -> Option<UserRecord> {
    sqlx::query("SELECT username, password_hash FROM account WHERE id = 1")
        .fetch_optional(pool())
        .await
        .ok()
        .flatten()
        .map(|r| UserRecord {
            username: r.get("username"),
            password_hash: r.get("password_hash"),
        })
}

pub async fn has_user() -> bool {
    user().await.is_some()
}

pub async fn create_user(username: &str, password_hash: &str) -> bool {
    sqlx::query(
        "INSERT OR IGNORE INTO account (id, username, password_hash, created) VALUES (1, ?, ?, ?)",
    )
    .bind(username)
    .bind(password_hash)
    .bind(now())
    .execute(pool())
    .await
    .map(|r| r.rows_affected() == 1)
    .unwrap_or(false)
}

fn agent_from_row(r: &SqliteRow) -> AgentRecord {
    let info: Option<String> = r.get("info");
    AgentRecord {
        id: r.get("id"),
        name: r.get("name"),
        secret_hash: r.get("secret_hash"),
        config: AgentConfig {
            update_interval_secs: r.get::<i64, _>("update_interval_secs") as u64,
            auto_update: r.get::<i64, _>("auto_update") != 0,
        },
        last_seen: r.get("last_seen"),
        info: info.and_then(|i| serde_json::from_str(&i).ok()),
        address: r.get("address"),
    }
}

pub async fn agents() -> Vec<AgentRecord> {
    sqlx::query("SELECT * FROM agent ORDER BY created, name")
        .fetch_all(pool())
        .await
        .map(|rows| rows.iter().map(agent_from_row).collect())
        .unwrap_or_default()
}

pub async fn agent(id: &str) -> Option<AgentRecord> {
    sqlx::query("SELECT * FROM agent WHERE id = ?")
        .bind(id)
        .fetch_optional(pool())
        .await
        .ok()
        .flatten()
        .map(|r| agent_from_row(&r))
}

pub async fn create_agent(
    name: &str,
    config: &AgentConfig,
) -> Result<(AgentRecord, String), String> {
    let id = random_token(6).to_lowercase().replace(['-', '_'], "x");
    let secret = random_token(32);
    let created = now();
    sqlx::query(
        "INSERT INTO agent (id, name, secret_hash, created, update_interval_secs, auto_update) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(hash(&secret))
    .bind(created)
    .bind(config.update_interval_secs as i64)
    .bind(config.auto_update as i64)
    .execute(pool())
    .await
    .map_err(db_error)?;
    let record = AgentRecord {
        id: id.clone(),
        name: name.to_string(),
        secret_hash: hash(&secret),
        config: config.clone(),
        last_seen: None,
        info: None,
        address: None,
    };
    Ok((record, format!("senn_{id}_{secret}")))
}

pub async fn rotate_agent_token(id: &str) -> Option<String> {
    let secret = random_token(32);
    let done = sqlx::query("UPDATE agent SET secret_hash = ? WHERE id = ?")
        .bind(hash(&secret))
        .bind(id)
        .execute(pool())
        .await
        .ok()?;
    (done.rows_affected() == 1).then(|| format!("senn_{id}_{secret}"))
}

pub async fn delete_agent(id: &str) -> bool {
    sqlx::query("DELETE FROM agent WHERE id = ?")
        .bind(id)
        .execute(pool())
        .await
        .map(|r| r.rows_affected() == 1)
        .unwrap_or(false)
}

pub async fn set_agent_settings(id: &str, name: &str, config: &AgentConfig, address: Option<&str>) -> Option<AgentRecord> {
    sqlx::query(
        "UPDATE agent SET name = ?, update_interval_secs = ?, auto_update = ?, address = ? WHERE id = ?",
    )
    .bind(name)
    .bind(config.update_interval_secs as i64)
    .bind(config.auto_update as i64)
    .bind(address)
    .bind(id)
    .execute(pool())
    .await
    .ok()?;
    agent(id).await
}

pub async fn set_agent_online(id: &str, info: &AgentInfo) -> Option<AgentRecord> {
    let text = serde_json::to_string(info).ok()?;
    sqlx::query("UPDATE agent SET info = ?, last_seen = ? WHERE id = ?")
        .bind(text)
        .bind(now())
        .bind(id)
        .execute(pool())
        .await
        .ok()?;
    agent(id).await
}

pub async fn touch_agent(id: &str) {
    let _ = sqlx::query("UPDATE agent SET last_seen = ? WHERE id = ?")
        .bind(now())
        .bind(id)
        .execute(pool())
        .await;
}

pub async fn verify_agent_token(token: &str) -> Option<String> {
    let rest = token.strip_prefix("senn_")?;
    let (id, secret) = rest.split_once('_')?;
    let record = agent(id).await?;
    let ok: bool = record
        .secret_hash
        .as_bytes()
        .ct_eq(hash(secret).as_bytes())
        .into();
    ok.then_some(record.id)
}

pub async fn add_session(token_hash: &str, expires: i64) {
    let _ = sqlx::query("DELETE FROM session WHERE expires <= ?")
        .bind(now())
        .execute(pool())
        .await;
    let _ = sqlx::query("INSERT OR REPLACE INTO session (token_hash, expires) VALUES (?, ?)")
        .bind(token_hash)
        .bind(expires)
        .execute(pool())
        .await;
}

pub async fn session_valid(token_hash: &str) -> bool {
    sqlx::query("SELECT 1 FROM session WHERE token_hash = ? AND expires > ?")
        .bind(token_hash)
        .bind(now())
        .fetch_optional(pool())
        .await
        .ok()
        .flatten()
        .is_some()
}

pub async fn remove_session(token_hash: &str) {
    let _ = sqlx::query("DELETE FROM session WHERE token_hash = ?")
        .bind(token_hash)
        .execute(pool())
        .await;
}

pub async fn setting(key: &str) -> Option<String> {
    sqlx::query("SELECT value FROM setting WHERE key = ?")
        .bind(key)
        .fetch_optional(pool())
        .await
        .ok()
        .flatten()
        .map(|r| r.get("value"))
}

pub async fn set_setting(key: &str, value: &str) -> bool {
    sqlx::query("INSERT INTO setting (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(key)
        .bind(value)
        .execute(pool())
        .await
        .is_ok()
}

#[derive(Debug, Clone, Default)]
pub struct Profile {
    pub username: String,
    pub display_name: Option<String>,
    pub avatar_version: Option<i64>,
}

impl Profile {
    pub fn name(&self) -> String {
        self.display_name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| self.username.clone())
    }

    pub fn initials(&self) -> String {
        self.name()
            .split(|c: char| c.is_whitespace() || c == '-' || c == '_' || c == '.')
            .filter(|p| !p.is_empty())
            .take(2)
            .filter_map(|p| p.chars().next())
            .flat_map(char::to_uppercase)
            .collect()
    }
}

pub async fn profile() -> Profile {
    sqlx::query("SELECT username, display_name, avatar_type, avatar_version FROM account WHERE id = 1")
        .fetch_optional(pool())
        .await
        .ok()
        .flatten()
        .map(|r| {
            let kind: Option<String> = r.get("avatar_type");
            Profile {
                username: r.get("username"),
                display_name: r.get("display_name"),
                avatar_version: kind.map(|_| r.get::<i64, _>("avatar_version")),
            }
        })
        .unwrap_or_default()
}

pub async fn set_display_name(name: Option<&str>) -> bool {
    sqlx::query("UPDATE account SET display_name = ? WHERE id = 1")
        .bind(name)
        .execute(pool())
        .await
        .is_ok()
}

pub async fn set_avatar(data: Option<(&[u8], &str)>) -> bool {
    sqlx::query("UPDATE account SET avatar = ?, avatar_type = ?, avatar_version = avatar_version + 1 WHERE id = 1")
        .bind(data.map(|d| d.0))
        .bind(data.map(|d| d.1))
        .execute(pool())
        .await
        .is_ok()
}

pub async fn avatar() -> Option<(Vec<u8>, String)> {
    let row = sqlx::query("SELECT avatar, avatar_type FROM account WHERE id = 1")
        .fetch_optional(pool())
        .await
        .ok()
        .flatten()?;
    Some((row.get::<Option<Vec<u8>>, _>("avatar")?, row.get::<Option<String>, _>("avatar_type")?))
}
