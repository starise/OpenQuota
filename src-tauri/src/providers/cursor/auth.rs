use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde_json::Value;

use crate::providers::credential_store::{read_generic_password, write_generic_password};

use super::CursorError;

const ACCESS_TOKEN_KEY: &str = "cursorAuth/accessToken";
const REFRESH_TOKEN_KEY: &str = "cursorAuth/refreshToken";
const MEMBERSHIP_TYPE_KEY: &str = "cursorAuth/stripeMembershipType";
const ACCESS_TOKEN_SERVICE: &str = "cursor-access-token";
const REFRESH_TOKEN_SERVICE: &str = "cursor-refresh-token";
const REFRESH_BUFFER_MINUTES: i64 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorAuthSource {
    Sqlite(PathBuf),
    Keychain { account: String },
    CliFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorAuthState {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub source: CursorAuthSource,
}

impl CursorAuthState {
    pub fn load() -> Result<Option<Self>, CursorError> {
        Ok(Self::load_from_sources(
            &state_database_paths(),
            &cli_auth_paths(),
            load_keychain_auth(),
        ))
    }

    fn load_from_sources(
        databases: &[PathBuf],
        cli_files: &[PathBuf],
        keychain: Option<Self>,
    ) -> Option<Self> {
        let sqlite = databases.iter().find_map(|path| load_sqlite_auth(path));
        let existing = select_auth_state(sqlite, keychain);
        let cli = cli_files.iter().find_map(|path| load_cli_auth(path));
        match (existing, cli) {
            (Some(current), Some(cli)) if cli.is_newer_session_for(&current) => Some(cli),
            (current, cli) => current.or(cli),
        }
    }

    fn is_newer_session_for(&self, current: &Self) -> bool {
        let access = self.access_token.as_deref();
        let current_access = current.access_token.as_deref();
        token_subject(access).is_some_and(|subject| {
            token_subject(current_access).as_deref() == Some(subject.as_str())
        }) && access.and_then(token_expiration) > current_access.and_then(token_expiration)
    }

    pub fn has_local_credentials() -> bool {
        Self::load().ok().flatten().is_some()
    }

    pub fn needs_refresh(&self, now: DateTime<Utc>) -> bool {
        self.access_token
            .as_deref()
            .and_then(token_expiration)
            .is_none_or(|expires| expires - now <= Duration::minutes(REFRESH_BUFFER_MINUTES))
    }

    pub fn save_access_token(&mut self, access_token: String) -> Result<(), CursorError> {
        match &self.source {
            CursorAuthSource::Sqlite(path) => self.save_sqlite_access_token(path, &access_token),
            CursorAuthSource::Keychain { account } => {
                // shortcut: separate keychain reads and writes, use a conditional write if available.
                if read_keychain_value(ACCESS_TOKEN_SERVICE, account)? != self.access_token
                    || read_keychain_value(REFRESH_TOKEN_SERVICE, account)? != self.refresh_token
                {
                    return Err(CursorError::AuthWrite);
                }
                write_generic_password(ACCESS_TOKEN_SERVICE, account, access_token.as_bytes())
                    .map_err(|_| CursorError::AuthWrite)
            }
            CursorAuthSource::CliFile => return Err(CursorError::AuthWrite),
        }?;
        self.access_token = Some(access_token);
        Ok(())
    }

    fn save_sqlite_access_token(&self, path: &Path, access_token: &str) -> Result<(), CursorError> {
        let mut connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|_| CursorError::AuthWrite)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| CursorError::AuthWrite)?;
        let current_access =
            read_state_value(&transaction, ACCESS_TOKEN_KEY).map_err(|_| CursorError::AuthWrite)?;
        let current_refresh = read_state_value(&transaction, REFRESH_TOKEN_KEY)
            .map_err(|_| CursorError::AuthWrite)?;
        if current_access != self.access_token || current_refresh != self.refresh_token {
            return Err(CursorError::AuthWrite);
        }
        transaction
            .execute(
                "INSERT OR REPLACE INTO ItemTable (key, value) VALUES (?1, ?2)",
                (ACCESS_TOKEN_KEY, access_token),
            )
            .map_err(|_| CursorError::AuthWrite)?;
        transaction.commit().map_err(|_| CursorError::AuthWrite)
    }
}

fn load_cli_auth(path: &Path) -> Option<CursorAuthState> {
    let contents = std::fs::read(path).ok()?;
    let json: Value = serde_json::from_slice(&contents).ok()?;
    let access_token = non_empty(json.get("accessToken")?.as_str()?)?;
    token_subject(Some(&access_token))?;
    Some(CursorAuthState {
        access_token: Some(access_token),
        refresh_token: None,
        source: CursorAuthSource::CliFile,
    })
}

fn cli_auth_paths() -> Vec<PathBuf> {
    if cfg!(target_os = "windows") {
        return std::env::var_os("APPDATA")
            .map(|path| PathBuf::from(path).join("Cursor/auth.json"))
            .into_iter()
            .collect();
    }
    let home = std::env::var_os("HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    let mut paths = Vec::new();
    if cfg!(target_os = "linux") {
        if let Some(config) = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| home.as_ref().map(|path| path.join(".config")))
        {
            paths.push(config.join("cursor/auth.json"));
        }
    }
    if let Some(home) = home {
        paths.push(home.join(".cursor/auth.json"));
    }
    paths
}

fn select_auth_state(
    sqlite: Option<(CursorAuthState, Option<String>)>,
    keychain: Option<CursorAuthState>,
) -> Option<CursorAuthState> {
    if let Some((sqlite, membership)) = sqlite {
        let subjects_differ = token_subject(sqlite.access_token.as_deref()).is_some_and(|left| {
            token_subject(
                keychain
                    .as_ref()
                    .and_then(|auth| auth.access_token.as_deref()),
            )
            .is_some_and(|right| right != left)
        });
        if membership.as_deref() == Some("free") && subjects_differ {
            return keychain;
        }
        return Some(sqlite);
    }
    keychain
}

fn load_sqlite_auth(path: &Path) -> Option<(CursorAuthState, Option<String>)> {
    let mut connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let transaction = connection.transaction().ok()?;
    let access_token = read_state_value(&transaction, ACCESS_TOKEN_KEY).ok()?;
    let refresh_token = read_state_value(&transaction, REFRESH_TOKEN_KEY).ok()?;
    if access_token.is_none() && refresh_token.is_none() {
        return None;
    }
    let membership = read_state_value(&transaction, MEMBERSHIP_TYPE_KEY)
        .ok()
        .flatten()
        .map(|value| value.to_ascii_lowercase());
    Some((
        CursorAuthState {
            access_token,
            refresh_token,
            source: CursorAuthSource::Sqlite(path.to_path_buf()),
        },
        membership,
    ))
}

fn read_state_value(connection: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    connection
        .query_row(
            "SELECT value FROM ItemTable WHERE key = ?1 LIMIT 1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map(|value| value.and_then(non_empty))
}

fn load_keychain_auth() -> Option<CursorAuthState> {
    for account in keychain_accounts() {
        let access_token = read_keychain_value(ACCESS_TOKEN_SERVICE, &account)
            .ok()
            .flatten();
        let refresh_token = read_keychain_value(REFRESH_TOKEN_SERVICE, &account)
            .ok()
            .flatten();
        if access_token.is_some() || refresh_token.is_some() {
            return Some(CursorAuthState {
                access_token,
                refresh_token,
                source: CursorAuthSource::Keychain { account },
            });
        }
    }
    None
}

fn read_keychain_value(service: &str, account: &str) -> Result<Option<String>, CursorError> {
    read_generic_password(service, account)
        .map_err(|_| CursorError::AuthWrite)?
        .map(|bytes| String::from_utf8(bytes).map_err(|_| CursorError::AuthWrite))
        .transpose()
        .map(|value| value.and_then(non_empty))
}

fn keychain_accounts() -> Vec<String> {
    let current = std::env::var("USER")
        .ok()
        .or_else(|| std::env::var("USERNAME").ok())
        .and_then(non_empty);
    let mut accounts = vec![String::new()];
    if let Some(current) = current {
        accounts.push(current);
    }
    accounts
}

pub fn token_subject(token: Option<&str>) -> Option<String> {
    jwt_payload(token?)?
        .get("sub")?
        .as_str()
        .and_then(non_empty)
}

fn token_expiration(token: &str) -> Option<DateTime<Utc>> {
    let seconds = jwt_payload(token)?.get("exp").and_then(number)?;
    DateTime::from_timestamp(seconds as i64, 0)
}

fn jwt_payload(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        .filter(|value| value.is_finite())
}

fn non_empty(value: impl AsRef<str>) -> Option<String> {
    let value = value.as_ref().trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn state_database_paths() -> Vec<PathBuf> {
    if let Some(path) = std::env::var_os("OPENQUOTA_CURSOR_STATE_DB").map(PathBuf::from) {
        return vec![path];
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default();
    let mut paths = Vec::new();
    if let Some(app_data) = std::env::var_os("APPDATA").map(PathBuf::from) {
        paths.push(app_data.join("Cursor/User/globalStorage/state.vscdb"));
    }
    paths.push(home.join("Library/Application Support/Cursor/User/globalStorage/state.vscdb"));
    if let Some(config) = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from) {
        paths.push(config.join("Cursor/User/globalStorage/state.vscdb"));
    }
    paths.push(home.join(".config/Cursor/User/globalStorage/state.vscdb"));
    paths
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use chrono::{TimeZone, Utc};
    use rusqlite::Connection;
    use tempfile::tempdir;

    use super::*;

    fn jwt(subject: &str, expiration: i64) -> String {
        let payload = serde_json::json!({"sub": subject, "exp": expiration});
        format!("a.{}.c", URL_SAFE_NO_PAD.encode(payload.to_string()))
    }

    fn state(source: CursorAuthSource, access: &str) -> CursorAuthState {
        CursorAuthState {
            access_token: Some(access.into()),
            refresh_token: Some("refresh".into()),
            source,
        }
    }

    #[test]
    fn free_sqlite_workspace_prefers_different_agent_subject() {
        let sqlite = state(
            CursorAuthSource::Sqlite("db".into()),
            &jwt("auth0|free", 100),
        );
        let keychain = state(
            CursorAuthSource::Keychain {
                account: String::new(),
            },
            &jwt("auth0|pro", 100),
        );
        let selected = select_auth_state(Some((sqlite, Some("free".into()))), Some(keychain));
        assert!(matches!(
            selected.unwrap().source,
            CursorAuthSource::Keychain { .. }
        ));
    }

    #[test]
    fn sqlite_is_preferred_for_paid_or_same_subject_sessions() {
        let sqlite = state(
            CursorAuthSource::Sqlite("db".into()),
            &jwt("auth0|paid", 100),
        );
        let keychain = state(
            CursorAuthSource::Keychain {
                account: String::new(),
            },
            &jwt("auth0|other", 100),
        );
        let selected = select_auth_state(Some((sqlite, Some("pro".into()))), Some(keychain));
        assert!(matches!(
            selected.unwrap().source,
            CursorAuthSource::Sqlite(_)
        ));
    }

    #[test]
    fn sqlite_values_are_parameterized_and_refreshed_token_is_saved() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("state.vscdb");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "CREATE TABLE ItemTable (key TEXT PRIMARY KEY, value TEXT)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO ItemTable (key, value) VALUES (?1, ?2)",
                (ACCESS_TOKEN_KEY, "old"),
            )
            .unwrap();
        let (mut auth, _) = load_sqlite_auth(&path).unwrap();
        let mut old_session = auth.clone();
        auth.save_access_token("new'quoted".into()).unwrap();
        assert!(old_session.save_access_token("outdated".into()).is_err());
        assert_eq!(
            read_state_value(&connection, ACCESS_TOKEN_KEY)
                .unwrap()
                .as_deref(),
            Some("new'quoted")
        );
    }

    #[test]
    fn missing_or_near_expiry_jwt_needs_refresh() {
        let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
        let mut auth = state(CursorAuthSource::Sqlite("db".into()), "invalid");
        assert!(auth.needs_refresh(now));
        auth.access_token = Some(jwt("auth0|user", (now + Duration::minutes(4)).timestamp()));
        assert!(auth.needs_refresh(now));
        auth.access_token = Some(jwt("auth0|user", (now + Duration::minutes(6)).timestamp()));
        assert!(!auth.needs_refresh(now));
    }

    #[test]
    fn renewed_agent_login_is_used_for_the_same_account() {
        let directory = tempdir().unwrap();
        let database = directory.path().join("state.vscdb");
        let cli_file = directory.path().join("auth.json");
        let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
        let old = jwt("auth0|enterprise", (now - Duration::hours(1)).timestamp());
        let renewed = jwt("auth0|enterprise", (now + Duration::hours(1)).timestamp());
        let connection = Connection::open(&database).unwrap();
        connection
            .execute_batch("CREATE TABLE ItemTable (key TEXT PRIMARY KEY, value TEXT);")
            .unwrap();
        connection
            .execute(
                "INSERT INTO ItemTable VALUES (?1, ?2)",
                (ACCESS_TOKEN_KEY, &old),
            )
            .unwrap();
        let contents = serde_json::json!({"accessToken": renewed, "refreshToken": "cli-refresh"});
        std::fs::write(&cli_file, contents.to_string()).unwrap();

        let auth =
            CursorAuthState::load_from_sources(&[database], std::slice::from_ref(&cli_file), None)
                .unwrap();

        assert_eq!(auth.access_token.as_deref(), Some(renewed.as_str()));
        assert!(!auth.needs_refresh(now));
        assert!(auth.refresh_token.is_none());
        assert_eq!(
            std::fs::read_to_string(cli_file).unwrap(),
            contents.to_string()
        );
    }

    #[test]
    fn agent_login_does_not_replace_a_different_desktop_account() {
        let directory = tempdir().unwrap();
        let cli_file = directory.path().join("auth.json");
        let current = jwt("auth0|desktop", 100);
        let agent = jwt("auth0|agent", 200);
        std::fs::write(
            &cli_file,
            serde_json::json!({"accessToken": agent}).to_string(),
        )
        .unwrap();
        let keychain = state(
            CursorAuthSource::Keychain {
                account: String::new(),
            },
            &current,
        );

        let auth = CursorAuthState::load_from_sources(
            &[],
            std::slice::from_ref(&cli_file),
            Some(keychain),
        )
        .unwrap();
        assert_eq!(auth.access_token.as_deref(), Some(current.as_str()));

        let cli_only = CursorAuthState::load_from_sources(&[], &[cli_file], None).unwrap();
        assert_eq!(cli_only.access_token.as_deref(), Some(agent.as_str()));
    }
}
