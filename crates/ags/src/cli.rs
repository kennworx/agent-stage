//! Argument parsing (argh) and dispatch for the `ags` CLI.

use std::process::ExitCode;

use argh::FromArgs;

/// agent-stage — validate and review agent-authored reasoning artifacts.
#[derive(FromArgs)]
struct Cli {
    /// print version information and exit
    #[argh(switch, short = 'V')]
    version: bool,
    #[argh(subcommand)]
    command: Option<Command>,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum Command {
    Present(PresentArgs),
    Poll(PollArgs),
    Catalog(CatalogArgs),
    Bake(BakeArgs),
    Draw(DrawArgs),
}

/// validate an artifact (Gate 1) and serve it for in-browser review; on failure
/// emit TOON errors and exit non-zero without serving
#[derive(FromArgs)]
#[argh(subcommand, name = "present")]
struct PresentArgs {
    /// validate only — do not serve or open a browser
    #[argh(switch)]
    check: bool,
    /// start a fresh review, discarding any existing feedback log
    #[argh(switch)]
    fresh: bool,
    /// port to serve on (default: a random free port)
    #[argh(option)]
    port: Option<u16>,
    /// path to the artifact markdown file
    #[argh(positional)]
    file: String,
}

/// long-poll the artifact's feedback session; print delivered items + ended status
/// as TOON
#[derive(FromArgs)]
#[argh(subcommand, name = "poll")]
struct PollArgs {
    /// path to the artifact whose feedback session to poll
    #[argh(positional)]
    file: String,
}

/// print the block catalog — the closed block vocabulary + per-type schema the agent
/// authors against (generated from the validator, so it never drifts from Gate 1)
#[derive(FromArgs)]
#[argh(subcommand, name = "catalog")]
struct CatalogArgs {}

/// validate an artifact (Gate 1), then write it out as one standalone HTML page —
/// finished markup with no script and nothing to fetch, so it opens from a file with
/// no server and no network. A baked page is read-only (no feedback loop).
#[derive(FromArgs)]
#[argh(subcommand, name = "bake")]
struct BakeArgs {
    /// write the page to this file instead of stdout
    #[argh(option)]
    out: Option<String>,
    /// path to the artifact markdown file
    #[argh(positional)]
    file: String,
}

/// Parse argv with `argh` and dispatch.
pub fn run_cli() -> ExitCode {
    let strings: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = strings.iter().map(String::as_str).collect();
    match Cli::from_args(&["ags"], &args) {
        Ok(cli) => run(&cli),
        Err(early) => render_early_exit(&early),
    }
}

/// Dispatch a successfully-parsed CLI.
fn run(cli: &Cli) -> ExitCode {
    if cli.version {
        println!("ags {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    match &cli.command {
        Some(Command::Present(a)) => crate::present::run(&a.file, a.check, a.port, a.fresh),
        Some(Command::Poll(a)) => crate::poll::run(&a.file),
        Some(Command::Catalog(_)) => crate::catalog::run(),
        Some(Command::Bake(a)) => crate::bake::run(&a.file, a.out.as_deref()),
        Some(Command::Draw(a)) => crate::draw::main(&crate::draw::Args {
            input: a.file.clone(),
            out: a.out.clone(),
            tokens: a.tokens,
            check: a.check,
        }),
        None => {
            eprintln!("usage: ags <present|poll|catalog|bake|draw> <file>  (try --help)");
            ExitCode::from(2)
        }
    }
}

/// draw one diagram on its own, outside any artifact — Mermaid source in, a
/// standalone SVG out. Literal colours by default, because an image has no page
/// behind it to resolve a theme token against.
#[derive(FromArgs)]
#[argh(subcommand, name = "draw")]
struct DrawArgs {
    /// write the drawing to this file instead of stdout
    #[argh(option, short = 'o')]
    out: Option<String>,
    /// reference theme tokens instead of writing literal colours, for an SVG that
    /// will be embedded in a page and themed by it
    #[argh(switch)]
    tokens: bool,
    /// report what the drawing gets wrong — edges through boxes, edges merged into
    /// one line, covered labels, anything off the canvas — and draw nothing. Exits
    /// non-zero when there is something to report
    #[argh(switch)]
    check: bool,
    /// diagram source file; reads standard input when absent
    #[argh(positional)]
    file: Option<String>,
}

/// Render `argh`'s early exit — help text (stdout, code 0) or a parse error
/// (stderr, code 2).
fn render_early_exit(early: &argh::EarlyExit) -> ExitCode {
    if early.status.is_ok() {
        print!("{}", early.output);
        ExitCode::SUCCESS
    } else {
        eprint!("{}", early.output);
        ExitCode::from(2)
    }
}
