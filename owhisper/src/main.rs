use clap::Parser;

mod commands;

#[derive(Parser)]
#[command(name = "owhisper")]
#[command(about = "A CLI tool for audio transcription")]
struct Args {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    Serve(commands::ServeArgs),
    Readme(commands::ReadmeArgs),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.cmd {
        Commands::Serve(args) => {
            commands::handle_serve(args).await.unwrap();
        }
        Commands::Readme(args) => {
            commands::handle_readme(args).await.unwrap();
        }
    }

    Ok(())
}
