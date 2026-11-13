mod cli;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{CommandFactory, Parser};
use stubprint_server::Config;

/// The colours of the help and of the errors of the command line. They are left out when the output
/// is not a terminal, and when NO_COLOR is set.
const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default())
    .error(AnsiColor::Red.on_default().effects(Effects::BOLD))
    .valid(AnsiColor::Cyan.on_default())
    .invalid(AnsiColor::Yellow.on_default());

/// A thermal printer simulator. Programs print to its TCP ports, and you see what came out in the
/// web app, in the API or in the commands below.
///
/// With no command it starts the server, from the options below. With a command it asks a server
/// that is running: say where it is with --server, or it is on 127.0.0.1:8080.
#[derive(Parser)]
#[command(name = "stubprint", version, styles = STYLES)]
struct Options {
    #[command(flatten)]
    remote: cli::Remote,

    #[command(subcommand)]
    command: Option<cli::Command>,

    /// Keep nothing: use no database. The printers, the receipts and the connections are gone when
    /// the server stops.
    #[arg(long)]
    ephemeral: bool,

    /// Delete what the database has, and start again from the printers given, or the default
    /// one.
    #[arg(long, conflicts_with = "ephemeral")]
    reset: bool,

    /// Where the API and the web app listen. It is saved in the database and used from then
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
    ///   max_connections how many programs it takes at once. The next one is closed at once.
    ///                   0 is no limit of its own, up to 64 [default: 0]
    ///   idle_timeout    how many seconds a connection can send nothing before it is closed.
    ///                   0 never [default: 0]
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

impl Options {
    /// Whether an option that starts or describes the server was given.
    fn is_for_the_server(&self) -> bool {
        self.ephemeral
            || self.reset
            || self.openapi
            || self.http.is_some()
            || self.ready_file.is_some()
            || !self.printer.is_empty()
    }
}

fn main() -> ExitCode {
    let mut options = Options::parse();
    if let Some(command) = options.command.take() {
        if options.is_for_the_server() {
            Options::command()
                .error(
                    clap::error::ErrorKind::ArgumentConflict,
                    "a command asks a running server, and the options that start a server do not go with it",
                )
                .exit();
        }
        return cli::run(&options.remote, command);
    }
    match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime.block_on(serve(options)),
        Err(error) => {
            eprintln!("stubprint: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn serve(options: Options) -> ExitCode {
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
