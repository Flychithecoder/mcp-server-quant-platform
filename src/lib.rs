//! Zed extension: launch the Quant Platform MCP server over stdio.
//!
//! Zed starts the command this returns and speaks MCP to it over the child's
//! stdin/stdout. That is why the server needs `--transport stdio`: its default
//! is a long-running HTTP listener, which Zed would launch and then wait on
//! forever with nothing on the pipe.
//!
//! Nothing is downloaded here. Zed's publishing rules require language servers
//! to be fetched rather than vendored, but this server is the user's own
//! installed Python package -- pulling a second copy from a release would give
//! them a *different* build than the one their dashboard and licence are bound
//! to, which is worse than asking them to point at the one they have.

use std::fs;

use zed_extension_api::{
    self as zed, serde_json, ContextServerId, Command, Project, Result,
};

/// Settings a user puts in `~/.config/zed/settings.json` under
/// `context_servers."quant-platform".settings`.
#[derive(Debug, serde::Deserialize)]
struct QuantSettings {
    /// Interpreter that has `jesse_live` installed. A bare `python3` is not a
    /// safe default: on Windows it resolves to the WindowsApps stub, and a
    /// venv's `Scripts/` contains no `python3.exe` at all, so the extension
    /// would launch an interpreter without the package and report an import
    /// error that names the wrong cause.
    python_path: String,

    /// Jesse project directory. The server reads `.env` from its working
    /// directory when no password is supplied.
    #[serde(default)]
    project_path: Option<String>,

    /// Dashboard URL the MCP tools call.
    #[serde(default = "default_api_url")]
    api_url: String,

    /// Dashboard password. Prefer leaving this unset and letting the server
    /// read the project's `.env` -- Zed's settings file is not a secret store,
    /// and a password here sits in plaintext in a file users routinely paste
    /// into issues.
    #[serde(default)]
    password: Option<String>,

    /// "research" (backtest/optimise only), "live" (trading tools only), or
    /// "both". Defaults to research: an editor agent gets the read-only
    /// surface unless someone deliberately widens it.
    #[serde(default = "default_surface")]
    surface: String,
}

fn default_api_url() -> String {
    "http://localhost:9000".to_string()
}

fn default_surface() -> String {
    "research".to_string()
}

struct QuantPlatformExtension;

impl zed::Extension for QuantPlatformExtension {
    fn new() -> Self {
        Self
    }

    fn context_server_command(
        &mut self,
        _context_server_id: &ContextServerId,
        project: &Project,
    ) -> Result<Command> {
        let settings = zed::settings::ContextServerSettings::for_project(
            "quant-platform",
            project,
        )?;

        let Some(settings) = settings.settings else {
            return Err(CONFIG_HELP.to_string());
        };
        let settings: QuantSettings = serde_json::from_value(settings)
            .map_err(|e| format!("{e}\n\n{CONFIG_HELP}"))?;

        // Fail here, with the path in the message, rather than letting Zed
        // report an opaque "server exited" when the interpreter does not exist.
        // The two ways this goes wrong on Windows -- the WindowsApps stub and a
        // venv without python3.exe -- both produce a path that looks right.
        if !fs::metadata(&settings.python_path).is_ok_and(|m| m.is_file()) {
            return Err(format!(
                "python_path does not point at a file: {}\n\
                 Use the interpreter that has jesse_live installed, e.g.\n  \
                 macOS/Linux: /path/to/.venv/bin/python\n  \
                 Windows:     C:\\\\path\\\\to\\\\.venv\\\\Scripts\\\\python.exe",
                settings.python_path
            ));
        }

        let mut args = vec![
            "-m".to_string(),
            "jesse_live.mcp.server".to_string(),
            "--transport".to_string(),
            "stdio".to_string(),
            "--api-url".to_string(),
            settings.api_url.clone(),
        ];

        match settings.surface.as_str() {
            "research" => args.push("--research-only".to_string()),
            "live" => args.push("--live-only".to_string()),
            "both" => {}
            other => {
                return Err(format!(
                    "surface is {other:?}; expected \"research\", \"live\", or \"both\""
                ))
            }
        }

        // The password goes in the environment, never in `args`. Argv is
        // visible in `ps` to every user on the machine; the environment of
        // another user's process is not.
        let mut env = Vec::new();
        if let Some(password) = settings.password.filter(|p| !p.is_empty()) {
            env.push(("JESSE_PASSWORD".to_string(), password));
        }
        if let Some(project_path) = settings.project_path.filter(|p| !p.is_empty()) {
            env.push(("JESSE_PROJECT_PATH".to_string(), project_path));
        }

        Ok(Command {
            command: settings.python_path,
            args,
            env,
        })
    }
}

const CONFIG_HELP: &str = r#"Quant Platform MCP needs to know which Python has jesse_live installed.

Add to Zed settings.json:

  "context_servers": {
    "quant-platform": {
      "source": "extension",
      "settings": {
        "python_path": "/absolute/path/to/.venv/bin/python",
        "project_path": "/absolute/path/to/your/jesse/project",
        "api_url": "http://localhost:9000",
        "surface": "research"
      }
    }
  }

surface: "research" = backtest and optimise only (default)
         "live"     = live-trading tools only
         "both"     = everything

Leave "password" unset so the server reads it from the project's .env rather
than storing it in plaintext in settings.json."#;

zed::register_extension!(QuantPlatformExtension);
