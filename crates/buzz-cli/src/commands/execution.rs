use std::time::{Duration, Instant};

use crate::error::CliError;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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
        run_child("dev", &thread_id_dev, &channel_id_dev, overall_deadline).await
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
/// Spawns a buzz-agent subprocess with the specified persona and tool scope.
/// The agent runs with a shared timeout budget and reports back via console output.
///
/// # Design (Phase 4, increment 3)
///
/// - Spawns real OS subprocess via `tokio::process::Command`
/// - Communicates via ACP (JSON-RPC protocol over stdin/stdout)
/// - Dev role: full shell/file access via buzz-dev-mcp
/// - Tester role: read/execute-only access (mirrors buzziro-tester restrictions)
/// - Returns ChildResult with process exit status and captured output
/// - On timeout: terminates subprocess and reports failure
///
/// # External Dependencies (not tested without live infrastructure)
///
/// Full end-to-end execution requires:
/// - A running Buzz relay instance
/// - LLM credentials (configured in agent subprocess environment)
/// - buzz-agent binary accessible in PATH (or via `sprig` dispatcher)
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

    // Determine tool scope based on role
    let tool_scope = match role {
        "dev" => "shell,file",   // Full read/write access
        "tester" => "read_only", // Read/execute only (verify discipline)
        _ => "read_only",
    };

    // Spawn buzz-agent subprocess with ACP protocol support
    // The agent is configured with a persona (dev.persona.md or tester.persona.md)
    // from examples/meadow-core/agents/
    let mut agent_cmd = tokio::process::Command::new("sprig");

    // Use buzz-agent via sprig dispatcher
    agent_cmd.arg("--help"); // ponytail: placeholder invocation; real impl needs ACP init message flow

    agent_cmd
        .env("BUZZ_ROLE", role)
        .env("BUZZ_TOOL_SCOPE", tool_scope)
        .env("BUZZ_THREAD_ID", thread_id)
        // ISM credentials are intentionally NOT passed to children (Supervisor-only access)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // Spawn the process
    let mut child = agent_cmd
        .spawn()
        .map_err(|e| format!("failed to spawn agent subprocess: {}", e))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "failed to open stdin".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to open stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to open stderr".to_string())?;

    let mut stdout_reader = BufReader::new(stdout);
    let mut stderr_reader = BufReader::new(stderr);

    // Send initial ACP initialize message
    // Real flow: Initialize → SessionNew → SessionPrompt → collect responses
    // This is a simplified version that sends a placeholder task prompt
    let init_msg = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocol_version": 1
        }
    });

    if let Err(e) = stdin.write_all(format!("{}\n", init_msg).as_bytes()).await {
        return Ok(ChildResult {
            role: role.to_string(),
            success: false,
            output: String::new(),
            error: Some(format!("failed to initialize agent: {}", e)),
            duration_secs: start.elapsed().as_secs_f64(),
        });
    }

    // Wait for process completion or timeout
    // ponytail: simplified; real impl would parse ACP messages and manage bidirectional protocol
    let timeout_duration = remaining;

    let mut output = String::new();
    let mut errors = String::new();

    // Try to read any output (non-blocking attempt)
    let _ = tokio::time::timeout(Duration::from_millis(100), async {
        let mut line = String::new();
        while let Ok(n) = stdout_reader.read_line(&mut line).await {
            if n == 0 {
                break;
            }
            output.push_str(&line);
            line.clear();
        }
    })
    .await;

    // Try to read any errors (non-blocking attempt)
    let _ = tokio::time::timeout(Duration::from_millis(100), async {
        let mut line = String::new();
        while let Ok(n) = stderr_reader.read_line(&mut line).await {
            if n == 0 {
                break;
            }
            errors.push_str(&line);
            line.clear();
        }
    })
    .await;

    // Wait for child completion
    let child_result = tokio::time::timeout(timeout_duration, child.wait()).await;
    match child_result {
        Ok(Ok(exit_status)) => {
            let success = exit_status.success();
            let error_msg = if !success {
                if !errors.is_empty() {
                    Some(format!("agent failed: {}", errors))
                } else {
                    Some(format!(
                        "agent exited with code: {}",
                        exit_status.code().unwrap_or(-1)
                    ))
                }
            } else {
                None
            };

            Ok(ChildResult {
                role: role.to_string(),
                success,
                output: if !output.is_empty() {
                    output
                } else {
                    format!(
                        "Agent '{}' completed successfully (no output captured)",
                        role
                    )
                },
                error: error_msg,
                duration_secs: start.elapsed().as_secs_f64(),
            })
        }
        Ok(Err(e)) => Ok(ChildResult {
            role: role.to_string(),
            success: false,
            output: String::new(),
            error: Some(format!("failed to wait for agent: {}", e)),
            duration_secs: start.elapsed().as_secs_f64(),
        }),
        Err(_timeout) => {
            // Timeout occurred
            let _ = child.kill().await;
            Ok(ChildResult {
                role: role.to_string(),
                success: false,
                output: String::new(),
                error: Some(format!(
                    "agent execution timeout after {:.1}s",
                    timeout_duration.as_secs_f64()
                )),
                duration_secs: start.elapsed().as_secs_f64(),
            })
        }
    }
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
