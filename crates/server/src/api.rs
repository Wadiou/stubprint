//! The REST API and the live events.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{Event as Sse, KeepAlive};
use axum::response::{IntoResponse, Response, Sse as SseResponse};
use axum::routing::{get, post};
use axum::{Json, Router};
use receipt::{Event, Fault, Paper, PrinterStatus, Profile};
use receipt_render::Options;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use utoipa::{OpenApi, ToSchema};

use crate::hub::{Hub, Notice, Stored};
use crate::session::Session;
use crate::wire::{hex, PrinterInfo, ReceiptDetail, ReceiptSummary};

/// The most bytes one `POST /api/print` takes.
const MAX_PRINT_BYTES: usize = 64 * 1024 * 1024;
/// How many receipts a list shows when it is not told.
const DEFAULT_LIMIT: usize = 100;
/// How long `next` waits when it is not told, and the longest it is allowed to.
const DEFAULT_WAIT: u64 = 10;
const LONGEST_WAIT: u64 = 120;

pub fn router(hub: Arc<Hub>) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/openapi.json", get(document))
        .route("/api/profiles", get(profiles))
        .route("/api/printers", get(printers))
        .route("/api/printers/{name}", get(printer).post(set_printer))
        .route("/api/receipts", get(list).delete(clear))
        .route("/api/receipts/next", get(next))
        .route("/api/receipts/{id}", get(receipt))
        .route("/api/print", post(print))
        .route("/api/events", get(events))
        .layer(DefaultBodyLimit::max(MAX_PRINT_BYTES))
        .with_state(hub)
}

/// What a request that went wrong says, as JSON.
#[derive(Debug)]
pub struct ApiError(StatusCode, String);

impl ApiError {
    fn not_found(what: impl Into<String>) -> Self {
        Self(StatusCode::NOT_FOUND, what.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

type Reply<T> = Result<T, ApiError>;

#[utoipa::path(get, path = "/api/health", responses((status = 200, description = "The server is running")))]
async fn health() -> Json<serde_json::Value> {
    Json(json!({ "ok": true }))
}

/// The description of this API, made from the code that serves it.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Receipt printer emulator",
        description = "A virtual receipt printer. Programs print to its TCP ports, and this API shows what was printed.",
        version = "0.0.1"
    ),
    paths(
        health,
        document,
        profiles,
        printers,
        printer,
        set_printer,
        list,
        clear,
        next,
        receipt,
        print,
        events
    ),
    components(schemas(Notice, Event, Printed, StatusChange))
)]
struct ApiDoc;

/// Used by the document of the API for the answer of an error.
#[derive(ToSchema)]
#[allow(dead_code)]
struct ErrorBody {
    error: String,
}

#[utoipa::path(get, path = "/api/openapi.json", responses((status = 200, description = "This document", content_type = "application/json")))]
async fn document() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[utoipa::path(get, path = "/api/profiles", responses((status = 200, description = "The names of the printer models that a printer can be set to", body = Vec<String>)))]
async fn profiles() -> Json<Vec<String>> {
    Json(Profile::names())
}

#[utoipa::path(get, path = "/api/printers", responses((status = 200, description = "Every virtual printer", body = Vec<PrinterInfo>)))]
async fn printers(State(hub): State<Arc<Hub>>) -> Json<Vec<PrinterInfo>> {
    Json(hub.printers().iter().map(|p| PrinterInfo::of(p)).collect())
}

#[utoipa::path(get, path = "/api/printers/{name}", params(("name" = String, Path, description = "The name of the printer")),
    responses((status = 200, body = PrinterInfo), (status = 404, body = ErrorBody)))]
async fn printer(
    State(hub): State<Arc<Hub>>,
    Path(name): Path<String>,
) -> Reply<Json<PrinterInfo>> {
    let printer = hub
        .printer(&name)
        .ok_or_else(|| ApiError::not_found(format!("no printer called {name}")))?;
    Ok(Json(PrinterInfo::of(printer)))
}

/// What a request changes about the state of a printer. What is left out stays as it is, and a
/// `fault` of `null` takes the fault away.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct StatusChange {
    online: Option<bool>,
    cover_open: Option<bool>,
    paper: Option<Paper>,
    drawer_open: Option<bool>,
    feed_button: Option<bool>,
    #[serde(default, deserialize_with = "present")]
    fault: Option<Option<Fault>>,
}

/// Tells a value that is `null` from one that is not there, which `Option` alone does not.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

impl StatusChange {
    fn apply(self, mut status: PrinterStatus) -> PrinterStatus {
        status.online = self.online.unwrap_or(status.online);
        status.cover_open = self.cover_open.unwrap_or(status.cover_open);
        status.paper = self.paper.unwrap_or(status.paper);
        status.drawer_open = self.drawer_open.unwrap_or(status.drawer_open);
        status.feed_button = self.feed_button.unwrap_or(status.feed_button);
        status.fault = self.fault.unwrap_or(status.fault);
        status
    }
}

/// Sets what the printer pretends: online or not, paper, cover, drawer, feed button and fault.
#[utoipa::path(post, path = "/api/printers/{name}", params(("name" = String, Path, description = "The name of the printer")),
    request_body = StatusChange, responses((status = 200, body = PrinterInfo), (status = 404, body = ErrorBody)))]
async fn set_printer(
    State(hub): State<Arc<Hub>>,
    Path(name): Path<String>,
    Json(change): Json<StatusChange>,
) -> Reply<Json<PrinterInfo>> {
    let printer = hub
        .printer(&name)
        .ok_or_else(|| ApiError::not_found(format!("no printer called {name}")))?;
    let status = change.apply(printer.status());
    printer.set_status(status);
    hub.notify(Notice::Status {
        printer: printer.name.clone(),
        status,
    });
    Ok(Json(PrinterInfo::of(printer)))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
struct ListQuery {
    printer: Option<String>,
    /// Only the receipts after the one with this id.
    after: Option<u64>,
    /// At most this many, the latest ones.
    limit: Option<usize>,
}

/// The receipts, oldest first. With a limit, the latest ones.
#[utoipa::path(get, path = "/api/receipts", params(ListQuery), responses((status = 200, body = Vec<ReceiptSummary>)))]
async fn list(
    State(hub): State<Arc<Hub>>,
    Query(query): Query<ListQuery>,
) -> Json<Vec<ReceiptSummary>> {
    let mut receipts: Vec<ReceiptSummary> = hub
        .receipts(query.printer.as_deref())
        .iter()
        .filter(|stored| query.after.is_none_or(|after| stored.id > after))
        .map(|stored| ReceiptSummary::of(stored))
        .collect();
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT);
    let extra = receipts.len().saturating_sub(limit);
    receipts.drain(..extra);
    Json(receipts)
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
struct ClearQuery {
    printer: Option<String>,
}

#[utoipa::path(delete, path = "/api/receipts", params(ClearQuery),
    responses((status = 200, description = "How many receipts were removed"), (status = 404, body = ErrorBody)))]
async fn clear(
    State(hub): State<Arc<Hub>>,
    Query(query): Query<ClearQuery>,
) -> Reply<Json<serde_json::Value>> {
    if let Some(name) = &query.printer {
        hub.printer(name)
            .ok_or_else(|| ApiError::not_found(format!("no printer called {name}")))?;
    }
    let removed = hub.clear(query.printer.as_deref());
    Ok(Json(json!({ "removed": removed })))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
struct NextQuery {
    printer: Option<String>,
    /// Waits for a receipt after the one with this id. Without it, for one after now.
    after: Option<u64>,
    /// How many seconds to wait.
    timeout: Option<u64>,
}

/// Waits for the next receipt. Test code prints, then calls this to get what came out.
#[utoipa::path(get, path = "/api/receipts/next", params(NextQuery),
    responses((status = 200, body = ReceiptDetail), (status = 204, description = "Nothing was printed in time")))]
async fn next(State(hub): State<Arc<Hub>>, Query(query): Query<NextQuery>) -> Response {
    let wait = Duration::from_secs(query.timeout.unwrap_or(DEFAULT_WAIT).min(LONGEST_WAIT));
    match hub.next(query.after, query.printer.as_deref(), wait).await {
        Some(stored) => Json(ReceiptDetail::of(&stored)).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

/// How a receipt is asked for: the id, and the form after a dot.
fn parse_receipt(text: &str) -> Reply<(u64, &str)> {
    let (id, form) = text.split_once('.').unwrap_or((text, "json"));
    let id = id
        .parse()
        .map_err(|_| ApiError::not_found(format!("{text} is not a receipt")))?;
    Ok((id, form))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
struct PictureQuery {
    /// How many times larger the picture is drawn, from 1 to 4.
    scale: Option<u32>,
}

/// One receipt. The id can end in `.bin` for the bytes the printer was sent, in `.txt` for the
/// paper as text, and in `.png` for a picture of the paper.
#[utoipa::path(get, path = "/api/receipts/{id}",
    params(("id" = String, Path, description = "The id of the receipt, with .json, .bin, .txt or .png after it"), PictureQuery),
    responses(
        (status = 200, body = ReceiptDetail),
        (status = 200, description = "With .png", content_type = "image/png"),
        (status = 200, description = "With .txt", content_type = "text/plain"),
        (status = 200, description = "With .bin", content_type = "application/octet-stream"),
        (status = 404, body = ErrorBody)))]
async fn receipt(
    State(hub): State<Arc<Hub>>,
    Path(text): Path<String>,
    Query(picture): Query<PictureQuery>,
) -> Reply<Response> {
    let (id, form) = parse_receipt(&text)?;
    let stored: Arc<Stored> = hub
        .receipt(id)
        .ok_or_else(|| ApiError::not_found(format!("no receipt {id}")))?;
    let mut options = Options::for_profile(&stored.printer.profile);
    options.scale = picture.scale.unwrap_or(1);
    match form {
        "json" => Ok(Json(ReceiptDetail::of(&stored)).into_response()),
        "bin" => Ok((
            [
                (header::CONTENT_TYPE, "application/octet-stream".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("inline; filename=\"receipt-{id}.bin\""),
                ),
            ],
            stored.receipt.raw().to_vec(),
        )
            .into_response()),
        "txt" => Ok((
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            receipt_render::text_of(&stored.receipt, &options),
        )
            .into_response()),
        "png" => {
            // Drawing takes a while for a long receipt, so it does not hold up the other requests.
            let file =
                tokio::task::spawn_blocking(move || receipt_render::png(&stored.receipt, &options))
                    .await
                    .map_err(|error| {
                        ApiError(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
                    })?
                    .map_err(|error| ApiError(StatusCode::INTERNAL_SERVER_ERROR, error))?;
            Ok(([(header::CONTENT_TYPE, "image/png")], file).into_response())
        }
        other => Err(ApiError::not_found(format!(
            "a receipt does not come as .{other}"
        ))),
    }
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
struct PrintQuery {
    /// Which printer gets the bytes. The first one when left out.
    printer: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
struct Printed {
    printer: String,
    /// The ids of the receipts the bytes made.
    receipts: Vec<u64>,
    /// What the printer answered, in hexadecimal.
    replies: String,
}

/// Sends bytes to a printer as if a program had connected and sent them.
#[utoipa::path(post, path = "/api/print", params(PrintQuery), request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 200, body = Printed), (status = 404, body = ErrorBody)))]
async fn print(
    State(hub): State<Arc<Hub>>,
    Query(query): Query<PrintQuery>,
    body: axum::body::Bytes,
) -> Reply<Json<Printed>> {
    let printer = match &query.printer {
        Some(name) => hub
            .printer(name)
            .ok_or_else(|| ApiError::not_found(format!("no printer called {name}")))?,
        None => &hub.printers()[0],
    }
    .clone();
    let name = printer.name.clone();
    let mut session = Session::open(hub.clone(), printer, "http".into());
    let mut replies = session.feed(&body);
    replies.extend(session.finish());
    Ok(Json(Printed {
        printer: name,
        receipts: session.stored().to_vec(),
        replies: hex(&replies),
    }))
}

/// The live events, as server sent events: receipt, cleared, status, connection and lagged.
#[utoipa::path(get, path = "/api/events", responses((status = 200, description = "A stream of events", content_type = "text/event-stream")))]
async fn events(
    State(hub): State<Arc<Hub>>,
) -> SseResponse<impl Stream<Item = Result<Sse, Infallible>>> {
    let stream = BroadcastStream::new(hub.subscribe()).map(|notice| {
        Ok(match notice {
            Ok(notice) => named(&notice),
            // The watcher was too slow and missed some. It should ask for the list again.
            Err(BroadcastStreamRecvError::Lagged(missed)) => Sse::default()
                .event("lagged")
                .data(json!({ "missed": missed }).to_string()),
        })
    });
    SseResponse::new(stream).keep_alive(KeepAlive::default())
}

fn named(notice: &Notice) -> Sse {
    Sse::default()
        .event(notice.name())
        .json_data(notice)
        .unwrap_or_else(|_| Sse::default().event("error"))
}
