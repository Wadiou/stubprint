use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use stubprint_server::Config;

/// A virtual receipt printer. Programs print to its TCP ports, and its REST API shows what was
/// printed.
#[derive(Parser)]
#[command(name = "stubprint", version)]
struct Options {
    /// A JSON file with `http`, `keep`, `web`, `ready_file`, `state_file` and `printers`. The other options are not used with it.
    #[arg(long, conflicts_with_all = ["http", "keep", "web", "ready_file", "state", "no_state", "clear_state", "printer"])]
    config: Option<PathBuf>,

    /// Where the REST API listens. It has no password, so keep it on this machine unless the
    /// network can be trusted. [default: 127.0.0.1:8080]
    #[arg(long)]
    http: Option<SocketAddr>,

    /// How many receipts to keep. [default: 500]
    #[arg(long)]
    keep: Option<usize>,

    /// A folder with a built web page to serve at the address of the API, instead of the one in
    /// the program.
    #[arg(long)]
    web: Option<PathBuf>,

    /// A file to write the addresses into once everything listens, as JSON with the process, the
    /// address of the API and the printers. It is removed when the server stops. With port 0 for
    /// the API or a printer, this is how a program finds out which port it got.
    #[arg(long)]
    ready_file: Option<PathBuf>,

    /// Where the printers are kept between runs, each with its settings and what it reports,
    /// written after every change. [default: the state folder of the system]
    #[arg(long, conflicts_with = "no_state")]
    state: Option<PathBuf>,

    /// Keep nothing between runs.
    #[arg(long)]
    no_state: bool,

    /// Forget what was kept: start with the printers given, or the default one, and keep those.
    #[arg(long, conflicts_with = "no_state")]
    clear_state: bool,

    /// Print the description of the API as an OpenAPI document, and stop.
    #[arg(long, conflicts_with_all = ["config", "http", "keep", "web", "ready_file", "state", "no_state", "clear_state", "printer"])]
    openapi: bool,

    /// A printer, as `name=front,listen=127.0.0.1:9100,dialect=escpos,profile=TM-T88IV`. Say it
    /// again for more printers. What is written wins over what was kept from the last run, and
    /// what is left out is what was kept. Without it there is one ESC/POS printer on 127.0.0.1:9100.
    #[arg(long)]
    printer: Vec<String>,
}

fn configuration(options: Options) -> Result<Config, String> {
    match options.config {
        Some(path) => {
            let text = std::fs::read_to_string(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            Config::from_json(&text)
        }
        None => Config::from_options(
            options.http,
            options.keep,
            options.web,
            options.ready_file,
            &options.printer,
        )
        .map(|config| Config {
            state: match options.no_state {
                true => None,
                false => options.state.or_else(stubprint_server::default_state_file),
            },
            restore: config.restore && !options.clear_state,
            ..config
        }),
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let options = Options::parse();
    if options.openapi {
        match stubprint_server::api::openapi().to_pretty_json() {
            Ok(document) => println!("{document}"),
            Err(error) => {
                eprintln!("stubprint: {error}");
                return ExitCode::FAILURE;
            }
        }
        return ExitCode::SUCCESS;
    }
    let config = match configuration(options) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("stubprint: {message}");
            return ExitCode::from(2);
        }
    };
    match stubprint_server::run(config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("stubprint: {error}");
            ExitCode::FAILURE
        }
    }
}
