/// Notification attached to a pane.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NotificationState {
    /// Logical preset/event name, eg. `stop` or `notification`.
    pub name: String,
    /// Emoji currently rendered for this notification.
    pub emoji: String,
    /// Monotonic sequence used to pick the latest notification for a tab.
    pub sequence: u64,
}
