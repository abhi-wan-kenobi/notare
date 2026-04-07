use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;

use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

use crate::error::Error;
use crate::events::{EventStream, ThreadEvent};
use crate::options::{ApprovalMode, ModelReasoningEffort, SandboxMode, WebSearchMode};

const INTERNAL_ORIGINATOR_ENV: &str = "CODEX_INTERNAL_ORIGINATOR_OVERRIDE";
const RUST_SDK_ORIGINATOR: &str = "codex_sdk_rs";

#[derive(Debug, Clone)]
pub(crate) struct CodexExec {
    executable_path: PathBuf,
    env_override: Option<BTreeMap<String, String>>,
    config_overrides: toml::Table,
}

#[derive(Debug, Clone)]
pub(crate) struct CodexExecArgs {
    pub input: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub thread_id: Option<String>,
    pub images: Vec<PathBuf>,
    pub model: Option<String>,
    pub sandbox_mode: Option<SandboxMode>,
    pub working_directory: Option<PathBuf>,
    pub additional_directories: Vec<PathBuf>,
    pub skip_git_repo_check: bool,
    pub output_schema_file: Option<PathBuf>,
    pub model_reasoning_effort: Option<ModelReasoningEffort>,
    pub network_access_enabled: Option<bool>,
    pub web_search_mode: Option<WebSearchMode>,
    pub approval_mode: Option<ApprovalMode>,
    pub cancellation_token: Option<CancellationToken>,
}

pub(crate) struct CodexExecRun {
    pub events: EventStream,
    pub shutdown: CancellationToken,
}

impl CodexExec {
    pub(crate) fn new(
        executable_path: Option<PathBuf>,
        env_override: Option<BTreeMap<String, String>>,
        config_overrides: toml::Table,
    ) -> Self {
        Self {
            executable_path: executable_path.unwrap_or_else(|| PathBuf::from("codex")),
            env_override,
            config_overrides,
        }
    }

    pub(crate) fn run(&self, args: CodexExecArgs) -> Result<CodexExecRun, Error> {
        if args
            .cancellation_token
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            return Err(Error::Cancelled);
        }

        let mut command_args = vec!["exec".to_string(), "--experimental-json".to_string()];

        for override_arg in serialize_config_overrides(&self.config_overrides)? {
            command_args.push("--config".to_string());
            command_args.push(override_arg);
        }

        if let Some(base_url) = &args.base_url {
            command_args.push("--config".to_string());
            command_args.push(format!("openai_base_url={}", toml_string(base_url)));
        }

        if let Some(model) = &args.model {
            command_args.push("--model".to_string());
            command_args.push(model.clone());
        }

        if let Some(sandbox_mode) = args.sandbox_mode {
            command_args.push("--sandbox".to_string());
            command_args.push(serde_variant(&sandbox_mode)?);
        }

        if let Some(working_directory) = &args.working_directory {
            command_args.push("--cd".to_string());
            command_args.push(working_directory.display().to_string());
        }

        for dir in &args.additional_directories {
            command_args.push("--add-dir".to_string());
            command_args.push(dir.display().to_string());
        }

        if args.skip_git_repo_check {
            command_args.push("--skip-git-repo-check".to_string());
        }

        if let Some(output_schema_file) = &args.output_schema_file {
            command_args.push("--output-schema".to_string());
            command_args.push(output_schema_file.display().to_string());
        }

        if let Some(reasoning_effort) = args.model_reasoning_effort {
            command_args.push("--config".to_string());
            command_args.push(format!(
                "model_reasoning_effort={}",
                toml_string(&serde_variant(&reasoning_effort)?)
            ));
        }

        if let Some(network_access_enabled) = args.network_access_enabled {
            command_args.push("--config".to_string());
            command_args.push(format!(
                "sandbox_workspace_write.network_access={network_access_enabled}"
            ));
        }

        if let Some(web_search_mode) = args.web_search_mode {
            command_args.push("--config".to_string());
            command_args.push(format!(
                "web_search={}",
                toml_string(&serde_variant(&web_search_mode)?)
            ));
        }

        if let Some(approval_mode) = args.approval_mode {
            command_args.push("--config".to_string());
            command_args.push(format!(
                "approval_policy={}",
                toml_string(&serde_variant(&approval_mode)?)
            ));
        }

        if let Some(thread_id) = &args.thread_id {
            command_args.push("resume".to_string());
            command_args.push(thread_id.clone());
        }

        for image in &args.images {
            command_args.push("--image".to_string());
            command_args.push(image.display().to_string());
        }

        let mut command = Command::new(&self.executable_path);
        command.args(command_args);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        command.kill_on_drop(true);

        if let Some(env) = &self.env_override {
            command.env_clear();
            for (key, value) in env {
                command.env(key, value);
            }
        }

        command.env(INTERNAL_ORIGINATOR_ENV, RUST_SDK_ORIGINATOR);
        if let Some(api_key) = &args.api_key {
            command.env("CODEX_API_KEY", api_key);
        }

        let mut child = command.spawn().map_err(Error::Spawn)?;
        let mut stdin = child.stdin.take().ok_or(Error::MissingStdin)?;
        let stdout = child.stdout.take().ok_or(Error::MissingStdout)?;
        let stderr = child.stderr.take();
        let prompt = args.input;
        let cancellation_token = args.cancellation_token;
        let shutdown = CancellationToken::new();
        let task_shutdown = shutdown.clone();

        let (tx, rx) = mpsc::channel(64);

        tokio::spawn(async move {
            let result = async {
                stdin
                    .write_all(prompt.as_bytes())
                    .await
                    .map_err(Error::StdinWrite)?;
                stdin.shutdown().await.map_err(Error::StdinWrite)?;

                let stderr_task = stderr.map(spawn_stderr_reader);
                let mut lines = BufReader::new(stdout).lines();
                loop {
                    let next_line = async { lines.next_line().await.map_err(Error::StdoutRead) };
                    let line = match cancellation_token.as_ref() {
                        Some(token) => tokio::select! {
                            _ = token.cancelled() => {
                                kill_child(&mut child).await?;
                                let _ = collect_stderr(stderr_task).await;
                                return Err(Error::Cancelled);
                            }
                            _ = task_shutdown.cancelled() => {
                                kill_child(&mut child).await?;
                                let _ = collect_stderr(stderr_task).await;
                                return Ok(());
                            }
                            line = next_line => line?,
                        },
                        None => tokio::select! {
                            _ = task_shutdown.cancelled() => {
                                kill_child(&mut child).await?;
                                let _ = collect_stderr(stderr_task).await;
                                return Ok(());
                            }
                            line = next_line => line?,
                        },
                    };

                    let Some(line) = line else {
                        break;
                    };

                    let event = serde_json::from_str::<ThreadEvent>(&line)?;
                    if tx.send(Ok(event)).await.is_err() {
                        kill_child(&mut child).await?;
                        let _ = collect_stderr(stderr_task).await;
                        return Ok(());
                    }
                }

                let status = child.wait().await.map_err(Error::Wait)?;
                let stderr_output = collect_stderr(stderr_task).await;
                if !status.success() {
                    let detail = if let Some(code) = status.code() {
                        format!("code {code}: {}", stderr_output.trim())
                    } else {
                        stderr_output.trim().to_string()
                    };
                    return Err(Error::ProcessFailed { detail });
                }

                Ok(())
            }
            .await;

            if let Err(error) = result {
                let _ = tx.send(Err(error)).await;
            }
        });

        Ok(CodexExecRun {
            events: Box::pin(ReceiverStream::new(rx)),
            shutdown,
        })
    }
}

async fn kill_child(child: &mut tokio::process::Child) -> Result<(), Error> {
    if child.try_wait().map_err(Error::Wait)?.is_some() {
        return Ok(());
    }

    match child.kill().await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {}
        Err(error) => return Err(Error::Kill(error)),
    }

    child.wait().await.map_err(Error::Wait)?;
    Ok(())
}

fn spawn_stderr_reader(stderr: tokio::process::ChildStderr) -> JoinHandle<String> {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buf = String::new();
        reader.read_to_string(&mut buf).await.ok();
        buf
    })
}

async fn collect_stderr(stderr_task: Option<JoinHandle<String>>) -> String {
    match stderr_task {
        Some(task) => task.await.unwrap_or_default(),
        None => String::new(),
    }
}

fn serde_variant<T: Serialize>(value: &T) -> Result<String, Error> {
    let serialized = serde_json::to_value(value)?;
    match serialized {
        serde_json::Value::String(value) => Ok(value),
        _ => Err(Error::ProcessFailed {
            detail: "enum failed to serialize to string".to_string(),
        }),
    }
}

fn serialize_config_overrides(table: &toml::Table) -> Result<Vec<String>, Error> {
    let mut overrides = Vec::new();
    flatten_config_table(table, String::new(), &mut overrides)?;
    Ok(overrides)
}

fn flatten_config_table(
    table: &toml::Table,
    prefix: String,
    overrides: &mut Vec<String>,
) -> Result<(), Error> {
    if !prefix.is_empty() && table.is_empty() {
        overrides.push(format!("{prefix}={{}}"));
        return Ok(());
    }

    for (key, value) in table {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };

        match value {
            toml::Value::Table(child) => flatten_config_table(child, path, overrides)?,
            _ => overrides.push(format!("{path}={}", toml_value(value)?)),
        }
    }
    Ok(())
}

fn toml_value(value: &toml::Value) -> Result<String, Error> {
    Ok(match value {
        toml::Value::String(value) => toml_string(value),
        toml::Value::Integer(value) => value.to_string(),
        toml::Value::Float(value) => value.to_string(),
        toml::Value::Boolean(value) => value.to_string(),
        toml::Value::Array(values) => {
            let parts = values
                .iter()
                .map(toml_value)
                .collect::<Result<Vec<_>, _>>()?;
            format!("[{}]", parts.join(", "))
        }
        toml::Value::Table(table) => {
            let parts = table
                .iter()
                .map(|(key, value)| toml_value(value).map(|value| format!("{key} = {value}")))
                .collect::<Result<Vec<_>, _>>()?;
            format!("{{{}}}", parts.join(", "))
        }
        toml::Value::Datetime(value) => toml_string(&value.to_string()),
    })
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("\"{value}\""))
}

#[cfg(test)]
mod tests {
    use super::serialize_config_overrides;

    #[test]
    fn preserves_empty_nested_table_overrides() {
        let mut nested = toml::Table::new();
        nested.insert("child".to_string(), toml::Value::Table(toml::Table::new()));

        let mut config = toml::Table::new();
        config.insert("parent".to_string(), toml::Value::Table(nested));

        assert_eq!(
            serialize_config_overrides(&config).expect("overrides"),
            vec!["parent.child={}".to_string()]
        );
    }
}
