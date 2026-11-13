//! The TCP side: programs connect to a port and send the bytes they would send to a printer.

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::sync::watch;
use tokio::time::{sleep_until, timeout, Instant};

use crate::hub::Hub;
use crate::printer::VirtualPrinter;
use crate::session::Session;

/// How many bytes one connection can send before it is closed, so a program that never stops
/// cannot fill the memory.
const MAX_CONNECTION_BYTES: usize = 256 * 1024 * 1024;
/// How often a connection that waits for room in the printer's buffer looks at whether there is.
const ROOM_RECHECK: Duration = Duration::from_millis(10);
/// How long an answer can wait for a program that does not read.
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// Opens the port. The network keeps `network_buffer` bytes for what arrives before it is read, or
/// what the system chooses when that is 0. It is set before the port opens, so the first thing a
/// program is told about how much it may send already says so.
pub(crate) fn open(listen: SocketAddr, network_buffer: usize) -> io::Result<TcpListener> {
    let socket = if listen.is_ipv4() {
        TcpSocket::new_v4()?
    } else {
        TcpSocket::new_v6()?
    };
    #[cfg(unix)]
    socket.set_reuseaddr(true)?;
    if network_buffer > 0 {
        socket.set_recv_buffer_size(buffer_size(network_buffer))?;
    }
    socket.bind(listen)?;
    socket.listen(1024)
}

/// The size to ask the system for, which does not fit in what it takes for a huge number.
fn buffer_size(bytes: usize) -> u32 {
    u32::try_from(bytes).unwrap_or(u32::MAX)
}

/// Sets how much the network keeps for what this program sends. The system may change it.
fn limit_network_buffer(stream: &TcpStream, network_buffer: usize) {
    if network_buffer > 0 {
        let _ = socket2::SockRef::from(stream)
            .set_recv_buffer_size(network_buffer.min(u32::MAX as usize));
    }
}

/// Waits until the printer is told to stop, which may have happened before the call.
async fn stopped(mut stop: watch::Receiver<bool>) {
    let _ = stop.wait_for(|stopped| *stopped).await;
}

/// Takes the connections of one printer until told to stop.
pub async fn listen(hub: Arc<Hub>, printer: Arc<VirtualPrinter>, listener: TcpListener) {
    let stop = printer.stopped();
    loop {
        tokio::select! {
            _ = stopped(stop.clone()) => return,
            accepted = listener.accept() => match accepted {
                Ok((stream, peer)) => {
                    // All its places are taken: it takes the connection and closes it at once,
                    // as a printer that is busy does, and the program can try again later.
                    if !printer.take_socket() {
                        drop(stream);
                        continue;
                    }
                    limit_network_buffer(&stream, printer.settings().network_buffer);
                    let session =
                        Session::open(hub.clone(), printer.clone(), peer.to_string()).paced();
                    let stop = stop.clone();
                    let status = printer.watch_status();
                    let place = printer.clone();
                    tokio::spawn(async move {
                        serve(stream, session, status, stop).await;
                        place.free_socket();
                    });
                }
                // Too many open files and the like: wait a little instead of spinning.
                Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
            },
        }
    }
}

/// Reads one connection until it closes, and answers what the printer would answer.
async fn serve(
    mut stream: TcpStream,
    mut session: Session,
    mut status: watch::Receiver<stubprint_core::PrinterStatus>,
    stop: watch::Receiver<bool>,
) {
    let mut buffer = vec![0u8; 16 * 1024];
    let mut total = 0usize;
    // When the program last sent something, for the printer that tires of one that sends nothing.
    let mut last = Instant::now();
    loop {
        // A printer that holds what it receives and has no room left does not read, so the program
        // that sends has to wait, as it would for a real printer.
        let room = session
            .room()
            .map_or(buffer.len(), |room| room.min(buffer.len()));
        // A program that waits for the printer to print is not idle, and the printer is not
        // waiting for it.
        if room == 0 {
            last = Instant::now();
        }
        let tired_at = session.idle_timeout().map(|idle| last + idle);
        let replies = tokio::select! {
            _ = stopped(stop.clone()) => break,
            // It sent nothing for too long: the printer closes the connection. What it sent is
            // printed when the session ends below.
            () = sleep_until(tired_at.unwrap_or(last)), if tired_at.is_some() => break,
            // With no room the printer reads nothing until the printing frees some, so what a
            // program sends waits in the network's buffer, and the program waits when it is full.
            () = tokio::time::sleep(ROOM_RECHECK), if room == 0 => Vec::new(),
            changed = status.changed() => {
                if changed.is_err() {
                    break;
                }
                let now = *status.borrow_and_update();
                session.set_status(now)
            }
            read = stream.read(&mut buffer[..room]), if room > 0 => match read {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    last = Instant::now();
                    total += count;
                    if total > MAX_CONNECTION_BYTES {
                        break;
                    }
                    session.feed(&buffer[..count])
                }
            },
        };
        if !replies.is_empty()
            && !matches!(
                timeout(WRITE_TIMEOUT, stream.write_all(&replies)).await,
                Ok(Ok(()))
            )
        {
            break;
        }
    }
    // The session prints what was not cut when it is dropped here.
    let replies = session.finish();
    if !replies.is_empty() {
        let _ = timeout(WRITE_TIMEOUT, stream.write_all(&replies)).await;
    }
}
