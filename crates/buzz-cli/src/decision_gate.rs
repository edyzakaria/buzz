//! Decision-gating state machine for Supervisor-led work.
//!
//! Tracks the four states of a decision gate:
//! - mentioned: Supervisor was @mentioned in a thread
//! - drafted: Supervisor posted a summary proposal
//! - confirmed: Lead confirmed/approved the proposal
//! - executing: Supervisor is executing the work
//!
//! State is tracked via Nostr event tags (`["decision_state", "<state>"]`).
//! Transitions are auditable via the event history.

use nostr::Tag;

use crate::error::CliError;

/// The four states of a decision gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionState {
    /// Supervisor was @mentioned in the thread.
    Mentioned,
    /// Supervisor posted a drafted summary.
    Drafted,
    /// Lead confirmed/approved the summary.
    Confirmed,
    /// Supervisor is executing the work.
    Executing,
}

impl DecisionState {
    /// String representation for event tags.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mentioned => "mentioned",
            Self::Drafted => "drafted",
            Self::Confirmed => "confirmed",
            Self::Executing => "executing",
        }
    }

    /// Parse from a string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "mentioned" => Some(Self::Mentioned),
            "drafted" => Some(Self::Drafted),
            "confirmed" => Some(Self::Confirmed),
            "executing" => Some(Self::Executing),
            _ => None,
        }
    }
}

/// Adds a decision-state tag to a message builder.
pub fn add_decision_state_tag(
    builder: buzz_sdk::EventBuilder,
    state: DecisionState,
) -> Result<buzz_sdk::EventBuilder, CliError> {
    let tag = Tag::parse(["decision_state", state.as_str()])
        .map_err(|e| CliError::Other(format!("failed to create decision_state tag: {}", e)))?;
    Ok(builder.tag(tag))
}

/// Posts a decision state transition comment to an ISM issue (if linked).
///
/// If the thread is linked to an ISM ticket (via `["ism_ticket", "<id>"]` tag),
/// this posts a status comment to that ticket. Otherwise, it's a no-op.
///
/// # Arguments
/// * `ism_ticket_id` - The ISM ticket ID (e.g., "ISM-123"), if linked
/// * `state` - The new decision state
/// * `detail` - Optional detail message (appended to the status comment)
pub async fn sync_state_to_ism(
    ism_ticket_id: Option<&str>,
    state: DecisionState,
    detail: Option<&str>,
) -> Result<(), CliError> {
    if let Some(ticket_id) = ism_ticket_id {
        // Get ISM client from environment
        let ism_base_url = std::env::var("BUZZ_ISM_BASE_URL")
            .map_err(|_| CliError::Usage("BUZZ_ISM_BASE_URL environment variable not set".into()))?;

        let ism_email = std::env::var("BUZZ_ISM_SERVICE_EMAIL").map_err(|_| {
            CliError::Usage("BUZZ_ISM_SERVICE_EMAIL environment variable not set".into())
        })?;

        let ism_password = std::env::var("BUZZ_ISM_SERVICE_PASSWORD").map_err(|_| {
            CliError::Usage("BUZZ_ISM_SERVICE_PASSWORD environment variable not set".into())
        })?;

        let ism_client = buzz_ism::IsmClient::new(ism_base_url, ism_email, ism_password)
            .map_err(|e| CliError::Other(format!("failed to create ISM client: {}", e)))?;

        // Build comment body
        let mut comment_body = format!("**Decision State**: `{}`", state.as_str());
        if let Some(d) = detail {
            comment_body.push_str("\n\n");
            comment_body.push_str(d);
        }

        // Post comment to ISM
        ism_client
            .post_comment(ticket_id, &comment_body)
            .await
            .map_err(|e| {
                CliError::Other(format!(
                    "failed to post comment to ISM ticket {}: {}",
                    ticket_id, e
                ))
            })?;
    }

    Ok(())
}

/// Creates an ISM issue for a decision thread (retroactively, on confirmation).
///
/// If a decision thread was confirmed but has no ISM ticket linked, this creates
/// one and returns the new ticket ID.
///
/// # Arguments
/// * `title` - The issue title
/// * `description` - The issue description (e.g., the drafted summary)
///
/// # Returns
/// The new ISM ticket ID (e.g., "ISM-124")
pub async fn create_issue_for_decision(
    title: &str,
    description: &str,
) -> Result<String, CliError> {
    // Get ISM client from environment
    let ism_base_url = std::env::var("BUZZ_ISM_BASE_URL")
        .map_err(|_| CliError::Usage("BUZZ_ISM_BASE_URL environment variable not set".into()))?;

    let ism_email = std::env::var("BUZZ_ISM_SERVICE_EMAIL")
        .map_err(|_| CliError::Usage("BUZZ_ISM_SERVICE_EMAIL environment variable not set".into()))?;

    let ism_password = std::env::var("BUZZ_ISM_SERVICE_PASSWORD").map_err(|_| {
        CliError::Usage("BUZZ_ISM_SERVICE_PASSWORD environment variable not set".into())
    })?;

    let ism_client = buzz_ism::IsmClient::new(ism_base_url, ism_email, ism_password)
        .map_err(|e| CliError::Other(format!("failed to create ISM client: {}", e)))?;

    // Create issue
    let issue_id = ism_client
        .create_issue(title, Some(description))
        .await
        .map_err(|e| CliError::Other(format!("failed to create ISM issue: {}", e)))?;

    Ok(issue_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_state_roundtrip() {
        let states = [
            DecisionState::Mentioned,
            DecisionState::Drafted,
            DecisionState::Confirmed,
            DecisionState::Executing,
        ];

        for state in states {
            let s = state.as_str();
            let parsed = DecisionState::from_str(s).expect("failed to parse");
            assert_eq!(parsed, state);
        }
    }

    #[test]
    fn test_decision_state_unknown() {
        assert_eq!(DecisionState::from_str("unknown"), None);
    }
}
