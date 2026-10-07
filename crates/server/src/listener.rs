//! The TCP side: programs connect to a port and send the bytes they would send to a printer.

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
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

/// Takes the connections of one printer until told to stop.
pub async fn listen(
    hub: Arc<Hub>,
    printer: Arc<VirtualPrinter>,
    listener: TcpListener,
    mut stop: watch::Receiver<bool>,
) {
    let room = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    loop {
        tokio::select! {
            _ = stop.changed() => return,
            accepted = listener.accept() => match accepted {
                Ok((stream, peer)) => {
                    let Ok(permit) = room.clone().try_acquire_owned() else {
                        continue;
                    };
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
    mut stop: watch::Receiver<bool>,
) {
    let mut buffer = vec![0u8; 16 * 1024];
    let mut total = 0usize;
    loop {
        let replies = tokio::select! {
            _ = stop.changed() => break,
            changed = status.changed() => {
                if changed.is_err() {
                    break;
                }
                let now = *status.borrow_and_update();
                session.set_status(now)
            }
            read = stream.read(&mut buffer) => match read {
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
