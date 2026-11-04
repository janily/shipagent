use super::*;
use agentkit_client::api::secrets::{ConnectionSetup, TestConnectionParams};
use agentkit_protocol::generated as g;
pub(super) fn connection_record(
    values: &ConnectionValues,
    existing: Option<&g::LlmConnectionWithStatus>,
) -> Result<g::LlmConnection> {
    anyhow::ensure!(
        !values.name.trim().is_empty() && !values.model.trim().is_empty(),
        agentkit_ui::t!("ak.settings.required")
    );
    anyhow::ensure!(
        !values.models.is_empty() && values.models.contains(&values.model),
        agentkit_ui::t!("ak.settings.required")
    );
    if values.kind == ConnectionKind::Preset && !values.existing {
        anyhow::ensure!(
            values.credential.is_some(),
            agentkit_ui::t!("ak.settings.key_required")
        );
    }
    if values.kind != ConnectionKind::Preset {
        let url = url::Url::parse(&values.url)?;
        anyhow::ensure!(
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none(),
            agentkit_ui::t!("ak.settings.invalid_url")
        );
    }
    if let Some(old) = existing {
        anyhow::ensure!(
            ConnectionKind::from_connection(old) == values.kind,
            agentkit_ui::t!("ak.settings.stale_editor")
        );
    }
    let mut record: g::LlmConnection = if let Some(old) = existing {
        serde_json::from_value(serde_json::to_value(old)?)?
    } else {
        serde_json::from_value(
            serde_json::json!({"slug":values.slug,"name":values.name,"createdAt":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis() as f64,"providerType":"pi","authType":"api_key"}),
        )?
    };
    record.name = values.name.clone();
    record.default_model = Some(values.model.clone());
    let previous = record.models.take().unwrap_or_default();
    record.models = Some(
        values
            .models
            .iter()
            .map(|id| {
                previous
                    .iter()
                    .find(|m| match m {
                        g::LlmConnectionModelsItem::String(s) => s == id,
                        g::LlmConnectionModelsItem::Object { id: old, .. } => old == id,
                    })
                    .cloned()
                    .unwrap_or_else(|| g::LlmConnectionModelsItem::String(id.clone()))
            })
            .collect(),
    );
    if record.auth_type != g::LlmConnectionAuthType::Oauth {
        record.provider_type = if values.kind == ConnectionKind::Preset {
            g::LlmConnectionProviderType::Pi
        } else {
            g::LlmConnectionProviderType::PiCompat
        };
        record.pi_auth_provider = Some(match values.kind {
            ConnectionKind::Preset => values.provider.clone(),
            ConnectionKind::Anthropic => "anthropic".into(),
            _ => "openai".into(),
        });
        record.base_url = if values.kind == ConnectionKind::Preset {
            None
        } else {
            Some(values.url.clone())
        };
        record.custom_endpoint = if values.kind == ConnectionKind::Preset {
            None
        } else {
            Some(g::CustomEndpointConfig {
                api: if values.kind == ConnectionKind::Anthropic {
                    g::CustomEndpointConfigApi::AnthropicMessages
                } else {
                    g::CustomEndpointConfigApi::OpenaiCompletions
                },
                ..record.custom_endpoint.unwrap_or(g::CustomEndpointConfig {
                    api: g::CustomEndpointConfigApi::OpenaiCompletions,
                    supports_images: None,
                    max_tokens: None,
                })
            })
        };
    }
    Ok(record)
}
pub(super) fn setup(values: &ConnectionValues, record: &g::LlmConnection) -> ConnectionSetup {
    ConnectionSetup {
        slug: values.slug.clone(),
        credential: values.credential.clone(),
        base_url: Some(record.base_url.clone()),
        default_model: Some(record.default_model.clone()),
        models: Some(Some(values.models.clone())),
        pi_auth_provider: record.pi_auth_provider.clone(),
        custom_endpoint: record.custom_endpoint.clone(),
        update_only: Some(values.existing),
        model_selection_mode: record.model_selection_mode,
        ..Default::default()
    }
}
pub(super) fn test_params(
    values: &ConnectionValues,
    record: &g::LlmConnection,
) -> TestConnectionParams {
    let mut params =
        TestConnectionParams::new(values.credential.clone().unwrap_or_else(|| "".into()));
    params.connection_slug = values.existing.then(|| values.slug.clone());
    params.base_url = record.base_url.clone();
    params.model = record.default_model.clone();
    params.pi_auth_provider = record.pi_auth_provider.clone();
    params.custom_endpoint = record.custom_endpoint.clone();
    params
}

#[cfg(test)]
#[path = "settings_connection_tests.rs"]
mod tests;
