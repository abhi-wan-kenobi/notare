use std::path::PathBuf;

use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ToolError(String);

fn char_exe() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("char"))
}

async fn run_char(args: &[&str]) -> Result<String, ToolError> {
    let output = tokio::process::Command::new(char_exe())
        .args(args)
        .output()
        .await
        .map_err(|e| ToolError(format!("failed to run char: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(ToolError(format!(
            "char exited with {}: {}",
            output.status,
            if stderr.is_empty() { &stdout } else { &stderr }
        )));
    }

    Ok(stdout)
}

// --- ListHumans ---

#[derive(Deserialize, JsonSchema)]
pub struct ListHumansArgs {}

#[derive(Serialize, Deserialize)]
pub struct ListHumans;

impl Tool for ListHumans {
    const NAME: &'static str = "list_humans";
    type Error = ToolError;
    type Args = ListHumansArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "list_humans".to_string(),
            description:
                "List all humans (contacts). Returns tab-separated lines: id, name, email."
                    .to_string(),
            parameters: serde_json::to_value(schema_for!(ListHumansArgs)).unwrap(),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        run_char(&["humans", "list"]).await
    }
}

// --- ShowHuman ---

#[derive(Deserialize, JsonSchema)]
pub struct ShowHumanArgs {
    /// The ID of the human to look up
    id: String,
}

#[derive(Serialize, Deserialize)]
pub struct ShowHuman;

impl Tool for ShowHuman {
    const NAME: &'static str = "show_human";
    type Error = ToolError;
    type Args = ShowHumanArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "show_human".to_string(),
            description: "Show details for a human by ID, including recent events.".to_string(),
            parameters: serde_json::to_value(schema_for!(ShowHumanArgs)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        run_char(&["humans", "show", "--id", &args.id]).await
    }
}

// --- AddHuman ---

#[derive(Deserialize, JsonSchema)]
pub struct AddHumanArgs {
    /// Full name of the human
    name: String,
    /// Email address (optional)
    email: Option<String>,
    /// Organization ID (optional)
    org: Option<String>,
    /// Job title (optional)
    title: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct AddHuman;

impl Tool for AddHuman {
    const NAME: &'static str = "add_human";
    type Error = ToolError;
    type Args = AddHumanArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "add_human".to_string(),
            description: "Add a new human (contact). Returns the new human's ID.".to_string(),
            parameters: serde_json::to_value(schema_for!(AddHumanArgs)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut cmd_args = vec!["humans", "add", &args.name];
        let email;
        if let Some(ref e) = args.email {
            email = e.clone();
            cmd_args.extend(["--email", &email]);
        }
        let org;
        if let Some(ref o) = args.org {
            org = o.clone();
            cmd_args.extend(["--org", &org]);
        }
        let title;
        if let Some(ref t) = args.title {
            title = t.clone();
            cmd_args.extend(["--title", &title]);
        }
        run_char(&cmd_args).await
    }
}
