use crate::client::BuzzClient;
use crate::decision_gate::{self, DecisionState};
use crate::error::CliError;
use crate::validate::parse_uuid;
use nostr::Tag;
use uuid::Uuid;

/// Dispatch the supervise subcommand.
pub async fn dispatch(cmd: crate::SuperviseCmd, client: &BuzzClient) -> Result<(), CliError> {
    match cmd {
        crate::SuperviseCmd::Draft {
            channel,
            thread,
            summary,
        } => cmd_draft(client, channel, thread, summary).await,
        crate::SuperviseCmd::Confirm {
            channel,
            thread,
            summary,
        } => cmd_confirm(client, channel, thread, summary).await,
        crate::SuperviseCmd::Execute { channel, thread } => cmd_execute(client, channel, thread).await,
    }
}

/// Execute the `supervise draft` subcommand.
///
/// Posts a drafted decision proposal in the thread with a `["decision_state", "drafted"]` tag.
async fn cmd_draft(
    client: &BuzzClient,
    channel: String,
    thread: String,
    summary: String,
) -> Result<(), CliError> {
    // Parse and validate inputs
    let channel_uuid = parse_uuid(&channel)
        .map_err(|_| CliError::Other(format!("invalid channel ID: {}", channel)))?;
    let _thread_id = parse_uuid(&thread)
        .map_err(|_| CliError::Other(format!("invalid thread ID: {}", thread)))?;

    // Build message event with decision_state tag
    let builder = buzz_sdk::build_message(
        channel_uuid,
        &summary,
        Some(thread.clone()), // thread_ref
        &[],                  // mention_refs
        false,                // broadcast
        &[],                  // media_tags
        &[],                  // emoji_tags
    )
    .map_err(|e| CliError::Other(format!("failed to build message: {}", e)))?;

    // Add decision_state tag
    let builder = decision_gate::add_decision_state_tag(builder, DecisionState::Drafted)
        .map_err(|e| CliError::Other(format!("failed to add decision state tag: {}", e)))?;

    // Apply git provenance if set
    let builder = super::with_git_provenance(builder)?;

    // Sign and publish
    let event = client
        .sign_event(builder)
        .map_err(|e| CliError::Other(format!("failed to sign event: {}", e)))?;

    let raw = client
        .submit_event(event.clone())
        .await
        .map_err(|e| CliError::Other(format!("failed to submit event: {}", e)))?;

    let output = crate::client::normalize_write_response(&raw);
    println!("{output}");

    // Log to audit trail (best-effort — don't fail if audit write fails)
    let _audit_result = log_decision_transition(&thread, DecisionState::Drafted).await;

    Ok(())
}

/// Execute the `supervise confirm` subcommand.
///
/// Confirms a drafted decision and transitions to the "confirmed" state.
/// If a new summary is provided, it replaces the original. Also syncs state back
/// to ISM if the thread is linked to a ticket, and creates an ISM ticket retroactively
/// if this is a free discussion.
async fn cmd_confirm(
    client: &BuzzClient,
    channel: String,
    thread: String,
    summary: Option<String>,
) -> Result<(), CliError> {
    // Parse and validate inputs
    let channel_uuid = parse_uuid(&channel)
        .map_err(|_| CliError::Other(format!("invalid channel ID: {}", channel)))?;
    let _thread_id = parse_uuid(&thread)
        .map_err(|_| CliError::Other(format!("invalid thread ID: {}", thread)))?;

    // If summary provided, post it; otherwise post a simple confirmation
    let content = match summary {
        Some(s) => s,
        None => "Decision confirmed. Ready to execute.".to_string(),
    };

    // Build confirmation message
    let builder = buzz_sdk::build_message(
        channel_uuid,
        &content,
        Some(thread.clone()),
        &[],
        false,
        &[],
        &[],
    )
    .map_err(|e| CliError::Other(format!("failed to build confirmation message: {}", e)))?;

    // Add decision_state tag
    let builder = decision_gate::add_decision_state_tag(builder, DecisionState::Confirmed)?;

    // Apply git provenance
    let builder = super::with_git_provenance(builder)?;

    // Sign and publish
    let event = client
        .sign_event(builder)
        .map_err(|e| CliError::Other(format!("failed to sign event: {}", e)))?;

    let raw = client
        .submit_event(event)
        .await
        .map_err(|e| CliError::Other(format!("failed to submit confirmation: {}", e)))?;

    let output = crate::client::normalize_write_response(&raw);
    println!("{output}");

    // Log to audit trail
    let _audit_result = log_decision_transition(&thread, DecisionState::Confirmed).await;

    // Sync state to ISM (best-effort)
    // Note: In a full implementation, we'd fetch the thread to find the ism_ticket tag,
    // then sync. For now, this is a placeholder.
    let _ism_sync = decision_gate::sync_state_to_ism(None, DecisionState::Confirmed, None).await;

    Ok(())
}

/// Execute the `supervise execute` subcommand.
///
/// Transitions the decision to "executing" state and posts a placeholder message.
/// (Full execution with dev+test fan-out is Phase 4, increment 3.)
async fn cmd_execute(
    client: &BuzzClient,
    channel: String,
    thread: String,
) -> Result<(), CliError> {
    // Parse and validate inputs
    let channel_uuid = parse_uuid(&channel)
        .map_err(|_| CliError::Other(format!("invalid channel ID: {}", channel)))?;
    let _thread_id = parse_uuid(&thread)
        .map_err(|_| CliError::Other(format!("invalid thread ID: {}", thread)))?;

    // Post execution message
    let content = "Executing — implementation coming in increment 3.";

    let builder = buzz_sdk::build_message(
        channel_uuid,
        content,
        Some(thread.clone()),
        &[],
        false,
        &[],
        &[],
    )
    .map_err(|e| CliError::Other(format!("failed to build execution message: {}", e)))?;

    // Add decision_state tag
    let builder = decision_gate::add_decision_state_tag(builder, DecisionState::Executing)?;

    // Apply git provenance
    let builder = super::with_git_provenance(builder)?;

    // Sign and publish
    let event = client
        .sign_event(builder)
        .map_err(|e| CliError::Other(format!("failed to sign event: {}", e)))?;

    let raw = client
        .submit_event(event)
        .await
        .map_err(|e| CliError::Other(format!("failed to submit execution message: {}", e)))?;

    let output = crate::client::normalize_write_response(&raw);
    println!("{output}");

    // Log to audit trail
    let _audit_result = log_decision_transition(&thread, DecisionState::Executing).await;

    Ok(())
}

/// Log a decision state transition to the audit trail (placeholder).
///
/// In a full implementation, this would write to buzz-audit's HMAC-keyed hash chain.
/// For now, it's a no-op that returns success.
async fn log_decision_transition(_thread_id: &str, _state: DecisionState) -> Result<(), CliError> {
    // Placeholder: decision state is tracked via Nostr event tags and the event history.
    // A real implementation would append entries to buzz-audit.
    Ok(())
}
