//! `ags` — the CLI for agent-stage. Thin root: argument parsing and dispatch live
//! in `cli`; each subcommand's logic lives in its own module.

mod bake;
mod catalog;
mod cli;
mod draw;
mod poll;
mod present;
mod serve;
mod store;

fn main() -> std::process::ExitCode {
    cli::run_cli()
}
