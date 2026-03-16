use std::path::PathBuf;

use clap::CommandFactory;

#[allow(dead_code)]
#[path = "src/cli.rs"]
mod cli;

fn main() -> std::io::Result<()> {
    let cmd = cli::Cli::command();
    let md = cli_docs::generate(&cmd);

    let frontmatter = "\
---
title: \"CLI Reference\"
section: \"CLI\"
description: \"Command-line reference for the char CLI\"
---\n\n";

    let mdx_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/content/docs/cli/index.mdx");
    std::fs::create_dir_all(mdx_path.parent().unwrap())?;
    std::fs::write(&mdx_path, format!("{frontmatter}{md}"))?;

    Ok(())
}
