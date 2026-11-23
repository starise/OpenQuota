use std::sync::{atomic::AtomicU64, Arc};

use tauri::{AppHandle, Emitter};
use tokio::time::Instant;

use crate::{
    commands::settings::emit_settings_if_account_changed,
    models::ProviderErrorKind,
    notifications::finish_refresh,
    pacing::NotificationEvaluator,
    policy::{FAILURE_RETRY_BACKOFF, REFRESH_INTERVAL},
    service::{ProviderService, UsageViewState},
    settings::SettingsService,
};

pub fn spawn(
    app: AppHandle,
    service: Arc<ProviderService>,
    settings: Arc<SettingsService>,
    notifications: Arc<NotificationEvaluator>,
) {
    tauri::async_runtime::spawn(async move {
        let mut next_full_refresh = Instant::now();
        loop {
            let full_refresh_due = Instant::now() >= next_full_refresh;
            let provider_ids = providers_to_refresh(
                settings.enabled_provider_ids(),
                &service.state(),
                full_refresh_due,
            );
            if !provider_ids.is_empty() {
                let progress_app = app.clone();
                let progress_settings = settings.clone();
                let observed_account_revision =
                    Arc::new(AtomicU64::new(settings.account_revision()));
                let progress_account_revision = observed_account_revision.clone();
                let on_progress = move |state: &UsageViewState| {
                    emit_settings_if_account_changed(
                        &progress_app,
                        &progress_settings,
                        &progress_account_revision,
                    );
                    let _ = progress_app.emit("usage-state", state);
                };
                let state = if full_refresh_due {
                    service
                        .refresh_all_with_progress(&provider_ids, false, on_progress)
                        .await
                } else {
                    service
                        .refresh_enabled_with_progress(&provider_ids, false, on_progress)
                        .await
                };
                emit_settings_if_account_changed(&app, &settings, &observed_account_revision);
                let _ = app.emit("usage-state", &state);
                finish_refresh(&app, &state, &settings, &notifications);
            }
            if full_refresh_due {
                next_full_refresh = Instant::now() + REFRESH_INTERVAL;
            }
            tokio::time::sleep_until(next_full_refresh.min(Instant::now() + FAILURE_RETRY_BACKOFF))
                .await;
        }
    });
}

fn providers_to_refresh(
    enabled: Vec<String>,
    state: &UsageViewState,
    full_refresh_due: bool,
) -> Vec<String> {
    if full_refresh_due {
        return enabled;
    }
    enabled
        .into_iter()
        .filter(|id| {
            state
                .providers
                .get(id)
                .is_some_and(|provider| provider.error_kind == Some(ProviderErrorKind::Network))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ProviderErrorKind, ProviderViewState};

    #[test]
    fn network_errors_retry_before_the_next_full_refresh() {
        let enabled = vec!["online".into(), "offline".into(), "expired".into()];
        let provider_error = |kind| ProviderViewState {
            error_kind: Some(kind),
            ..ProviderViewState::default()
        };
        let state = crate::service::UsageViewState {
            providers: [
                ("offline".into(), provider_error(ProviderErrorKind::Network)),
                (
                    "disabled".into(),
                    provider_error(ProviderErrorKind::Network),
                ),
                (
                    "expired".into(),
                    provider_error(ProviderErrorKind::Authentication),
                ),
            ]
            .into(),
            last_full_refresh_at: None,
        };

        assert_eq!(
            providers_to_refresh(enabled.clone(), &state, false),
            ["offline"]
        );
        assert_eq!(providers_to_refresh(enabled.clone(), &state, true), enabled);
    }
}
