//! A virtual receipt printer on the network.
//!
//! Each printer listens on a TCP port and reads the bytes a program sends it, in the command
//! language that port is set to. What it printed is kept as receipts, which the REST API in [`api`]
//! shows, and every change is sent as a live event. [`serve`] starts everything and [`run`] does
//! so until the process is told to stop.

pub mod api;
mod config;
mod driver;
mod engine;
mod hub;
mod listener;
mod migrations;
mod network;
mod page;
mod pictures;
mod printer;
mod samples;
mod session;
mod settings;
mod store;
mod wire;

use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;

pub use config::{default_database, Config, PrinterConfig, DEFAULT_HTTP};
pub use hub::{Hub, Notice, Stored};
pub use printer::VirtualPrinter;
pub use settings::{Settings, SettingsChange};

use wire::PrinterInfo;

/// Whether this program was built with the web page inside it. If so, the page is served at the
/// address of the API.
pub fn has_page() -> bool {
    page::is_built()
}

/// How long the API gets to finish its answers when the server stops.
const API_GRACE: Duration = Duration::from_secs(2);

/// A server that is running.
pub struct Running {
    hub: Arc<Hub>,
    http: SocketAddr,
    stop: watch::Sender<bool>,
    api: JoinHandle<()>,
    ready: Option<PathBuf>,
    web: bool,
}

impl Running {
    /// The address the API really listens on. It differs from the configured one when the port was 0.
    pub fn http_addr(&self) -> SocketAddr {
        self.http
    }

    pub fn hub(&self) -> &Arc<Hub> {
        &self.hub
    }

    /// The address a printer really listens on.
    pub fn printer_addr(&self, name: &str) -> Option<SocketAddr> {
        self.hub.printer(name).map(|printer| printer.listen)
    }

    /// Stops the printers, closes their connections and waits for them to finish, then waits until
    /// everything is written to the database. The live event stream never ends by itself, so the
    /// API gets 2 seconds to finish and is then cut.
    pub async fn stop(mut self) {
        self.hub.stop_printers().await;
        let _ = self.stop.send(true);
        if tokio::time::timeout(API_GRACE, &mut self.api)
            .await
            .is_err()
        {
            self.api.abort();
        }
        self.hub.close_store();
        if let Some(path) = self.ready {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Starts the API and the printers. If an address is taken it fails before anything runs.
pub async fn serve(config: Config) -> io::Result<Running> {
    config
        .check()
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;

    // What the database has, unless nothing is kept (`--ephemeral`).
    let (store, loaded) = match &config.database {
        Some(path) => {
            let (store, loaded) = store::Store::open(path, config.reset)?;
            (Some(store), loaded)
        }
        None => (None, store::Loaded::default()),
    };
    // The API address: each part given, else the one in the database, else the default.
    let base = loaded.http.unwrap_or(DEFAULT_HTTP);
    let wanted = SocketAddr::new(
        config.host.unwrap_or(base.ip()),
        config.port.unwrap_or(base.port()),
    );

    let web = TcpListener::bind(wanted).await.map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("the API cannot listen on {wanted}: {error}"),
        )
    })?;
    let http = web.local_addr()?;

    let hub = Arc::new(Hub::new());
    hub.started_as(wire::ServerInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        http: http.to_string(),
        next_http: None,
        kept: config.database.is_some(),
    });
    let store::Loaded {
        printers: saved,
        receipts,
        connections,
        next_receipt,
        next_connection,
        ..
    } = loaded;
    let started = if saved.is_empty() {
        start(&hub, config.printers).await
    } else {
        let written = if config.given {
            config.printers
        } else {
            Vec::new()
        };
        restore(&hub, saved, written).await
    };
    if let Err(error) = started {
        hub.stop_printers().await;
        return Err(error);
    }
    hub.restore_history(store::Loaded {
        receipts,
        connections,
        next_receipt,
        next_connection,
        ..store::Loaded::default()
    });
    if let Some(store) = store {
        hub.keep_state_in(store, wanted);
    }
    if let Some(path) = &config.ready {
        if let Err(error) = write_ready(path, http, &hub) {
            hub.stop_printers().await;
            return Err(io::Error::new(
                error.kind(),
                format!("cannot write {}: {error}", path.display()),
            ));
        }
    }
    let (stop, mut stopped) = watch::channel(false);
    let app = api::router(hub.clone());
    let api = tokio::spawn(async move {
        let _ = axum::serve(web, app)
            .with_graceful_shutdown(async move {
                let _ = stopped.changed().await;
            })
            .await;
    });
    Ok(Running {
        hub,
        http,
        stop,
        api,
        ready: config.ready,
        web: page::is_built(),
    })
}

/// Starts every printer in the list.
async fn start(hub: &Arc<Hub>, printers: Vec<PrinterConfig>) -> io::Result<()> {
    for settings in printers {
        hub.add_printer(settings).await?;
    }
    Ok(())
}

/// Starts the printers saved in the database, each with the status it had. Then adds the printers
/// from `--printer` whose names the database does not have: the saved version of a printer always
/// wins. If a saved printer's port is taken now, it listens on a free port instead and a
/// message says so.
async fn restore(
    hub: &Arc<Hub>,
    saved: Vec<store::SavedPrinter>,
    written: Vec<PrinterConfig>,
) -> io::Result<()> {
    for printer in &saved {
        let mut settings = printer
            .config()
            .map_err(|message| io::Error::new(io::ErrorKind::InvalidData, message))?;
        let started = match hub.add_printer(settings.clone()).await {
            Err(error) if error.kind() != io::ErrorKind::InvalidInput => {
                let wanted = settings.listen;
                settings.listen = if error.kind() == io::ErrorKind::AddrNotAvailable {
                    ([127, 0, 0, 1], 0).into()
                } else {
                    SocketAddr::new(wanted.ip(), 0)
                };
                let started = hub.add_printer(settings).await?;
                eprintln!(
                    "stubprint: {} cannot listen on {wanted} any more, it listens on {}",
                    started.name, started.listen
                );
                started
            }
            other => other?,
        };
        started.restore_roll(printer.paper_left(), printer.usage());
        started.set_status(printer.status());
    }
    for settings in written {
        if saved.iter().all(|printer| printer.name() != settings.name) {
            hub.add_printer(settings).await?;
        }
    }
    Ok(())
}

/// Writes the process id, the address of the API and the printers as they are at the start
/// (printers added later are not in it). The file is written whole or not at all.
fn write_ready(path: &Path, http: SocketAddr, hub: &Hub) -> io::Result<()> {
    let printers: Vec<PrinterInfo> = hub
        .printers()
        .iter()
        .map(|printer| PrinterInfo::of(printer))
        .collect();
    let text = serde_json::to_vec_pretty(&serde_json::json!({
        "pid": std::process::id(),
        "http": http.to_string(),
        "printers": printers,
    }))?;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, text)?;
    std::fs::rename(temporary, path)
}

/// The address of the server that this computer kept in its database, which is where a command
/// finds it when it is not told. `None` when nothing was kept.
pub fn saved_address() -> Option<SocketAddr> {
    store::saved_http(&default_database()?)
}

/// Serves until the process gets Ctrl+C or a termination signal.
pub async fn run(config: Config) -> io::Result<()> {
    let running = serve(config).await?;
    {
        use anstream::println;
        use anstyle::{AnsiColor, Effects, Style};
        let ok = AnsiColor::Green.on_default().effects(Effects::BOLD);
        let name = Style::new().effects(Effects::BOLD);
        let label = AnsiColor::Cyan.on_default();
        let dim = Style::new().effects(Effects::DIMMED);
        let row = |first: bool, what: &str| {
            let what = if first { what } else { "" };
            format!("  {label}{what:<8}{label:#} ")
        };
        println!();
        println!(
            "{ok}\u{25cf}{ok:#} {name}Stubprint {}{name:#} is running",
            env!("CARGO_PKG_VERSION")
        );
        println!();
        if running.web {
            println!("{}http://{}/", row(true, "web app"), running.http_addr());
        }
        for (index, printer) in running.hub().printers().iter().enumerate() {
            println!(
                "{}{}  {dim}{}{dim:#}",
                row(index == 0, "printer"),
                printer.listen,
                printer.dialect.name()
            );
        }
        println!();
        println!("{dim}Press Ctrl+C to stop.{dim:#}");
    }
    shutdown_signal().await;
    running.stop().await;
    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut term) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
