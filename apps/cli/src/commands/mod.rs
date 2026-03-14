pub mod auth;
pub mod batch;
pub mod cactus_server;
pub mod debug;
pub mod desktop;
pub mod listen;
pub mod model;
pub mod transcribe;

use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    Pretty,
    Text,
    Json,
}
