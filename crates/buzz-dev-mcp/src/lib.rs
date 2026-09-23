#![cfg_attr(not(windows), forbid(unsafe_code))]
#![cfg_attr(windows, deny(unsafe_code))]
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData, ServerHandler, ServiceExt,
};
use std::path::Path;
use std::sync::Arc;

mod paths;
mod read_file;
mod rg;
mod shell;
mod shim;
mod str_replace;
mod todo;
mod tree;
mod view_image;

/// Tool scope controls which tools can be executed by the dev MCP server.
/// ReadOnly restricts file modification and destructive shell commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolScope {
    /// All tools available (default, backward compatible).
    Full,
    /// Read-only: denies str_replace and a hardcoded deny-list of shell commands
    /// (docker compose, rm -rf, git push --force, git reset --hard, git clean -fdx, etc.).
    /// This is a guard against a well-behaved-but-misdirected agent, not a security boundary
    /// against a fully adversarial one — full OS-level sandboxing remains deferred.
    ReadOnly,
}

impl ToolScope {
    /// Parse from env var `BUZZ_DEV_MCP_TOOL_SCOPE` (full/read-only, case-insensitive).
    /// Defaults to Full if unset or unrecognized.
    fn from_env() -> Self {
        match std::env::var("BUZZ_DEV_MCP_TOOL_SCOPE")
            .ok()
            .as_deref()
            .map(|s| s.to_ascii_lowercase())
        {
            Some(s) if s == "read-only" || s == "readonly" => Self::ReadOnly,
            _ => Self::Full,
        }
    }

    /// Check if a command string is blocked in read-only mode.
    /// Returns Some(reason) if blocked, None if allowed.
    /// This is a pattern-match deny-list on literal strings/substrings,
    /// not a cryptographically-strong sandbox.
    fn check_blocked_command(&self, cmd: &str) -> Option<String> {
        if *self != ToolScope::ReadOnly {
            return None;
        }

        // Convert to lowercase for case-insensitive matching
        let cmd_lower = cmd.to_ascii_lowercase();

        // Docker compose: any subcommand variant
        if (cmd_lower.contains("docker compose") || cmd_lower.contains("docker-compose"))
            && (cmd_lower.contains("up")
                || cmd_lower.contains("down")
                || cmd_lower.contains("restart")
                || cmd_lower.contains("stop")
                || cmd_lower.contains("rm ")
                || (cmd_lower.contains("docker compose") && !cmd_lower.contains("ps")))
        {
            return Some("docker compose subcommands are blocked in read-only mode".to_string());
        }

        // docker volume rm
        if cmd_lower.contains("docker volume rm") {
            return Some("docker volume rm is blocked in read-only mode".to_string());
        }

        // docker system prune
        if cmd_lower.contains("docker system prune") {
            return Some("docker system prune is blocked in read-only mode".to_string());
        }

        // docker kill
        if cmd_lower.contains("docker kill") {
            return Some("docker kill is blocked in read-only mode".to_string());
        }

        // rm -rf (catch common patterns)
        if cmd_lower.contains("rm -rf") || cmd_lower.contains("rm -r") {
            return Some("rm -rf / rm -r is blocked in read-only mode".to_string());
        }

        // git push --force or --force-with-lease
        if cmd_lower.contains("git push")
            && (cmd_lower.contains("--force-with-lease") || cmd_lower.contains("--force"))
        {
            return Some("git push --force is blocked in read-only mode".to_string());
        }

        // git reset --hard
        if cmd_lower.contains("git reset") && cmd_lower.contains("--hard") {
            return Some("git reset --hard is blocked in read-only mode".to_string());
        }

        // git clean -fdx
        if cmd_lower.contains("git clean")
            && (cmd_lower.contains("-fdx") || cmd_lower.contains("-f"))
        {
            return Some("git clean -fdx is blocked in read-only mode".to_string());
        }

        // git branch -D / branch -d (deletion)
        if cmd_lower.contains("git branch -d") || cmd_lower.contains("git branch -D") {
            return Some("git branch deletion is blocked in read-only mode".to_string());
        }

        None
    }
}

#[derive(Clone)]
struct DevMcp {
    state: Arc<shell::SharedState>,
    todos: Arc<todo::TodoState>,
    tool_scope: ToolScope,
    tool_router: ToolRouter<DevMcp>,
}

#[tool_router]
impl DevMcp {
    fn new(state: Arc<shell::SharedState>) -> Self {
        let tool_scope = ToolScope::from_env();
        Self {
            state,
            todos: Arc::new(todo::TodoState::new()),
            tool_scope,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "shell",
        description = "Run a shell command (bash by default; set `BUZZ_SHELL` to use cmd, PowerShell, or another shell). Ephemeral process per call. Output tail-truncated to ~8KB for the LLM; full output (first 10MB) saved to artifact file. timeout_ms defaults to 120000 (2 min) if omitted; capped at 1,200,000 (20 min). For long-running commands (git push with hooks, cargo build, test suites), use 300000+. On PATH: rg (prefer over grep; flags: -n -i -l -g <glob> -C <n> --files), tree (flags: -d <depth>; shows line counts), and buzz (Buzz relay CLI — run buzz --help for commands)."
    )]
    async fn shell(
        &self,
        Parameters(p): Parameters<shell::ShellParams>,
        context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        // Check deny-list for read-only scope
        if let Some(reason) = self.tool_scope.check_blocked_command(&p.command) {
            return Err(ErrorData::invalid_params(reason, None));
        }
        shell::run(&self.state, p, context.ct).await
    }

    #[tool(
        name = "read_file",
        description = "Read a text file and return its contents with line numbers. Returns lines in `{number}:{content}` format. Use `offset` (0-based) and `limit` (default 2000) to window into large files. Path resolved relative to workdir (defaults to server cwd). Prefer over cat/head/tail."
    )]
    async fn read_file(
        &self,
        Parameters(p): Parameters<read_file::ReadFileParams>,
    ) -> Result<String, ErrorData> {
        read_file::run(&self.state, p)
    }

    #[tool(
        name = "view_image",
        description = "Load an image from a file path, http(s) URL, or data: URL and return it as an MCP image content block that multimodal LLMs (Anthropic, OpenAI-compatible, etc.) can see. Resizes to a longest-edge of 1568px by default (override with `max_dim`, range 64..=2048). Pass-through for already-small PNG/JPEG; transcodes oversize input to PNG (if alpha) or JPEG q85. Animated GIF/WebP rejected — provide a still frame. Hard cap 20 MiB source, ~4 MiB on the wire. Relative paths resolve under `workdir` (defaults to server cwd) and may not escape it."
    )]
    async fn view_image(
        &self,
        Parameters(p): Parameters<view_image::ViewImageParams>,
    ) -> Result<CallToolResult, ErrorData> {
        view_image::run(&self.state, p).await
    }

    #[tool(
        name = "str_replace",
        description = "Atomic find-and-replace in a file. old_str must occur exactly once unless replace_all is true, in which case all occurrences are replaced. Returns a unified diff. Path resolved relative to workdir (defaults to server cwd). Prefer over sed/awk."
    )]
    async fn str_replace(
        &self,
        Parameters(p): Parameters<str_replace::StrReplaceParams>,
    ) -> Result<String, ErrorData> {
        if self.tool_scope == ToolScope::ReadOnly {
            return Err(ErrorData::invalid_params(
                "str_replace tool is not available in read-only mode",
                None,
            ));
        }
        str_replace::run(&self.state, p)
    }

    #[tool(
        name = "todo",
        description = "Session checklist only for work that must continue across turns or survive context compaction. Do not use for work you can finish in the current turn. Omit `todos` to read; provide the full {text, done} list to replace it. Open items let the _Stop hook advise against ending."
    )]
    async fn todo(
        &self,
        Parameters(p): Parameters<todo::TodoParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.todos.handle_todo(p) {
            Ok(text) => todo::text_result(text),
            Err(e) => todo::error_result(format!("Error: {e}")),
        }
    }

    /// Hook: called by the agent before honoring end_turn. Returns
    /// non-empty objection text iff items remain open.
    #[tool(
        name = "_Stop",
        description = "Returns open todo items if any exist. Used by the agent's _Stop lifecycle hook to advise against ending with incomplete work."
    )]
    async fn stop_hook(
        &self,
        Parameters(_): Parameters<todo::HookParams>,
    ) -> Result<CallToolResult, ErrorData> {
        todo::text_result(self.todos.stop_objection())
    }

    /// Hook: called by the agent after context compaction/handoff so the
    /// todo list survives history truncation.
    #[tool(
        name = "_PostCompact",
        description = "Internal hook. Agent invokes after handoff; returns todo state for re-injection."
    )]
    async fn post_compact_hook(
        &self,
        Parameters(_): Parameters<todo::HookParams>,
    ) -> Result<CallToolResult, ErrorData> {
        todo::text_result(self.todos.post_compact())
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DevMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(rmcp::model::Implementation::new(
                "buzz-dev-mcp",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(self.state.bootstrap_instructions.clone())
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let argv0 = std::env::args().next().unwrap_or_default();
    let cmd = Path::new(&argv0)
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // Multicall dispatch — sync personalities exit before any runtime is built.
    // No tracing, no tokio, no allocations beyond argv parsing.
    match cmd.as_str() {
        "rg" => std::process::exit(rg::run(std::env::args().skip(1).collect())),
        "tree" => std::process::exit(tree::run(std::env::args().skip(1).collect())),
        "git-credential-nostr" => std::process::exit(git_credential_nostr::run()),
        "git-sign-nostr" => std::process::exit(git_sign_nostr::run()),
        _ => {}
    }

    // Async personalities and MCP server mode — build the runtime.
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async_main(cmd))
}

async fn async_main(cmd: String) -> Result<(), Box<dyn std::error::Error>> {
    // HTTPS clients invoked through this MCP process need a Rustls provider;
    // repeated installation is harmless.
    let _ = rustls::crypto::ring::default_provider().install_default();

    // buzz CLI needs tokio (async HTTP client).
    if cmd == "buzz" {
        std::process::exit(buzz_cli::run_from_args(std::env::args()).await);
    }

    // MCP server mode — safe to init tracing now.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let cwd = std::env::current_dir()?;
    let shim = shim::Shim::install()?;
    let state = Arc::new(shell::SharedState::new(cwd, shim)?);

    let service = DevMcp::new(state).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

/// Suppress the console window that Windows otherwise allocates for every
/// console-subsystem child process spawned from a non-console parent.
/// No-op on non-Windows platforms.
pub(crate) fn configure_no_window(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// Suppress the console window for async (`tokio::process::Command`) spawns.
/// Equivalent to `configure_no_window` but accepts a tokio command.
/// No-op on non-Windows platforms.
pub(crate) fn configure_no_window_async(cmd: &mut tokio::process::Command) {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_scope_default_is_full() {
        // Unset env var should default to Full
        std::env::remove_var("BUZZ_DEV_MCP_TOOL_SCOPE");
        assert_eq!(ToolScope::from_env(), ToolScope::Full);
    }

    #[test]
    fn test_tool_scope_read_only_variants() {
        // "read-only" should parse as ReadOnly
        std::env::set_var("BUZZ_DEV_MCP_TOOL_SCOPE", "read-only");
        assert_eq!(ToolScope::from_env(), ToolScope::ReadOnly);

        // "readonly" (no dash) should also parse
        std::env::set_var("BUZZ_DEV_MCP_TOOL_SCOPE", "readonly");
        assert_eq!(ToolScope::from_env(), ToolScope::ReadOnly);

        // Case insensitive
        std::env::set_var("BUZZ_DEV_MCP_TOOL_SCOPE", "READ-ONLY");
        assert_eq!(ToolScope::from_env(), ToolScope::ReadOnly);

        std::env::remove_var("BUZZ_DEV_MCP_TOOL_SCOPE");
    }

    #[test]
    fn test_tool_scope_invalid_defaults_to_full() {
        std::env::set_var("BUZZ_DEV_MCP_TOOL_SCOPE", "invalid");
        assert_eq!(ToolScope::from_env(), ToolScope::Full);

        std::env::set_var("BUZZ_DEV_MCP_TOOL_SCOPE", "full");
        assert_eq!(ToolScope::from_env(), ToolScope::Full);

        std::env::remove_var("BUZZ_DEV_MCP_TOOL_SCOPE");
    }

    #[test]
    fn test_blocked_commands_docker_compose() {
        let scope = ToolScope::Full;
        // Full scope should not block anything
        assert_eq!(scope.check_blocked_command("docker compose down"), None);

        let scope = ToolScope::ReadOnly;
        // ReadOnly should block docker compose down
        assert!(scope.check_blocked_command("docker compose down").is_some());
        // ReadOnly should block docker-compose (with dash)
        assert!(scope.check_blocked_command("docker-compose up").is_some());
        // ReadOnly should block docker compose restart
        assert!(scope
            .check_blocked_command("docker compose restart")
            .is_some());
        // ReadOnly should block docker compose rm
        assert!(scope.check_blocked_command("docker compose rm").is_some());
        // ReadOnly should block docker compose stop
        assert!(scope.check_blocked_command("docker compose stop").is_some());
    }

    #[test]
    fn test_blocked_commands_rm_rf() {
        let scope = ToolScope::ReadOnly;
        // Should block rm -rf
        assert!(scope.check_blocked_command("rm -rf /tmp/foo").is_some());
        // Should block rm -r (without f)
        assert!(scope.check_blocked_command("rm -r /tmp/foo").is_some());
    }

    #[test]
    fn test_blocked_commands_git_push_force() {
        let scope = ToolScope::ReadOnly;
        // Should block git push --force
        assert!(scope.check_blocked_command("git push --force").is_some());
        // Should block git push --force-with-lease
        assert!(scope
            .check_blocked_command("git push --force-with-lease origin main")
            .is_some());
    }

    #[test]
    fn test_blocked_commands_git_reset_hard() {
        let scope = ToolScope::ReadOnly;
        // Should block git reset --hard
        assert!(scope
            .check_blocked_command("git reset --hard HEAD")
            .is_some());
    }

    #[test]
    fn test_blocked_commands_git_clean() {
        let scope = ToolScope::ReadOnly;
        // Should block git clean -fdx
        assert!(scope.check_blocked_command("git clean -fdx").is_some());
        // Should block git clean -f
        assert!(scope.check_blocked_command("git clean -f").is_some());
    }

    #[test]
    fn test_blocked_commands_git_branch_delete() {
        let scope = ToolScope::ReadOnly;
        // Should block git branch -D
        assert!(scope
            .check_blocked_command("git branch -D feature")
            .is_some());
        // Should block git branch -d
        assert!(scope
            .check_blocked_command("git branch -d feature")
            .is_some());
    }

    #[test]
    fn test_blocked_commands_docker_volume_rm() {
        let scope = ToolScope::ReadOnly;
        // Should block docker volume rm
        assert!(scope
            .check_blocked_command("docker volume rm my-volume")
            .is_some());
    }

    #[test]
    fn test_blocked_commands_docker_system_prune() {
        let scope = ToolScope::ReadOnly;
        // Should block docker system prune
        assert!(scope.check_blocked_command("docker system prune").is_some());
    }

    #[test]
    fn test_blocked_commands_docker_kill() {
        let scope = ToolScope::ReadOnly;
        // Should block docker kill
        assert!(scope
            .check_blocked_command("docker kill container-name")
            .is_some());
    }

    #[test]
    fn test_allowed_commands_in_read_only() {
        let scope = ToolScope::ReadOnly;
        // Safe commands should be allowed
        assert_eq!(scope.check_blocked_command("echo hello"), None);
        assert_eq!(scope.check_blocked_command("cargo test"), None);
        assert_eq!(scope.check_blocked_command("just ci"), None);
        assert_eq!(scope.check_blocked_command("git status"), None);
        assert_eq!(scope.check_blocked_command("docker ps"), None);
        assert_eq!(scope.check_blocked_command("ls -la"), None);
    }

    #[test]
    fn test_full_scope_allows_everything() {
        let scope = ToolScope::Full;
        // Full scope should allow everything (doesn't block)
        assert_eq!(scope.check_blocked_command("docker compose down"), None);
        assert_eq!(scope.check_blocked_command("rm -rf /tmp/foo"), None);
        assert_eq!(scope.check_blocked_command("git push --force"), None);
        assert_eq!(scope.check_blocked_command("git reset --hard"), None);
    }

    #[test]
    fn test_case_insensitive_blocking() {
        let scope = ToolScope::ReadOnly;
        // Should match case-insensitively
        assert!(scope.check_blocked_command("DOCKER COMPOSE DOWN").is_some());
        assert!(scope.check_blocked_command("Docker Compose Up").is_some());
        assert!(scope.check_blocked_command("RM -RF /tmp").is_some());
        assert!(scope.check_blocked_command("GIT PUSH --FORCE").is_some());
    }
}
