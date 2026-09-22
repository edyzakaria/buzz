use buzz_ism::IsmClient;
use nostr::Tag;
use uuid::Uuid;

use crate::client::BuzzClient;
use crate::error::CliError;
use crate::validate::parse_uuid;

/// Format ISM issue details as a Markdown summary.
fn format_ism_summary(
    issue_id: &str,
    issue: &buzz_ism::Issue,
    comments: &[buzz_ism::Comment],
    attachments: &[buzz_ism::Attachment],
) -> String {
    let mut md = String::new();

    // Header with issue ID and title
    md.push_str(&format!("## {} — {}\n\n", issue_id, issue.title));

    // Metadata: status, priority, assignee
    md.push_str("**Status:** ");
    md.push_str(&issue.status);
    md.push('\n');

    if let Some(priority) = &issue.priority {
        md.push_str("**Priority:** ");
        md.push_str(priority);
        md.push('\n');
    }

    if let Some(assignee) = &issue.assignee {
        md.push_str("**Assigned to:** ");
        md.push_str(assignee);
        md.push('\n');
    }

    if !issue.labels.is_empty() {
        md.push_str("**Labels:** ");
        md.push_str(&issue.labels.join(", "));
        md.push('\n');
    }

    md.push('\n');

    // Description
    if let Some(description) = &issue.description {
        if !description.is_empty() {
            md.push_str(description);
            md.push_str("\n\n");
        }
    }

    // Comments
    if !comments.is_empty() {
        md.push_str("### Comments\n\n");
        for (i, comment) in comments.iter().enumerate() {
            md.push_str(&format!(
                "**{}** ({}):\n\n{}\n\n",
                comment.author, comment.created_at, comment.body
            ));
            if i < comments.len() - 1 {
                md.push_str("---\n\n");
            }
        }
    }

    // Attachments
    if !attachments.is_empty() {
        md.push_str("### Attachments\n\n");
        for attachment in attachments {
            md.push_str(&format!(
                "- [{}]({}) ({})\n",
                attachment.filename, attachment.url, attachment.content_type
            ));
        }
        md.push('\n');
    }

    md
}

/// Dispatch the discuss subcommand.
pub async fn dispatch(cmd: crate::DiscussCmd, client: &BuzzClient) -> Result<(), CliError> {
    match cmd {
        crate::DiscussCmd::Thread { issue_id, channel } => {
            cmd_discuss_thread(client, issue_id, channel).await
        }
    }
}

/// Execute the `discuss thread` subcommand.
async fn cmd_discuss_thread(
    client: &BuzzClient,
    issue_id: String,
    channel: Option<String>,
) -> Result<(), CliError> {
    // Resolve channel ID
    let channel_id = match channel {
        Some(ch) => parse_uuid(&ch)?.to_string(),
        None => std::env::var("BUZZ_CHANNEL_ID").map_err(|_| {
            CliError::Usage("channel ID not provided; use --channel or set BUZZ_CHANNEL_ID".into())
        })?,
    };

    // Validate channel UUID
    let _ = Uuid::parse_str(&channel_id)
        .map_err(|_| CliError::Other(format!("invalid channel ID: {}", channel_id)))?;

    // Get ISM client from environment
    let ism_base_url = std::env::var("BUZZ_ISM_BASE_URL")
        .map_err(|_| CliError::Usage("BUZZ_ISM_BASE_URL environment variable not set".into()))?;

    let ism_email = std::env::var("BUZZ_ISM_SERVICE_EMAIL").map_err(|_| {
        CliError::Usage("BUZZ_ISM_SERVICE_EMAIL environment variable not set".into())
    })?;

    let ism_password = std::env::var("BUZZ_ISM_SERVICE_PASSWORD").map_err(|_| {
        CliError::Usage("BUZZ_ISM_SERVICE_PASSWORD environment variable not set".into())
    })?;

    let ism_client = IsmClient::new(ism_base_url, ism_email, ism_password)
        .map_err(|e| CliError::Other(format!("failed to create ISM client: {}", e)))?;

    // Fetch issue details
    let issue = ism_client
        .get_issue(&issue_id)
        .await
        .map_err(|e| CliError::Other(format!("failed to fetch issue {}: {}", issue_id, e)))?;

    let comments = ism_client.get_comments(&issue_id).await.unwrap_or_default();

    let attachments = ism_client
        .get_attachments(&issue_id)
        .await
        .unwrap_or_default();

    // Format summary
    let content = format_ism_summary(&issue_id, &issue, &comments, &attachments);

    // Build message event with ism_ticket tag
    let channel_uuid = Uuid::parse_str(&channel_id)
        .map_err(|_| CliError::Other(format!("invalid channel ID: {}", channel_id)))?;

    let builder = buzz_sdk::build_message(
        channel_uuid,
        &content,
        None,  // thread_ref
        &[],   // mention_refs
        false, // broadcast
        &[],   // media_tags
        &[],   // emoji_tags
    )
    .map_err(|e| CliError::Other(format!("failed to build message: {}", e)))?;

    // Add ism_ticket tag
    let ism_tag = Tag::parse(["ism_ticket", &issue_id])
        .map_err(|e| CliError::Other(format!("failed to create tag: {}", e)))?;
    let builder = builder.tag(ism_tag);

    // Apply git provenance if set
    let builder = super::with_git_provenance(builder)?;

    // Sign and publish
    let event = client
        .sign_event(builder)
        .map_err(|e| CliError::Other(format!("failed to sign event: {}", e)))?;

    let raw = client
        .submit_event(event)
        .await
        .map_err(|e| CliError::Other(format!("failed to submit event: {}", e)))?;

    let output = crate::client::normalize_write_response(&raw);
    println!("{output}");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_ism_summary_basic() {
        let issue = buzz_ism::Issue {
            id: "ISM-123".to_string(),
            title: "Test Issue".to_string(),
            description: Some("A test issue description".to_string()),
            status: "Open".to_string(),
            priority: Some("High".to_string()),
            created_at: "2026-09-22T10:00:00Z".to_string(),
            updated_at: "2026-09-22T11:00:00Z".to_string(),
            assignee: Some("alice@example.com".to_string()),
            labels: vec!["bug".to_string()],
            extra: Default::default(),
        };

        let summary = format_ism_summary("ISM-123", &issue, &[], &[]);
        assert!(summary.contains("ISM-123"));
        assert!(summary.contains("Test Issue"));
        assert!(summary.contains("Open"));
        assert!(summary.contains("High"));
        assert!(summary.contains("alice@example.com"));
        assert!(summary.contains("bug"));
    }

    #[test]
    fn test_format_ism_summary_with_comments() {
        let issue = buzz_ism::Issue {
            id: "ISM-123".to_string(),
            title: "Test Issue".to_string(),
            description: None,
            status: "Open".to_string(),
            priority: None,
            created_at: "2026-09-22T10:00:00Z".to_string(),
            updated_at: "2026-09-22T11:00:00Z".to_string(),
            assignee: None,
            labels: vec![],
            extra: Default::default(),
        };

        let comments = vec![buzz_ism::Comment {
            id: "C1".to_string(),
            body: "This is a comment".to_string(),
            author: "bob@example.com".to_string(),
            created_at: "2026-09-22T10:30:00Z".to_string(),
            updated_at: None,
            extra: Default::default(),
        }];

        let summary = format_ism_summary("ISM-123", &issue, &comments, &[]);
        assert!(summary.contains("Comments"));
        assert!(summary.contains("bob@example.com"));
        assert!(summary.contains("This is a comment"));
    }
}
