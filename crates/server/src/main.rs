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
    /// Keep nothing: use no database. The printers, the receipts and the connections are gone when
    /// the server stops.
    #[arg(long)]
    ephemeral: bool,

    /// Delete what the database has, and start again from the printers given, or the default
    /// one.
    #[arg(long, conflicts_with = "ephemeral")]
    reset: bool,

    /// Where the REST API and the web page listen. It is saved in the database and used from then
    /// on. It has no password, so keep it on this machine unless the network can be trusted.
    /// [default: 127.0.0.1:8080]
    #[arg(long)]
    http: Option<SocketAddr>,

    /// A file to write the addresses into once everything listens, as JSON with the process, the
    /// address of the API and the printers. It is removed when the server stops. With port 0 for
    /// the API or a printer, this is how a program finds out which port it got.
    #[arg(long)]
    ready_file: Option<PathBuf>,

    /// Print the description of the API as an OpenAPI document, and stop.
    #[arg(long, conflicts_with_all = ["ephemeral", "reset", "http", "ready_file", "printer"])]
    openapi: bool,

    /// A printer, as settings written `key=value` and separated by commas. Say it again for more
    /// printers.
    ///
    /// It is added when the database has no printer of that name, and the database wins for one it
    /// has. Without it there is one ESC/POS printer on 127.0.0.1:9100.
    ///
    /// Settings, all of them optional:
    ///   name            what the API calls it: letters, digits, - and _ [default: printer]
    ///   listen          the address of its TCP port, where port 0 picks a free one
    ///                   [default: 127.0.0.1:9100]
    ///   dialect         the command language it reads: escpos, starline or starprnt
    ///                   [default: escpos]
    ///   profile         the printer model, as GET /api/profiles lists them [default: default]
    ///   printer_buffer  how many bytes it holds while it cannot print, such as with its cover
    ///                   open. 0 holds nothing and prints whatever it receives [default: 4096]
    ///   network_buffer  how many bytes the network keeps for a program before the printer reads
    ///                   them. 0 leaves it to the system, which keeps a lot [default: 0]
    ///   paper_roll      how long its paper roll is, in millimetres. It runs down as receipts
    ///                   print, and at the end the printer stops. 0 never runs out [default: 0]
    ///   print_speed     how fast it prints, in millimetres of paper a second. A receipt keeps it
    ///                   busy while its paper comes out, and a program that sends more waits.
    ///                   0 takes no time [default: 0]
    ///
    /// For example: name=front,listen=0.0.0.0:9100,dialect=escpos,profile=TM-T88IV
    #[arg(long, verbatim_doc_comment, value_name = "SETTINGS")]
    printer: Vec<String>,
}

fn configuration(options: Options) -> Result<Config, String> {
    let database = match options.ephemeral {
        true => None,
        false => stubprint_server::default_database(),
    };
    Config::from_options(options.http, options.ready_file, &options.printer).map(|config| Config {
        database,
        reset: options.reset,
        ..config
    })
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
