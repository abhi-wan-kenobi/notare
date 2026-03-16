use std::path::PathBuf;

use clap::CommandFactory;

#[allow(dead_code)]
#[path = "src/cli.rs"]
mod cli;

fn main() -> std::io::Result<()> {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").ok_or(std::io::ErrorKind::NotFound)?);
    let cmd = cli::Cli::command();
    clap_mangen::generate_to(cmd, &out_dir)?;
    Ok(())
}
