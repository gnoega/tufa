use clap::Parser;

use crate::{
    app::App,
    cli::{Cli, handle_command},
};
mod app;
mod cli;
mod clipboard;
mod migration;
mod notification;
mod screen;
mod totp;
mod totp_uri;
mod ui;
mod vault;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Some(cmd) => handle_command(cmd).map_err(|e| {
            eprintln!("{e}");
            std::process::exit(1)
        }),
        None => {
            ratatui::run(|terminal| App::new().run(terminal))?;
            Ok(())
        }
    }
}
