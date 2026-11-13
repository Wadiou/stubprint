//! The commands: each asks a running server through its REST API and says what it answered. They
//! hold no printer of their own, so what the page shows and what a command shows are the same.

mod client;
mod show;
mod update;

use std::io::{BufRead, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand, ValueEnum};
use serde_json::{json, Map, Value};

use client::{Client, Failure};

/// The exit code of `wait` when nothing was printed in time.
const NOTHING_CAME: u8 = 3;

/// Where the server is, and how to say what it answered. They go with every command.
#[derive(Args, Debug)]
pub struct Remote {
    /// The address of the running server that a command asks. Without it, the address this
    /// computer saved for its server is used, and else http://127.0.0.1:8080.
    #[arg(long, global = true, env = "STUBPRINT_URL", value_name = "URL")]
    pub url: Option<String>,

    /// Print what the server answered as JSON, which is what its API says, and not as text.
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Send a file, or a ready made sample, to a printer
    Print(PrintArgs),
    /// Wait for the next receipt and show it
    Wait(WaitArgs),
    /// Read bytes as a printer would and list their commands, keeping nothing
    Decode(DecodeArgs),
    /// The printers: list, add, change, set what they report, remove
    #[command(subcommand)]
    Printer(PrinterCommand),
    /// What was printed: list, show in any form, remove
    #[command(subcommand)]
    Receipt(ReceiptCommand),
    /// The ready made receipts that real printing libraries wrote
    #[command(subcommand)]
    Sample(SampleCommand),
    /// The printer models that a printer can be set to
    #[command(subcommand)]
    Model(ModelCommand),
    /// The programs that connected to the printers
    #[command(subcommand)]
    Connection(ConnectionCommand),
    /// Say which server it is and where it listens
    Info,
    /// Follow what happens as it happens, one event a line
    Events,
    /// Install the latest release over this program, or the version of `--to`
    Update(update::UpdateArgs),
}

#[derive(Args, Debug)]
pub struct PrintArgs {
    /// The file to send, or `-` for what is typed in. Not with `--sample`.
    #[arg(required_unless_present = "sample")]
    file: Option<PathBuf>,
    /// Print this sample instead, such as `grocery`. See `stubprint sample list`.
    #[arg(long, conflicts_with = "file")]
    sample: Option<String>,
    /// The printer to send it to. The first one when left out.
    #[arg(long, short)]
    printer: Option<String>,
    /// For a sample: the library that wrote the variant, such as `python-escpos`.
    #[arg(long, requires = "sample")]
    library: Option<String>,
    /// For a sample: the command language of the variant. The printer's own when left out.
    #[arg(long, requires = "sample")]
    dialect: Option<String>,
}

#[derive(Args, Debug)]
pub struct WaitArgs {
    /// Only a receipt of this printer.
    #[arg(long, short)]
    printer: Option<String>,
    /// Wait for a receipt after the one with this number. Without it, for one after now.
    #[arg(long)]
    after: Option<u64>,
    /// How many seconds to wait, up to 120. The exit code is 3 when nothing came.
    #[arg(long, default_value_t = 10)]
    timeout: u64,
    /// Print only the number of the receipt, for a script.
    #[arg(long, conflicts_with = "json")]
    id: bool,
}

#[derive(Args, Debug)]
pub struct DecodeArgs {
    /// The file to read, or `-` for what is typed in.
    file: PathBuf,
    /// The command language: `escpos`, `starline` or `starprnt`. ESC/POS when left out.
    #[arg(long)]
    dialect: Option<String>,
    /// The printer model to read it as, as `stubprint model list` lists them.
    #[arg(long)]
    profile: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum PrinterCommand {
    /// List the printers and how each one is
    List,
    /// Show one printer with everything it is set to
    Show { name: String },
    /// Add a printer to the running server, from settings written `key=value`
    Add {
        /// Such as `name=front listen=127.0.0.1:9100 dialect=escpos print_speed=150`
        settings: Vec<String>,
    },
    /// Change what a printer is set to, from settings written `key=value`
    Set {
        name: String,
        /// Such as `profile=TM-T88IV printer_buffer=2048 idle_timeout=30`
        #[arg(required = true)]
        settings: Vec<String>,
    },
    /// Make a printer report something: open its cover, run out of paper, go offline
    Status {
        name: String,
        /// Such as `cover_open=true paper=near_end online=false fault=cutter`. `fault=null` clears it
        #[arg(required = true)]
        changes: Vec<String>,
    },
    /// Remove a printer. The receipts it printed stay
    Remove { name: String },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Form {
    /// The paper as text
    Text,
    /// The receipt as data
    Json,
    /// The bytes the printer was sent
    Bytes,
    /// A picture of the paper, as PNG
    Picture,
    /// Every command with the place of its bytes
    Trace,
}

#[derive(Subcommand, Debug)]
pub enum ReceiptCommand {
    /// List the receipts, the latest last
    List {
        /// Only the receipts of this printer.
        #[arg(long, short)]
        printer: Option<String>,
        /// Only the receipts after the one with this number.
        #[arg(long)]
        after: Option<u64>,
        /// At most this many, the latest ones.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Show one receipt
    Show {
        id: u64,
        /// How to show it.
        #[arg(long, short, value_enum, default_value = "text")]
        form: Form,
        /// Write it to this file. Bytes and a picture need it, or a pipe.
        #[arg(long, short)]
        output: Option<PathBuf>,
        /// For a picture: how many times larger, from 1 to 4.
        #[arg(long)]
        scale: Option<u32>,
        /// Show it as it was after the first this many commands.
        #[arg(long)]
        upto: Option<usize>,
    },
    /// Remove one receipt
    Remove { id: u64 },
    /// Remove all the receipts, or those of one printer
    Clear {
        #[arg(long, short)]
        printer: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum SampleCommand {
    /// List the samples
    List,
    /// Show one sample with the libraries that wrote it
    Show { id: String },
}

#[derive(Subcommand, Debug)]
pub enum ModelCommand {
    /// List the printer models
    List,
}

#[derive(Subcommand, Debug)]
pub enum ConnectionCommand {
    /// List the connections, the latest last
    List {
        /// Only the connections to this printer.
        #[arg(long, short)]
        printer: Option<String>,
    },
    /// Show one connection with the pieces its bytes came in
    Show { id: u64 },
}

/// Where the server is: the address that was given, with `http://` put in front when it is left
/// out, else the one this computer saved for its server, else the default one.
fn server_url(given: Option<&str>) -> String {
    if let Some(text) = given {
        return if text.contains("://") {
            text.to_string()
        } else {
            format!("http://{text}")
        };
    }
    let address = stubprint_server::saved_address()
        .filter(|address| address.port() != 0)
        .unwrap_or(stubprint_server::DEFAULT_HTTP);
    // A server that listens on every address is reached on the local one.
    let ip = match address.ip() {
        std::net::IpAddr::V4(ip) if ip.is_unspecified() => std::net::Ipv4Addr::LOCALHOST.into(),
        std::net::IpAddr::V6(ip) if ip.is_unspecified() => std::net::Ipv6Addr::LOCALHOST.into(),
        ip => ip,
    };
    format!("http://{}", std::net::SocketAddr::new(ip, address.port()))
}

/// Runs a command, and says what went wrong when something did.
pub fn run(remote: &Remote, command: Command) -> ExitCode {
    let client = Client::new(&server_url(remote.url.as_deref()));
    match execute(&client, remote.json, command) {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("stubprint: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Writes a line to the output. A reader that has gone, as `head` does, is not an error.
fn say(text: impl std::fmt::Display) {
    let _ = writeln!(std::io::stdout(), "{text}");
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

/// Settings written `key=value` as a JSON object. With `typed`, `true`, `false` and `null` keep
/// their type, which what a printer reports needs. Otherwise every value is text, which the server
/// reads as the number or the name it is.
fn pairs(args: &[String], typed: bool) -> Result<Value, Failure> {
    let mut object = Map::new();
    for arg in args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("`{arg}` should be written key=value"))?;
        let value = match value {
            "true" if typed => Value::Bool(true),
            "false" if typed => Value::Bool(false),
            "null" if typed => Value::Null,
            _ => Value::String(value.to_string()),
        };
        object.insert(key.to_string(), value);
    }
    Ok(Value::Object(object))
}

/// The bytes of a file, or of what is typed in for `-`.
fn read_input(file: &PathBuf) -> Result<Vec<u8>, Failure> {
    if file.as_os_str() == "-" {
        let mut bytes = Vec::new();
        std::io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|error| format!("cannot read what is typed in: {error}"))?;
        Ok(bytes)
    } else {
        std::fs::read(file).map_err(|error| format!("cannot read {}: {error}", file.display()))
    }
}

/// Binary output goes to a file, or to a pipe, and never to a terminal.
fn give_bytes(bytes: &[u8], output: Option<&PathBuf>) -> Result<(), Failure> {
    match output {
        Some(path) => {
            std::fs::write(path, bytes)
                .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
            say(format!("wrote {} bytes to {}", bytes.len(), path.display()));
        }
        None if std::io::stdout().is_terminal() => {
            return Err(
                "it is binary: say where to write it with --output FILE, or pipe it".into(),
            );
        }
        None => {
            let _ = std::io::stdout().write_all(bytes);
        }
    }
    Ok(())
}

fn execute(client: &Client, json: bool, command: Command) -> Result<u8, Failure> {
    match command {
        Command::Print(args) => print(client, json, args)?,
        Command::Wait(args) => return wait(client, json, &args),
        Command::Decode(args) => decode(client, json, &args)?,
        Command::Printer(command) => printer(client, json, command)?,
        Command::Receipt(command) => receipt(client, json, command)?,
        Command::Sample(command) => sample(client, json, command)?,
        Command::Model(ModelCommand::List) => models(client, json)?,
        Command::Connection(command) => connection(client, json, command)?,
        Command::Info => info(client, json)?,
        Command::Events => events(client, json)?,
        Command::Update(args) => update::update(&args)?,
    }
    Ok(0)
}

fn printer_query(printer: Option<String>) -> Vec<(&'static str, String)> {
    printer.map(|name| ("printer", name)).into_iter().collect()
}

fn print(client: &Client, json: bool, args: PrintArgs) -> Result<(), Failure> {
    let mut query = printer_query(args.printer);
    let answer = match (&args.sample, &args.file) {
        (Some(sample), _) => {
            query.extend(args.library.map(|library| ("library", library)));
            query.extend(args.dialect.map(|dialect| ("dialect", dialect)));
            client.post_empty(&format!("/api/samples/{sample}/print"), &query)?
        }
        (None, Some(file)) => client.post_bytes("/api/print", &query, &read_input(file)?)?,
        (None, None) => return Err("say what to print: a file, or --sample".into()),
    };
    let printed = answer.json()?;
    if json {
        say(pretty(&printed));
        return Ok(());
    }
    let receipts: Vec<String> = printed["receipts"]
        .as_array()
        .map(|ids| ids.iter().map(|id| format!("#{id}")).collect())
        .unwrap_or_default();
    let made = if receipts.is_empty() {
        "no receipt".to_string()
    } else {
        format!("receipt {}", receipts.join(", "))
    };
    say(format!(
        "printed to {}: {made}, connection #{}",
        show::text(&printed, "printer"),
        show::number(&printed, "connection")
    ));
    Ok(())
}

fn wait(client: &Client, json: bool, args: &WaitArgs) -> Result<u8, Failure> {
    let mut query = printer_query(args.printer.clone());
    query.extend(args.after.map(|after| ("after", after.to_string())));
    query.push(("timeout", args.timeout.to_string()));
    let answer = client.get("/api/receipts/next", &query)?;
    if answer.status == 204 {
        eprintln!("stubprint: no receipt came in {}s", args.timeout);
        return Ok(NOTHING_CAME);
    }
    let receipt = answer.json()?;
    let id = show::number(&receipt, "id");
    if args.id {
        say(id);
    } else if json {
        say(pretty(&receipt));
    } else {
        say(client
            .get(&format!("/api/receipts/{id}/text"), &[])?
            .text()
            .trim_end());
    }
    Ok(0)
}

fn decode(client: &Client, json: bool, args: &DecodeArgs) -> Result<(), Failure> {
    let mut query = Vec::new();
    query.extend(args.dialect.clone().map(|dialect| ("dialect", dialect)));
    query.extend(args.profile.clone().map(|profile| ("profile", profile)));
    let decoded = client
        .post_bytes("/api/decode", &query, &read_input(&args.file)?)?
        .json()?;
    if json {
        say(pretty(&decoded));
        return Ok(());
    }
    for item in decoded["trace"].as_array().into_iter().flatten() {
        say(format!(
            "{:>4}  {:>6}-{:<6}  {}",
            show::number(item, "index") + 1,
            show::number(item, "start"),
            show::number(item, "end"),
            show::event_line(&item["event"])
        ));
    }
    say(format!(
        "read as {} on {}: {} receipts",
        show::text(&decoded, "dialect"),
        show::text(&decoded, "profile"),
        show::number(&decoded, "receipts")
    ));
    let replies = show::text(&decoded, "replies");
    if !replies.is_empty() {
        say(format!("the printer answered {replies}"));
    }
    Ok(())
}

fn printer(client: &Client, json: bool, command: PrinterCommand) -> Result<(), Failure> {
    match command {
        PrinterCommand::List => {
            let printers = client.get("/api/printers", &[])?.json()?;
            if json {
                say(pretty(&printers));
                return Ok(());
            }
            let rows: Vec<Vec<String>> = printers
                .as_array()
                .into_iter()
                .flatten()
                .map(|printer| {
                    vec![
                        show::text(printer, "name").to_string(),
                        show::text(printer, "dialect").to_string(),
                        show::text(printer, "profile").to_string(),
                        show::text(printer, "listen").to_string(),
                        show::state(&printer["status"]),
                        show::number(printer, "connections").to_string(),
                        show::number(&printer["usage"], "receipts").to_string(),
                    ]
                })
                .collect();
            say(show::table(
                &[
                    "NAME", "LANGUAGE", "MODEL", "ADDRESS", "STATE", "OPEN", "PRINTED",
                ],
                &rows,
            ));
        }
        PrinterCommand::Show { name } => {
            let printer = client.get(&format!("/api/printers/{name}"), &[])?.json()?;
            if json {
                say(pretty(&printer));
                return Ok(());
            }
            show_printer(&printer);
        }
        PrinterCommand::Add { settings } => {
            let added = client
                .post_json("/api/printers", &pairs(&settings, false)?)?
                .json()?;
            if json {
                say(pretty(&added));
            } else {
                say(format!(
                    "added {} on {}, reading {} as {}",
                    show::text(&added, "name"),
                    show::text(&added, "listen"),
                    show::text(&added, "dialect"),
                    show::text(&added, "profile")
                ));
            }
        }
        PrinterCommand::Set { name, settings } => {
            let changed = client
                .patch_json(&format!("/api/printers/{name}"), &pairs(&settings, false)?)?
                .json()?;
            if json {
                say(pretty(&changed));
            } else {
                say(format!("changed {}", show::text(&changed, "name")));
            }
        }
        PrinterCommand::Status { name, changes } => {
            let changed = client
                .post_json(&format!("/api/printers/{name}"), &pairs(&changes, true)?)?
                .json()?;
            if json {
                say(pretty(&changed));
            } else {
                say(format!(
                    "{}: {}",
                    show::text(&changed, "name"),
                    show::state(&changed["status"])
                ));
            }
        }
        PrinterCommand::Remove { name } => {
            client.delete(&format!("/api/printers/{name}"), &[])?;
            if !json {
                say(format!("removed {name}"));
            }
        }
    }
    Ok(())
}

/// A printer as lines: what it is, how it is, and each setting.
fn show_printer(printer: &Value) {
    let line = |label: &str, value: String| say(format!("{label:<20}{value}"));
    line("name", show::text(printer, "name").to_string());
    line("language", show::text(printer, "dialect").to_string());
    line("model", show::text(printer, "profile").to_string());
    line("address", show::text(printer, "listen").to_string());
    line("state", show::state(&printer["status"]));
    line(
        "open connections",
        show::number(printer, "connections").to_string(),
    );
    line("held bytes", show::number(printer, "held").to_string());
    line(
        "buffered bytes",
        show::number(printer, "buffered").to_string(),
    );
    line(
        "paper left",
        if show::number(printer, "paper_roll") == 0 {
            "a roll that never runs out".to_string()
        } else {
            format!("{:.0} mm", printer["paper_left"].as_f64().unwrap_or(0.0))
        },
    );
    for key in [
        "printer_buffer",
        "network_buffer",
        "paper_roll",
        "print_speed",
        "max_connections",
        "idle_timeout",
    ] {
        line(key, show::number(printer, key).to_string());
    }
    let usage = &printer["usage"];
    line(
        "receipts printed",
        show::number(usage, "receipts").to_string(),
    );
    line("cuts", show::number(usage, "cuts").to_string());
    line("line feeds", show::number(usage, "line_feeds").to_string());
    line(
        "paper used",
        format!("{:.0} mm", usage["paper_used"].as_f64().unwrap_or(0.0)),
    );
}

fn receipt(client: &Client, json: bool, command: ReceiptCommand) -> Result<(), Failure> {
    match command {
        ReceiptCommand::List {
            printer,
            after,
            limit,
        } => {
            let mut query = printer_query(printer);
            query.extend(after.map(|after| ("after", after.to_string())));
            query.extend(limit.map(|limit| ("limit", limit.to_string())));
            let receipts = client.get("/api/receipts", &query)?.json()?;
            if json {
                say(pretty(&receipts));
                return Ok(());
            }
            let rows: Vec<Vec<String>> = receipts
                .as_array()
                .into_iter()
                .flatten()
                .map(|receipt| {
                    let preview: Vec<&str> = receipt["preview"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect();
                    vec![
                        format!("#{}", show::number(receipt, "id")),
                        show::text(receipt, "printer").to_string(),
                        show::text(receipt, "state").to_string(),
                        show::clock(show::number(receipt, "at")),
                        show::number(receipt, "bytes").to_string(),
                        show::shorten(&preview.join(" / "), 50),
                    ]
                })
                .collect();
            say(show::table(
                &["ID", "PRINTER", "STATE", "TIME (UTC)", "BYTES", "TEXT"],
                &rows,
            ));
        }
        ReceiptCommand::Show {
            id,
            form,
            output,
            scale,
            upto,
        } => {
            let form = if json { Form::Json } else { form };
            let mut query: Vec<(&str, String)> = Vec::new();
            query.extend(upto.map(|upto| ("upto", upto.to_string())));
            let base = format!("/api/receipts/{id}");
            match form {
                Form::Text => say(client
                    .get(&format!("{base}/text"), &query)?
                    .text()
                    .trim_end()),
                Form::Json => say(pretty(&client.get(&base, &[])?.json()?)),
                Form::Bytes => give_bytes(
                    &client.get(&format!("{base}/bytes"), &query)?.body,
                    output.as_ref(),
                )?,
                Form::Picture => {
                    query.extend(scale.map(|scale| ("scale", scale.to_string())));
                    give_bytes(
                        &client.get(&format!("{base}/picture"), &query)?.body,
                        output.as_ref(),
                    )?;
                }
                Form::Trace => {
                    let trace = client.get(&format!("{base}/trace"), &[])?.json()?;
                    for item in trace.as_array().into_iter().flatten() {
                        say(format!(
                            "{:>4}  {:>6}-{:<6}  {}",
                            show::number(item, "index") + 1,
                            show::number(item, "start"),
                            show::number(item, "end"),
                            show::event_line(&item["event"])
                        ));
                    }
                }
            }
        }
        ReceiptCommand::Remove { id } => {
            client.delete(&format!("/api/receipts/{id}"), &[])?;
            if !json {
                say(format!("removed receipt #{id}"));
            }
        }
        ReceiptCommand::Clear { printer } => {
            let removed = client
                .delete("/api/receipts", &printer_query(printer))?
                .json()?;
            if json {
                say(pretty(&removed));
            } else {
                say(format!(
                    "removed {} receipts",
                    show::number(&removed, "removed")
                ));
            }
        }
    }
    Ok(())
}

fn sample(client: &Client, json: bool, command: SampleCommand) -> Result<(), Failure> {
    match command {
        SampleCommand::List => {
            let samples = client.get("/api/samples", &[])?.json()?;
            if json {
                say(pretty(&samples));
                return Ok(());
            }
            let rows: Vec<Vec<String>> = samples
                .as_array()
                .into_iter()
                .flatten()
                .map(|sample| {
                    let libraries: Vec<&str> = sample["variants"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|variant| show::text(variant, "library"))
                        .collect();
                    vec![
                        show::text(sample, "id").to_string(),
                        show::text(sample, "title").to_string(),
                        show::text(sample, "profile").to_string(),
                        libraries.join(", "),
                    ]
                })
                .collect();
            say(show::table(
                &["ID", "TITLE", "MADE FOR", "WRITTEN BY"],
                &rows,
            ));
        }
        SampleCommand::Show { id } => {
            let sample = client.get(&format!("/api/samples/{id}"), &[])?.json()?;
            if json {
                say(pretty(&sample));
                return Ok(());
            }
            say(show::text(&sample, "title"));
            say(show::text(&sample, "about"));
            say(format!("made for {}", show::text(&sample, "profile")));
            for variant in sample["variants"].as_array().into_iter().flatten() {
                say(format!(
                    "  {} {}, {}, {} bytes",
                    show::text(variant, "library"),
                    show::text(variant, "version"),
                    show::text(variant, "dialect"),
                    show::number(variant, "bytes")
                ));
            }
        }
    }
    Ok(())
}

fn models(client: &Client, json: bool) -> Result<(), Failure> {
    let models = client.get("/api/profiles", &[])?.json()?;
    if json {
        say(pretty(&models));
        return Ok(());
    }
    let rows: Vec<Vec<String>> = models
        .as_array()
        .into_iter()
        .flatten()
        .map(|model| {
            vec![
                show::text(model, "name").to_string(),
                show::text(model, "vendor").to_string(),
                model["width_mm"]
                    .as_f64()
                    .map_or(String::new(), |mm| format!("{mm:.0} mm")),
                model["columns"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_u64)
                    .map(|columns| columns.to_string())
                    .collect::<Vec<_>>()
                    .join("/"),
            ]
        })
        .collect();
    say(show::table(&["NAME", "MAKER", "PAPER", "COLUMNS"], &rows));
    Ok(())
}

fn connection(client: &Client, json: bool, command: ConnectionCommand) -> Result<(), Failure> {
    match command {
        ConnectionCommand::List { printer } => {
            let connections = client
                .get("/api/connections", &printer_query(printer))?
                .json()?;
            if json {
                say(pretty(&connections));
                return Ok(());
            }
            let rows: Vec<Vec<String>> = connections
                .as_array()
                .into_iter()
                .flatten()
                .map(|connection| {
                    let receipts: Vec<String> = connection["receipts"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|id| format!("#{id}"))
                        .collect();
                    vec![
                        format!("#{}", show::number(connection, "id")),
                        show::text(connection, "printer").to_string(),
                        show::text(connection, "peer").to_string(),
                        show::clock(show::number(connection, "opened_at")),
                        if connection["closed_at"].is_null() {
                            "open".to_string()
                        } else {
                            "closed".to_string()
                        },
                        show::number(connection, "bytes_in").to_string(),
                        receipts.join(" "),
                    ]
                })
                .collect();
            say(show::table(
                &[
                    "ID",
                    "PRINTER",
                    "FROM",
                    "OPENED (UTC)",
                    "STATE",
                    "BYTES IN",
                    "RECEIPTS",
                ],
                &rows,
            ));
        }
        ConnectionCommand::Show { id } => {
            let connection = client.get(&format!("/api/connections/{id}"), &[])?.json()?;
            say(pretty(&connection));
        }
    }
    Ok(())
}

fn info(client: &Client, json: bool) -> Result<(), Failure> {
    let server = client.get("/api/server", &[])?.json()?;
    if json {
        say(pretty(&server));
        return Ok(());
    }
    say(format!(
        "stubprint {} listening on {}",
        show::text(&server, "version"),
        show::text(&server, "http")
    ));
    if let Some(next) = server["next_http"].as_str() {
        say(format!("it listens on {next} after the next start"));
    }
    say(if server["kept"] == true {
        "its state is kept in a database"
    } else {
        "nothing is kept: it is all gone when the server stops"
    });
    Ok(())
}

/// Follows the live events: a line for each, with its name and what it carries.
fn events(client: &Client, json: bool) -> Result<(), Failure> {
    let stream = client.stream("/api/events")?;
    let mut name = String::new();
    for line in stream.lines() {
        let line = line.map_err(|error| format!("the stream of events broke: {error}"))?;
        if let Some(event) = line.strip_prefix("event:") {
            name = event.trim().to_string();
        } else if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim();
            if json {
                let value: Value = serde_json::from_str(data).unwrap_or(Value::Null);
                say(json!({ "event": name, "data": value }));
            } else {
                say(format!("{name} {data}"));
            }
        }
    }
    Err("the server closed the stream of events".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_written_key_value_are_text_unless_the_type_matters() {
        let args = ["name=front".to_string(), "print_speed=150".to_string()];
        assert_eq!(
            pairs(&args, false).unwrap(),
            json!({"name": "front", "print_speed": "150"})
        );
        let status = [
            "cover_open=true".to_string(),
            "fault=null".to_string(),
            "paper=end".to_string(),
        ];
        assert_eq!(
            pairs(&status, true).unwrap(),
            json!({"cover_open": true, "fault": null, "paper": "end"})
        );
        // A name that looks like a number is still a name.
        assert_eq!(
            pairs(&["name=123".to_string()], true).unwrap(),
            json!({"name": "123"})
        );
    }

    #[test]
    fn a_setting_without_an_equals_sign_says_how_it_is_written() {
        let error = pairs(&["front".to_string()], false).unwrap_err();
        assert!(error.contains("key=value"), "{error}");
    }
}
