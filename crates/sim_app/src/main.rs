//! `simpylink` binary: GUI by default, CLI subcommands otherwise.
use clap::Parser;
use sim_app::cli::{run_cli, Cli, CliCommand};

fn main() {
    let args = Cli::parse();
    match args.command {
        None => exit_gui(None),
        Some(CliCommand::Gui { file }) => exit_gui(file),
        Some(other) => std::process::exit(run_cli(&other)),
    }
}

fn exit_gui(file: Option<std::path::PathBuf>) {
    if let Err(e) = sim_gui::run_gui(file) {
        eprintln!("error[E_GUI]: {e}");
        std::process::exit(1);
    }
}
