//! A virtual receipt printer on the network.
//!
//! Each printer listens on a TCP port and reads the bytes a program sends it, in the command
//! language that port is set to. What it printed is kept as receipts, which the REST API in [`api`]
//! shows, and every change is sent as a live event. [`serve`] starts everything and [`run`] does
//! so until the process is told to stop.

pub mod api;
mod config;
mod hub;
mod listener;
mod printer;
mod samples;
mod session;
mod wire;

use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;

pub use config::{Config, PrinterConfig};
pub use hub::{Hub, Notice, Stored};
pub use printer::VirtualPrinter;

use wire::PrinterInfo;

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
    /// Where the REST API is, which differs from the configured address for port 0.
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

    /// Stops listening, closes the connections and waits until they are done.
    pub async fn stop(self) {
        self.hub.stop_printers().await;
        let _ = self.stop.send(true);
        let _ = self.api.await;
        if let Some(path) = self.ready {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Binds every address and starts serving. Fails before anything runs if an address is taken.
pub async fn serve(config: Config) -> io::Result<Running> {
    config
        .check()
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;

    let web = TcpListener::bind(config.http).await.map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("the API cannot listen on {}: {error}", config.http),
        )
    })?;
    let http = web.local_addr()?;

    let hub = Arc::new(Hub::new(config.keep));
    for settings in config.printers {
        if let Err(error) = hub.add_printer(settings).await {
            hub.stop_printers().await;
            return Err(error);
        }
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
    let app = api::router(hub.clone(), config.web.as_deref());
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
        web: config.web.is_some(),
    })
}

/// Writes the process, the address of the API and the printers as they are at the start, apart
/// from the ones added later. The file appears whole or not at all.
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

/// Serves until the process gets Ctrl+C or a termination signal.
pub async fn run(config: Config) -> io::Result<()> {
    let running = serve(config).await?;
    println!(
        "API and live events: http://{}/api/openapi.json",
        running.http_addr()
    );
    if running.web {
        println!("web page: http://{}/", running.http_addr());
    }
    for printer in running.hub().printers() {
        println!(
            "printer {} reads {} on {} as {}",
            printer.name,
            printer.dialect.name(),
            printer.listen,
            printer.profile.name
        );
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
