mod install;
mod plist;
mod screen;
mod uninstall;

pub(crate) mod daemon;
pub(crate) mod hotkey;

use clap::Subcommand;

use crate::error::CliResult;

#[derive(Subcommand)]
pub enum Commands {
    /// Install the global shortcut daemon
    Install,
    /// Uninstall the shortcut daemon
    Uninstall,
    /// Check if the shortcut daemon is running
    Status,
}

pub async fn run(command: Option<Commands>) -> CliResult<()> {
    match command {
        Some(Commands::Install) => install::run(),
        Some(Commands::Uninstall) => uninstall::run(),
        Some(Commands::Status) => status(),
        None => screen::run(),
    }
}

fn status() -> CliResult<()> {
    let output = std::process::Command::new("launchctl")
        .args(["list", plist::LAUNCHD_LABEL])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            eprintln!("Shortcut daemon is running.");
            let stdout = String::from_utf8_lossy(&out.stdout);
            if !stdout.is_empty() {
                eprint!("{stdout}");
            }
        }
        _ => {
            eprintln!("Shortcut daemon is not running.");
            if !plist::plist_path().exists() {
                eprintln!("Run `char shortcut install` to set it up.");
            }
        }
    }

    Ok(())
}
