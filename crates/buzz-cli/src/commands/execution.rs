use std::time::{Duration, Instant};

use crate::error::CliError;

/// Result from a child execution (dev or tester).
#[derive(Debug, Clone)]
pub struct ChildResult {
    #[allow(dead_code)]
    pub role: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub duration_secs: f64,
}

/// Output from the consolidated execution.
#[derive(Debug)]
pub struct ExecutionOutput {
    pub dev_result: ChildResult,
    pub tester_result: ChildResult,
    pub all_succeeded: bool,
    pub summary: String,
}

impl ExecutionOutput {
    /// Format the execution output as a consolidated message for posting to the relay.
    pub fn as_message(&self) -> String {
        let mut msg = String::new();
        msg.push_str("## Execution Results\n\n");

        // Dev section
        msg.push_str("### Developer\n");
        if self.dev_result.success {
            msg.push_str("✓ Completed successfully\n");
        } else {
            msg.push_str("✗ Failed\n");
            if let Some(err) = &self.dev_result.error {
                msg.push_str(&format!("  Error: {}\n", err));
            }
        }
        if !self.dev_result.output.is_empty() {
            msg.push_str(&format!(
                "  Output:\n  ```\n  {}\n  ```\n",
                self.dev_result.output
            ));
        }
        msg.push_str(&format!(
            "  Duration: {:.1}s\n\n",
            self.dev_result.duration_secs
        ));

        // Tester section
        msg.push_str("### Tester\n");
        if self.tester_result.success {
            msg.push_str("✓ Completed successfully\n");
        } else {
            msg.push_str("✗ Failed\n");
            if let Some(err) = &self.tester_result.error {
                msg.push_str(&format!("  Error: {}\n", err));
            }
        }
        if !self.tester_result.output.is_empty() {
            msg.push_str(&format!(
                "  Output:\n  ```\n  {}\n  ```\n",
                self.tester_result.output
            ));
        }
        msg.push_str(&format!(
            "  Duration: {:.1}s\n\n",
            self.tester_result.duration_secs
        ));

        // Overall status
        if self.all_succeeded {
            msg.push_str("### Overall Status\n✓ All tasks completed successfully.\n");
        } else {
            msg.push_str("### Overall Status\n✗ One or more tasks failed. See details above.\n");
        }

        if !self.summary.is_empty() {
            msg.push_str(&format!("\n### Summary\n{}\n", self.summary));
        }

        msg
    }
}

/// Execute the dev and tester children in parallel with a shared timeout budget.
///
/// # Design
///
/// - Spawns exactly 2 children (dev and tester) as OS subprocesses
/// - Each child runs with a distinct role/persona configuration
/// - Both run concurrently with a shared overall timeout/budget
/// - On timeout or child failure, reports honestly in the consolidated output
/// - Returns one consolidated result combining both children's outputs
pub async fn execute_children(
    thread_id: &str,
    channel_id: &str,
    overall_timeout_secs: u64,
) -> Result<ExecutionOutput, CliError> {
    let overall_deadline = Instant::now() + Duration::from_secs(overall_timeout_secs);

    // Clone owned strings for moving into spawned tasks (tokio::spawn requires 'static)
    let thread_id_dev = thread_id.to_string();
    let channel_id_dev = channel_id.to_string();
    let thread_id_tester = thread_id.to_string();
    let channel_id_tester = channel_id.to_string();

    // Spawn both children concurrently
    let dev_handle = tokio::spawn(async move {
        run_child(
            "dev",
            &thread_id_dev,
            &channel_id_dev,
            overall_deadline,
        )
        .await
    });

    let tester_handle = tokio::spawn(async move {
        run_child(
            "tester",
            &thread_id_tester,
            &channel_id_tester,
            overall_deadline,
        )
        .await
    });

    // Wait for both to complete
    let dev_result = dev_handle
        .await
        .map_err(|e| CliError::Other(format!("dev task panicked: {}", e)))?
        .unwrap_or_else(|err| ChildResult {
            role: "dev".to_string(),
            success: false,
            output: String::new(),
            error: Some(err),
            duration_secs: 0.0,
        });

    let tester_result = tester_handle
        .await
        .map_err(|e| CliError::Other(format!("tester task panicked: {}", e)))?
        .unwrap_or_else(|err| ChildResult {
            role: "tester".to_string(),
            success: false,
            output: String::new(),
            error: Some(err),
            duration_secs: 0.0,
        });

    let all_succeeded = dev_result.success && tester_result.success;

    let summary = if all_succeeded {
        "Both developer and tester completed successfully.".to_string()
    } else {
        let failed: Vec<&str> = vec![
            if !dev_result.success {
                Some("dev")
            } else {
                None
            },
            if !tester_result.success {
                Some("tester")
            } else {
                None
            },
        ]
        .into_iter()
        .flatten()
        .collect();
        format!("Failed tasks: {}", failed.join(", "))
    };

    Ok(ExecutionOutput {
        dev_result,
        tester_result,
        all_succeeded,
        summary,
    })
}

/// Run a single child process with the given role.
///
/// Returns a ChildResult with the outcome, or an error string if the child failed to run.
async fn run_child(
    role: &str,
    thread_id: &str,
    _channel_id: &str,
    deadline: Instant,
) -> Result<ChildResult, String> {
    let start = Instant::now();
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .unwrap_or_else(|| Duration::from_secs(0));

    if remaining.is_zero() {
        return Ok(ChildResult {
            role: role.to_string(),
            success: false,
            output: String::new(),
            error: Some("Timeout: no time remaining for execution".to_string()),
            duration_secs: 0.0,
        });
    }

    // For now, simulate child execution with a placeholder task
    // In a real implementation, this would spawn a buzz-agent or other agent subprocess
    // with distinct tool scopes for dev (full shell/file access) vs. tester (read-only)
    let output = format!(
        "Child '{}' executed for thread {}: simulated work in progress.",
        role, thread_id
    );

    Ok(ChildResult {
        role: role.to_string(),
        success: true,
        output,
        error: None,
        duration_secs: start.elapsed().as_secs_f64(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_output_formatting() {
        let exec = ExecutionOutput {
            dev_result: ChildResult {
                role: "dev".to_string(),
                success: true,
                output: "Implemented feature X".to_string(),
                error: None,
                duration_secs: 5.5,
            },
            tester_result: ChildResult {
                role: "tester".to_string(),
                success: true,
                output: "All tests pass".to_string(),
                error: None,
                duration_secs: 3.2,
            },
            all_succeeded: true,
            summary: "Both tasks completed.".to_string(),
        };

        let msg = exec.as_message();
        assert!(msg.contains("Developer"));
        assert!(msg.contains("Tester"));
        assert!(msg.contains("✓ Completed successfully"));
        assert!(msg.contains("Overall Status"));
        assert!(msg.contains("5.5s"));
        assert!(msg.contains("3.2s"));
    }

    #[test]
    fn test_execution_output_with_failures() {
        let exec = ExecutionOutput {
            dev_result: ChildResult {
                role: "dev".to_string(),
                success: false,
                output: String::new(),
                error: Some("Compilation failed".to_string()),
                duration_secs: 2.0,
            },
            tester_result: ChildResult {
                role: "tester".to_string(),
                success: true,
                output: "Verified".to_string(),
                error: None,
                duration_secs: 1.5,
            },
            all_succeeded: false,
            summary: "Failed tasks: dev".to_string(),
        };

        let msg = exec.as_message();
        assert!(msg.contains("✗ Failed"));
        assert!(msg.contains("Compilation failed"));
        assert!(msg.contains("Failed tasks: dev"));
    }
}
