//! The server is started for real and driven the way a program and a person would: bytes over TCP,
//! then the REST API over HTTP.

use std::net::SocketAddr;
use std::time::Duration;

use serde_json::{json, Value};
use stubprint_server::{has_page, serve, Config, PrinterConfig, Running};
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
        let printers: Vec<PrinterConfig> = printers
            .iter()
            .map(|text| PrinterConfig::parse(&format!("listen=127.0.0.1:0,{text}")).unwrap())
            .collect();
        let config = Config {
            http: Some(([127, 0, 0, 1], 0).into()),
            printers,
            ready: None,
            database: None,
            reset: false,
            given: false,
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
    let answer = server.get("/api/receipts/1/bytes").await;
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
    // An id that is not a number is a request that cannot be right, not a receipt that is missing.
    assert_eq!(server.get("/api/receipts/abc").await.status(), 400);
    assert_eq!(server.get("/api/receipts/99/picture").await.status(), 404);
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
async fn the_receipts_can_be_listed_after_an_id_or_as_the_latest() {
    let server = Harness::start(&["name=till"]).await;
    for text in ["a", "b", "c"] {
        server
            .client
            .post(format!("{}/api/print", server.base))
            .body(receipt_bytes(text))
            .send()
            .await
            .unwrap();
    }
    assert_eq!(server.ids().await, [1, 2, 3]);
    let after: Value = server.json("/api/receipts?after=2").await;
    assert_eq!(after.as_array().unwrap().len(), 1);
    let last: Value = server.json("/api/receipts?limit=1").await;
    assert_eq!(last[0]["id"], 3);
}

#[tokio::test]
async fn the_network_lists_addresses_that_can_be_connected_to() {
    let server = Harness::start(&["name=till"]).await;
    let network: Value = server
        .client
        .get(format!("{}/api/network", server.base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    for entry in network["addresses"].as_array().unwrap() {
        let address: std::net::Ipv4Addr = entry["address"].as_str().unwrap().parse().unwrap();
        assert!(!address.is_loopback() && !address.is_link_local());
        assert!(!entry["name"].as_str().unwrap().is_empty());
    }
}

#[tokio::test]
async fn one_receipt_can_be_removed() {
    let server = Harness::start(&["name=till"]).await;
    for _ in 0..2 {
        server
            .client
            .post(format!("{}/api/print", server.base))
            .body(receipt_bytes("x"))
            .send()
            .await
            .unwrap();
    }
    let url = format!("{}/api/receipts/1", server.base);
    let removed = server.client.delete(&url).send().await.unwrap();
    assert_eq!(removed.status(), 204);
    assert_eq!(server.ids().await, [2]);
    let again = server.client.delete(&url).send().await.unwrap();
    assert_eq!(again.status(), 404);
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
async fn an_open_connection_is_told_as_its_bytes_arrive_and_its_receipt_is_printed() {
    let server = Harness::start(&["name=till"]).await;
    let response = server.get("/api/events").await;
    let mut events = Live {
        response,
        seen: String::new(),
    };

    // The connection stays open the whole time, so every change is told while it is open.
    let mut stream = TcpStream::connect(server.printer("till")).await.unwrap();
    events.until("\"open\":true").await;
    stream.write_all(b"ab\n").await.unwrap();
    events.until("\"bytes_in\":3").await;
    events.until("\"pieces\":1").await;
    events.until("\"chunk\":{\"at\":").await;
    stream.write_all(&CUT).await.unwrap();
    events.until("\"bytes_in\":7").await;
    events.until("\"pieces\":2").await;
    events.until("\"receipts\":[1]").await;
    // And it is still open until the program closes it.
    stream.shutdown().await.unwrap();
    events.until("\"open\":false").await;
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
    let epson = profiles
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "TM-T88IV")
        .expect("TM-T88IV is listed");
    assert_eq!(epson["width_dots"], 512);
    assert_eq!(epson["width_mm"], 80.0);
    assert_eq!(epson["dpi"], 180);
    assert!(epson["manuals"][0].as_str().unwrap().contains("tm-t88iv"));
    assert_eq!(epson["columns"], json!([42, 56]));
    assert!(epson["features"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f == "qrCode"));
    assert!(epson["code_pages"].as_u64().unwrap() >= 5);
    assert_eq!(
        server.json("/api/printers/till").await["profile"],
        "TM-T88IV"
    );
}

#[tokio::test]
async fn a_sample_comes_as_a_picture_on_the_paper_of_the_model_it_was_written_for() {
    let server = Harness::start(&["name=till"]).await;
    let width = |file: &[u8]| u32::from_be_bytes(file[16..20].try_into().unwrap());
    // The grocery receipt is for TM-T20III, 576 dots, and the bakery for POS-5890, 384.
    for (id, dots) in [("grocery", 576), ("bakery", 384)] {
        let answer = server.get(&format!("/api/samples/{id}/picture")).await;
        assert_eq!(answer.status(), 200, "{id}");
        assert_eq!(answer.headers()["content-type"], "image/png");
        let file = answer.bytes().await.unwrap();
        assert_eq!(&file[..8], b"\x89PNG\r\n\x1a\n", "{id}");
        assert_eq!(width(&file), dots, "{id}");
    }
    // The same paper as text, and nothing was printed to get either.
    let text = server.get("/api/samples/grocery/text").await;
    assert_eq!(text.status(), 200);
    assert!(text.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/plain"));
    let first = server.json("/api/samples/grocery").await["expect"]["texts"][0]
        .as_str()
        .unwrap()
        .trim()
        .to_string();
    assert!(text.text().await.unwrap().contains(&first));
    assert!(server.ids().await.is_empty());
    assert_eq!(server.get("/api/samples/nope/picture").await.status(), 404);
    assert_eq!(
        server
            .get("/api/samples/grocery/picture?library=nobody")
            .await
            .status(),
        404
    );
}

#[tokio::test]
async fn a_printer_can_be_set_to_another_model_and_each_receipt_keeps_the_one_it_was_printed_on() {
    let server = Harness::start(&["name=till,profile=TM-T88IV"]).await;
    server.print("till", &receipt_bytes("one"), 64).await;
    server.after(0).await;

    let patch = |body: Value| {
        server
            .client
            .patch(format!("{}/api/printers/till", server.base))
            .json(&body)
    };
    let answer = patch(json!({"profile": "TM-T20III"})).send().await.unwrap();
    assert_eq!(answer.status(), 200);
    assert_eq!(
        answer.json::<Value>().await.unwrap()["profile"],
        "TM-T20III"
    );
    server.print("till", &receipt_bytes("two"), 64).await;
    server.after(1).await;

    let width = |file: &[u8]| u32::from_be_bytes(file[16..20].try_into().unwrap());
    let mut widths = Vec::new();
    for (id, model) in [(1, "TM-T88IV"), (2, "TM-T20III")] {
        let summary = server.json(&format!("/api/receipts/{id}")).await;
        assert_eq!(summary["profile"], model);
        let picture = server.get(&format!("/api/receipts/{id}/picture")).await;
        widths.push(width(&picture.bytes().await.unwrap()));
    }
    // What each model prints: 512 dots, then 576.
    assert_eq!(widths, [512, 576]);

    for body in [json!({"profile": "nope"}), json!({"dialect": "starline"})] {
        let answer = patch(body.clone()).send().await.unwrap();
        assert!(answer.status().is_client_error(), "{body}");
    }
    assert_eq!(
        server.json("/api/printers/till").await["profile"],
        "TM-T20III"
    );
    let missing = server
        .client
        .patch(format!("{}/api/printers/nowhere", server.base))
        .json(&json!({"profile": "TM-T20III"}))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 404);
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
        http: Some(([127, 0, 0, 1], 0).into()),
        printers: vec![PrinterConfig {
            listen: taken,
            ..PrinterConfig::default()
        }],
        ready: None,
        database: None,
        reset: false,
        given: false,
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

    let text = server.get("/api/receipts/1/text").await;
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

    let picture = server.get("/api/receipts/1/picture").await;
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
        .get("/api/receipts/1/picture?scale=2")
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
    let text = |upto: u32| format!("/api/receipts/1/text?upto={upto}");
    assert_eq!(server.get(&text(2)).await.text().await.unwrap(), "one\n");
    assert_eq!(
        server.get(&text(4)).await.text().await.unwrap(),
        "one\ntwo\n"
    );
    assert_eq!(server.get(&text(0)).await.text().await.unwrap().trim(), "");
    assert_eq!(
        server
            .get("/api/receipts/1/bytes?upto=2")
            .await
            .bytes()
            .await
            .unwrap()
            .as_ref(),
        b"one\n"
    );
    let size = |file: &[u8]| u32::from_be_bytes(file[20..24].try_into().unwrap());
    let part = server
        .get("/api/receipts/1/picture?upto=2")
        .await
        .bytes()
        .await
        .unwrap();
    let whole = server
        .get("/api/receipts/1/picture")
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
async fn the_page_in_the_program_is_served_when_it_was_built_in() {
    let server = Harness::start(&["name=till"]).await;
    assert_eq!(server.get("/api/health").await.status(), 200);
    // The check of the page's build sets this to make sure the page really is in the program.
    if std::env::var_os("STUBPRINT_REQUIRE_PAGE").is_some() {
        assert!(has_page(), "the program was built without the page");
    }
    if !has_page() {
        assert_eq!(server.get("/").await.status(), 404);
        return;
    }
    let page = server.get("/").await;
    assert_eq!(page.status(), 200);
    assert!(page.text().await.unwrap().contains("<div id=\"root\">"));
    assert_eq!(server.get("/printers").await.status(), 200);
    assert_eq!(server.get("/assets/nothing.js").await.status(), 404);
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

impl Harness {
    async fn post_empty(&self, path: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{path}", self.base))
            .send()
            .await
            .unwrap()
    }

    async fn post_json(&self, path: &str, body: Value) -> reqwest::Response {
        self.client
            .post(format!("{}{path}", self.base))
            .json(&body)
            .send()
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn a_printer_can_be_added_while_the_server_runs_and_prints_at_once() {
    let server = Harness::start(&["name=till"]).await;
    let added = server
        .post_json(
            "/api/printers",
            json!({"name": "bar", "listen": "127.0.0.1:0", "dialect": "starline", "profile": "SP2000"}),
        )
        .await;
    assert_eq!(added.status(), 201);
    let info: Value = added.json().await.unwrap();
    assert_eq!(
        (info["name"].as_str(), info["dialect"].as_str()),
        (Some("bar"), Some("starline"))
    );
    assert_eq!(info["profile"], "SP2000");
    // Port 0 picked a free port, which the answer tells.
    let address: SocketAddr = info["listen"].as_str().unwrap().parse().unwrap();
    assert_ne!(address.port(), 0);

    // ENQ is a Star Line question, so this port is Star Line mode and answers it.
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream.write_all(&[0x05, b'x', b'\n']).await.unwrap();
    stream.shutdown().await.unwrap();
    let mut answer = Vec::new();
    timeout(Duration::from_secs(5), stream.read_to_end(&mut answer))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(answer, [0x00]);

    let names: Vec<String> = server
        .json("/api/printers")
        .await
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, ["till", "bar"]);
}

#[tokio::test]
async fn a_printer_without_a_port_gets_a_free_one_and_a_taken_name_gets_a_number() {
    let server = Harness::start(&["name=till,listen=127.0.0.1:0"]).await;
    let mut seen = Vec::new();
    // Nothing is said: the name is `printer` and the port is any that is free, again and again.
    for (body, name) in [
        (json!({}), "printer"),
        (json!({}), "printer_2"),
        (json!({"name": "till"}), "till_2"),
        (json!({"name": "till"}), "till_3"),
        (json!({"name": "printer_2"}), "printer_2_2"),
    ] {
        let answer = server.post_json("/api/printers", body.clone()).await;
        assert_eq!(answer.status(), 201, "{body}");
        let info: Value = answer.json().await.unwrap();
        assert_eq!(info["name"], name, "{body}");
        let address: SocketAddr = info["listen"].as_str().unwrap().parse().unwrap();
        assert_ne!(address.port(), 0);
        assert!(!seen.contains(&address), "{address} was given twice");
        seen.push(address);
    }
}

#[tokio::test]
async fn a_printer_that_cannot_be_added_says_why() {
    let server = Harness::start(&["name=till"]).await;
    let taken = server.printer("till");
    let cases = [
        (
            json!({"name": "a/b", "listen": "127.0.0.1:0"}),
            400,
            "letters",
        ),
        (
            json!({"name": "x", "listen": "127.0.0.1:0", "dialect": "zpl"}),
            400,
            "dialect",
        ),
        (
            json!({"name": "x", "listen": "127.0.0.1:0", "profile": "nope"}),
            400,
            "profile",
        ),
        (json!({"name": "x", "listen": "somewhere"}), 400, "address"),
        (
            json!({"name": "x", "listen": taken.to_string()}),
            409,
            "cannot listen",
        ),
        (json!({"name": "x", "colour": "red"}), 422, ""),
    ];
    for (body, status, about) in cases {
        let answer = server.post_json("/api/printers", body.clone()).await;
        assert_eq!(answer.status(), status, "{body}");
        if !about.is_empty() {
            let message: Value = answer.json().await.unwrap();
            assert!(
                message["error"].as_str().unwrap().contains(about),
                "{body}: {message}"
            );
        }
    }
    // None of them was added.
    assert_eq!(
        server.json("/api/printers").await.as_array().unwrap().len(),
        1
    );
}

#[tokio::test]
async fn a_printer_can_be_removed_and_its_receipts_stay() {
    let server = Harness::start(&["name=till", "name=bar"]).await;
    let bar = server.printer("bar");
    server.print("bar", &receipt_bytes("kept"), 100).await;
    server.after(0).await;

    // A program that is connected when the printer goes is disconnected.
    let mut open = TcpStream::connect(bar).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let removed = server
        .client
        .delete(format!("{}/api/printers/bar", server.base))
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), 204);
    let mut rest = Vec::new();
    timeout(Duration::from_secs(5), open.read_to_end(&mut rest))
        .await
        .expect("the connection is closed")
        .unwrap();
    assert!(TcpStream::connect(bar).await.is_err(), "the port is closed");

    assert_eq!(server.get("/api/printers/bar").await.status(), 404);
    assert_eq!(server.json("/api/receipts/1").await["printer"], "bar");
    let again = server
        .client
        .delete(format!("{}/api/printers/bar", server.base))
        .send()
        .await
        .unwrap();
    assert_eq!(again.status(), 404);

    // The name and the port can be used again.
    let back = server
        .post_json(
            "/api/printers",
            json!({"name": "bar", "listen": bar.to_string()}),
        )
        .await;
    assert_eq!(back.status(), 201);
}

#[tokio::test]
async fn with_no_printer_there_is_nothing_to_print_to_and_the_rest_still_works() {
    let server = Harness::start(&["name=till"]).await;
    server
        .client
        .delete(format!("{}/api/printers/till", server.base))
        .send()
        .await
        .unwrap();
    assert_eq!(server.json("/api/printers").await, json!([]));
    let print = server
        .client
        .post(format!("{}/api/print", server.base))
        .body("x")
        .send()
        .await
        .unwrap();
    assert_eq!(print.status(), 400);
    assert_eq!(server.get("/api/health").await.status(), 200);
}

#[tokio::test]
async fn the_live_events_tell_when_a_printer_is_added_or_removed() {
    let server = Harness::start(&["name=till"]).await;
    let response = server.get("/api/events").await;
    let mut events = Live {
        response,
        seen: String::new(),
    };
    server
        .post_json(
            "/api/printers",
            json!({"name": "bar", "listen": "127.0.0.1:0"}),
        )
        .await;
    events.until("event: printer_added").await;
    events.until("\"name\":\"bar\"").await;
    server
        .client
        .delete(format!("{}/api/printers/bar", server.base))
        .send()
        .await
        .unwrap();
    events.until("event: printer_removed").await;
}

#[tokio::test]
async fn the_description_can_be_printed_without_starting_a_server_and_is_the_one_served() {
    let server = Harness::start(&["name=till"]).await;
    let served = server.json("/api/openapi.json").await;
    let printed: Value = serde_json::to_value(stubprint_server::api::openapi()).unwrap();
    assert_eq!(printed, served);
}

const NO_SAMPLES: &str = "no samples are built: run `npm ci` in samples/node and then \
    `python samples/python/build.py`, then build again";

#[tokio::test]
async fn the_samples_are_listed_with_every_library_that_wrote_them() {
    let server = Harness::start(&["name=till"]).await;
    let list = server.json("/api/samples").await;
    let list = list.as_array().unwrap();
    assert!(!list.is_empty(), "{NO_SAMPLES}");
    let grocery = list.iter().find(|s| s["id"] == "grocery").unwrap();
    let libraries: Vec<&str> = grocery["variants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["library"].as_str().unwrap())
        .collect();
    assert_eq!(libraries, ["python-escpos", "node-thermal-printer"]);
    assert_eq!(grocery["variants"][0]["language"], "python");
    assert_eq!(grocery["variants"][0]["dialect"], "escpos");
    assert!(!grocery["variants"][0]["version"]
        .as_str()
        .unwrap()
        .is_empty());
    assert_eq!(grocery["profile"], "TM-T20III");
    assert!(
        grocery.get("receipt").is_none(),
        "a list does not carry the receipts"
    );

    let detail = server.json("/api/samples/grocery").await;
    assert_eq!(detail["receipt"]["id"], "grocery");
    assert!(detail["expect"]["texts"][0] == "Green Market");
    assert_eq!(detail["expect"]["cuts"], 1);
}

#[tokio::test]
async fn the_bytes_of_a_sample_come_for_the_variant_asked_for() {
    let server = Harness::start(&["name=till"]).await;
    assert!(
        !server
            .json("/api/samples")
            .await
            .as_array()
            .unwrap()
            .is_empty(),
        "{NO_SAMPLES}"
    );
    let first = server.get("/api/samples/grocery/bytes").await;
    assert_eq!(first.status(), 200);
    assert_eq!(first.headers()["content-type"], "application/octet-stream");
    let first = first.bytes().await.unwrap();
    let node = server
        .get("/api/samples/grocery/bytes?library=node-thermal-printer")
        .await
        .bytes()
        .await
        .unwrap();
    assert_ne!(
        first, node,
        "two libraries write the same receipt differently"
    );
    let sizes = server.json("/api/samples/grocery").await;
    assert_eq!(
        first.len() as u64,
        sizes["variants"][0]["bytes"].as_u64().unwrap()
    );
    assert_eq!(
        node.len() as u64,
        sizes["variants"][1]["bytes"].as_u64().unwrap()
    );

    for path in [
        "/api/samples/nothing/bytes",
        "/api/samples/grocery.bin",
        "/api/samples/grocery/bytes?library=nothing",
        "/api/samples/grocery/bytes?dialect=starline",
        "/api/samples/grocery/bytes?dialect=nothing",
    ] {
        assert_eq!(server.get(path).await.status(), 404, "{path}");
    }
}

#[tokio::test]
async fn a_sample_is_printed_in_the_language_of_the_printer() {
    let server = Harness::start(&["name=epson,dialect=escpos", "name=star,dialect=starline"]).await;
    assert!(
        !server
            .json("/api/samples")
            .await
            .as_array()
            .unwrap()
            .is_empty(),
        "{NO_SAMPLES}"
    );
    let made = server
        .post_empty("/api/samples/bookshop/print?printer=star")
        .await;
    assert_eq!(made.status(), 200);
    let made: Value = made.json().await.unwrap();
    let id = made["receipts"][0].as_u64().unwrap();
    let receipt = server.json(&format!("/api/receipts/{id}")).await;
    assert_eq!(receipt["dialect"], "starline");
    assert!(receipt["texts"][0]
        .as_str()
        .unwrap()
        .contains("Harbour Books"));

    for path in [
        "/api/samples/bookshop/print?printer=epson",
        "/api/samples/nothing/print?printer=epson",
        "/api/samples/grocery/print?printer=back",
        "/api/samples/grocery/print?printer=epson&library=nothing",
    ] {
        assert_eq!(server.post_empty(path).await.status(), 404, "{path}");
    }
    let chosen = server
        .post_empty("/api/samples/grocery/print?printer=epson&library=node-thermal-printer")
        .await;
    assert_eq!(chosen.status(), 200);
    let chosen: Value = chosen.json().await.unwrap();
    let id = chosen["receipts"][0].as_u64().unwrap();
    let sent = server
        .get("/api/samples/grocery/bytes?library=node-thermal-printer")
        .await
        .bytes()
        .await
        .unwrap();
    let kept = server
        .get(&format!("/api/receipts/{id}/bytes"))
        .await
        .bytes()
        .await
        .unwrap();
    // the library adds a reset after the cut
    assert_eq!(sent.strip_prefix(kept.as_ref()), Some(b"\x1b@".as_slice()));
}

fn lines_of(receipt: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for event in receipt["events"].as_array().unwrap() {
        match event["type"].as_str().unwrap() {
            "text" => line.push_str(event["value"]["text"].as_str().unwrap()),
            "line_feed" => lines.push(std::mem::take(&mut line)),
            _ => {}
        }
    }
    lines.push(line);
    lines.retain(|line| !line.is_empty());
    lines
}

#[tokio::test]
async fn every_variant_of_every_sample_reads_as_the_receipt_says() {
    let server = Harness::start(&["name=epson,dialect=escpos", "name=star,dialect=starline"]).await;
    let list = server.json("/api/samples").await;
    assert!(!list.as_array().unwrap().is_empty(), "{NO_SAMPLES}");
    for sample in list.as_array().unwrap() {
        let id = sample["id"].as_str().unwrap();
        let expect = server.json(&format!("/api/samples/{id}")).await["expect"].clone();
        for variant in sample["variants"].as_array().unwrap() {
            let (library, dialect) = (
                variant["library"].as_str().unwrap(),
                variant["dialect"].as_str().unwrap(),
            );
            let what = format!("{id} by {library} in {dialect}");
            let printer = if dialect == "starline" {
                "star"
            } else {
                "epson"
            };
            let made = server
                .post_empty(&format!(
                    "/api/samples/{id}/print?printer={printer}&library={library}"
                ))
                .await;
            let made: Value = made.json().await.unwrap();
            let receipts = made["receipts"].as_array().unwrap();
            assert_eq!(receipts.len(), 1, "{what} makes one receipt");
            let receipt = server
                .json(&format!("/api/receipts/{}", receipts[0].as_u64().unwrap()))
                .await;
            let events = receipt["events"].as_array().unwrap();
            let of = |kind: &str| -> Vec<&Value> {
                events
                    .iter()
                    .filter(|e| e["type"] == kind)
                    .map(|e| &e["value"])
                    .collect()
            };
            assert!(
                of("unknown").is_empty(),
                "{what} has commands nobody reads: {:?}",
                of("unknown")
            );
            assert_eq!(
                json!(lines_of(&receipt)),
                expect["texts"],
                "{what}: the lines"
            );
            let codes = |kind: &str, key: &str| -> Vec<Value> {
                of(kind).iter().map(|v| v[key].clone()).collect()
            };
            let wanted = |key: &str, inner: &str| -> Vec<Value> {
                expect[key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v[inner].clone())
                    .collect()
            };
            assert_eq!(
                codes("barcode", "data"),
                wanted("barcodes", "data"),
                "{what}: the barcodes"
            );
            assert_eq!(
                codes("barcode", "symbology"),
                wanted("barcodes", "symbology"),
                "{what}: the barcode kinds"
            );
            assert_eq!(
                codes("qr", "data"),
                wanted("qrs", "data"),
                "{what}: the QR codes"
            );
            assert_eq!(
                codes("pdf417", "data"),
                wanted("pdf417", "data"),
                "{what}: the PDF417 codes"
            );
            assert_eq!(
                of("cut").len() as u64,
                expect["cuts"].as_u64().unwrap(),
                "{what}: the cuts"
            );
            assert_eq!(
                !of("image").is_empty(),
                expect["images"].as_u64().unwrap() > 0,
                "{what}: the picture"
            );
        }
    }
}

#[tokio::test]
async fn the_version_of_the_api_is_the_version_of_the_program() {
    let server = Harness::start(&["name=till"]).await;
    let version = env!("CARGO_PKG_VERSION");
    assert_eq!(server.json("/api").await["version"], version);
    assert_eq!(
        server.json("/api/openapi.json").await["info"]["version"],
        version
    );
}

#[tokio::test]
async fn the_description_puts_each_parameter_where_it_belongs() {
    let server = Harness::start(&["name=till"]).await;
    let document = server.json("/api/openapi.json").await;
    let parameter = |path: &str, method: &str, name: &str| {
        document["paths"][path][method]["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == name)
            .cloned()
            .unwrap_or_else(|| panic!("{method} {path} has no parameter {name}"))
    };
    for (path, method, name) in [
        ("/api/receipts", "get", "limit"),
        ("/api/receipts/next", "get", "timeout"),
        ("/api/receipts/{id}/picture", "get", "scale"),
        ("/api/receipts/{id}/text", "get", "upto"),
        ("/api/print", "post", "printer"),
        ("/api/samples/{id}/print", "post", "library"),
    ] {
        let found = parameter(path, method, name);
        assert_eq!(found["in"], "query", "{path} {name}");
        assert_eq!(found["required"], false, "{path} {name}");
    }
    assert_eq!(parameter("/api/receipts/{id}", "get", "id")["in"], "path");
}

#[tokio::test]
async fn the_ready_file_says_where_a_server_on_port_zero_listens_and_goes_with_it() {
    let path = std::env::temp_dir().join(format!("stubprint-ready-{}.json", std::process::id()));
    let config = Config {
        http: Some(([127, 0, 0, 1], 0).into()),
        printers: vec![PrinterConfig::parse("name=front,listen=127.0.0.1:0").unwrap()],
        ready: Some(path.clone()),
        ..Config::default()
    };
    let running = serve(config).await.unwrap();

    let ready: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(ready["pid"], std::process::id());
    assert_eq!(ready["http"], running.http_addr().to_string());
    assert_eq!(ready["printers"][0]["name"], "front");
    assert_eq!(
        ready["printers"][0]["listen"],
        running.printer_addr("front").unwrap().to_string()
    );
    assert_ne!(running.http_addr().port(), 0);

    running.stop().await;
    assert!(!path.exists());
}

#[tokio::test]
async fn a_ready_file_that_cannot_be_written_stops_the_server_from_starting() {
    let config = Config {
        http: Some(([127, 0, 0, 1], 0).into()),
        printers: vec![PrinterConfig::parse("listen=127.0.0.1:0").unwrap()],
        ready: Some("/nowhere/at/all/ready.json".into()),
        ..Config::default()
    };
    let error = serve(config).await.err().expect("it should not start");
    assert!(error.to_string().contains("cannot write"), "{error}");
}

#[tokio::test]
async fn a_printer_that_is_stopped_right_after_it_started_stops() {
    let running = serve(Config {
        http: Some(([127, 0, 0, 1], 0).into()),
        printers: vec![PrinterConfig::parse("listen=127.0.0.1:0").unwrap()],
        ..Config::default()
    })
    .await
    .unwrap();
    timeout(Duration::from_secs(5), running.stop())
        .await
        .expect("it should stop");
}

#[tokio::test]
async fn the_server_stops_while_a_page_is_watching_the_live_events() {
    let server = Harness::start(&["name=till"]).await;
    let _watching = server.get("/api/events").await;
    timeout(Duration::from_secs(10), server.running.stop())
        .await
        .expect("it should stop with the stream open");
}

#[tokio::test]
async fn the_description_says_the_bytes_and_the_picture_are_files_and_the_text_is_text() {
    let server = Harness::start(&["name=till"]).await;
    let document = server.json("/api/openapi.json").await;
    let answer = |path: &str, kind: &str| {
        document["paths"][path]["get"]["responses"]["200"]["content"][kind]["schema"].clone()
    };
    for (path, kind) in [
        ("/api/receipts/{id}/bytes", "application/octet-stream"),
        ("/api/receipts/{id}/picture", "image/png"),
    ] {
        assert_eq!(
            answer(path, kind)["$ref"],
            "#/components/schemas/Binary",
            "{path}"
        );
    }
    let binary = &document["components"]["schemas"]["Binary"];
    assert_eq!(binary["type"], "string");
    assert_eq!(binary["format"], "binary");
    assert_eq!(
        answer("/api/receipts/{id}/text", "text/plain")["type"],
        "string"
    );
}

impl Harness {
    /// Changes what the printer reports, such as `{"cover_open": true}`.
    async fn report(&self, name: &str, change: Value) {
        let answer = self
            .post_json(&format!("/api/printers/{name}"), change)
            .await;
        assert_eq!(answer.status(), 200);
    }

    /// The connections, the oldest first as the API gives them.
    async fn connections(&self) -> Vec<Value> {
        self.json("/api/connections")
            .await
            .as_array()
            .unwrap()
            .clone()
    }

    /// Waits until a condition holds, or fails.
    async fn until<F: FnMut(&Value) -> bool>(&self, path: &str, mut holds: F) -> Value {
        for _ in 0..100 {
            let now = self.json(path).await;
            if holds(&now) {
                return now;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("{path} never became what the test waits for");
    }
}

const SETTLE: Duration = Duration::from_millis(300);

#[tokio::test]
async fn a_printer_that_holds_prints_nothing_while_its_cover_is_open_and_all_of_it_when_it_closes()
{
    let server = Harness::start(&["name=till,printer_buffer=4096"]).await;
    server.report("till", json!({"cover_open": true})).await;

    let mut stream = TcpStream::connect(server.printer("till")).await.unwrap();
    stream.write_all(&receipt_bytes("Held")).await.unwrap();
    tokio::time::sleep(SETTLE).await;
    assert!(
        server.ids().await.is_empty(),
        "nothing prints with the cover open"
    );
    let open = server.connections().await;
    assert_eq!(open[0]["bytes_in"], 9);
    assert_eq!(open[0]["receipts"], json!([]));

    server.report("till", json!({"cover_open": false})).await;
    let receipt = server.after(0).await;
    assert_eq!(receipt["texts"], json!(["Held"]));
    assert_eq!(server.connections().await[0]["receipts"], json!([1]));
}

#[tokio::test]
async fn a_printer_with_no_settings_holds_what_it_receives_while_its_cover_is_open() {
    let server = Harness::start(&["name=till"]).await;
    server.report("till", json!({"cover_open": true})).await;
    let mut stream = TcpStream::connect(server.printer("till")).await.unwrap();
    stream.write_all(&receipt_bytes("Held")).await.unwrap();
    tokio::time::sleep(SETTLE).await;
    assert!(server.ids().await.is_empty());
    server.report("till", json!({"cover_open": false})).await;
    assert_eq!(server.after(0).await["texts"], json!(["Held"]));
}

#[tokio::test]
async fn a_printer_that_holds_nothing_prints_with_its_cover_open() {
    let server = Harness::start(&["name=till,printer_buffer=0"]).await;
    server.report("till", json!({"cover_open": true})).await;
    server.print("till", &receipt_bytes("Anyway"), 100).await;
    assert_eq!(server.after(0).await["texts"], json!(["Anyway"]));
}

#[tokio::test]
async fn what_a_program_left_in_a_printer_that_cannot_print_prints_when_it_can_and_belongs_to_that_program(
) {
    let server = Harness::start(&["name=till,printer_buffer=4096"]).await;
    server.report("till", json!({"online": false})).await;
    server.print("till", &receipt_bytes("First"), 100).await;
    server.print("till", &receipt_bytes("Second"), 100).await;
    tokio::time::sleep(SETTLE).await;
    assert!(server.ids().await.is_empty());

    server.report("till", json!({"online": true})).await;
    let both = server
        .until("/api/receipts", |list| list.as_array().unwrap().len() == 2)
        .await;
    // They print in the order they were sent.
    assert_eq!(both[0]["preview"], json!(["First"]));
    assert_eq!(both[1]["preview"], json!(["Second"]));
    // Each receipt is listed on the connection that sent it.
    let connections = server.connections().await;
    assert_eq!(connections[0]["receipts"], json!([1]));
    assert_eq!(connections[1]["receipts"], json!([2]));
}

#[tokio::test]
async fn a_status_request_is_answered_at_once_by_a_printer_that_holds_and_is_not_asked_again_later()
{
    let server = Harness::start(&["name=till,printer_buffer=4096"]).await;
    server.report("till", json!({"cover_open": true})).await;
    let mut stream = TcpStream::connect(server.printer("till")).await.unwrap();
    // DLE EOT 2 says why the printer is off line: the bit of the cover.
    stream.write_all(&[0x10, 0x04, 0x02]).await.unwrap();
    let mut answer = [0u8; 1];
    timeout(Duration::from_secs(5), stream.read_exact(&mut answer))
        .await
        .expect("the printer answers while it holds")
        .unwrap();
    assert_eq!(answer, [0x16]);

    // The request is not kept: when the cover closes nothing else is answered.
    server.report("till", json!({"cover_open": false})).await;
    stream.shutdown().await.unwrap();
    let mut rest = Vec::new();
    timeout(Duration::from_secs(5), stream.read_to_end(&mut rest))
        .await
        .unwrap()
        .unwrap();
    assert!(rest.is_empty(), "{rest:?}");
}

#[tokio::test]
async fn a_program_that_sends_more_than_the_printer_holds_is_not_read_until_the_printer_can_print()
{
    let server = Harness::start(&["name=till,printer_buffer=64"]).await;
    server.report("till", json!({"paper": "end"})).await;
    let mut job = Vec::new();
    for line in 0..20 {
        job.extend_from_slice(format!("line {line:02}\n").as_bytes());
    }
    job.extend_from_slice(&CUT);
    assert!(job.len() > 100);

    let mut stream = TcpStream::connect(server.printer("till")).await.unwrap();
    stream.write_all(&job).await.unwrap();
    tokio::time::sleep(SETTLE).await;
    // Only what the buffer holds was read.
    assert_eq!(server.connections().await[0]["bytes_in"], 64);
    assert!(server.ids().await.is_empty());

    // Paper is put in: the rest is read and the whole job prints.
    server.report("till", json!({"paper": "adequate"})).await;
    let receipt = server.after(0).await;
    assert_eq!(receipt["texts"].as_array().unwrap().len(), 20);
    server
        .until("/api/connections", |list| list[0]["bytes_in"] == job.len())
        .await;
}

#[tokio::test]
async fn a_request_that_does_not_fit_in_a_printer_that_cannot_print_is_refused_and_one_that_fits_waits(
) {
    let server = Harness::start(&["name=till,printer_buffer=32"]).await;
    server.report("till", json!({"cover_open": true})).await;

    let big = server
        .client
        .post(format!("{}/api/print", server.base))
        .body(vec![b'x'; 100])
        .send()
        .await
        .unwrap();
    assert_eq!(big.status(), 503);
    let said: Value = big.json().await.unwrap();
    assert!(said["error"].as_str().unwrap().contains("32"), "{said}");
    assert!(server.connections().await.is_empty(), "nothing connected");

    let small = server
        .client
        .post(format!("{}/api/print", server.base))
        .body(receipt_bytes("Fits"))
        .send()
        .await
        .unwrap();
    assert_eq!(small.status(), 200);
    let printed: Value = small.json().await.unwrap();
    assert_eq!(printed["receipts"], json!([]));

    server.report("till", json!({"cover_open": false})).await;
    assert_eq!(server.after(0).await["texts"], json!(["Fits"]));
}

#[tokio::test]
async fn how_much_a_printer_holds_is_set_and_told_by_the_api() {
    let server = Harness::start(&["name=till"]).await;
    let info = server.json("/api/printers").await;
    assert_eq!(info[0]["printer_buffer"], 4096);

    let changed = server
        .client
        .patch(format!("{}/api/printers/till", server.base))
        .json(&json!({"printer_buffer": 2048}))
        .send()
        .await
        .unwrap();
    assert_eq!(changed.status(), 200);
    let info: Value = changed.json().await.unwrap();
    assert_eq!(info["printer_buffer"], 2048);
    assert_eq!(
        server.json("/api/printers").await[0]["printer_buffer"],
        2048
    );

    // A program that connects after the change holds, with the number it has now.
    server.report("till", json!({"cover_open": true})).await;
    server.print("till", &receipt_bytes("Later"), 100).await;
    assert!(server.ids().await.is_empty());
    server.report("till", json!({"cover_open": false})).await;
    assert_eq!(server.after(0).await["texts"], json!(["Later"]));
}

#[tokio::test]
async fn a_connection_that_is_open_follows_how_much_the_printer_holds_as_it_changes() {
    let server = Harness::start(&["name=till"]).await;
    let mut stream = TcpStream::connect(server.printer("till")).await.unwrap();
    tokio::time::sleep(SETTLE).await;

    let patch = |bytes: u32| {
        server
            .client
            .patch(format!("{}/api/printers/till", server.base))
            .json(&json!({ "printer_buffer": bytes }))
            .send()
    };
    // It was open before the printer was set to hold, and it holds now.
    assert_eq!(patch(4096).await.unwrap().status(), 200);
    server.report("till", json!({"cover_open": true})).await;
    stream.write_all(&receipt_bytes("Held")).await.unwrap();
    tokio::time::sleep(SETTLE).await;
    assert!(
        server.ids().await.is_empty(),
        "nothing prints with the cover open"
    );

    // Turning the holding off prints what it held, though the cover is still open.
    assert_eq!(patch(0).await.unwrap().status(), 200);
    assert_eq!(server.after(0).await["texts"], json!(["Held"]));
}

#[tokio::test]
async fn a_small_network_buffer_makes_a_program_that_sends_a_lot_wait_until_the_printer_can_print()
{
    let server = Harness::start(&["name=till,network_buffer=4096"]).await;
    assert_eq!(
        server.json("/api/printers").await[0]["network_buffer"],
        4096
    );
    server.report("till", json!({"cover_open": true})).await;

    // A program that keeps little itself, so what it sends has nowhere to wait but the printer.
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    socket.set_send_buffer_size(4096).unwrap();
    let mut stream = socket.connect(server.printer("till")).await.unwrap();
    let job = "A".repeat(63) + "\n";
    let job = job.repeat(1100);
    let size = job.len();
    let sender = tokio::spawn(async move {
        stream.write_all(job.as_bytes()).await.unwrap();
        stream
    });
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(
        !sender.is_finished(),
        "the program has to wait while the printer cannot print"
    );

    server.report("till", json!({"cover_open": false})).await;
    let stream = tokio::time::timeout(Duration::from_secs(10), sender)
        .await
        .expect("the program goes on when the printer can print")
        .unwrap();
    drop(stream);
    server.after(0).await;
    assert_eq!(server.connections().await[0]["bytes_in"], size);
}

#[tokio::test]
async fn the_size_of_the_network_buffer_is_set_by_the_api_for_the_connections_that_open_after() {
    let server = Harness::start(&["name=till"]).await;
    assert_eq!(server.json("/api/printers").await[0]["network_buffer"], 0);
    let changed = server
        .client
        .patch(format!("{}/api/printers/till", server.base))
        .json(&json!({"network_buffer": 8192}))
        .send()
        .await
        .unwrap();
    assert_eq!(changed.status(), 200);
    let info: Value = changed.json().await.unwrap();
    assert_eq!(info["network_buffer"], 8192);
}

/// A database file in the temporary folder that no other test uses, and that does not exist yet.
fn state_file(name: &str) -> std::path::PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "stubprint-{name}-{}-{unique}.db",
        std::process::id()
    ))
}

/// Deletes a database and the files SQLite keeps beside it.
fn remove_database(file: &std::path::Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = file.as_os_str().to_owned();
        name.push(suffix);
        let _ = std::fs::remove_file(name);
    }
}

/// Asks the database a question with one answer, for a test that wants to see what was written.
fn asked<T: rusqlite::types::FromSql>(file: &std::path::Path, sql: &str) -> T {
    rusqlite::Connection::open(file)
        .unwrap()
        .query_row(sql, [], |row| row.get(0))
        .unwrap()
}

/// A free address of this machine, which is free until something takes it.
async fn free_address() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap()
}

/// A configuration that keeps everything in `file` and has the one printer the database or the
/// default gives it, listening wherever there is room.
fn keeping_in(file: &std::path::Path, http: Option<SocketAddr>) -> Config {
    Config {
        http,
        printers: vec![PrinterConfig {
            listen: ([127, 0, 0, 1], 0).into(),
            ..PrinterConfig::default()
        }],
        ready: None,
        database: Some(file.to_path_buf()),
        reset: false,
        given: false,
    }
}

impl Harness {
    /// Starts a server that keeps everything in a database, with the printers given when the
    /// database has none yet. No printers is a run that is given none, which has the default one.
    async fn start_keeping_state(printers: &[&str], file: &std::path::Path, reset: bool) -> Self {
        let given = !printers.is_empty();
        let mut printers: Vec<PrinterConfig> = printers
            .iter()
            .map(|text| PrinterConfig::parse(&format!("listen=127.0.0.1:0,{text}")).unwrap())
            .collect();
        if printers.is_empty() {
            printers.push(PrinterConfig {
                listen: ([127, 0, 0, 1], 0).into(),
                ..PrinterConfig::default()
            });
        }
        let config = Config {
            printers,
            reset,
            given,
            ..keeping_in(file, Some(([127, 0, 0, 1], 0).into()))
        };
        let running = serve(config).await.unwrap();
        let base = format!("http://{}", running.http_addr());
        Self {
            running,
            client: reqwest::Client::new(),
            base,
        }
    }

    async fn delete(&self, path: &str) -> reqwest::Response {
        self.client
            .delete(format!("{}{path}", self.base))
            .send()
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn the_printers_and_what_they_report_are_there_again_after_a_restart() {
    let file = state_file("restart");
    let first = Harness::start_keeping_state(&["name=till"], &file, false).await;
    assert!(file.exists(), "the database is made when the server starts");
    first
        .post_json(
            "/api/printers",
            json!({"name": "bar", "dialect": "starline", "profile": "TM-T20III", "printer_buffer": 2048}),
        )
        .await;
    first
        .post_json("/api/printers", json!({"name": "gone"}))
        .await;
    first
        .report("till", json!({"cover_open": true, "paper": "near_end"}))
        .await;
    first
        .client
        .patch(format!("{}/api/printers/till", first.base))
        .json(&json!({"printer_buffer": 512}))
        .send()
        .await
        .unwrap();
    first.delete("/api/printers/gone").await;
    first.running.stop().await;

    // A run that is given no printers has the ones that were kept, and not the default one.
    let second = Harness::start_keeping_state(&[], &file, false).await;
    let printers = second.json("/api/printers").await;
    let names: Vec<&str> = printers
        .as_array()
        .unwrap()
        .iter()
        .map(|printer| printer["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["till", "bar"]);
    assert_eq!(printers[0]["status"]["cover_open"], true);
    assert_eq!(printers[0]["status"]["paper"], "near_end");
    assert_eq!(printers[0]["printer_buffer"], 512);
    assert_eq!(printers[1]["dialect"], "starline");
    assert_eq!(printers[1]["profile"], "TM-T20III");
    assert_eq!(printers[1]["printer_buffer"], 2048);
    // What is restored works: a printer that holds, with its cover open, holds.
    second.print("till", &receipt_bytes("Held"), 100).await;
    assert!(second.ids().await.is_empty());
    second.running.stop().await;
    remove_database(&file);
}

#[tokio::test]
async fn a_port_that_is_taken_after_a_restart_gives_the_printer_another_and_it_still_works() {
    let file = state_file("taken");
    let first = Harness::start_keeping_state(&["name=till"], &file, false).await;
    let before = first.printer("till");
    first.running.stop().await;

    let _squatter = tokio::net::TcpListener::bind(before).await.unwrap();
    let second = Harness::start_keeping_state(&[], &file, false).await;
    assert_ne!(second.printer("till"), before);
    second
        .print("till", &receipt_bytes("Still here"), 100)
        .await;
    assert_eq!(second.after(0).await["texts"], json!(["Still here"]));
    second.running.stop().await;
    // The database says where it listens now.
    let listen: String = asked(&file, "SELECT listen FROM printers WHERE name = 'till'");
    assert_ne!(listen, before.to_string());
    remove_database(&file);
}

#[tokio::test]
async fn a_file_that_is_not_a_database_stops_the_server_from_starting_and_is_left_alone() {
    let file = state_file("broken");
    std::fs::write(
        &file,
        "this is not a database, only a long enough text to be read as one",
    )
    .unwrap();
    let error = match serve(keeping_in(&file, Some(([127, 0, 0, 1], 0).into()))).await {
        Ok(_) => panic!("the file is not a database"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("database"), "{error}");
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "this is not a database, only a long enough text to be read as one"
    );
    remove_database(&file);
}

#[tokio::test]
async fn a_database_of_something_else_is_refused_and_not_changed() {
    let file = state_file("other");
    rusqlite::Connection::open(&file)
        .unwrap()
        .execute_batch("CREATE TABLE notes (text TEXT); INSERT INTO notes VALUES ('mine');")
        .unwrap();
    let error = match serve(keeping_in(&file, Some(([127, 0, 0, 1], 0).into()))).await {
        Ok(_) => panic!("the database is not ours"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("something else"), "{error}");
    let note: String = asked(&file, "SELECT text FROM notes");
    assert_eq!(note, "mine");
    remove_database(&file);
}

#[tokio::test]
async fn a_printer_that_is_written_is_added_when_the_database_has_none_of_that_name_and_is_left_alone_when_it_has(
) {
    let file = state_file("written");
    let first = Harness::start_keeping_state(&["name=till"], &file, false).await;
    first
        .report("till", json!({"cover_open": true, "paper": "near_end"}))
        .await;
    first
        .client
        .patch(format!("{}/api/printers/till", first.base))
        .json(&json!({"printer_buffer": 512, "network_buffer": 8192, "profile": "TM-T20III"}))
        .send()
        .await
        .unwrap();
    first.running.stop().await;

    // The same command line again, which writes other settings for `till` and one more printer.
    let second = Harness::start_keeping_state(
        &[
            "name=till,printer_buffer=4096,profile=TM-T88IV",
            "name=other",
        ],
        &file,
        false,
    )
    .await;
    let printers = second.json("/api/printers").await;
    let names: Vec<&str> = printers
        .as_array()
        .unwrap()
        .iter()
        .map(|printer| printer["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["till", "other"]);
    // The database says what `till` is.
    assert_eq!(printers[0]["printer_buffer"], 512);
    assert_eq!(printers[0]["network_buffer"], 8192);
    assert_eq!(printers[0]["profile"], "TM-T20III");
    assert_eq!(printers[0]["status"]["cover_open"], true);
    assert_eq!(printers[0]["status"]["paper"], "near_end");
    second.running.stop().await;

    // A reset starts with what is written and nothing else.
    let third = Harness::start_keeping_state(&["name=other"], &file, true).await;
    let printers = third.json("/api/printers").await;
    assert_eq!(printers.as_array().unwrap().len(), 1);
    assert_eq!(printers[0]["name"], "other");
    third.running.stop().await;
    let count: i64 = asked(&file, "SELECT count(*) FROM printers");
    assert_eq!(count, 1);
    remove_database(&file);
}

#[tokio::test]
async fn the_address_of_the_api_that_is_given_is_kept_and_used_again_until_another_is_given() {
    let file = state_file("http");
    let (first_address, second_address) = (free_address().await, free_address().await);

    let first = serve(keeping_in(&file, Some(first_address))).await.unwrap();
    assert_eq!(first.http_addr(), first_address);
    first.stop().await;
    let saved: String = asked(&file, "SELECT value FROM settings WHERE key = 'http'");
    assert_eq!(saved, first_address.to_string());

    // Given nothing, it listens where the database says.
    let again = serve(keeping_in(&file, None)).await.unwrap();
    assert_eq!(again.http_addr(), first_address);
    again.stop().await;

    // What is given wins, and the database says it from then on.
    let other = serve(keeping_in(&file, Some(second_address)))
        .await
        .unwrap();
    assert_eq!(other.http_addr(), second_address);
    other.stop().await;
    let saved: String = asked(&file, "SELECT value FROM settings WHERE key = 'http'");
    assert_eq!(saved, second_address.to_string());
    remove_database(&file);
}

#[tokio::test]
async fn the_receipts_and_the_connections_are_there_again_after_a_restart() {
    let file = state_file("history");
    let first = Harness::start_keeping_state(&["name=till"], &file, false).await;
    let bytes = receipt_bytes("One");
    first.print("till", &bytes, 100).await;
    first.print("till", &receipt_bytes("Two"), 100).await;
    first.print("till", &receipt_bytes("Three"), 100).await;
    assert_eq!(first.ids().await, [1, 2, 3]);
    assert_eq!(first.delete("/api/receipts/3").await.status(), 204);
    let connections = first.json("/api/connections").await;
    assert_eq!(connections.as_array().unwrap().len(), 3);
    first.running.stop().await;

    let second = Harness::start_keeping_state(&[], &file, false).await;
    assert_eq!(second.ids().await, [1, 2], "what was removed stays removed");
    let one = second.json("/api/receipts/1").await;
    assert_eq!(one["texts"], json!(["One"]));
    assert_eq!(one["printer"], "till");
    // It is the same receipt, which can be read and drawn again.
    let kept = second.get("/api/receipts/1/bytes").await;
    assert_eq!(kept.bytes().await.unwrap().as_ref(), bytes.as_slice());
    assert_eq!(second.get("/api/receipts/1/picture").await.status(), 200);
    assert_eq!(second.get("/api/receipts/1/trace").await.status(), 200);
    // The connections that printed them, closed, with the receipts they made.
    let connections = second.json("/api/connections").await;
    assert_eq!(connections.as_array().unwrap().len(), 3);
    assert!(connections[0]["closed_at"].is_u64());
    assert_eq!(connections[0]["receipts"], json!([1]));

    // The number of the receipt that was removed is not used again, nor are the numbers of the
    // connections.
    second.print("till", &receipt_bytes("Four"), 100).await;
    assert_eq!(second.ids().await, [1, 2, 4]);
    let connections = second.json("/api/connections").await;
    assert_eq!(connections[3]["id"], 4);
    second.running.stop().await;
    remove_database(&file);
}

#[tokio::test]
async fn the_receipts_of_a_printer_that_was_removed_are_there_again_after_a_restart() {
    let file = state_file("removed-printer");
    let first = Harness::start_keeping_state(&["name=till"], &file, false).await;
    first
        .post_json(
            "/api/printers",
            json!({"name": "bar", "dialect": "starline"}),
        )
        .await;
    first.print("bar", &receipt_bytes("From bar"), 100).await;
    first.print("till", &receipt_bytes("From till"), 100).await;
    first.delete("/api/printers/bar").await;
    first.running.stop().await;

    let second = Harness::start_keeping_state(&[], &file, false).await;
    assert_eq!(second.ids().await, [1, 2]);
    let from_bar = second.json("/api/receipts/1").await;
    assert_eq!(from_bar["printer"], "bar");
    assert_eq!(from_bar["dialect"], "starline");
    assert_eq!(second.get("/api/receipts/1/picture").await.status(), 200);
    second.running.stop().await;
    remove_database(&file);
}

#[tokio::test]
async fn the_receipts_that_were_cleared_stay_cleared_after_a_restart() {
    let file = state_file("cleared");
    let first = Harness::start_keeping_state(&["name=till"], &file, false).await;
    first
        .post_json("/api/printers", json!({"name": "bar"}))
        .await;
    first.print("till", &receipt_bytes("a"), 100).await;
    first.print("bar", &receipt_bytes("b"), 100).await;
    first.print("till", &receipt_bytes("c"), 100).await;
    assert_eq!(
        first.delete("/api/receipts?printer=till").await.status(),
        200
    );
    first.running.stop().await;

    let second = Harness::start_keeping_state(&[], &file, false).await;
    assert_eq!(second.ids().await, [2]);
    assert_eq!(second.delete("/api/receipts").await.status(), 200);
    second.running.stop().await;

    let third = Harness::start_keeping_state(&[], &file, false).await;
    assert!(third.ids().await.is_empty());
    third.running.stop().await;
    remove_database(&file);
}

#[tokio::test]
async fn a_reset_forgets_the_receipts_and_the_connections_too() {
    let file = state_file("reset");
    let first = Harness::start_keeping_state(&["name=till"], &file, false).await;
    first.print("till", &receipt_bytes("One"), 100).await;
    first.running.stop().await;

    let second = Harness::start_keeping_state(&["name=till"], &file, true).await;
    assert!(second.ids().await.is_empty());
    assert!(second
        .json("/api/connections")
        .await
        .as_array()
        .unwrap()
        .is_empty());
    second.print("till", &receipt_bytes("Two"), 100).await;
    assert_eq!(second.ids().await, [1]);
    second.running.stop().await;
    remove_database(&file);
}

#[tokio::test]
async fn the_names_a_setting_had_before_it_was_renamed_are_still_read_in_a_request() {
    let server = Harness::start(&["name=till"]).await;
    let changed = server
        .client
        .patch(format!("{}/api/printers/till", server.base))
        .json(&json!({"hold_offline": 512, "receive_buffer": 4096}))
        .send()
        .await
        .unwrap();
    assert_eq!(changed.status(), 200);
    let changed: Value = changed.json().await.unwrap();
    assert_eq!(changed["printer_buffer"], 512);
    assert_eq!(changed["network_buffer"], 4096);
}

#[tokio::test]
async fn nothing_is_read_or_written_when_nothing_is_kept() {
    let server = Harness::start(&["name=till"]).await;
    server.report("till", json!({"cover_open": true})).await;
    assert_eq!(server.json("/api/printers").await[0]["name"], "till");
    server.running.stop().await;
}

#[tokio::test]
async fn the_folder_of_the_database_is_made_when_it_is_missing() {
    let folder = state_file("folder").with_extension("");
    let file = folder.join("deeper").join("stubprint.db");
    let server = Harness::start_keeping_state(&["name=till"], &file, false).await;
    assert!(file.exists());
    server.running.stop().await;
    let _ = std::fs::remove_dir_all(&folder);
}

/// The bytes of a receipt that kicks the cash drawer: `ESC p 0 25 250`.
fn kicking_receipt(text: &str) -> Vec<u8> {
    [&[0x1b, b'p', 0, 25, 250][..], &receipt_bytes(text)].concat()
}

#[tokio::test]
async fn a_cash_drawer_that_is_kicked_opens_and_stays_open_until_it_is_shut_by_hand() {
    let server = Harness::start(&["name=till"]).await;
    assert_eq!(
        server.json("/api/printers/till").await["status"]["drawer_open"],
        false
    );
    server.print("till", &kicking_receipt("Open"), 100).await;
    assert_eq!(
        server.json("/api/printers/till").await["status"]["drawer_open"],
        true
    );
    // The printer says so when it is asked: `GS r 2` is the cash drawer.
    let answer = server.print("till", &[0x1d, b'r', 2], 100).await;
    assert_eq!(answer, [1]);
    // It is shut by hand, which is what the status says.
    server.report("till", json!({"drawer_open": false})).await;
    assert_eq!(
        server.json("/api/printers/till").await["status"]["drawer_open"],
        false
    );
    let answer = server.print("till", &[0x1d, b'r', 2], 100).await;
    assert_eq!(answer, [0]);
}

#[tokio::test]
async fn a_printer_that_cannot_print_does_not_kick_the_drawer_until_it_prints() {
    let server = Harness::start(&["name=till"]).await;
    server.report("till", json!({"cover_open": true})).await;
    server.print("till", &kicking_receipt("Later"), 100).await;
    assert_eq!(
        server.json("/api/printers/till").await["status"]["drawer_open"],
        false
    );
    server.report("till", json!({"cover_open": false})).await;
    assert_eq!(
        server.json("/api/printers/till").await["status"]["drawer_open"],
        true
    );
}

#[tokio::test]
async fn what_a_printer_printed_is_counted_and_a_program_can_ask_for_the_counters() {
    let server = Harness::start(&["name=till"]).await;
    let nothing = server.json("/api/printers/till").await;
    assert_eq!(nothing["usage"]["receipts"], 0);
    assert_eq!(nothing["usage"]["paper_used"], 0.0);
    for text in ["a", "b", "c"] {
        server.print("till", &receipt_bytes(text), 100).await;
    }
    let printer = server.json("/api/printers/till").await;
    assert_eq!(printer["usage"]["receipts"], 3);
    assert_eq!(printer["usage"]["cuts"], 3);
    assert_eq!(printer["usage"]["line_feeds"], 3);
    assert!(printer["usage"]["paper_used"].as_f64().unwrap() > 0.0);

    // `GS g 2 0 20 0` asks for the line feeds, and `GS g 2 0 50 0` for the cuts. The answer is a
    // header, the digits and a NUL.
    let feeds = server
        .print("till", &[0x1d, b'g', b'2', 0, 20, 0], 100)
        .await;
    assert_eq!(feeds, [0x5f, b'3', 0]);
    let cuts = server
        .print("till", &[0x1d, b'g', b'2', 0, 50, 0], 100)
        .await;
    assert_eq!(cuts, [0x5f, b'3', 0]);
    // A counter the printer does not have is not answered.
    let none = server
        .print("till", &[0x1d, b'g', b'2', 0, 99, 0], 100)
        .await;
    assert!(none.is_empty());
}

#[tokio::test]
async fn the_counters_follow_a_receipt_that_feeds_lines() {
    let server = Harness::start(&["name=till"]).await;
    // `ESC d 5` feeds five lines, and a line feed ends the text before it.
    server
        .print("till", &[&b"x\n"[..], &[0x1b, b'd', 5], &CUT].concat(), 100)
        .await;
    let usage = server.json("/api/printers/till").await["usage"].clone();
    assert_eq!(usage["line_feeds"], 6);
    let counted = server
        .print("till", &[0x1d, b'g', b'2', 0, 20, 0], 100)
        .await;
    assert_eq!(counted, [0x5f, b'6', 0]);
}

/// How much paper one small receipt takes, in millimetres, as the API counts it.
async fn paper_of_one_receipt() -> f64 {
    let server = Harness::start(&["name=probe"]).await;
    server.print("probe", &receipt_bytes("x"), 100).await;
    server.json("/api/printers/probe").await["usage"]["paper_used"]
        .as_f64()
        .unwrap()
}

#[tokio::test]
async fn a_paper_roll_runs_down_with_each_receipt_and_the_printer_stops_when_it_is_used_up() {
    let one = paper_of_one_receipt().await;
    assert!(one > 1.0, "a receipt takes some paper: {one}");
    // A roll that holds ten receipts and a half: it is near its end after the tenth.
    let length: u32 = format!("{:.0}", one * 10.5).parse().unwrap();
    let full = f64::from(length);
    let server = Harness::start(&["name=till"]).await;
    let added = server
        .post_json(
            "/api/printers",
            json!({"name": "roll", "listen": "127.0.0.1:0", "paper_roll": length}),
        )
        .await;
    assert_eq!(added.status(), 201);
    let (level, left) = paper_state(&server, "roll").await;
    assert_eq!(level, "adequate");
    assert_eq!(left, full);

    for _ in 0..9 {
        server.print("roll", &receipt_bytes("x"), 100).await;
    }
    assert_eq!(paper_state(&server, "roll").await.0, "adequate");
    server.print("roll", &receipt_bytes("x"), 100).await;
    let (level, left) = paper_state(&server, "roll").await;
    assert_eq!(level, "near_end", "10 receipts leave {left} of {length}");
    // Near its end it still prints.
    server.print("roll", &receipt_bytes("x"), 100).await;
    assert_eq!(paper_state(&server, "roll").await, ("end".to_string(), 0.0));
    assert_eq!(server.ids().await.len(), 11);

    // With no paper it holds what it is sent, and does not print it.
    server.print("roll", &receipt_bytes("held"), 100).await;
    assert_eq!(server.ids().await.len(), 11);
    assert!(
        server.json("/api/printers/roll").await["held"]
            .as_u64()
            .unwrap()
            > 0
    );

    // A new roll is full, and the printer prints what it held.
    let loaded = server
        .client
        .patch(format!("{}/api/printers/roll", server.base))
        .json(&json!({"paper_roll": length}))
        .send()
        .await
        .unwrap();
    assert_eq!(loaded.status(), 200);
    assert_eq!(server.ids().await.len(), 12);
    let (level, left) = paper_state(&server, "roll").await;
    assert_eq!(level, "adequate");
    assert!(left < full && left > full - 2.0 * one);
    assert_eq!(server.json("/api/printers/roll").await["held"], 0);
}

/// What the sensors of a printer report, and how much paper is left on its roll.
async fn paper_state(server: &Harness, name: &str) -> (String, f64) {
    let printer = server.json(&format!("/api/printers/{name}")).await;
    (
        printer["status"]["paper"].as_str().unwrap().to_string(),
        printer["paper_left"].as_f64().unwrap(),
    )
}

#[tokio::test]
async fn the_paper_set_by_hand_is_what_a_roll_has_left() {
    let server = Harness::start(&["name=till,paper_roll=1000"]).await;
    assert_eq!(paper_state(&server, "till").await.1, 1000.0);
    server.report("till", json!({"paper": "near_end"})).await;
    assert_eq!(paper_state(&server, "till").await.1, 100.0);
    server.report("till", json!({"paper": "end"})).await;
    assert_eq!(paper_state(&server, "till").await.1, 0.0);
    assert_eq!(
        server.json("/api/printers/till").await["status"]["paper"],
        "end"
    );
    // Setting it right again is putting in a new roll.
    server.report("till", json!({"paper": "adequate"})).await;
    assert_eq!(paper_state(&server, "till").await.1, 1000.0);
}

#[tokio::test]
async fn a_printer_without_a_roll_never_runs_out_and_leaves_the_paper_to_be_set_by_hand() {
    let server = Harness::start(&["name=till"]).await;
    for _ in 0..3 {
        server.print("till", &receipt_bytes("x"), 100).await;
    }
    let printer = server.json("/api/printers/till").await;
    assert_eq!(printer["paper_roll"], 0);
    assert_eq!(printer["status"]["paper"], "adequate");
    server.report("till", json!({"paper": "near_end"})).await;
    server.print("till", &receipt_bytes("x"), 100).await;
    assert_eq!(
        server.json("/api/printers/till").await["status"]["paper"],
        "near_end"
    );
}

#[tokio::test]
async fn the_paper_and_the_counters_are_there_again_after_a_restart() {
    let file = state_file("roll");
    let first = Harness::start_keeping_state(&["name=till,paper_roll=100000"], &file, false).await;
    for text in ["a", "b"] {
        first.print("till", &receipt_bytes(text), 100).await;
    }
    let before = first.json("/api/printers/till").await;
    first.running.stop().await;

    let second = Harness::start_keeping_state(&[], &file, false).await;
    let after = second.json("/api/printers/till").await;
    assert_eq!(after["paper_roll"], 100000);
    assert_eq!(after["paper_left"], before["paper_left"]);
    assert_eq!(after["usage"], before["usage"]);
    assert_eq!(after["usage"]["receipts"], 2);
    // A program that asks the printer gets what it counted before.
    let cuts = second
        .print("till", &[0x1d, b'g', b'2', 0, 50, 0], 100)
        .await;
    assert_eq!(cuts, [0x5f, b'2', 0]);
    second.running.stop().await;
    remove_database(&file);
}

#[tokio::test]
async fn a_database_from_before_the_paper_roll_is_brought_up_to_date_and_keeps_its_printers() {
    let file = state_file("version-one");
    {
        let db = rusqlite::Connection::open(&file).unwrap();
        db.execute_batch(&format!(
            "BEGIN; {} PRAGMA user_version = 1; COMMIT;
             INSERT INTO printers VALUES (0, 'till', '127.0.0.1:0', 'escpos', 'default', 4096, 0, '{{}}');",
            include_str!("../migrations/001_printers_receipts_connections.sql")
        ))
        .unwrap();
    }
    let server = Harness::start_keeping_state(&[], &file, false).await;
    let printer = server.json("/api/printers/till").await;
    assert_eq!(printer["paper_roll"], 0);
    assert_eq!(printer["usage"]["receipts"], 0);
    server.print("till", &receipt_bytes("x"), 100).await;
    server.running.stop().await;
    let version: i64 = asked(&file, "PRAGMA user_version");
    assert!(version >= 2);
    let receipts: i64 = asked(&file, "SELECT receipts FROM printers WHERE name = 'till'");
    assert_eq!(receipts, 1);
    remove_database(&file);
}

#[tokio::test]
async fn how_much_a_printer_holds_is_told_by_the_api_and_by_a_live_event_as_it_changes() {
    let server = Harness::start(&["name=till,printer_buffer=4096"]).await;
    let mut events = Live {
        response: server.get("/api/events").await,
        seen: String::new(),
    };
    assert_eq!(server.json("/api/printers").await[0]["held"], 0);

    server.report("till", json!({"cover_open": true})).await;
    // A program that is still connected holds 9 bytes.
    let mut stream = TcpStream::connect(server.printer("till")).await.unwrap();
    stream.write_all(&receipt_bytes("Held")).await.unwrap();
    events.until("event: held").await;
    events.until("\"held\":9").await;
    let now = server
        .until("/api/printers", |list| list[0]["held"] == 9)
        .await;
    assert_eq!(now[0]["printer_buffer"], 4096);

    // The program goes: its bytes stay in the printer and are still counted.
    drop(stream);
    tokio::time::sleep(SETTLE).await;
    assert_eq!(server.json("/api/printers").await[0]["held"], 9);

    // Another program adds to it.
    server.print("till", &receipt_bytes("More"), 100).await;
    server
        .until("/api/printers", |list| list[0]["held"] == 18)
        .await;

    // The cover closes: everything prints and nothing is held.
    server.report("till", json!({"cover_open": false})).await;
    events.until("\"held\":0").await;
    server
        .until("/api/printers", |list| list[0]["held"] == 0)
        .await;
    assert_eq!(server.ids().await.len(), 2);
}

/// How long a second receipt takes to be kept after it was sent, when the first one was kept before.
async fn wait_for_the_second_receipt(server: &Harness, name: &str) -> Duration {
    let mut stream = TcpStream::connect(server.printer(name)).await.unwrap();
    stream.write_all(&receipt_bytes("first")).await.unwrap();
    let first = server.after(0).await["id"].as_u64().unwrap();
    let sent = std::time::Instant::now();
    stream.write_all(&receipt_bytes("second")).await.unwrap();
    server.after(first).await;
    sent.elapsed()
}

#[tokio::test]
async fn a_printer_that_prints_at_a_speed_keeps_a_receipt_when_it_is_printed_and_one_with_no_speed_takes_no_time(
) {
    let one = paper_of_one_receipt().await;
    // A speed at which one receipt takes 0.6 seconds.
    let speed = (one / 0.6).ceil();
    let slow = Harness::start(&[&format!("name=slow,print_speed={speed}")]).await;
    let waited = wait_for_the_second_receipt(&slow, "slow").await;
    assert!(
        waited >= Duration::from_millis(400),
        "the second receipt came after {waited:?}"
    );
    assert!(waited < Duration::from_secs(3), "{waited:?}");

    // A printer with no speed takes no time.
    let quick = Harness::start(&["name=quick"]).await;
    let waited = wait_for_the_second_receipt(&quick, "quick").await;
    assert!(waited < Duration::from_millis(300), "{waited:?}");
}

#[tokio::test]
async fn the_speed_of_a_printer_can_be_changed_and_zero_prints_what_is_waiting_at_once() {
    let one = paper_of_one_receipt().await;
    // One receipt takes a minute at this speed.
    let speed = (one / 60.0).ceil().max(1.0) as u64;
    let server = Harness::start(&["name=till"]).await;
    assert_eq!(server.json("/api/printers/till").await["print_speed"], 0);
    let changed = server
        .client
        .patch(format!("{}/api/printers/till", server.base))
        .json(&json!({"print_speed": speed}))
        .send()
        .await
        .unwrap();
    assert_eq!(changed.status(), 200);
    assert_eq!(changed.json::<Value>().await.unwrap()["print_speed"], speed);

    let mut stream = TcpStream::connect(server.printer("till")).await.unwrap();
    stream.write_all(&receipt_bytes("first")).await.unwrap();
    let first = server.after(0).await["id"].as_u64().unwrap();
    stream.write_all(&receipt_bytes("second")).await.unwrap();
    // It is still printing the first, and the second is not printed yet.
    let early = server
        .get(&format!("/api/receipts/next?after={first}&timeout=1"))
        .await;
    assert_eq!(early.status(), 204);

    // A printer that is set to take no time reads at once.
    server
        .client
        .patch(format!("{}/api/printers/till", server.base))
        .json(&json!({"print_speed": 0}))
        .send()
        .await
        .unwrap();
    server.after(first).await;
}

#[tokio::test]
async fn the_speed_of_a_printer_is_there_again_after_a_restart() {
    let file = state_file("speed");
    let first = Harness::start_keeping_state(&["name=till,print_speed=150"], &file, false).await;
    assert_eq!(first.json("/api/printers/till").await["print_speed"], 150);
    first.running.stop().await;

    let second = Harness::start_keeping_state(&[], &file, false).await;
    assert_eq!(second.json("/api/printers/till").await["print_speed"], 150);
    second.running.stop().await;
    remove_database(&file);
}

/// The bytes of a receipt of `lines` lines, each as long as a line of the printer is tall, and its
/// cut.
fn long_receipt(lines: usize) -> Vec<u8> {
    let mut bytes = long_job(lines);
    bytes.extend([0x1d, b'V', 0x42, 0x00]);
    bytes
}

/// The same, with no cut.
fn long_job(lines: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    for number in 0..lines {
        bytes.extend(format!("line {number:03}\n").bytes());
    }
    bytes
}

/// How many lines of text a receipt has now, as the API shows it, and the state it is in.
async fn receipt_now(server: &Harness, id: u64) -> (usize, String) {
    let receipt = server.json(&format!("/api/receipts/{id}")).await;
    let text = server
        .get(&format!("/api/receipts/{id}/text"))
        .await
        .text()
        .await
        .unwrap();
    (
        text.lines().filter(|line| line.starts_with("line")).count(),
        receipt["state"].as_str().unwrap().to_string(),
    )
}

/// Waits until a receipt is not printing any more and says what it ended as.
async fn until_printed(server: &Harness, id: u64) -> (usize, String) {
    let started = std::time::Instant::now();
    loop {
        let now = receipt_now(server, id).await;
        if now.1 != "printing" {
            return now;
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "still printing: {now:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn a_receipt_exists_from_its_first_line_and_grows_as_it_prints_until_it_is_cut() {
    // A line of paper is 3.75 mm, so 40 lines are 150 mm, which take 2 seconds at 75 mm/s.
    let server = Harness::start(&["name=till,print_speed=75"]).await;
    assert!(server.ids().await.is_empty());
    let mut stream = TcpStream::connect(server.printer("till")).await.unwrap();
    let started = std::time::Instant::now();
    stream.write_all(&long_receipt(40)).await.unwrap();

    // It has its number at once, and part of the way through it has some of its lines.
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert_eq!(server.ids().await, [1]);
    let (early, state) = receipt_now(&server, 1).await;
    assert_eq!(state, "printing");
    assert!(
        (3..40).contains(&early),
        "after 0.7 s there are {early} lines"
    );
    let commands = server.json("/api/receipts/1/trace").await;
    let early_commands = commands.as_array().unwrap().len();
    // What it has is a receipt like any other: its bytes, its picture and its commands.
    let bytes = server
        .get("/api/receipts/1/bytes")
        .await
        .bytes()
        .await
        .unwrap();
    assert!(bytes.starts_with(b"line 000"));
    assert!(bytes.len() < long_receipt(40).len());
    let picture = server.get("/api/receipts/1/picture").await;
    assert_eq!(picture.status(), 200);
    // The next receipt is one that is done, so a program that waits for it does not get this one.
    let waiting = server.get("/api/receipts/next?after=0&timeout=0").await;
    assert_eq!(waiting.status(), 204);

    // Later it has more, and it is still the same receipt.
    tokio::time::sleep(Duration::from_millis(600)).await;
    let (later, state) = receipt_now(&server, 1).await;
    assert_eq!(state, "printing");
    assert!(later > early);
    assert!(
        server
            .json("/api/receipts/1/trace")
            .await
            .as_array()
            .unwrap()
            .len()
            > early_commands
    );
    assert_eq!(server.ids().await, [1]);

    // The cut finishes it, with every line, and nothing changes its number.
    let receipt = server.after(0).await;
    assert!(
        started.elapsed() >= Duration::from_millis(1700),
        "it was done after {:?}",
        started.elapsed()
    );
    assert_eq!(receipt["id"], 1);
    assert_eq!(receipt["state"], "complete");
    assert_eq!(receipt["texts"].as_array().unwrap().len(), 40);
    assert_eq!(server.ids().await, [1]);
}

#[tokio::test]
async fn the_buffer_of_a_printer_takes_a_program_a_little_at_a_time_as_it_prints() {
    // 120 lines are 450 mm, which take 3 seconds at 150 mm/s, and the buffer is far smaller than
    // what is sent.
    let server = Harness::start(&["name=till,print_speed=150,printer_buffer=256"]).await;
    let bytes = long_receipt(120);
    assert!(bytes.len() > 1000);
    let mut stream = TcpStream::connect(server.printer("till")).await.unwrap();
    stream.write_all(&bytes).await.unwrap();

    let mut most = 0;
    let mut samples = Vec::new();
    let started = std::time::Instant::now();
    loop {
        let printer = server.json("/api/printers/till").await;
        let buffered = usize::try_from(printer["buffered"].as_u64().unwrap()).unwrap();
        most = most.max(buffered);
        samples.push(buffered);
        let done = !server.ids().await.is_empty() && receipt_now(&server, 1).await.1 == "complete";
        if done {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(10), "{samples:?}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // It never held more than its buffer, it was holding something most of the time, and nothing
    // was lost on the way.
    assert!(most <= 256, "it held {most}");
    assert!(
        samples.iter().filter(|held| **held > 0).count() * 2 > samples.len(),
        "{samples:?}"
    );
    assert!(started.elapsed() >= Duration::from_millis(2000));
    assert_eq!(receipt_now(&server, 1).await.0, 120);
    assert_eq!(server.json("/api/printers/till").await["buffered"], 0);
}

#[tokio::test]
async fn the_live_events_tell_that_a_receipt_began_how_far_it_is_and_that_it_ended() {
    let server = Harness::start(&["name=till,print_speed=150"]).await;
    let mut events = Live {
        response: server.get("/api/events").await,
        seen: String::new(),
    };
    let mut stream = TcpStream::connect(server.printer("till")).await.unwrap();
    stream.write_all(&long_receipt(60)).await.unwrap();
    // It begins with its number, still printing.
    events.until("event: receipt").await;
    events.until("\"state\":\"printing\"").await;
    // It grows, and each event says which receipt and how many commands.
    events.until("event: printing").await;
    events.until("\"receipt\":1").await;
    events.until("\"commands\":").await;
    // It ends with the cut.
    events.until("\"state\":\"complete\"").await;
}

#[tokio::test]
async fn a_printer_that_cannot_print_does_not_spend_time_and_goes_on_when_it_can() {
    let server = Harness::start(&["name=till,print_speed=75"]).await;
    let mut stream = TcpStream::connect(server.printer("till")).await.unwrap();
    stream.write_all(&long_receipt(40)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    server.report("till", json!({"cover_open": true})).await;
    let (at_stop, state) = receipt_now(&server, 1).await;
    assert_eq!(state, "printing");
    // Nothing more comes out while its cover is open, however long it takes.
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(
        receipt_now(&server, 1).await,
        (at_stop, "printing".to_string())
    );

    // With the cover closed it goes on from where it was, and the receipt is whole.
    server.report("till", json!({"cover_open": false})).await;
    let receipt = server.after(0).await;
    assert_eq!(receipt["state"], "complete");
    assert_eq!(receipt["texts"].as_array().unwrap().len(), 40);
}

#[tokio::test]
async fn a_program_that_goes_without_a_cut_leaves_a_receipt_that_ends_where_it_was_sent() {
    let server = Harness::start(&["name=till,print_speed=75"]).await;
    let mut stream = TcpStream::connect(server.printer("till")).await.unwrap();
    stream.write_all(&long_job(40)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    drop(stream);
    // The printer prints what it was sent, and with no cut the receipt is not a complete one.
    let (lines, state) = until_printed(&server, 1).await;
    assert_eq!(state, "incomplete");
    assert_eq!(lines, 40);
    // It stays as it is.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        receipt_now(&server, 1).await,
        (40, "incomplete".to_string())
    );
}

#[tokio::test]
async fn a_printer_that_is_removed_while_it_prints_leaves_its_receipt_on_the_last_thing_it_printed()
{
    let server = Harness::start(&["name=till"]).await;
    let added = server
        .post_json(
            "/api/printers",
            json!({"name": "slow", "listen": "127.0.0.1:0", "print_speed": 75}),
        )
        .await;
    assert_eq!(added.status(), 201);
    let mut stream = TcpStream::connect(server.printer("slow")).await.unwrap();
    stream.write_all(&long_receipt(40)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(600)).await;
    let (before, state) = receipt_now(&server, 1).await;
    assert_eq!(state, "printing");

    server.delete("/api/printers/slow").await;
    let (lines, state) = receipt_now(&server, 1).await;
    assert_eq!(state, "incomplete");
    assert!(lines >= before && lines < 40, "{before} then {lines}");
    // It does not go on without its printer.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        receipt_now(&server, 1).await,
        (lines, "incomplete".to_string())
    );
}

#[tokio::test]
async fn a_receipt_that_ended_without_a_cut_is_still_incomplete_after_a_restart() {
    let file = state_file("incomplete");
    let first = Harness::start_keeping_state(&["name=till"], &file, false).await;
    first.print("till", &long_job(3), 100).await;
    first.print("till", &long_receipt(2), 100).await;
    assert_eq!(receipt_now(&first, 1).await, (3, "incomplete".to_string()));
    assert_eq!(receipt_now(&first, 2).await, (2, "complete".to_string()));
    first.running.stop().await;

    let second = Harness::start_keeping_state(&[], &file, false).await;
    assert_eq!(receipt_now(&second, 1).await, (3, "incomplete".to_string()));
    assert_eq!(receipt_now(&second, 2).await, (2, "complete".to_string()));
    second.running.stop().await;
    remove_database(&file);
}
