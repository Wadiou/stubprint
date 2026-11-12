//! The TCP side: programs connect to a port and send the bytes they would send to a printer.

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::sync::{watch, Semaphore};
use tokio::time::timeout;

use crate::hub::Hub;
use crate::printer::VirtualPrinter;
use crate::session::Session;

/// How many programs can be connected to one printer. More are turned away.
const MAX_CONNECTIONS: usize = 64;
/// How many bytes one connection can send before it is closed, so a program that never stops
/// cannot fill the memory.
const MAX_CONNECTION_BYTES: usize = 256 * 1024 * 1024;
/// How long an answer can wait for a program that does not read.
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// Opens the port. The network keeps `receive_buffer` bytes for what arrives before it is read, or
/// what the system chooses when that is 0. It is set before the port opens, so the first thing a
/// program is told about how much it may send already says so.
pub(crate) fn open(listen: SocketAddr, receive_buffer: usize) -> io::Result<TcpListener> {
    let socket = if listen.is_ipv4() {
        TcpSocket::new_v4()?
    } else {
        TcpSocket::new_v6()?
    };
    #[cfg(unix)]
    socket.set_reuseaddr(true)?;
    if receive_buffer > 0 {
        socket.set_recv_buffer_size(buffer_size(receive_buffer))?;
    }
    socket.bind(listen)?;
    socket.listen(1024)
}

/// The size to ask the system for, which does not fit in what it takes for a huge number.
fn buffer_size(bytes: usize) -> u32 {
    u32::try_from(bytes).unwrap_or(u32::MAX)
}

/// Sets how much the network keeps for what this program sends. The system may change it.
fn limit_receive_buffer(stream: &TcpStream, receive_buffer: usize) {
    if receive_buffer > 0 {
        let _ = socket2::SockRef::from(stream)
            .set_recv_buffer_size(receive_buffer.min(u32::MAX as usize));
    }
}

/// Waits until the printer is told to stop, which may have happened before the call.
async fn stopped(mut stop: watch::Receiver<bool>) {
    let _ = stop.wait_for(|stopped| *stopped).await;
}

/// Takes the connections of one printer until told to stop.
pub async fn listen(hub: Arc<Hub>, printer: Arc<VirtualPrinter>, listener: TcpListener) {
    let stop = printer.stopped();
    let room = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    loop {
        tokio::select! {
            _ = stopped(stop.clone()) => return,
            accepted = listener.accept() => match accepted {
                Ok((stream, peer)) => {
                    let Ok(permit) = room.clone().try_acquire_owned() else {
                        continue;
                    };
                    limit_receive_buffer(&stream, printer.receive_buffer());
                    let session = Session::open(hub.clone(), printer.clone(), peer.to_string());
                    let stop = stop.clone();
                    let status = printer.watch_status();
                    tokio::spawn(async move {
                        serve(stream, session, status, stop).await;
                        drop(permit);
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
    loop {
        // A printer that holds what it receives and has no room left does not read, so the program
        // that sends has to wait, as it would for a real printer.
        let room = session
            .room()
            .map_or(buffer.len(), |room| room.min(buffer.len()));
        let replies = tokio::select! {
            _ = stopped(stop.clone()) => break,
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
