//! Native delivery only; visibility, deduplication and user preferences belong to the controller.
/// Sanitized OS notification failures; no transcript or platform error contents are retained.
#[derive(Debug, thiserror::Error)]
pub enum NotificationError {
    /// The OS authorization service is unavailable or the application is not bundled.
    #[error("notification authorization unavailable")]
    AuthorizationUnavailable,
    /// The user declined OS authorization.
    #[error("notification authorization declined")]
    AuthorizationDeclined,
    /// The native notification service could not accept the alert.
    #[error("system notification unavailable")]
    Unavailable,
}
/// Deliver a private, generic approval alert from a background worker.
/// macOS uses UNUserNotificationCenter and requests authorization on first use.
/// OS denial/unavailability is recoverable and must not fail the Agent session.
pub fn approval(app_name: &str, bundle_id: &str, body: &str) -> Result<(), NotificationError> {
    #[cfg(target_os = "macos")]
    if !notify_rust::request_auth_blocking()
        .map_err(|_| NotificationError::AuthorizationUnavailable)?
    {
        return Err(NotificationError::AuthorizationDeclined);
    }
    let mut notification = notify_rust::Notification::new();
    notification
        .appname(app_name)
        .summary(app_name)
        .body(body)
        .timeout(10_000);
    #[cfg(target_os = "windows")]
    notification.app_id(bundle_id);
    #[cfg(not(target_os = "windows"))]
    let _ = bundle_id;
    notification
        .show()
        .map(|_| ())
        .map_err(|_| NotificationError::Unavailable)
}
