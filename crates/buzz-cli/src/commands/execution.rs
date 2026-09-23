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

/// Default persona file path for a role, relative to the repo root. Overridable
/// via `BUZZ_DEV_PERSONA_PATH` / `BUZZ_TESTER_PERSONA_PATH` for deployments that
/// don't ship `examples/` alongside the binary.
fn persona_path_for_role(role: &str) -> String {
    let (env_var, default_path) = match role {
        "dev" => (
            "BUZZ_DEV_PERSONA_PATH",
            "examples/meadow-core/agents/dev.persona.md",
        ),
        _ => (
            "BUZZ_TESTER_PERSONA_PATH",
            "examples/meadow-core/agents/tester.persona.md",
        ),
    };
    std::env::var(env_var).unwrap_or_else(|_| default_path.to_string())
}

/// Builds the task prompt handed to a child via `buzz-acp run-task --prompt`.
///
/// The persona (system prompt) tells the agent *how* to behave; this text tells
/// it *what* to do right now — point it at the confirmed decision thread via the
/// `buzz` CLI (which the agent has shell access to via `buzz-dev-mcp`) so it pulls
/// full context itself rather than us pre-summarizing it here.
fn task_prompt_for_role(role: &str, channel_id: &str, thread_id: &str) -> String {
    let fetch_cmd =
        format!("buzz --format compact messages thread --channel {channel_id} --event {thread_id}");
    match role {
        "dev" => format!(
            "A decision was confirmed in this Buzz thread. Run `{fetch_cmd}` to read the \
             full discussion and the confirmed scope/acceptance criteria. Implement the \
             confirmed decision: read the existing codebase, write the code, commit, and \
             open a pull request. Do not merge or deploy. Post progress updates to the \
             thread as you go (channel {channel_id}, thread {thread_id})."
        ),
        _ => format!(
            "A decision was confirmed and implemented in this Buzz thread. Run `{fetch_cmd}` \
             to read the full discussion, the confirmed acceptance criteria, and the \
             developer's progress updates (including any PR link). Verify the implementation: \
             run the real test suite and check each acceptance criterion against actual \
             output. Do not modify any code. Post your pass/fail report to the thread \
             (channel {channel_id}, thread {thread_id})."
        ),
    }
}

/// Run a single child by spawning `buzz-acp run-task` — a real one-shot agent
/// execution (see `crates/buzz-acp`'s `run-task` subcommand), not a simulated
/// or placeholder subprocess.
///
/// # Design (Phase 4, increment 3)
///
/// - Spawns real `buzz-acp run-task`, which spawns a real LLM-backed agent,
///   runs one real ACP turn, and exits 0/1/2 based on the real outcome.
/// - Dev role: `--tool-scope full`. Tester role: `--tool-scope read-only`.
///   NOTE: `run-task`'s tool-scope flag is currently a documented placeholder
///   (`buzz-dev-mcp` has no tool-gating mechanism yet, tracked under Phase 3.5)
///   — both roles currently run with the same (bypass) permission mode. This
///   is a real, known gap, not hidden: don't rely on this for isolation yet.
/// - ISM credentials (`BUZZ_ISM_*`) are never forwarded to either child.
/// - One overall shared deadline; a child that can't start before the deadline
///   is reported as timed out rather than spawned.
async fn run_child(
    role: &str,
    thread_id: &str,
    channel_id: &str,
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

    let tool_scope = match role {
        "dev" => "full",
        _ => "read-only",
    };

    let persona_path = persona_path_for_role(role);
    let system_prompt = std::fs::read_to_string(&persona_path)
        .map_err(|e| format!("failed to read persona file {}: {}", persona_path, e))?;
    let task_prompt = task_prompt_for_role(role, channel_id, thread_id);

    let mut agent_cmd = tokio::process::Command::new("buzz-acp");
    agent_cmd
        .arg("run-task")
        .arg("--prompt")
        .arg(&task_prompt)
        .arg("--system-prompt")
        .arg(&system_prompt)
        .arg("--tool-scope")
        .arg(tool_scope)
        .arg("--timeout-secs")
        .arg(remaining.as_secs().to_string())
        // ISM credentials are intentionally NOT set here — Supervisor-only access.
        // BUZZ_PRIVATE_KEY (required by `run-task`) is inherited from this
        // process's own environment, same as buzz-cli's own signing key.
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = agent_cmd
        .spawn()
        .map_err(|e| format!("failed to spawn buzz-acp run-task: {}", e))?;

    // Take the pipes before waiting so a timeout can still kill the process
    // without losing the `Child` handle to `wait_with_output()`.
    let mut stdout_pipe = child.stdout.take().ok_or("failed to open stdout pipe")?;
    let mut stderr_pipe = child.stderr.take().ok_or("failed to open stderr pipe")?;
    let stdout_task = tokio::spawn(async move {
        let mut buf = String::new();
        let _ = tokio::io::AsyncReadExt::read_to_string(&mut stdout_pipe, &mut buf).await;
        buf
    });
    let stderr_task = tokio::spawn(async move {
        let mut buf = String::new();
        let _ = tokio::io::AsyncReadExt::read_to_string(&mut stderr_pipe, &mut buf).await;
        buf
    });

    match tokio::time::timeout(remaining, child.wait()).await {
        Ok(Ok(status)) => {
            let stdout = stdout_task.await.unwrap_or_default();
            let stderr = stderr_task.await.unwrap_or_default();
            let success = status.success();
            Ok(ChildResult {
                role: role.to_string(),
                success,
                output: if !stdout.is_empty() {
                    stdout
                } else {
                    stderr.clone()
                },
                error: if success {
                    None
                } else if !stderr.is_empty() {
                    Some(stderr)
                } else {
                    Some(format!(
                        "run-task exited with code {}",
                        status.code().unwrap_or(-1)
                    ))
                },
                duration_secs: start.elapsed().as_secs_f64(),
            })
        }
        Ok(Err(e)) => {
            stdout_task.abort();
            stderr_task.abort();
            Ok(ChildResult {
                role: role.to_string(),
                success: false,
                output: String::new(),
                error: Some(format!("failed to wait for run-task: {}", e)),
                duration_secs: start.elapsed().as_secs_f64(),
            })
        }
        Err(_timeout) => {
            let _ = child.kill().await;
            stdout_task.abort();
            stderr_task.abort();
            Ok(ChildResult {
                role: role.to_string(),
                success: false,
                output: String::new(),
                error: Some(format!(
                    "run-task timed out after {:.1}s",
                    remaining.as_secs_f64()
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
