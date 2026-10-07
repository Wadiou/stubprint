//! The server is started for real and driven the way a program and a person would: bytes over TCP,
//! then the REST API over HTTP.

use std::net::SocketAddr;
use std::time::Duration;

use serde_json::{json, Value};
use stubprint_server::{serve, Config, PrinterConfig, Running};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

const CUT: [u8; 4] = [0x1d, b'V', 0x42, 0x00];

struct Harness {
    running: Running,
    client: reqwest::Client,
    base: String,
}

impl Harness {
    async fn start(printers: &[&str]) -> Self {
        Self::start_keeping(printers, 500).await
    }

    async fn start_keeping(printers: &[&str], keep: usize) -> Self {
        Self::start_with(printers, keep, None).await
    }

    async fn start_with(printers: &[&str], keep: usize, web: Option<std::path::PathBuf>) -> Self {
        let printers: Vec<PrinterConfig> = printers
            .iter()
            .map(|text| PrinterConfig::parse(&format!("listen=127.0.0.1:0,{text}")).unwrap())
            .collect();
        let config = Config {
            http: ([127, 0, 0, 1], 0).into(),
            printers,
            keep,
            web,
        };
        let running = serve(config).await.unwrap();
        let base = format!("http://{}", running.http_addr());
        Self {
            running,
            client: reqwest::Client::new(),
            base,
        }
    }

    fn printer(&self, name: &str) -> SocketAddr {
        self.running.printer_addr(name).unwrap()
    }

    async fn get(&self, path: &str) -> reqwest::Response {
        self.client
            .get(format!("{}{path}", self.base))
            .send()
            .await
            .unwrap()
    }

    async fn json(&self, path: &str) -> Value {
        self.get(path).await.json().await.unwrap()
    }

    /// Sends bytes over a connection, in pieces, closes it and returns what the printer answered.
    async fn print(&self, name: &str, bytes: &[u8], piece: usize) -> Vec<u8> {
        let mut stream = TcpStream::connect(self.printer(name)).await.unwrap();
        for chunk in bytes.chunks(piece) {
            stream.write_all(chunk).await.unwrap();
            stream.flush().await.unwrap();
        }
        stream.shutdown().await.unwrap();
        let mut replies = Vec::new();
        timeout(Duration::from_secs(5), stream.read_to_end(&mut replies))
            .await
            .expect("the printer closes the connection")
            .unwrap();
        replies
    }

    /// Waits until a receipt after this id exists and returns it.
    async fn after(&self, id: u64) -> Value {
        let answer = self
            .get(&format!("/api/receipts/next?after={id}&timeout=5"))
            .await;
        assert_eq!(answer.status(), 200, "a receipt came");
        answer.json().await.unwrap()
    }

    async fn ids(&self) -> Vec<u64> {
        self.json("/api/receipts")
            .await
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_u64().unwrap())
            .collect()
    }
}

fn receipt_bytes(text: &str) -> Vec<u8> {
    [text.as_bytes(), b"\n", &CUT].concat()
}

#[tokio::test]
async fn what_a_program_prints_over_tcp_is_a_receipt() {
    let server = Harness::start(&["name=till"]).await;
    server.print("till", &receipt_bytes("Hello"), 1000).await;
    let receipt = server.after(0).await;
    assert_eq!(receipt["id"], 1);
    assert_eq!(receipt["printer"], "till");
    assert_eq!(receipt["dialect"], "escpos");
    assert_eq!(receipt["texts"], json!(["Hello"]));
    assert_eq!(receipt["bytes"], 10);
    assert_eq!(receipt["events"][0]["type"], "text");
    assert_eq!(receipt["events"][0]["value"]["text"], "Hello");

    let list = server.json("/api/receipts").await;
    assert_eq!(list[0]["preview"], json!(["Hello"]));
}

#[tokio::test]
async fn the_bytes_of_a_receipt_come_back_as_they_were_sent() {
    let server = Harness::start(&["name=till"]).await;
    let bytes = receipt_bytes("exact bytes \u{1b}E\u{1}bold");
    server.print("till", &bytes, 1000).await;
    server.after(0).await;
    let answer = server.get("/api/receipts/1.bin").await;
    assert_eq!(answer.status(), 200);
    assert_eq!(answer.headers()["content-type"], "application/octet-stream");
    assert_eq!(answer.bytes().await.unwrap().as_ref(), bytes.as_slice());
}

#[tokio::test]
async fn however_the_network_cuts_the_bytes_the_receipt_is_the_same() {
    let server = Harness::start(&["name=till"]).await;
    let bytes = [
        b"Total".as_slice(),
        b"\n",
        &[0x1b, b'a', 1],
        b"centred\n",
        &CUT,
    ]
    .concat();
    server.print("till", &bytes, 1000).await;
    let whole = server.after(0).await;
    for (index, piece) in [1, 2, 3, 7].into_iter().enumerate() {
        server.print("till", &bytes, piece).await;
        let mut again = server.after(index as u64 + 1).await;
        let mut expected = whole.clone();
        for receipt in [&mut again, &mut expected] {
            receipt["id"] = json!(0);
            receipt["at"] = json!(0);
        }
        assert_eq!(again, expected, "pieces of {piece}");
    }
}

#[tokio::test]
async fn a_connection_that_closes_before_the_cut_still_printed() {
    let server = Harness::start(&["name=till"]).await;
    server.print("till", b"no cut here\n", 1000).await;
    assert_eq!(server.after(0).await["texts"], json!(["no cut here"]));
}

#[tokio::test]
async fn a_receipt_that_prints_nothing_is_not_kept() {
    let server = Harness::start(&["name=till"]).await;
    server
        .print("till", &[b"\n".as_slice(), &CUT].concat(), 100)
        .await;
    server.print("till", &receipt_bytes("x"), 100).await;
    assert_eq!(server.after(0).await["id"], 1);
}

#[tokio::test]
async fn the_next_receipt_is_waited_for_and_none_gives_204() {
    let server = Harness::start(&["name=till"]).await;
    let empty = server.get("/api/receipts/next?timeout=0").await;
    assert_eq!(empty.status(), 204);

    let waiting = {
        let client = server.client.clone();
        let url = format!("{}/api/receipts/next?timeout=10", server.base);
        tokio::spawn(async move { client.get(url).send().await.unwrap() })
    };
    tokio::time::sleep(Duration::from_millis(200)).await;
    server.print("till", &receipt_bytes("late"), 100).await;
    let answer = waiting.await.unwrap();
    assert_eq!(answer.status(), 200);
    let receipt: Value = answer.json().await.unwrap();
    assert_eq!(receipt["texts"], json!(["late"]));
}

#[tokio::test]
async fn next_does_not_return_what_was_printed_before_the_call() {
    let server = Harness::start(&["name=till"]).await;
    server.print("till", &receipt_bytes("old"), 100).await;
    server.after(0).await;
    assert_eq!(
        server.get("/api/receipts/next?timeout=0").await.status(),
        204
    );
    // With the id of an earlier receipt it is returned at once.
    let again = server.get("/api/receipts/next?after=0&timeout=0").await;
    assert_eq!(again.status(), 200);
}

#[tokio::test]
async fn post_print_sends_bytes_without_a_socket_and_says_which_receipts() {
    let server = Harness::start(&["name=till", "name=bar"]).await;
    let sent = server
        .client
        .post(format!("{}/api/print?printer=bar", server.base))
        .body([receipt_bytes("a"), receipt_bytes("b")].concat())
        .send()
        .await
        .unwrap();
    assert_eq!(sent.status(), 200);
    let sent: Value = sent.json().await.unwrap();
    assert_eq!(sent["printer"], "bar");
    assert_eq!(sent["receipts"], json!([1, 2]));
    assert_eq!(server.json("/api/receipts/2").await["printer"], "bar");

    // The first printer is the default.
    let sent: Value = server
        .client
        .post(format!("{}/api/print", server.base))
        .body(receipt_bytes("c"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(sent["printer"], "till");
}

#[tokio::test]
async fn post_print_hands_back_what_the_printer_answered() {
    let server = Harness::start(&["name=till"]).await;
    let sent: Value = server
        .client
        .post(format!("{}/api/print", server.base))
        .body(vec![0x10, 0x04, 0x01])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(sent["replies"], "12");
    assert_eq!(sent["receipts"], json!([]));
}

#[tokio::test]
async fn an_unknown_printer_is_a_404_with_a_message() {
    let server = Harness::start(&["name=till"]).await;
    let answer = server
        .client
        .post(format!("{}/api/print?printer=nope", server.base))
        .body("x")
        .send()
        .await
        .unwrap();
    assert_eq!(answer.status(), 404);
    let body: Value = answer.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("nope"));
    assert_eq!(server.get("/api/printers/nope").await.status(), 404);
    assert_eq!(server.get("/api/receipts/99").await.status(), 404);
    assert_eq!(server.get("/api/receipts/abc").await.status(), 404);
    assert_eq!(server.get("/api/receipts/1.gif").await.status(), 404);
    assert_eq!(server.get("/api/receipts/99.png").await.status(), 404);
    assert_eq!(
        server
            .client
            .delete(format!("{}/api/receipts?printer=nope", server.base))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}

#[tokio::test]
async fn every_port_reads_its_own_command_language() {
    let server = Harness::start(&[
        "name=epson,dialect=escpos",
        "name=star,dialect=starline",
        "name=prnt,dialect=starprnt",
    ])
    .await;
    // ENQ asks a Star Line printer for its status. It is nothing to an ESC/POS printer, and
    // StarPRNT has no such request.
    let enq = [0x05, b'x', b'\n'];
    assert_eq!(server.print("epson", &enq, 10).await, b"");
    assert_eq!(server.print("star", &enq, 10).await, [0x00]);
    assert_eq!(server.print("prnt", &enq, 10).await, b"");
    let printers = server.json("/api/printers").await;
    let dialects: Vec<&str> = printers
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["dialect"].as_str().unwrap())
        .collect();
    assert_eq!(dialects, ["escpos", "starline", "starprnt"]);
}

#[tokio::test]
async fn a_printer_answers_a_status_request_on_the_socket_and_the_api_changes_the_answer() {
    let server = Harness::start(&["name=till"]).await;
    // DLE EOT 4 asks about the paper. 0x12 is paper present, and with the paper end the sensor
    // near the end is on too: 0x12 with bits 2, 3, 5 and 6.
    let ask = [0x10, 0x04, 0x04];
    assert_eq!(server.print("till", &ask, 10).await, [0x12]);

    let changed = server
        .client
        .post(format!("{}/api/printers/till", server.base))
        .json(&json!({"paper": "end"}))
        .send()
        .await
        .unwrap();
    assert_eq!(changed.status(), 200);
    let info: Value = changed.json().await.unwrap();
    assert_eq!(info["status"]["paper"], "end");
    assert_eq!(info["status"]["online"], true);
    assert_eq!(server.print("till", &ask, 10).await, [0x7e]);

    // Other fields stay, a fault goes away with null.
    let info: Value = server
        .client
        .post(format!("{}/api/printers/till", server.base))
        .json(&json!({"fault": "cutter"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(info["status"]["fault"], "cutter");
    assert_eq!(info["status"]["paper"], "end");
    let info: Value = server
        .client
        .post(format!("{}/api/printers/till", server.base))
        .json(&json!({"fault": null, "paper": "adequate"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(info["status"]["fault"], Value::Null);
    assert_eq!(info["status"]["paper"], "adequate");
}

#[tokio::test]
async fn a_status_that_is_not_one_is_refused() {
    let server = Harness::start(&["name=till"]).await;
    for body in [
        json!({"paper": "full"}),
        json!({"colour": "red"}),
        json!({"online": 3}),
    ] {
        let answer = server
            .client
            .post(format!("{}/api/printers/till", server.base))
            .json(&body)
            .send()
            .await
            .unwrap();
        assert!(answer.status().is_client_error(), "{body}");
    }
    assert_eq!(
        server.json("/api/printers/till").await["status"]["paper"],
        "adequate"
    );
}

#[tokio::test]
async fn a_star_printer_reports_by_itself_to_the_open_connection_when_the_api_changes_it() {
    let server = Harness::start(&["name=star,dialect=starline"]).await;
    let mut stream = TcpStream::connect(server.printer("star")).await.unwrap();
    // ESC RS a 1: tell me when the status changes.
    stream.write_all(&[0x1b, 0x1e, b'a', 1]).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let changed = server
        .client
        .post(format!("{}/api/printers/star", server.base))
        .json(&json!({"paper": "end"}))
        .send()
        .await
        .unwrap();
    assert_eq!(changed.status(), 200);
    let mut report = [0u8; 9];
    timeout(Duration::from_secs(5), stream.read_exact(&mut report))
        .await
        .expect("the status arrives")
        .unwrap();
    assert_eq!(report, [0x23, 0x06, 0x08, 0, 0, 0x0c, 0, 0, 0]);
}

#[tokio::test]
async fn the_receipts_keep_only_the_latest_and_can_be_listed_after_an_id() {
    let server = Harness::start_keeping(&["name=till"], 2).await;
    for text in ["a", "b", "c"] {
        server
            .client
            .post(format!("{}/api/print", server.base))
            .body(receipt_bytes(text))
            .send()
            .await
            .unwrap();
    }
    assert_eq!(server.ids().await, [2, 3]);
    assert_eq!(server.get("/api/receipts/1").await.status(), 404);
    let after: Value = server.json("/api/receipts?after=2").await;
    assert_eq!(after.as_array().unwrap().len(), 1);
    let last: Value = server.json("/api/receipts?limit=1").await;
    assert_eq!(last[0]["id"], 3);
}

#[tokio::test]
async fn receipts_can_be_cleared_for_one_printer_or_for_all() {
    let server = Harness::start(&["name=till", "name=bar"]).await;
    for printer in ["till", "bar", "bar"] {
        server
            .client
            .post(format!("{}/api/print?printer={printer}", server.base))
            .body(receipt_bytes("x"))
            .send()
            .await
            .unwrap();
    }
    let removed: Value = server
        .client
        .delete(format!("{}/api/receipts?printer=bar", server.base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(removed["removed"], 2);
    assert_eq!(server.ids().await, [1]);
    server
        .client
        .delete(format!("{}/api/receipts", server.base))
        .send()
        .await
        .unwrap();
    assert!(server.ids().await.is_empty());
    // Ids are not used again.
    server
        .client
        .post(format!("{}/api/print", server.base))
        .body(receipt_bytes("y"))
        .send()
        .await
        .unwrap();
    assert_eq!(server.ids().await, [4]);
}

/// A live event stream that is read as far as the wanted text each time, and no further.
struct Live {
    response: reqwest::Response,
    seen: String,
}

impl Live {
    /// Reads until the wanted text shows, and returns what came before it.
    async fn until(&mut self, wanted: &str) -> String {
        timeout(Duration::from_secs(5), async {
            while !self.seen.contains(wanted) {
                let chunk = self
                    .response
                    .chunk()
                    .await
                    .unwrap()
                    .expect("the stream goes on");
                self.seen.push_str(&String::from_utf8_lossy(&chunk));
            }
        })
        .await
        .unwrap_or_else(|_| panic!("`{wanted}` never came, only: {}", self.seen));
        let end = self.seen.find(wanted).unwrap() + wanted.len();
        self.seen.drain(..end).collect()
    }
}

#[tokio::test]
async fn the_live_events_tell_about_connections_receipts_and_status() {
    let server = Harness::start(&["name=till"]).await;
    let response = server.get("/api/events").await;
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let mut events = Live {
        response,
        seen: String::new(),
    };

    server.print("till", &receipt_bytes("live"), 100).await;
    events.until("event: connection").await;
    events.until("\"open\":true").await;
    events.until("event: receipt").await;
    events.until("\"preview\":[\"live\"]").await;
    events.until("\"open\":false").await;

    server
        .client
        .post(format!("{}/api/printers/till", server.base))
        .json(&json!({"cover_open": true}))
        .send()
        .await
        .unwrap();
    events.until("event: status").await;
    events.until("\"cover_open\":true").await;

    server
        .client
        .delete(format!("{}/api/receipts", server.base))
        .send()
        .await
        .unwrap();
    events.until("event: cleared").await;
}

#[tokio::test]
async fn the_connections_of_a_printer_are_counted() {
    let server = Harness::start(&["name=till"]).await;
    let first = TcpStream::connect(server.printer("till")).await.unwrap();
    let second = TcpStream::connect(server.printer("till")).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(server.json("/api/printers/till").await["connections"], 2);
    drop(first);
    drop(second);
    for _ in 0..50 {
        if server.json("/api/printers/till").await["connections"] == 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the connections were never counted as closed");
}

#[tokio::test]
async fn many_programs_can_print_at_once_and_none_is_lost() {
    let server = Harness::start(&["name=till"]).await;
    let address = server.printer("till");
    let mut jobs = Vec::new();
    for number in 0..30 {
        jobs.push(tokio::spawn(async move {
            let mut stream = TcpStream::connect(address).await.unwrap();
            stream
                .write_all(&receipt_bytes(&format!("job {number}")))
                .await
                .unwrap();
            stream.shutdown().await.unwrap();
            let mut rest = Vec::new();
            stream.read_to_end(&mut rest).await.unwrap();
        }));
    }
    for job in jobs {
        job.await.unwrap();
    }
    let mut seen = Vec::new();
    for _ in 0..50 {
        seen = server.ids().await;
        if seen.len() == 30 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(seen.len(), 30);
    let mut texts: Vec<String> = Vec::new();
    for id in seen {
        let receipt = server.json(&format!("/api/receipts/{id}")).await;
        texts.push(receipt["texts"][0].as_str().unwrap().to_string());
    }
    texts.sort();
    let mut wanted: Vec<String> = (0..30).map(|n| format!("job {n}")).collect();
    wanted.sort();
    assert_eq!(texts, wanted);
}

#[tokio::test]
async fn bytes_that_make_no_sense_never_stop_the_server() {
    let server = Harness::start(&["name=till", "name=star,dialect=starprnt"]).await;
    // A fixed pseudo random stream, so a failure can be repeated.
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let noise: Vec<u8> = (0..200_000)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect();
    server.print("till", &noise, 4096).await;
    server.print("star", &noise, 777).await;
    // Still working.
    let sent = server
        .client
        .post(format!("{}/api/print?printer=till", server.base))
        .body(receipt_bytes("after the noise"))
        .send()
        .await
        .unwrap();
    assert_eq!(sent.status(), 200);
    assert_eq!(server.get("/api/health").await.status(), 200);
}

#[tokio::test]
async fn unknown_commands_are_reported_in_the_receipt_with_their_place() {
    let server = Harness::start(&["name=till"]).await;
    let bytes = [b"a\n".as_slice(), &[0x1b, 0x01], b"b\n", &CUT].concat();
    server.print("till", &bytes, 100).await;
    let receipt = server.after(0).await;
    let unknown = receipt["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["type"] == "unknown")
        .expect("the unknown command is there");
    assert_eq!(unknown["value"]["offset"], 2);
    assert_eq!(unknown["value"]["bytes"], "1b01");
}

#[tokio::test]
async fn a_picture_comes_out_as_its_size_and_rows() {
    let server = Harness::start(&["name=till"]).await;
    // GS v 0: 1 byte wide, 2 rows.
    let bytes = [&[0x1d, b'v', b'0', 0, 1, 0, 2, 0][..], &[0xf0, 0x0f], &CUT].concat();
    server.print("till", &bytes, 100).await;
    let receipt = server.after(0).await;
    let image = &receipt["events"][0]["value"];
    assert_eq!(receipt["events"][0]["type"], "image");
    assert_eq!(image["width"], 8);
    assert_eq!(image["height"], 2);
    assert_eq!(image["data"], "8A8=");
}

#[tokio::test]
async fn the_models_that_can_be_chosen_are_listed_and_a_printer_uses_its_model() {
    let server = Harness::start(&["name=till,profile=TM-T88IV"]).await;
    let profiles = server.json("/api/profiles").await;
    assert!(profiles.as_array().unwrap().iter().any(|p| p == "TM-T88IV"));
    assert_eq!(
        server.json("/api/printers/till").await["profile"],
        "TM-T88IV"
    );
}

#[tokio::test]
async fn the_description_of_the_api_covers_every_route_and_the_names_events_use() {
    let server = Harness::start(&["name=till"]).await;
    let document = server.json("/api/openapi.json").await;
    assert!(document["openapi"].as_str().unwrap().starts_with('3'));

    // Every documented route answers, which is to say it is really served.
    for (path, methods) in document["paths"].as_object().unwrap() {
        let url = path
            .replace("{name}", "till")
            .replace("{id}", "1")
            .replace("/api/events", "/api/health")
            .replace("/api/receipts/next", "/api/receipts/next?timeout=0");
        for method in methods.as_object().unwrap().keys() {
            let method = reqwest::Method::from_bytes(method.to_uppercase().as_bytes()).unwrap();
            let answer = server
                .client
                .request(method.clone(), format!("{}{url}", server.base))
                .body("{}")
                .header("content-type", "application/json")
                .send()
                .await
                .unwrap();
            assert_ne!(answer.status(), 405, "{method} {path} is not served");
        }
    }

    // And what it says about the names matches what is sent.
    let symbologies = &document["components"]["schemas"]["Symbology"]["enum"];
    assert!(
        symbologies.as_array().unwrap().contains(&json!("gs1-128")),
        "{symbologies}"
    );
    assert!(document["components"]["schemas"]["Event"].is_object());
    assert!(document["components"]["schemas"]["Notice"].is_object());
}

#[tokio::test]
async fn an_address_that_is_taken_is_said_before_anything_runs() {
    let first = Harness::start(&["name=till"]).await;
    let taken = first.printer("till");
    let config = Config {
        http: ([127, 0, 0, 1], 0).into(),
        printers: vec![PrinterConfig {
            listen: taken,
            ..PrinterConfig::default()
        }],
        keep: 5,
        web: None,
    };
    let error = match serve(config).await {
        Ok(_) => panic!("the port was taken"),
        Err(error) => error,
    };
    assert!(error.to_string().contains(&taken.to_string()), "{error}");
}

#[tokio::test]
async fn stopping_closes_the_connections_and_the_ports() {
    let server = Harness::start(&["name=till"]).await;
    let address = server.printer("till");
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream.write_all(b"half a receipt\n").await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let hub = server.running.hub().clone();
    timeout(Duration::from_secs(5), server.running.stop())
        .await
        .expect("it stops");
    let mut rest = Vec::new();
    timeout(Duration::from_secs(5), stream.read_to_end(&mut rest))
        .await
        .expect("the connection is closed")
        .unwrap();
    // What was sent before is still printed.
    assert_eq!(hub.receipts(None).len(), 1);
    assert!(TcpStream::connect(address).await.is_err());
}

#[tokio::test]
async fn a_receipt_comes_as_text_and_as_a_picture_of_the_paper_of_its_printer() {
    // The model decides the paper: 42 columns of 12 dots are 504 dots.
    let server = Harness::start(&["name=till"]).await;
    let bytes = [b"\x1ba\x01Hello\n\x1ba\x00Bye\n".as_slice(), &CUT].concat();
    server.print("till", &bytes, 100).await;
    server.after(0).await;

    let text = server.get("/api/receipts/1.txt").await;
    assert_eq!(text.status(), 200);
    assert!(text.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/plain"));
    // 42 columns: "Hello" is centred at column (42 - 5) / 2 = 18.
    assert_eq!(
        text.text().await.unwrap(),
        format!("{}Hello\nBye\n", " ".repeat(18))
    );

    let picture = server.get("/api/receipts/1.png").await;
    assert_eq!(picture.status(), 200);
    assert_eq!(picture.headers()["content-type"], "image/png");
    let file = picture.bytes().await.unwrap();
    assert_eq!(&file[..8], b"\x89PNG\r\n\x1a\n");
    let size = |file: &[u8]| {
        (
            u32::from_be_bytes(file[16..20].try_into().unwrap()),
            u32::from_be_bytes(file[20..24].try_into().unwrap()),
        )
    };
    let (width, height) = size(&file);
    assert_eq!(width, 504);
    // 8 dots of edge, two lines of 33 dots, 8 dots of edge.
    assert_eq!(height, 8 + 33 + 33 + 8);

    let double = server
        .get("/api/receipts/1.png?scale=2")
        .await
        .bytes()
        .await
        .unwrap();
    assert_eq!(size(&double), (width * 2, height * 2));
}

#[tokio::test]
async fn a_receipt_can_be_read_command_by_command_with_what_the_printer_believed() {
    let server = Harness::start(&["name=till,profile=TM-T88IV"]).await;
    // Code page 2, bold, "Hi", a line feed, and the cut.
    let bytes = [b"\x1bt\x02\x1bE\x01Hi\n".as_slice(), &CUT].concat();
    server.print("till", &bytes, 100).await;
    server.after(0).await;
    let trace = server.json("/api/receipts/1/trace").await;
    let items = trace.as_array().unwrap();
    let kinds: Vec<&str> = items
        .iter()
        .map(|i| i["event"]["type"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["text", "line_feed", "cut"]);
    // The text and the line feed share the bytes from the start to the line feed, and the cut is
    // the last 4 bytes.
    let places: Vec<(u64, u64)> = items
        .iter()
        .map(|i| (i["start"].as_u64().unwrap(), i["end"].as_u64().unwrap()))
        .collect();
    assert_eq!(places, [(0, 9), (9, 9), (9, 13)]);
    assert_eq!(items[2]["index"], 2);
    let state = &items[0]["state"];
    assert_eq!(state["code_page_number"], 2);
    assert_eq!(state["code_page"], "CP850");
    assert_eq!(state["style"]["bold"], true);
    assert_eq!(state["layout"]["margin"], 0);
    assert_eq!(server.get("/api/receipts/9/trace").await.status(), 404);
}

#[tokio::test]
async fn a_receipt_can_be_looked_at_part_of_the_way_through() {
    let server = Harness::start(&["name=till"]).await;
    let bytes = [b"one\ntwo\n".as_slice(), &CUT].concat();
    server.print("till", &bytes, 100).await;
    server.after(0).await;
    // Events: text one, line feed, text two, line feed, cut.
    let text = |upto: u32| format!("/api/receipts/1.txt?upto={upto}");
    assert_eq!(server.get(&text(2)).await.text().await.unwrap(), "one\n");
    assert_eq!(
        server.get(&text(4)).await.text().await.unwrap(),
        "one\ntwo\n"
    );
    assert_eq!(server.get(&text(0)).await.text().await.unwrap().trim(), "");
    assert_eq!(
        server
            .get("/api/receipts/1.bin?upto=2")
            .await
            .bytes()
            .await
            .unwrap()
            .as_ref(),
        b"one\n"
    );
    let size = |file: &[u8]| u32::from_be_bytes(file[20..24].try_into().unwrap());
    let part = server
        .get("/api/receipts/1.png?upto=2")
        .await
        .bytes()
        .await
        .unwrap();
    let whole = server
        .get("/api/receipts/1.png")
        .await
        .bytes()
        .await
        .unwrap();
    assert!(size(&part) < size(&whole));
}

#[tokio::test]
async fn bytes_can_be_decoded_without_printing_them_and_in_any_language() {
    let server = Harness::start(&["name=till"]).await;
    let decode = |query: &str, body: Vec<u8>| {
        let request = server
            .client
            .post(format!("{}/api/decode{query}", server.base))
            .body(body);
        async move { request.send().await.unwrap() }
    };
    // ENQ is a question in Star Line mode, which is answered, and nothing in ESC/POS.
    let star: Value = decode("?dialect=starline", vec![0x05, b'x', b'\n'])
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(star["dialect"], "starline");
    assert_eq!(star["replies"], "00");
    assert_eq!(star["trace"][0]["event"]["type"], "text");
    assert_eq!(star["trace"][0]["end"], 3);
    let epson: Value = decode("", vec![0x05, b'x', b'\n'])
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(epson["replies"], "");
    assert_eq!(epson["trace"][0]["event"]["type"], "unknown");

    let two: Value = decode(
        "?profile=TM-T88IV",
        [receipt_bytes("a"), receipt_bytes("b")].concat(),
    )
    .await
    .json()
    .await
    .unwrap();
    assert_eq!(two["receipts"], 2);
    assert_eq!(two["profile"], "TM-T88IV");

    assert_eq!(decode("?dialect=zpl", vec![]).await.status(), 400);
    assert_eq!(decode("?profile=nope", vec![]).await.status(), 400);
    // Nothing was kept.
    assert!(server.ids().await.is_empty());
}

#[tokio::test]
async fn every_connection_is_remembered_with_its_pieces_and_its_receipts() {
    let server = Harness::start(&["name=till", "name=bar"]).await;
    // Two pieces, which the printer has to be given time to read separately.
    let mut stream = TcpStream::connect(server.printer("till")).await.unwrap();
    stream.write_all(b"a\n").await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    stream.write_all(&CUT).await.unwrap();
    stream.shutdown().await.unwrap();
    server
        .client
        .post(format!("{}/api/print?printer=bar", server.base))
        .body(receipt_bytes("b"))
        .send()
        .await
        .unwrap();
    // The socket's close is seen by the server a moment later.
    let mut all = Value::Null;
    for _ in 0..50 {
        all = server.json("/api/connections").await;
        if all.as_array().unwrap().len() == 2 && all[0]["closed_at"] != Value::Null {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let first = &all[0];
    // "a\n" and the cut are 6 bytes, in the two pieces they were sent in.
    assert_eq!(
        (first["printer"].as_str(), first["bytes_in"].as_u64()),
        (Some("till"), Some(6))
    );
    let sizes: Vec<u64> = first["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["bytes"].as_u64().unwrap())
        .collect();
    assert_eq!(sizes, [2, 4]);
    assert_eq!(first["receipts"], json!([1]));
    assert!(first["opened_at"].as_u64().unwrap() <= first["closed_at"].as_u64().unwrap());
    assert_eq!(all[1]["peer"], "http");
    assert_eq!(all[1]["receipts"], json!([2]));

    let only: Value = server.json("/api/connections?printer=bar").await;
    assert_eq!(only.as_array().unwrap().len(), 1);
    assert_eq!(server.json("/api/connections/2").await["printer"], "bar");
    assert_eq!(server.get("/api/connections/99").await.status(), 404);
}

#[tokio::test]
async fn a_status_answer_is_counted_in_the_bytes_the_printer_sent() {
    let server = Harness::start(&["name=till"]).await;
    server.print("till", &[0x10, 0x04, 0x01], 10).await;
    for _ in 0..50 {
        if server.json("/api/connections/1").await["bytes_out"] == 1 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the answer was never counted");
}

#[tokio::test]
async fn each_command_is_told_as_it_is_read_to_those_who_ask_and_costs_nothing_to_the_rest() {
    let server = Harness::start(&["name=till"]).await;
    let hub = server.running.hub().clone();
    assert!(!hub.actions_wanted());

    let plain = server.get("/api/events").await;
    assert!(
        !hub.actions_wanted(),
        "a plain watcher does not ask for commands"
    );
    let mut plain = Live {
        response: plain,
        seen: String::new(),
    };

    let response = server.get("/api/events?detail=actions").await;
    let mut actions = Live {
        response,
        seen: String::new(),
    };
    assert!(hub.actions_wanted());

    // Half of a line: nothing is told until its line feed arrives.
    let mut stream = TcpStream::connect(server.printer("till")).await.unwrap();
    stream.write_all(b"half").await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    stream.write_all(b" a line\n").await.unwrap();
    let before = actions.until("event: action").await;
    assert!(before.contains("event: connection"));
    let told = actions.until("\"end\":").await;
    let _ = told;
    actions.until("\"half a line\"").await;

    // The one who did not ask has the receipt and the connection, and no commands.
    stream.write_all(&CUT).await.unwrap();
    let seen = plain.until("event: receipt").await;
    assert!(!seen.contains("event: action"), "{seen}");
    actions.until("event: receipt").await;

    // When the watcher goes away the commands stop being made.
    drop(actions);
    for _ in 0..50 {
        if !hub.actions_wanted() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("commands are still made for nobody");
}

#[tokio::test]
async fn the_built_page_is_served_with_the_api_and_an_unknown_route_of_the_api_is_not_the_page() {
    let folder = std::env::temp_dir().join(format!("stubprint-web-{}", std::process::id()));
    std::fs::create_dir_all(folder.join("assets")).unwrap();
    std::fs::write(folder.join("index.html"), "<title>Stubprint</title>").unwrap();
    std::fs::write(folder.join("assets/app.js"), "console.log(1)").unwrap();
    let server = Harness::start_with(&["name=till"], 10, Some(folder.clone())).await;

    let page = server.get("/").await;
    assert_eq!(page.status(), 200);
    assert!(page.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/html"));
    assert!(page.text().await.unwrap().contains("Stubprint"));
    let script = server.get("/assets/app.js").await;
    assert_eq!(script.status(), 200);
    assert!(script.headers()["content-type"]
        .to_str()
        .unwrap()
        .contains("javascript"));
    // A route of the page, which the page itself finds in the address, is the page.
    assert!(server
        .get("/receipts/3")
        .await
        .text()
        .await
        .unwrap()
        .contains("Stubprint"));

    // The API keeps its own answers.
    assert_eq!(server.get("/api/health").await.status(), 200);
    let missing = server.get("/api/nothing/here").await;
    assert_eq!(missing.status(), 404);
    let body: Value = missing.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("no such route"));
    std::fs::remove_dir_all(folder).unwrap();
}

#[tokio::test]
async fn without_a_page_the_server_has_only_the_api() {
    let server = Harness::start(&["name=till"]).await;
    assert_eq!(server.get("/").await.status(), 404);
    assert_eq!(server.get("/api/nothing").await.status(), 404);
}

/// Everything the description of the API says about is described, so reading it is enough.
fn undescribed(document: &Value) -> Vec<String> {
    let mut missing = Vec::new();
    let described = |value: &Value| {
        value["description"]
            .as_str()
            .is_some_and(|d| !d.trim().is_empty())
    };
    for (path, methods) in document["paths"].as_object().unwrap() {
        for (method, operation) in methods.as_object().unwrap() {
            let name = format!("{} {path}", method.to_uppercase());
            if !described(operation) && operation["summary"].as_str().is_none_or(str::is_empty) {
                missing.push(format!("{name} says nothing about itself"));
            }
            if operation["tags"].as_array().is_none_or(Vec::is_empty) {
                missing.push(format!("{name} has no tag"));
            }
            for parameter in operation["parameters"].as_array().into_iter().flatten() {
                if !described(parameter) {
                    missing.push(format!("{name}: parameter {}", parameter["name"]));
                }
            }
            for (status, response) in operation["responses"].as_object().into_iter().flatten() {
                if !described(response) {
                    missing.push(format!("{name}: response {status}"));
                }
            }
        }
    }
    for (schema, definition) in document["components"]["schemas"].as_object().unwrap() {
        for (property, value) in definition["properties"].as_object().into_iter().flatten() {
            // A field that is another type is described inside its `$ref` or its `oneOf`.
            let inside = ["oneOf", "anyOf", "allOf"].iter().any(|key| {
                value[*key]
                    .as_array()
                    .is_some_and(|parts| parts.iter().any(&described))
            });
            let reference = value.get("$ref").is_some();
            if !reference && !inside && !described(value) {
                missing.push(format!("{schema}.{property}"));
            }
        }
    }
    missing
}

#[tokio::test]
async fn the_description_of_the_api_describes_every_route_parameter_and_field() {
    let server = Harness::start(&["name=till"]).await;
    let document = server.json("/api/openapi.json").await;
    let missing = undescribed(&document);
    assert!(
        missing.is_empty(),
        "{} undescribed:\n{}",
        missing.len(),
        missing.join("\n")
    );
}

#[tokio::test]
async fn the_entry_point_says_what_this_is_and_where_its_description_is() {
    let server = Harness::start(&["name=till", "name=star,dialect=starline"]).await;
    let index = server.json("/api").await;
    assert_eq!(index["name"], "stubprint");
    assert_eq!(index["openapi"], "/api/openapi.json");
    assert_eq!(index["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(index["printers"][1]["dialect"], "starline");
    assert_eq!(
        server
            .get(index["openapi"].as_str().unwrap())
            .await
            .status(),
        200
    );
}
