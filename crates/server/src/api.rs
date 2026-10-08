//! The REST API and the live events.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{Event as Sse, KeepAlive};
use axum::response::{IntoResponse, Response, Sse as SseResponse};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use stubprint_core::{Dialect, Event, Fault, Paper, Printer, PrinterStatus, Profile};
use stubprint_render::Options;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use tower_http::services::{ServeDir, ServeFile};
use utoipa::{OpenApi, ToSchema};

use crate::config::{PrinterConfig, PrinterSettings, Web};
use crate::hub::{Connection, Hub, Notice, Stored};
use crate::page;
use crate::printer::VirtualPrinter;
use crate::samples;
use crate::session::Session;
use crate::wire::{
    hex, trace_of, trace_of_stream, PrinterInfo, ProfileInfo, ReceiptDetail, ReceiptSummary,
    SampleDetail, SampleSummary, TraceItem,
};

/// The most bytes one `POST /api/print` takes.
const MAX_PRINT_BYTES: usize = 64 * 1024 * 1024;
/// How many receipts a list shows when it is not told.
const DEFAULT_LIMIT: usize = 100;
/// How long `next` waits when it is not told, and the longest it is allowed to.
const DEFAULT_WAIT: u64 = 10;
const LONGEST_WAIT: u64 = 120;

/// The API, and the web page of `web` when there is one. A path of the API that does not exist is a
/// 404 and not the page, so a program that asks for it gets an answer it can read.
pub fn router(hub: Arc<Hub>, web: &Web) -> Router {
    let api = Router::new()
        .route("/api", get(index))
        .route("/api/health", get(health))
        .route("/api/openapi.json", get(document))
        .route("/api/profiles", get(profiles))
        .route("/api/printers", get(printers).post(add_printer))
        .route(
            "/api/printers/{name}",
            get(printer)
                .post(set_printer)
                .patch(update_printer)
                .delete(remove_printer),
        )
        .route("/api/receipts", get(list).delete(clear))
        .route("/api/receipts/next", get(next))
        .route("/api/receipts/{id}", get(receipt).delete(remove_receipt))
        .route("/api/receipts/{id}/bytes", get(receipt_bytes))
        .route("/api/receipts/{id}/text", get(receipt_text))
        .route("/api/receipts/{id}/picture", get(receipt_picture))
        .route("/api/receipts/{id}/trace", get(trace))
        .route("/api/connections", get(connections))
        .route("/api/connections/{id}", get(connection))
        .route("/api/decode", post(decode))
        .route("/api/print", post(print))
        .route("/api/samples", get(sample_list))
        .route("/api/samples/{id}", get(sample))
        .route("/api/samples/{id}/bytes", get(sample_bytes))
        .route("/api/samples/{id}/picture", get(sample_picture))
        .route("/api/samples/{id}/text", get(sample_text))
        .route("/api/samples/{id}/print", post(print_sample))
        .route("/api/events", get(events))
        .route("/api/{*rest}", any(no_such_route))
        .layer(DefaultBodyLimit::max(MAX_PRINT_BYTES))
        .with_state(hub);
    match web {
        // Any other path is the page, which finds its own route in the address.
        Web::Folder(folder) => api.fallback_service(
            ServeDir::new(folder).fallback(ServeFile::new(folder.join("index.html"))),
        ),
        Web::Embedded if page::is_built() => api.fallback(get(page::serve)),
        Web::Embedded | Web::Off => api,
    }
}

async fn no_such_route() -> ApiError {
    ApiError::not_found("there is no such route in the API")
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

/// Where to start: what this is, and where its description is.
#[utoipa::path(get, path = "/api", operation_id = "get_index", tag = "meta", responses((status = 200, description = "The name, the version, where the description of the API is, and the printers")))]
async fn index(State(hub): State<Arc<Hub>>) -> Json<serde_json::Value> {
    Json(json!({
        "name": "stubprint",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "A virtual receipt printer: programs print to its TCP ports, and this API shows what was printed.",
        "openapi": "/api/openapi.json",
        "printers": hub.printers().iter().map(|printer| json!({
            "name": printer.name,
            "dialect": printer.dialect.name(),
            "listen": printer.listen.to_string(),
        })).collect::<Vec<_>>(),
    }))
}

/// Answers when the server is running.
#[utoipa::path(get, path = "/api/health", operation_id = "get_health", tag = "meta", responses((status = 200, description = "The server is running")))]
async fn health() -> Json<serde_json::Value> {
    Json(json!({ "ok": true }))
}

/// The description of this API, made from the code that serves it.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Stubprint",
        description = "A virtual receipt printer. Programs print to its TCP ports, and this API shows what was printed.",
        license(name = "MIT", identifier = "MIT")
    ),
    tags(
        (name = "meta", description = "Where to start, whether it is running, and which printer models exist"),
        (name = "printers", description = "The virtual printers, and what each one pretends to be going through"),
        (name = "receipts", description = "What was printed: a receipt is everything a printer printed up to a cut. Each comes as data, as the bytes that made it, as text and as a picture"),
        (name = "printing", description = "Sending bytes to a printer without a socket"),
        (name = "samples", description = "Ready made receipts that real printing libraries wrote, each in every library that can print it, to try a printer or a viewer with"),
        (name = "debugging", description = "Why a receipt came out as it did: every command with its bytes and the state after it, the programs that connected, and reading bytes without keeping them"),
        (name = "live", description = "What happens, as it happens")
    ),
    paths(
        index,
        health,
        document,
        profiles,
        printers,
        printer,
        add_printer,
        remove_printer,
        set_printer,
        update_printer,
        list,
        clear,
        next,
        receipt,
        remove_receipt,
        receipt_bytes,
        receipt_text,
        receipt_picture,
        trace,
        connections,
        connection,
        decode,
        print,
        sample_list,
        sample,
        sample_bytes,
        sample_picture,
        sample_text,
        print_sample,
        events
    ),
    components(schemas(Notice, Event, Printed, StatusChange, PrinterSettings))
)]
struct ApiDoc;

/// The raw bytes of a file, not JSON.
#[derive(ToSchema)]
#[schema(value_type = String, format = Binary)]
#[allow(dead_code)]
struct Binary(Vec<u8>);

/// Used by the document of the API for the answer of an error.
#[derive(ToSchema)]
#[allow(dead_code)]
struct ErrorBody {
    /// What went wrong, in words.
    error: String,
}

/// The description of this API, as an OpenAPI document.
#[utoipa::path(get, path = "/api/openapi.json", operation_id = "get_openapi", tag = "meta", responses((status = 200, description = "This document", content_type = "application/json")))]
async fn document() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi())
}

/// The description of the API as an OpenAPI document, without starting a server. It is what
/// `GET /api/openapi.json` answers, and what the types of the web page are made from.
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

/// The printer models that a printer can be set to.
#[utoipa::path(get, path = "/api/profiles", operation_id = "list_profiles", tag = "meta", responses((status = 200, description = "The printer models that a printer can be set to, with what each says about itself", body = Vec<ProfileInfo>)))]
async fn profiles() -> Json<Vec<ProfileInfo>> {
    Json(
        Profile::names()
            .iter()
            .filter_map(|name| Profile::named(name))
            .map(|profile| ProfileInfo::of(&profile))
            .collect(),
    )
}

/// Every virtual printer, with its status and how many programs are connected.
#[utoipa::path(get, path = "/api/printers", operation_id = "list_printers", tag = "printers", responses((status = 200, description = "Every virtual printer", body = Vec<PrinterInfo>)))]
async fn printers(State(hub): State<Arc<Hub>>) -> Json<Vec<PrinterInfo>> {
    Json(hub.printers().iter().map(|p| PrinterInfo::of(p)).collect())
}

/// Adds a printer while the server runs: it opens its TCP port at once. What is left out is the
/// default of a printer: `printer`, `escpos` and the `generic` model, on a port that the system
/// picks because it is free. A name that is taken gets `_2`, `_3` and so on after it, and the
/// answer says the name the printer has.
#[utoipa::path(post, path = "/api/printers", operation_id = "add_printer", tag = "printers", request_body = PrinterSettings,
    responses(
        (status = 201, description = "The printer, listening", body = PrinterInfo),
        (status = 400, description = "The request is not valid, as `error` says", body = ErrorBody),
        (status = 409, description = "The address is taken, as `error` says", body = ErrorBody)))]
async fn add_printer(
    State(hub): State<Arc<Hub>>,
    Json(settings): Json<PrinterSettings>,
) -> Reply<(StatusCode, Json<PrinterInfo>)> {
    let any_port = settings.listen.is_none();
    let mut config = settings
        .build()
        .map_err(|message| ApiError(StatusCode::BAD_REQUEST, message))?;
    PrinterConfig::check_name(&config.name)
        .map_err(|message| ApiError(StatusCode::BAD_REQUEST, message))?;
    if any_port {
        config.listen = ([127, 0, 0, 1], 0).into();
    }
    config.name = hub.free_name(&config.name);
    let printer = hub.add_printer(config).await.map_err(|error| {
        let status = match error.kind() {
            std::io::ErrorKind::InvalidInput => StatusCode::BAD_REQUEST,
            std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::AddrInUse => {
                StatusCode::CONFLICT
            }
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::AddrNotAvailable => {
                StatusCode::BAD_REQUEST
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError(status, error.to_string())
    })?;
    Ok((StatusCode::CREATED, Json(PrinterInfo::of(&printer))))
}

/// Removes a printer: its port and its connections close. The receipts it printed are kept.
#[utoipa::path(delete, path = "/api/printers/{name}", operation_id = "remove_printer", tag = "printers", params(("name" = String, Path, description = "The name of the printer")),
    responses(
        (status = 204, description = "The printer is gone"),
        (status = 404, description = "There is no such printer, as `error` says", body = ErrorBody)))]
async fn remove_printer(
    State(hub): State<Arc<Hub>>,
    Path(name): Path<String>,
) -> Reply<StatusCode> {
    if hub.remove_printer(&name).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!("no printer called {name}")))
    }
}

/// One virtual printer.
#[utoipa::path(get, path = "/api/printers/{name}", operation_id = "get_printer", tag = "printers", params(("name" = String, Path, description = "The name of the printer")),
    responses((status = 200, description = "The printer", body = PrinterInfo), (status = 404, description = "There is no such printer, receipt or connection, as `error` says", body = ErrorBody)))]
async fn printer(
    State(hub): State<Arc<Hub>>,
    Path(name): Path<String>,
) -> Reply<Json<PrinterInfo>> {
    let printer = hub
        .printer(&name)
        .ok_or_else(|| ApiError::not_found(format!("no printer called {name}")))?;
    Ok(Json(PrinterInfo::of(&printer)))
}

/// What a request changes about the state of a printer. What is left out stays as it is, and a
/// `fault` of `null` takes the fault away.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct StatusChange {
    /// The printer is on line.
    online: Option<bool>,
    /// The cover is open.
    cover_open: Option<bool>,
    /// How much paper is left: `adequate`, `near_end` or `end`.
    paper: Option<Paper>,
    /// The cash drawer is open.
    drawer_open: Option<bool>,
    /// The paper feed button is held.
    feed_button: Option<bool>,
    #[serde(default, deserialize_with = "present")]
    /// What is wrong, or `null` for nothing.
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
#[utoipa::path(post, path = "/api/printers/{name}", operation_id = "set_printer_status", tag = "printers", params(("name" = String, Path, description = "The name of the printer")),
    request_body = StatusChange, responses((status = 200, description = "The printer", body = PrinterInfo), (status = 404, description = "There is no such printer, receipt or connection, as `error` says", body = ErrorBody)))]
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
    Ok(Json(PrinterInfo::of(&printer)))
}

/// What a request changes about the printer itself. What is left out stays as it is.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct PrinterChange {
    /// The printer model, as `GET /api/profiles` lists them. Receipts printed from now on are read
    /// and drawn as this model. The ones already kept stay as they were printed.
    profile: Option<String>,
}

/// Sets the printer to another model, as if another printer stood on its port.
#[utoipa::path(patch, path = "/api/printers/{name}", operation_id = "update_printer", tag = "printers", params(("name" = String, Path, description = "The name of the printer")),
    request_body = PrinterChange, responses(
        (status = 200, description = "The printer", body = PrinterInfo),
        (status = 400, description = "There is no such model, as `error` says", body = ErrorBody),
        (status = 404, description = "There is no such printer, receipt or connection, as `error` says", body = ErrorBody)))]
async fn update_printer(
    State(hub): State<Arc<Hub>>,
    Path(name): Path<String>,
    Json(change): Json<PrinterChange>,
) -> Reply<Json<PrinterInfo>> {
    let printer = hub
        .printer(&name)
        .ok_or_else(|| ApiError::not_found(format!("no printer called {name}")))?;
    if let Some(model) = change.profile {
        let profile = Profile::named(&model).ok_or_else(|| {
            ApiError(
                StatusCode::BAD_REQUEST,
                format!("{model} is not a printer model, see GET /api/profiles"),
            )
        })?;
        printer.set_profile(profile);
        hub.notify(Notice::PrinterChanged(PrinterInfo::of(&printer)));
    }
    Ok(Json(PrinterInfo::of(&printer)))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
struct ListQuery {
    /// Only the receipts of this printer, by name.
    printer: Option<String>,
    /// Only the receipts after the one with this id.
    after: Option<u64>,
    /// At most this many, the latest ones.
    limit: Option<usize>,
}

/// The receipts, oldest first. With a limit, the latest ones.
#[utoipa::path(get, path = "/api/receipts", operation_id = "list_receipts", tag = "receipts", params(ListQuery), responses((status = 200, description = "The receipts", body = Vec<ReceiptSummary>)))]
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
#[into_params(parameter_in = Query)]
struct ClearQuery {
    /// Only the receipts of this printer, by name.
    printer: Option<String>,
}

/// Forgets the receipts, all of them or those of one printer. Their numbers are not used again.
#[utoipa::path(delete, path = "/api/receipts", operation_id = "clear_receipts", tag = "receipts", params(ClearQuery),
    responses((status = 200, description = "How many receipts were removed"), (status = 404, description = "There is no such printer, receipt or connection, as `error` says", body = ErrorBody)))]
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
#[into_params(parameter_in = Query)]
struct NextQuery {
    /// Only a receipt of this printer, by name.
    printer: Option<String>,
    /// Waits for a receipt after the one with this id. Without it, for one after now.
    after: Option<u64>,
    /// How many seconds to wait.
    timeout: Option<u64>,
}

/// Waits for the next receipt. Test code prints, then calls this to get what came out.
#[utoipa::path(get, path = "/api/receipts/next", operation_id = "wait_for_receipt", tag = "receipts", params(NextQuery),
    responses((status = 200, description = "The receipt", body = ReceiptDetail), (status = 204, description = "Nothing was printed in time")))]
async fn next(State(hub): State<Arc<Hub>>, Query(query): Query<NextQuery>) -> Response {
    let wait = Duration::from_secs(query.timeout.unwrap_or(DEFAULT_WAIT).min(LONGEST_WAIT));
    match hub.next(query.after, query.printer.as_deref(), wait).await {
        Some(stored) => Json(ReceiptDetail::of(&stored)).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
struct ViewQuery {
    /// How many times larger the picture is drawn, from 1 to 4.
    scale: Option<u32>,
    /// Only the first this many events: the paper as it was part of the way through the job.
    upto: Option<usize>,
}

fn stored(hub: &Hub, id: u64) -> Reply<Arc<Stored>> {
    hub.receipt(id)
        .ok_or_else(|| ApiError::not_found(format!("no receipt {id}")))
}

/// One receipt as data: where and when it was printed, its commands and the text on it.
#[utoipa::path(get, path = "/api/receipts/{id}", operation_id = "get_receipt", tag = "receipts",
    params(("id" = u64, Path, description = "The id of the receipt")),
    responses(
        (status = 200, description = "The receipt", body = ReceiptDetail),
        (status = 404, description = "There is no such printer, receipt or connection, as `error` says", body = ErrorBody)))]
async fn receipt(State(hub): State<Arc<Hub>>, Path(id): Path<u64>) -> Reply<Response> {
    let stored = stored(&hub, id)?;
    Ok(Json(ReceiptDetail::of(&stored)).into_response())
}

/// Forgets one receipt. Its number is not used again.
#[utoipa::path(delete, path = "/api/receipts/{id}", operation_id = "remove_receipt", tag = "receipts",
    params(("id" = u64, Path, description = "The id of the receipt")),
    responses(
        (status = 204, description = "The receipt was removed"),
        (status = 404, description = "There is no such printer, receipt or connection, as `error` says", body = ErrorBody)))]
async fn remove_receipt(State(hub): State<Arc<Hub>>, Path(id): Path<u64>) -> Reply<StatusCode> {
    if hub.remove_receipt(id) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!("no receipt {id}")))
    }
}

/// The bytes the printer was sent for a receipt. With `upto`, only those of its first events.
#[utoipa::path(get, path = "/api/receipts/{id}/bytes", operation_id = "get_receipt_bytes", tag = "receipts",
    params(("id" = u64, Path, description = "The id of the receipt"), ViewQuery),
    responses(
        (status = 200, description = "The bytes", content_type = "application/octet-stream", body = Binary),
        (status = 404, description = "There is no such printer, receipt or connection, as `error` says", body = ErrorBody)))]
async fn receipt_bytes(
    State(hub): State<Arc<Hub>>,
    Path(id): Path<u64>,
    Query(view): Query<ViewQuery>,
) -> Reply<Response> {
    let stored = stored(&hub, id)?;
    let part = view.upto.map(|count| stored.receipt.until(count));
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"receipt-{id}.bin\""),
            ),
        ],
        part.as_ref().unwrap_or(&stored.receipt).raw().to_vec(),
    )
        .into_response())
}

/// The paper of a receipt as text. Pictures and codes are tags such as `[qr M: hello]`. With
/// `upto`, the paper as it was after the first events.
#[utoipa::path(get, path = "/api/receipts/{id}/text", operation_id = "get_receipt_text", tag = "receipts",
    params(("id" = u64, Path, description = "The id of the receipt"), ViewQuery),
    responses(
        (status = 200, description = "The paper as text", content_type = "text/plain", body = String),
        (status = 404, description = "There is no such printer, receipt or connection, as `error` says", body = ErrorBody)))]
async fn receipt_text(
    State(hub): State<Arc<Hub>>,
    Path(id): Path<u64>,
    Query(view): Query<ViewQuery>,
) -> Reply<Response> {
    let stored = stored(&hub, id)?;
    let options = Options::for_profile(&stored.profile);
    let part = view.upto.map(|count| stored.receipt.until(count));
    Ok((
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        stubprint_render::text_of(part.as_ref().unwrap_or(&stored.receipt), &options),
    )
        .into_response())
}

/// A picture of the paper of a receipt. With `upto`, the paper as it was after the first events.
#[utoipa::path(get, path = "/api/receipts/{id}/picture", operation_id = "get_receipt_picture", tag = "receipts",
    params(("id" = u64, Path, description = "The id of the receipt"), ViewQuery),
    responses(
        (status = 200, description = "The picture", content_type = "image/png", body = Binary),
        (status = 404, description = "There is no such printer, receipt or connection, as `error` says", body = ErrorBody)))]
async fn receipt_picture(
    State(hub): State<Arc<Hub>>,
    Path(id): Path<u64>,
    Query(view): Query<ViewQuery>,
) -> Reply<Response> {
    let stored = stored(&hub, id)?;
    let mut options = Options::for_profile(&stored.profile);
    options.scale = view.scale.unwrap_or(1);
    let part = view.upto.map(|count| stored.receipt.until(count));
    // Drawing takes a while for a long receipt, so it does not hold up the other requests.
    let file = tokio::task::spawn_blocking(move || {
        stubprint_render::png(part.as_ref().unwrap_or(&stored.receipt), &options)
    })
    .await
    .map_err(|error| ApiError(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
    .map_err(|error| ApiError(StatusCode::INTERNAL_SERVER_ERROR, error))?;
    Ok(([(header::CONTENT_TYPE, "image/png")], file).into_response())
}

/// The commands of a receipt, one by one: the bytes of each, what it asked for, and what the
/// printer believed after it. This is what to look at to see why a line came out as it did.
#[utoipa::path(get, path = "/api/receipts/{id}/trace", operation_id = "get_trace", tag = "debugging", params(("id" = u64, Path, description = "The id of the receipt")),
    responses((status = 200, description = "The commands of the receipt", body = Vec<TraceItem>), (status = 404, description = "There is no such printer, receipt or connection, as `error` says", body = ErrorBody)))]
async fn trace(State(hub): State<Arc<Hub>>, Path(id): Path<u64>) -> Reply<Response> {
    let stored = hub
        .receipt(id)
        .ok_or_else(|| ApiError::not_found(format!("no receipt {id}")))?;
    Ok(Json(trace_of(&stored.receipt)).into_response())
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
struct ConnectionQuery {
    /// Only the connections to this printer, by name.
    printer: Option<String>,
}

/// The programs that connected, oldest first: when, how many bytes in what pieces, what the
/// printer answered, and which receipts they printed.
#[utoipa::path(get, path = "/api/connections", operation_id = "list_connections", tag = "debugging", params(ConnectionQuery), responses((status = 200, description = "The connections", body = Vec<Connection>)))]
async fn connections(
    State(hub): State<Arc<Hub>>,
    Query(query): Query<ConnectionQuery>,
) -> Json<Vec<Connection>> {
    Json(hub.connections(query.printer.as_deref()))
}

/// One connection, with the pieces its bytes came in.
#[utoipa::path(get, path = "/api/connections/{id}", operation_id = "get_connection", tag = "debugging", params(("id" = u64, Path, description = "The id of the connection")),
    responses((status = 200, description = "The connection", body = Connection), (status = 404, description = "There is no such printer, receipt or connection, as `error` says", body = ErrorBody)))]
async fn connection(State(hub): State<Arc<Hub>>, Path(id): Path<u64>) -> Reply<Json<Connection>> {
    hub.connection(id)
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("no connection {id}")))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
struct DecodeQuery {
    /// `escpos`, `starline` or `starprnt`. ESC/POS when left out.
    dialect: Option<String>,
    /// The model, as `GET /api/profiles` lists them.
    profile: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
struct Decoded<'a> {
    dialect: Dialect,
    /// The printer model the bytes were read as.
    profile: String,
    /// Every command of the bytes.
    trace: Vec<TraceItem<'a>>,
    /// What the printer answered, in hexadecimal.
    replies: String,
    /// How many receipts the bytes made.
    receipts: usize,
}

/// Reads bytes as a printer would and says what they ask for, without keeping anything. Use it to
/// see what the same bytes mean to another language or another model.
#[utoipa::path(post, path = "/api/decode", operation_id = "decode_bytes", tag = "debugging", params(DecodeQuery), request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 200, description = "What the bytes asked for", body = Decoded), (status = 400, description = "The request is not valid, as `error` says", body = ErrorBody)))]
async fn decode(Query(query): Query<DecodeQuery>, body: axum::body::Bytes) -> Reply<Response> {
    let dialect = match &query.dialect {
        Some(name) => Dialect::from_name(name)
            .ok_or_else(|| ApiError(StatusCode::BAD_REQUEST, format!("{name} is not a dialect")))?,
        None => Dialect::EscPos,
    };
    let profile = match &query.profile {
        Some(name) => Profile::named(name).ok_or_else(|| {
            ApiError(
                StatusCode::BAD_REQUEST,
                format!("{name} is not a printer model"),
            )
        })?,
        None => Profile::generic(),
    };
    let profile_name = profile.name.clone();
    let mut printer = Printer::with_dialect(dialect, profile);
    let mut pushed = printer.push_traced(&body);
    let rest = printer.finish_traced();
    pushed.traced.extend(rest.traced);
    pushed.receipts.extend(rest.receipts);
    let answer = Decoded {
        dialect,
        profile: profile_name,
        trace: trace_of_stream(&pushed.traced),
        replies: hex(&printer.take_replies()),
        receipts: pushed.receipts.len(),
    };
    Ok(Json(answer).into_response())
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
struct PrintQuery {
    /// Which printer gets the bytes. The first one when left out.
    printer: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
struct Printed {
    /// The name of the printer the bytes went to.
    printer: String,
    /// The id of the connection it was sent as, in `GET /api/connections`.
    connection: u64,
    /// The ids of the receipts the bytes made.
    receipts: Vec<u64>,
    /// What the printer answered, in hexadecimal.
    replies: String,
}

/// Sends bytes to a printer as if a program had connected and sent them.
#[utoipa::path(post, path = "/api/print", operation_id = "print_bytes", tag = "printing", params(PrintQuery), request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 200, description = "What the bytes made", body = Printed), (status = 404, description = "There is no such printer, receipt or connection, as `error` says", body = ErrorBody)))]
async fn print(
    State(hub): State<Arc<Hub>>,
    Query(query): Query<PrintQuery>,
    body: axum::body::Bytes,
) -> Reply<Json<Printed>> {
    let printer = find_printer(&hub, query.printer.as_deref())?;
    Ok(Json(send(&hub, printer, &body)))
}

/// The named printer, or the first one.
fn find_printer(hub: &Hub, name: Option<&str>) -> Reply<Arc<VirtualPrinter>> {
    match name {
        Some(name) => hub
            .printer(name)
            .ok_or_else(|| ApiError::not_found(format!("no printer called {name}"))),
        None => hub.printers().first().cloned().ok_or_else(|| {
            ApiError(
                StatusCode::BAD_REQUEST,
                "there is no printer to print to".into(),
            )
        }),
    }
}

/// Sends bytes to a printer as a connection that sends them and closes.
fn send(hub: &Arc<Hub>, printer: Arc<VirtualPrinter>, bytes: &[u8]) -> Printed {
    let name = printer.name.clone();
    let mut session = Session::open(hub.clone(), printer, "http".into());
    let mut replies = session.feed(bytes);
    replies.extend(session.finish());
    Printed {
        connection: session.id(),
        printer: name,
        receipts: session.stored().to_vec(),
        replies: hex(&replies),
    }
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
struct VariantQuery {
    /// Only the variant that this library wrote, such as `python-escpos` or `node-thermal-printer`.
    library: Option<String>,
    /// Only the variant in this command language, `escpos` or `starline`.
    dialect: Option<String>,
}

impl VariantQuery {
    fn dialect(&self) -> Reply<Option<Dialect>> {
        self.dialect
            .as_deref()
            .map(|name| {
                Dialect::from_name(name).ok_or_else(|| {
                    ApiError::not_found(format!("no command language called {name}"))
                })
            })
            .transpose()
    }
}

fn internal(message: String) -> ApiError {
    ApiError(StatusCode::INTERNAL_SERVER_ERROR, message)
}

fn find_sample(id: &str) -> Reply<&'static samples::Sample> {
    samples::find(id).ok_or_else(|| ApiError::not_found(format!("no sample called {id}")))
}

fn no_variant(sample: &samples::Sample) -> ApiError {
    ApiError::not_found(format!(
        "{} has no variant like that, see GET /api/samples/{}",
        sample.id, sample.id
    ))
}

/// The ready made receipts, without their bytes.
#[utoipa::path(get, path = "/api/samples", operation_id = "list_samples", tag = "samples", responses((status = 200, description = "The samples, in the order to show them", body = Vec<SampleSummary>)))]
async fn sample_list() -> Json<Vec<SampleSummary>> {
    Json(samples::all().iter().map(SampleSummary::of).collect())
}

/// One ready made receipt: what it is, and what it should read as.
#[utoipa::path(get, path = "/api/samples/{id}", operation_id = "get_sample", tag = "samples",
    params(("id" = String, Path, description = "The id of the sample")),
    responses(
        (status = 200, description = "The sample", body = SampleDetail),
        (status = 404, description = "There is no such sample, as `error` says", body = ErrorBody)))]
async fn sample(Path(id): Path<String>) -> Reply<Json<SampleDetail>> {
    Ok(Json(SampleDetail::of(find_sample(&id)?)))
}

/// The bytes of one variant of a sample, which any program can send to a printer. `library` and
/// `dialect` pick the variant, and the first one is used when they are left out.
#[utoipa::path(get, path = "/api/samples/{id}/bytes", operation_id = "get_sample_bytes", tag = "samples",
    params(("id" = String, Path, description = "The id of the sample"), VariantQuery),
    responses(
        (status = 200, description = "The bytes", content_type = "application/octet-stream", body = Binary),
        (status = 404, description = "There is no such sample or variant, as `error` says", body = ErrorBody)))]
async fn sample_bytes(
    Path(id): Path<String>,
    Query(query): Query<VariantQuery>,
) -> Reply<Response> {
    let found = find_sample(&id)?;
    let variant = found
        .variant(query.library.as_deref(), query.dialect()?)
        .ok_or_else(|| no_variant(found))?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!(
                    "attachment; filename=\"{id}.{}.{}.bin\"",
                    variant.library,
                    variant.dialect.name()
                ),
            ),
        ],
        variant.bytes.clone(),
    )
        .into_response())
}

/// What a variant of a sample prints, read as the model the sample was written for, and the paper
/// of that model. Nothing is printed to get it.
fn read_sample(
    found: &samples::Sample,
    query: &VariantQuery,
) -> Reply<(stubprint_core::Receipt, Options)> {
    let variant = found
        .variant(query.library.as_deref(), query.dialect()?)
        .ok_or_else(|| no_variant(found))?;
    let profile = Profile::named(&found.profile).ok_or_else(|| {
        internal(format!(
            "sample {} has no model {}",
            found.id, found.profile
        ))
    })?;
    let options = Options::for_profile(&profile);
    let mut printer = Printer::with_dialect(variant.dialect, profile);
    let mut receipts = printer.push(&variant.bytes);
    receipts.extend(printer.finish());
    let receipt = receipts
        .into_iter()
        .next()
        .ok_or_else(|| internal(format!("sample {} prints nothing", found.id)))?;
    Ok((receipt, options))
}

/// A picture of what a variant of a sample prints, on the paper of the model it was written for.
/// Nothing is printed to draw it. `library` and `dialect` pick the variant.
#[utoipa::path(get, path = "/api/samples/{id}/picture", operation_id = "get_sample_picture", tag = "samples",
    params(("id" = String, Path, description = "The id of the sample"), VariantQuery),
    responses(
        (status = 200, description = "The picture", content_type = "image/png", body = Binary),
        (status = 404, description = "There is no such sample or variant, as `error` says", body = ErrorBody)))]
async fn sample_picture(
    Path(id): Path<String>,
    Query(query): Query<VariantQuery>,
) -> Reply<Response> {
    let (receipt, options) = read_sample(find_sample(&id)?, &query)?;
    // Drawing takes a while, so it does not hold up the other requests.
    let file = tokio::task::spawn_blocking(move || stubprint_render::png(&receipt, &options))
        .await
        .map_err(|error| internal(error.to_string()))?
        .map_err(internal)?;
    Ok(([(header::CONTENT_TYPE, "image/png")], file).into_response())
}

/// What a variant of a sample prints, as text on the paper of the model it was written for.
/// Pictures and codes are tags such as `[qr M: hello]`. Nothing is printed to get it.
#[utoipa::path(get, path = "/api/samples/{id}/text", operation_id = "get_sample_text", tag = "samples",
    params(("id" = String, Path, description = "The id of the sample"), VariantQuery),
    responses(
        (status = 200, description = "The paper as text", content_type = "text/plain", body = String),
        (status = 404, description = "There is no such sample or variant, as `error` says", body = ErrorBody)))]
async fn sample_text(Path(id): Path<String>, Query(query): Query<VariantQuery>) -> Reply<Response> {
    let (receipt, options) = read_sample(find_sample(&id)?, &query)?;
    Ok((
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        stubprint_render::text_of(&receipt, &options),
    )
        .into_response())
}

/// Prints a sample to a printer, as if a program had connected and sent it. Without `dialect` it
/// sends a variant in the command language the printer is set to, because a printer only reads
/// its own.
#[utoipa::path(post, path = "/api/samples/{id}/print", operation_id = "print_sample", tag = "samples",
    params(("id" = String, Path, description = "The id of the sample"), PrintQuery, VariantQuery),
    responses(
        (status = 200, description = "What the bytes made", body = Printed),
        (status = 404, description = "There is no such sample, variant or printer, as `error` says", body = ErrorBody)))]
async fn print_sample(
    State(hub): State<Arc<Hub>>,
    Path(id): Path<String>,
    Query(printing): Query<PrintQuery>,
    Query(query): Query<VariantQuery>,
) -> Reply<Json<Printed>> {
    let found = find_sample(&id)?;
    let printer = find_printer(&hub, printing.printer.as_deref())?;
    let dialect = query.dialect()?.unwrap_or(printer.dialect);
    let variant = found
        .variant(query.library.as_deref(), Some(dialect))
        .ok_or_else(|| no_variant(found))?;
    Ok(Json(send(&hub, printer, &variant.bytes)))
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
struct EventsQuery {
    /// `actions` also sends every command as it is read, which a long job makes thousands of.
    detail: Option<String>,
}

/// The live events, as server sent events: receipt, receipt_removed, cleared, status, connection and lagged, and
/// with `detail=actions` also action, one for each command as it is read.
#[utoipa::path(get, path = "/api/events", operation_id = "stream_events", tag = "live", params(EventsQuery), responses((status = 200, description = "A stream of events", content_type = "text/event-stream")))]
async fn events(
    State(hub): State<Arc<Hub>>,
    Query(query): Query<EventsQuery>,
) -> SseResponse<impl Stream<Item = Result<Sse, Infallible>>> {
    let actions = query.detail.as_deref() == Some("actions");
    // Commands are only made for the ones who ask, and for as long as the stream is open.
    let watch = actions.then(|| hub.watch_actions());
    let stream = BroadcastStream::new(hub.subscribe()).filter_map(move |notice| {
        let _watching = &watch;
        match notice {
            Ok(Notice::Action { .. }) if !actions => None,
            Ok(notice) => Some(Ok(named(&notice))),
            // The watcher was too slow and missed some. It should ask for the list again.
            Err(BroadcastStreamRecvError::Lagged(missed)) => Some(Ok(Sse::default()
                .event("lagged")
                .data(json!({ "missed": missed }).to_string()))),
        }
    });
    SseResponse::new(stream).keep_alive(KeepAlive::default())
}

fn named(notice: &Notice) -> Sse {
    Sse::default()
        .event(notice.name())
        .json_data(notice)
        .unwrap_or_else(|_| Sse::default().event("error"))
}
