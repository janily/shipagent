use agentkit_protocol::generated as g;
use serde::{Deserialize, Serialize};
/// Keyboard shortcut used to submit messages. Local to this desktop client.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SendKey {
    /// Enter sends and Shift+Enter inserts a newline.
    #[default]
    Enter,
    /// Cmd on macOS, Ctrl elsewhere; plain Enter inserts a newline.
    CmdEnter,
}
/// A safe model choice resolved from an authenticated connection catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelChoice {
    /// Owning connection identity.
    pub connection: String,
    /// Connection's visible name, used for grouping.
    pub group: String,
    /// Exact backend model identifier.
    pub id: String,
    /// Visible model name.
    pub name: String,
}
/// Exclude unavailable credentials and deduplicate IDs within each connection.
pub fn model_choices(connections: &[g::LlmConnectionWithStatus]) -> Vec<ModelChoice> {
    let mut out = Vec::new();
    for c in connections.iter().filter(|c| c.is_authenticated) {
        let mut push = |id: &str, name: &str| {
            if !id.is_empty()
                && !out
                    .iter()
                    .any(|m: &ModelChoice| m.connection == c.slug && m.id == id)
            {
                out.push(ModelChoice {
                    connection: c.slug.clone(),
                    group: c.name.clone(),
                    id: id.into(),
                    name: name.into(),
                });
            }
        };
        for m in c.models.iter().flatten() {
            match m {
                g::LlmConnectionWithStatusModelsItem::String(id) => push(id, id),
                g::LlmConnectionWithStatusModelsItem::Object { id, name, .. } => push(id, name),
            }
        }
        if let Some(id) = &c.default_model {
            push(id, id);
        }
    }
    out
}

pub(crate) fn resolved_model(
    connections: &[g::LlmConnectionWithStatus],
    session: Option<&g::Session>,
    settings: Option<&g::WorkspaceSettings>,
) -> (Option<String>, Option<String>) {
    let slug = session
        .and_then(|s| s.llm_connection.clone())
        .or_else(|| settings.and_then(|s| s.default_llm_connection.clone()))
        .or_else(|| {
            connections
                .iter()
                .find(|c| c.is_authenticated && c.is_default == Some(true))
                .or_else(|| connections.iter().find(|c| c.is_authenticated))
                .map(|c| c.slug.clone())
        });
    let Some(c) = connections
        .iter()
        .find(|c| Some(&c.slug) == slug.as_ref() && c.is_authenticated)
    else {
        return (None, slug);
    };
    let model = session
        .and_then(|s| s.model.clone())
        .or_else(|| settings.and_then(|s| s.model.clone()))
        .or_else(|| c.default_model.clone());
    let choices = model_choices(std::slice::from_ref(c));
    (model.filter(|id| choices.iter().any(|m| &m.id == id)), slug)
}
