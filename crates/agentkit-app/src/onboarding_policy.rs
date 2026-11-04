use agentkit_protocol::generated::LlmConnectionWithStatus;
pub(super) fn can_save(
    saved: Option<&(String, f64)>,
    slug: &str,
    catalog: &[LlmConnectionWithStatus],
) -> bool {
    match catalog.iter().find(|c| c.slug == slug) {
        Some(c) => saved.is_some_and(|(id, created)| id == &c.slug && *created == c.created_at),
        None => saved.is_none(),
    }
}
pub(super) fn completed(
    saved: Option<&(String, f64)>,
    catalog: &[LlmConnectionWithStatus],
) -> bool {
    saved.is_some_and(|(slug, created)| {
        catalog.iter().any(|c| {
            &c.slug == slug
                && *created == c.created_at
                && c.is_authenticated
                && c.is_default == Some(true)
        })
    })
}
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    fn connection() -> LlmConnectionWithStatus {
        serde_json::from_value(serde_json::json!({"slug":"pi-api-key","name":"First model","providerType":"pi_compat","authType":"none","createdAt":42,"isAuthenticated":true,"isDefault":true})).unwrap()
    }
    #[test]
    fn first_write_rejects_collisions_and_retries_require_the_same_owned_identity() {
        let c = connection();
        let saved = (c.slug.clone(), c.created_at);
        assert!(can_save(None, &c.slug, &[]));
        assert!(!can_save(None, &c.slug, std::slice::from_ref(&c)));
        assert!(can_save(Some(&saved), &c.slug, std::slice::from_ref(&c)));
        assert!(!can_save(Some(&saved), &c.slug, &[]));
        let mut replacement = c.clone();
        replacement.created_at += 1.;
        assert!(!can_save(Some(&saved), &c.slug, &[replacement]));
    }
    #[test]
    fn completion_requires_our_authenticated_default_and_detects_removal_or_replacement() {
        let c = connection();
        let saved = (c.slug.clone(), c.created_at);
        assert!(completed(Some(&saved), std::slice::from_ref(&c)));
        assert!(!completed(Some(&saved), &[]));
        let mut changed = c.clone();
        changed.is_authenticated = false;
        assert!(!completed(Some(&saved), &[changed]));
        let mut changed = c;
        changed.created_at += 1.;
        assert!(!completed(Some(&saved), &[changed]));
    }
}
