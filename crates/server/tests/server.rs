//! The server is started for real and driven the way a program and a person would: bytes over TCP,
//! then the REST API over HTTP.

use std::net::SocketAddr;
use std::time::Duration;

use receipt_server::{serve, Config, PrinterConfig, Running};
use serde_json::{json, Value};
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
        let printers: Vec<PrinterConfig> = printers
            .iter()
            .map(|text| PrinterConfig::parse(&format!("listen=127.0.0.1:0,{text}")).unwrap())
            .collect();
        let config = Config {
            http: ([127, 0, 0, 1], 0).into(),
            printers,
            keep,
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
    async fn until(&mut self, wanted: &str) {
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
        self.seen.drain(..end);
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
