pub mod auth;
pub mod client;
pub mod local_usage;
pub mod mapper;
pub mod reset_claim;

use std::sync::Arc;

use chrono::Utc;
use reqwest::StatusCode;
use thiserror::Error;

use crate::{
    hashing::sha256_hex,
    models::{
        MetricDefinition, MetricSection, ProviderDefinition, ProviderLink, ProviderSnapshot,
        UsagePeriodSelection,
    },
    pricing::PricingStore,
    storage::Storage,
};

use self::{
    auth::CodexAuthState, client::CodexClient, local_usage::scan_local_usage, mapper::map_usage,
};
use crate::providers::log_usage::scan_or_cached_usage;

pub(crate) fn definition() -> ProviderDefinition {
    ProviderDefinition {
        id: "codex".into(),
        display_name: "Codex".into(),
        short_name: "Cx".into(),
        fallback_enabled: true,
        local_usage_source_note: Some("From your Codex logs (estimated)".into()),
        links: vec![
            ProviderLink::new("Status", "https://status.openai.com/"),
            ProviderLink::new(
                "Dashboard",
                "https://chatgpt.com/settings/usage?tab=overview",
            ),
        ],
        metrics: vec![
            MetricDefinition::quota(
                "codex.session",
                "Session",
                "session",
                false,
                true,
                MetricSection::AlwaysVisible,
                true,
                "S",
            ),
            MetricDefinition::quota(
                "codex.weekly",
                "Weekly",
                "weekly",
                false,
                true,
                MetricSection::AlwaysVisible,
                true,
                "W",
            ),
            MetricDefinition::quota(
                "codex.spark",
                "Spark",
                "spark",
                false,
                true,
                MetricSection::OnDemand,
                false,
                "Sp",
            ),
            MetricDefinition::quota(
                "codex.sparkWeekly",
                "Spark Weekly",
                "sparkWeekly",
                false,
                true,
                MetricSection::OnDemand,
                false,
                "SW",
            ),
            MetricDefinition::trend("codex.trend"),
            MetricDefinition::value(
                "codex.credits",
                "Extra Usage",
                "credits",
                true,
                MetricSection::OnDemand,
                false,
                "E",
                None,
            ),
            MetricDefinition::value(
                "codex.rateLimitResets",
                "Rate Limit Resets",
                "rateLimitResets",
                true,
                MetricSection::OnDemand,
                false,
                "R",
                Some("resets"),
            ),
            MetricDefinition::usage(
                "codex.today",
                "Today",
                UsagePeriodSelection::Today,
                MetricSection::OnDemand,
                "T",
            ),
            MetricDefinition::usage(
                "codex.yesterday",
                "Yesterday",
                UsagePeriodSelection::Yesterday,
                MetricSection::OnDemand,
                "Y",
            ),
            MetricDefinition::usage(
                "codex.last30",
                "Last 30 Days",
                UsagePeriodSelection::Last30Days,
                MetricSection::OnDemand,
                "M",
            ),
        ],
    }
}

#[derive(Debug, Error)]
pub enum CodexError {
    #[error("Not logged in. Run `codex` to authenticate.")]
    NotLoggedIn,
    #[error(
        "Subscription usage is unavailable for API-key-only logins. Sign in to Codex with ChatGPT."
    )]
    ApiKeyOnly,
    #[error("Open Codex to renew your session, then refresh OpenQuota.")]
    TokenExpired,
    #[error("Codex auth data is invalid. Run `codex` to sign in again.")]
    InvalidAuth,
    #[error("The Codex account changed while usage was refreshing. Refresh again.")]
    AccountChanged,
    #[error("Codex usage request failed (HTTP {0}).")]
    RequestFailed(u16),
    #[error("Codex returned an invalid usage response.")]
    InvalidResponse,
    #[error("Could not connect to Codex. Check your internet connection.")]
    ConnectionFailed,
    #[error("Local Codex usage logs could not be processed.")]
    LocalUsage,
    #[error("OpenQuota cache is unavailable.")]
    Storage,
}

impl From<crate::storage::StorageError> for CodexError {
    fn from(_: crate::storage::StorageError) -> Self {
        Self::Storage
    }
}

pub struct CodexProvider {
    account_identity: Option<String>,
    storage: Arc<Storage>,
    pricing: Arc<PricingStore>,
    client: CodexClient,
}

impl CodexProvider {
    pub fn new(storage: Arc<Storage>, pricing: Arc<PricingStore>) -> Result<Self, CodexError> {
        let account_identity = CodexAuthState::observed_account_identity()
            .map(|identity| account_identity_key(&identity));
        if let Some(identity) = account_identity.as_deref() {
            crate::providers::remember_default_account(&storage, "codex", identity)?;
        }
        Ok(Self {
            account_identity,
            storage,
            pricing,
            client: CodexClient::new()?,
        })
    }

    pub fn refresh(&self) -> Result<ProviderSnapshot, CodexError> {
        self.refresh_with_identity().map(|(snapshot, _)| snapshot)
    }

    fn refresh_with_identity(&self) -> Result<(ProviderSnapshot, Option<String>), CodexError> {
        let now = Utc::now();
        let candidates = CodexAuthState::load_candidates()?;
        crate::app_debug!(
            "auth:codex",
            "credential candidates loaded ({})",
            candidates.len()
        );
        let mut last_auth_error = None;
        for mut auth in candidates {
            let identity = auth
                .account_identity()
                .map(|identity| account_identity_key(&identity));
            match self.refresh_candidate(&mut auth, now, identity.as_deref()) {
                Ok(snapshot) => return Ok((snapshot, identity)),
                Err(error @ CodexError::TokenExpired) => last_auth_error = Some(error),
                Err(error) => return Err(error),
            }
        }
        Err(last_auth_error.unwrap_or(CodexError::NotLoggedIn))
    }

    fn ensure_candidate_identity(
        auth: &CodexAuthState,
        expected: Option<&str>,
    ) -> Result<(), CodexError> {
        let observed = auth
            .account_identity()
            .map(|identity| account_identity_key(&identity));
        validate_account_identity(expected, observed.as_deref())
    }

    fn ensure_candidate_source_current(
        auth: &CodexAuthState,
        expected: Option<&str>,
    ) -> Result<(), CodexError> {
        let current = auth.reload().map_err(|_| CodexError::AccountChanged)?;
        Self::ensure_candidate_identity(&current, expected)
    }

    fn refresh_candidate(
        &self,
        auth: &mut CodexAuthState,
        now: chrono::DateTime<Utc>,
        account_identity: Option<&str>,
    ) -> Result<ProviderSnapshot, CodexError> {
        let mut warnings = Vec::new();

        Self::ensure_candidate_identity(auth, account_identity)?;

        let mut response = self
            .client
            .fetch_usage(&auth.access_token, auth.account_id.as_deref())?;
        if matches!(
            response.status,
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            let latest = auth.reload().map_err(|_| CodexError::AccountChanged)?;
            Self::ensure_candidate_identity(&latest, account_identity)?;
            if latest.access_token != auth.access_token {
                *auth = latest;
                response = self
                    .client
                    .fetch_usage(&auth.access_token, auth.account_id.as_deref())?;
            }
        }
        let reset_credits = if response.status.is_success() {
            self.client
                .fetch_reset_credits(&auth.access_token, auth.account_id.as_deref())
                .ok()
        } else {
            None
        };
        let mapped = map_usage(&response, reset_credits.as_ref(), now)?;
        let pricing = self.pricing.current();
        let usage = scan_or_cached_usage(
            &self.storage,
            "codex",
            account_identity
                .map(crate::providers::CacheIdentity::Resolved)
                .unwrap_or(crate::providers::CacheIdentity::Unresolved),
            "Codex",
            || scan_local_usage(&self.storage, now, &pricing),
            &mut warnings,
        );
        Self::ensure_candidate_source_current(auth, account_identity)?;
        Ok(ProviderSnapshot {
            provider_id: "codex".into(),
            plan: mapped.plan,
            quotas: mapped.quotas,
            value_metrics: mapped.value_metrics,
            status_metrics: Vec::new(),
            notices: Vec::new(),
            usage,
            warnings,
            refreshed_at: now,
        })
    }
}

fn account_identity_key(identity: &str) -> String {
    sha256_hex(identity.as_bytes())
}

fn validate_account_identity(
    expected: Option<&str>,
    observed: Option<&str>,
) -> Result<(), CodexError> {
    (expected == observed)
        .then_some(())
        .ok_or(CodexError::AccountChanged)
}

fn provider_error(error: CodexError) -> crate::providers::ProviderError {
    use crate::models::ProviderErrorKind as Kind;

    let kind = match error {
        CodexError::NotLoggedIn
        | CodexError::TokenExpired
        | CodexError::InvalidAuth
        | CodexError::AccountChanged => Kind::Authentication,
        CodexError::ApiKeyOnly => Kind::Permission,
        CodexError::RequestFailed(429) => Kind::RateLimited,
        CodexError::RequestFailed(_) | CodexError::ConnectionFailed => Kind::Network,
        CodexError::InvalidResponse => Kind::InvalidResponse,
        CodexError::LocalUsage => Kind::LocalData,
        CodexError::Storage => Kind::Storage,
    };
    crate::providers::ProviderError::from_display(kind, error)
}

impl crate::providers::UsageProvider for CodexProvider {
    fn definition(&self) -> ProviderDefinition {
        definition()
    }

    fn has_local_credentials(&self) -> bool {
        CodexAuthState::has_local_credentials()
    }

    fn cache_identity(&self) -> crate::providers::CacheIdentity<'_> {
        self.account_identity
            .as_deref()
            .map(crate::providers::CacheIdentity::Resolved)
            .unwrap_or(crate::providers::CacheIdentity::Unresolved)
    }

    fn supports_account_names(&self) -> bool {
        true
    }

    fn account_identity(&self) -> Option<&str> {
        self.account_identity.as_deref()
    }

    fn refresh(&self) -> Result<ProviderSnapshot, crate::providers::ProviderError> {
        CodexProvider::refresh(self).map_err(provider_error)
    }

    fn refresh_for_service(
        &self,
    ) -> Result<crate::providers::ProviderRefresh, crate::providers::ProviderError> {
        let (snapshot, identity) = self.refresh_with_identity().map_err(provider_error)?;
        Ok(crate::providers::ProviderRefresh {
            snapshot,
            cache_identity: identity.clone(),
            account: identity.map(|identity| crate::providers::AccountRefresh {
                family: "codex",
                provider_id: "codex",
                identity,
            }),
        })
    }
}

#[cfg(test)]
mod account_tests {
    use std::{
        fs,
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        sync::Arc,
        thread,
        time::Duration,
    };

    use tempfile::tempdir;

    use super::{validate_account_identity, CodexClient, CodexError, CodexProvider};
    use crate::{
        pricing::PricingStore,
        providers::{test_http::serve_once, CacheIdentity, UsageProvider},
        storage::Storage,
    };

    #[test]
    fn quota_refresh_leaves_shared_credentials_to_codex() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("auth.json");
        let original = r#"{"last_refresh":"2025-01-01T00:00:00Z","tokens":{"access_token":"saved-access","refresh_token":"shared-refresh"}}"#;
        fs::write(&path, original).unwrap();
        let mut auth = super::auth::load_from_path(&path).unwrap();
        let usage = serve_once(
            200,
            &[],
            r#"{"rate_limit":{"primary_window":{"used_percent":10,"reset_after_seconds":60}}}"#,
        );
        let resets = serve_once(200, &[], "{}");
        let provider = CodexProvider {
            account_identity: None,
            storage: Arc::new(Storage::open(&directory.path().join("openquota.db")).unwrap()),
            pricing: Arc::new(
                PricingStore::new_without_refresh_for_test(directory.path().join("pricing"))
                    .unwrap(),
            ),
            client: CodexClient::with_endpoints(&usage, &resets, &resets, Duration::from_secs(2))
                .unwrap(),
        };

        let snapshot = provider
            .refresh_candidate(&mut auth, chrono::Utc::now(), None)
            .unwrap();

        assert_eq!(snapshot.quotas[0].used_percent, 10.0);
        assert_eq!(fs::read_to_string(path).unwrap(), original);
    }

    #[test]
    fn rejected_access_token_retries_with_the_login_published_by_codex() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("auth.json");
        let original = r#"{"tokens":{"access_token":"old-access","account_id":"login-a"}}"#;
        let replacement = r#"{"tokens":{"access_token":"new-access","account_id":"login-a"}}"#;
        fs::write(&path, original).unwrap();
        let mut auth = super::auth::load_from_path(&path).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let usage = format!("http://{}", listener.local_addr().unwrap());
        let server_path = path.clone();
        let server = thread::spawn(move || {
            for (token, status, body) in [
                ("old-access", "401 Unauthorized", "{}"),
                (
                    "new-access",
                    "200 OK",
                    r#"{"rate_limit":{"primary_window":{"used_percent":25,"reset_after_seconds":60}}}"#,
                ),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let headers: Vec<_> = BufReader::new(&stream)
                    .lines()
                    .map(Result::unwrap)
                    .take_while(|line| !line.is_empty())
                    .collect();
                assert!(headers.iter().any(
                    |line| line.eq_ignore_ascii_case(&format!("Authorization: Bearer {token}"))
                ));
                if token == "old-access" {
                    fs::write(&server_path, replacement).unwrap();
                }
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        let resets = serve_once(200, &[], "{}");
        let provider = CodexProvider {
            account_identity: None,
            storage: Arc::new(Storage::open(&directory.path().join("openquota.db")).unwrap()),
            pricing: Arc::new(
                PricingStore::new_without_refresh_for_test(directory.path().join("pricing"))
                    .unwrap(),
            ),
            client: CodexClient::with_endpoints(&usage, &resets, &resets, Duration::from_secs(2))
                .unwrap(),
        };

        let identity = super::account_identity_key("login-a");
        let snapshot = provider
            .refresh_candidate(&mut auth, chrono::Utc::now(), Some(&identity))
            .unwrap();

        server.join().unwrap();
        assert_eq!(snapshot.quotas[0].used_percent, 25.0);
        assert_eq!(fs::read_to_string(path).unwrap(), replacement);
    }

    #[test]
    fn pinned_account_rejects_a_different_or_unreadable_login() {
        assert!(validate_account_identity(Some("account-a"), Some("account-a")).is_ok());
        assert!(matches!(
            validate_account_identity(Some("account-a"), Some("account-b")),
            Err(CodexError::AccountChanged)
        ));
        assert!(matches!(
            validate_account_identity(Some("account-a"), None),
            Err(CodexError::AccountChanged)
        ));
        assert!(matches!(
            validate_account_identity(None, Some("account-b")),
            Err(CodexError::AccountChanged)
        ));
        assert!(validate_account_identity(None, None).is_ok());
    }

    #[test]
    fn cache_identity_tracks_the_launch_resolved_account() {
        let directory = tempdir().unwrap();
        let storage = Arc::new(Storage::open(&directory.path().join("openquota.db")).unwrap());
        let pricing = Arc::new(
            PricingStore::new_without_refresh_for_test(directory.path().join("pricing")).unwrap(),
        );
        let provider = CodexProvider {
            account_identity: Some("account-a".into()),
            storage: storage.clone(),
            pricing: pricing.clone(),
            client: CodexClient::new().unwrap(),
        };
        let unresolved = CodexProvider {
            account_identity: None,
            storage,
            pricing,
            client: CodexClient::new().unwrap(),
        };

        assert_eq!(
            UsageProvider::cache_identity(&provider),
            CacheIdentity::Resolved("account-a")
        );
        assert_eq!(
            UsageProvider::cache_identity(&unresolved),
            CacheIdentity::Unresolved
        );
    }
}
