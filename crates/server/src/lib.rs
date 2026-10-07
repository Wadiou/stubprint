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
mod session;
mod wire;

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;

pub use config::{Config, PrinterConfig};
pub use hub::{Hub, Notice, Stored};
pub use printer::VirtualPrinter;

/// A server that is running.
pub struct Running {
    hub: Arc<Hub>,
    http: SocketAddr,
    stop: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
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
        let _ = self.stop.send(true);
        for task in self.tasks {
            let _ = task.await;
        }
    }
}

/// Binds every address and starts serving. Fails before anything runs if an address is taken.
pub async fn serve(config: Config) -> io::Result<Running> {
    config
        .check()
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;

    let mut bound = Vec::new();
    for settings in &config.printers {
        let socket = TcpListener::bind(settings.listen).await.map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "printer {} cannot listen on {}: {error}",
                    settings.name, settings.listen
                ),
            )
        })?;
        let printer = Arc::new(VirtualPrinter::new(
            settings.name.clone(),
            settings.dialect,
            settings.profile.clone(),
            socket.local_addr()?,
        ));
        bound.push((printer, socket));
    }
    let web = TcpListener::bind(config.http).await.map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("the API cannot listen on {}: {error}", config.http),
        )
    })?;
    let http = web.local_addr()?;

    let hub = Arc::new(Hub::new(
        bound.iter().map(|(printer, _)| printer.clone()).collect(),
        config.keep,
    ));
    let (stop, stopped) = watch::channel(false);
    let mut tasks = Vec::new();
    for (printer, socket) in bound {
        tasks.push(tokio::spawn(listener::listen(
            hub.clone(),
            printer,
            socket,
            stopped.clone(),
        )));
    }
    let mut api_stop = stopped.clone();
    let app = api::router(hub.clone(), config.web.as_deref());
    tasks.push(tokio::spawn(async move {
        let _ = axum::serve(web, app)
            .with_graceful_shutdown(async move {
                let _ = api_stop.changed().await;
            })
            .await;
    }));
    Ok(Running {
        hub,
        http,
        stop,
        tasks,
    })
}

/// Serves until the process gets Ctrl+C or a termination signal.
pub async fn run(config: Config) -> io::Result<()> {
    let running = serve(config).await?;
    println!(
        "API and live events: http://{}/api/openapi.json",
        running.http_addr()
    );
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
