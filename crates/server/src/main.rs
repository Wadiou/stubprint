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
    /// A JSON file with `http`, `keep`, `web` and `printers`. The other options are not used with it.
    #[arg(long, conflicts_with_all = ["http", "keep", "web", "printer"])]
    config: Option<PathBuf>,

    /// Where the REST API listens. It has no password, so keep it on this machine unless the
    /// network can be trusted. [default: 127.0.0.1:8080]
    #[arg(long)]
    http: Option<SocketAddr>,

    /// How many receipts to keep. [default: 500]
    #[arg(long)]
    keep: Option<usize>,

    /// A folder with the built web page to serve at the address of the API.
    #[arg(long)]
    web: Option<PathBuf>,

    /// Print the description of the API as an OpenAPI document, and stop.
    #[arg(long, conflicts_with_all = ["config", "http", "keep", "web", "printer"])]
    openapi: bool,

    /// A printer, as `name=front,listen=127.0.0.1:9100,dialect=escpos,profile=TM-T88IV`. Say it
    /// again for more printers. Without it there is one ESC/POS printer on 127.0.0.1:9100.
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
        None => Config::from_options(options.http, options.keep, options.web, &options.printer),
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
