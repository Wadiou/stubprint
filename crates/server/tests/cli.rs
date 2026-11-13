//! The commands of the program, run as the program against a server that runs in this process.

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value;
use stubprint_server::{serve, Config, PrinterConfig, Running};

struct Server {
    running: Running,
    address: String,
}

async fn start(printers: &[&str]) -> Server {
    let config = Config {
        http: Some(([127, 0, 0, 1], 0).into()),
        printers: printers
            .iter()
            .map(|text| PrinterConfig::parse(&format!("listen=127.0.0.1:0,{text}")).unwrap())
            .collect(),
        ready: None,
        database: None,
        reset: false,
        given: false,
    };
    let running = serve(config).await.unwrap();
    let address = format!("http://{}", running.http_addr());
    Server { running, address }
}

/// What a command printed and how it ended.
struct Run {
    code: i32,
    out: Vec<u8>,
    err: String,
}

impl Run {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.out).into_owned()
    }

    fn json(&self) -> Value {
        serde_json::from_slice(&self.out).unwrap_or_else(|error| {
            panic!("not JSON ({error}): {}", self.text());
        })
    }
}

/// Runs the program with these arguments, with `input` typed in, against a server.
async fn stubprint_at(server: &str, args: &[&str], input: &[u8]) -> Run {
    let server = server.to_string();
    let args: Vec<String> = args.iter().map(ToString::to_string).collect();
    let input = input.to_vec();
    tokio::task::spawn_blocking(move || {
        let mut child = Command::new(env!("CARGO_BIN_EXE_stubprint"))
            .arg("--server")
            .arg(server)
            .args(args)
            .env_remove("STUBPRINT_SERVER")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&input).unwrap();
        let output = child.wait_with_output().unwrap();
        Run {
            code: output.status.code().unwrap_or(-1),
            out: output.stdout,
            err: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    })
    .await
    .unwrap()
}

async fn stubprint(server: &Server, args: &[&str]) -> Run {
    stubprint_at(&server.address, args, b"").await
}

async fn stubprint_with(server: &Server, args: &[&str], input: &[u8]) -> Run {
    stubprint_at(&server.address, args, input).await
}

/// A job that prints these lines and cuts.
fn receipt(lines: &[&str]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for line in lines {
        bytes.extend(format!("{line}\n").bytes());
    }
    bytes.extend([0x1d, b'V', 0x42, 0x00]);
    bytes
}

#[tokio::test]
async fn the_printers_are_listed_added_changed_and_removed() {
    let server = start(&["name=till"]).await;

    let list = stubprint(&server, &["printer", "list"]).await;
    assert_eq!(list.code, 0, "{}", list.err);
    assert!(list.text().contains("till"), "{}", list.text());
    assert!(list.text().contains("online"), "{}", list.text());

    let added = stubprint(&server, &["printer", "add", "name=bar", "dialect=starline"]).await;
    assert_eq!(added.code, 0, "{}", added.err);
    assert!(added.text().contains("added bar"), "{}", added.text());
    let json = stubprint(&server, &["printer", "list", "--json"])
        .await
        .json();
    let names: Vec<&str> = json
        .as_array()
        .unwrap()
        .iter()
        .map(|printer| printer["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["till", "bar"]);

    // What a printer reports keeps its type, and what it is set to takes numbers written as text.
    let status = stubprint(
        &server,
        &[
            "printer",
            "status",
            "till",
            "cover_open=true",
            "paper=near_end",
        ],
    )
    .await;
    assert_eq!(status.code, 0, "{}", status.err);
    assert!(status.text().contains("cover open"), "{}", status.text());
    let set = stubprint(
        &server,
        &[
            "printer",
            "set",
            "till",
            "idle_timeout=30",
            "profile=TM-T88IV",
        ],
    )
    .await;
    assert_eq!(set.code, 0, "{}", set.err);
    let shown = stubprint(&server, &["printer", "show", "till", "--json"])
        .await
        .json();
    assert_eq!(shown["idle_timeout"], 30);
    assert_eq!(shown["profile"], "TM-T88IV");
    assert_eq!(shown["status"]["cover_open"], true);
    assert_eq!(shown["status"]["paper"], "near_end");
    let clear = stubprint(
        &server,
        &[
            "printer",
            "status",
            "till",
            "cover_open=false",
            "paper=adequate",
            "fault=null",
        ],
    )
    .await;
    assert!(clear.text().contains("online"), "{}", clear.text());

    let removed = stubprint(&server, &["printer", "remove", "bar"]).await;
    assert_eq!(removed.code, 0, "{}", removed.err);
    let list = stubprint(&server, &["printer", "list"]).await.text();
    assert!(!list.contains("bar"), "{list}");
    server.running.stop().await;
}

#[tokio::test]
async fn a_file_and_a_sample_are_printed_and_the_receipt_is_shown_in_every_form() {
    let server = start(&["name=till"]).await;
    let bytes = receipt(&["Hello CLI", "Second line"]);

    let printed = stubprint_with(&server, &["print", "-"], &bytes).await;
    assert_eq!(printed.code, 0, "{}", printed.err);
    assert!(printed.text().contains("receipt #1"), "{}", printed.text());
    let sample = stubprint(
        &server,
        &["print", "--sample", "grocery", "--printer", "till"],
    )
    .await;
    assert_eq!(sample.code, 0, "{}", sample.err);
    assert!(sample.text().contains("receipt #2"), "{}", sample.text());

    let list = stubprint(&server, &["receipt", "list"]).await;
    assert!(
        list.text().contains("#1") && list.text().contains("#2"),
        "{}",
        list.text()
    );
    assert!(list.text().contains("Hello CLI"), "{}", list.text());
    let only = stubprint(&server, &["receipt", "list", "--limit", "1", "--json"])
        .await
        .json();
    assert_eq!(only.as_array().unwrap().len(), 1);

    let text = stubprint(&server, &["receipt", "show", "1"]).await;
    assert!(text.text().contains("Hello CLI"), "{}", text.text());
    let data = stubprint(&server, &["receipt", "show", "1", "--json"])
        .await
        .json();
    assert_eq!(data["id"], 1);
    assert_eq!(data["state"], "complete");
    // The bytes come back as they were sent, to a pipe and to a file.
    let raw = stubprint(&server, &["receipt", "show", "1", "--form", "bytes"]).await;
    assert_eq!(raw.out, bytes);
    let file = std::env::temp_dir().join(format!("stubprint-cli-{}.png", std::process::id()));
    let picture = stubprint(
        &server,
        &[
            "receipt",
            "show",
            "1",
            "--form",
            "picture",
            "-o",
            file.to_str().unwrap(),
        ],
    )
    .await;
    assert_eq!(picture.code, 0, "{}", picture.err);
    assert_eq!(&std::fs::read(&file).unwrap()[1..4], b"PNG");
    let _ = std::fs::remove_file(&file);
    let trace = stubprint(&server, &["receipt", "show", "1", "--form", "trace"]).await;
    assert!(trace.text().contains("Hello CLI"), "{}", trace.text());
    assert!(trace.text().contains("cut"), "{}", trace.text());

    let removed = stubprint(&server, &["receipt", "remove", "1"]).await;
    assert_eq!(removed.code, 0, "{}", removed.err);
    let gone = stubprint(&server, &["receipt", "show", "1"]).await;
    assert_eq!(gone.code, 1);
    assert!(gone.err.contains("no receipt 1"), "{}", gone.err);
    let cleared = stubprint(&server, &["receipt", "clear"]).await;
    assert!(
        cleared.text().contains("removed 1 receipts"),
        "{}",
        cleared.text()
    );
    server.running.stop().await;
}

#[tokio::test]
async fn wait_gives_the_next_receipt_and_ends_with_3_when_none_comes() {
    let server = start(&["name=till"]).await;

    let nothing = stubprint(&server, &["wait", "--timeout", "1"]).await;
    assert_eq!(nothing.code, 3);
    assert!(nothing.err.contains("no receipt came"), "{}", nothing.err);

    let waiting = {
        let address = server.address.clone();
        tokio::spawn(async move { stubprint_at(&address, &["wait", "--timeout", "10"], b"").await })
    };
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    stubprint_with(&server, &["print", "-"], &receipt(&["Waited for"])).await;
    let came = waiting.await.unwrap();
    assert_eq!(came.code, 0, "{}", came.err);
    assert!(came.text().contains("Waited for"), "{}", came.text());

    // Only the number, for a script, and as data.
    let by_id = {
        let address = server.address.clone();
        tokio::spawn(
            async move { stubprint_at(&address, &["wait", "--id", "--after", "1"], b"").await },
        )
    };
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    stubprint_with(&server, &["print", "-"], &receipt(&["Again"])).await;
    assert_eq!(by_id.await.unwrap().text().trim(), "2");
    server.running.stop().await;
}

#[tokio::test]
async fn decode_lists_the_commands_and_keeps_nothing() {
    let server = start(&["name=till"]).await;
    let decoded = stubprint_with(&server, &["decode", "-"], &receipt(&["Hi"])).await;
    assert_eq!(decoded.code, 0, "{}", decoded.err);
    assert!(decoded.text().contains("text \"Hi\""), "{}", decoded.text());
    assert!(decoded.text().contains("cut"), "{}", decoded.text());
    assert!(decoded.text().contains("1 receipts"), "{}", decoded.text());
    let list = stubprint(&server, &["receipt", "list", "--json"])
        .await
        .json();
    assert!(list.as_array().unwrap().is_empty());
    server.running.stop().await;
}

#[tokio::test]
async fn the_samples_models_connections_and_server_are_listed() {
    let server = start(&["name=till"]).await;
    let samples = stubprint(&server, &["sample", "list"]).await;
    assert!(samples.text().contains("grocery"), "{}", samples.text());
    let sample = stubprint(&server, &["sample", "show", "grocery"]).await;
    assert!(sample.text().contains("python-escpos"), "{}", sample.text());
    let models = stubprint(&server, &["model", "list", "--json"])
        .await
        .json();
    assert!(!models.as_array().unwrap().is_empty());
    let table = stubprint(&server, &["model", "list"]).await;
    assert!(table.text().contains("TM-T88IV"), "{}", table.text());

    stubprint_with(&server, &["print", "-"], &receipt(&["x"])).await;
    let connections = stubprint(&server, &["connection", "list"]).await;
    assert!(
        connections.text().contains("http"),
        "{}",
        connections.text()
    );
    let one = stubprint(&server, &["connection", "show", "1"])
        .await
        .json();
    assert_eq!(one["printer"], "till");

    let info = stubprint(&server, &["info"]).await;
    assert!(
        info.text().contains(&server.address[7..]),
        "{}",
        info.text()
    );
    let json = stubprint(&server, &["info", "--json"]).await.json();
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
    server.running.stop().await;
}

#[tokio::test]
async fn what_goes_wrong_is_said_in_words_and_ends_with_1() {
    let server = start(&["name=till"]).await;
    let unknown = stubprint(&server, &["print", "--printer", "nope", "-"]).await;
    assert_eq!(unknown.code, 1);
    assert!(
        unknown.err.contains("no printer called nope"),
        "{}",
        unknown.err
    );
    let sample = stubprint(&server, &["print", "--sample", "nothing"]).await;
    assert_eq!(sample.code, 1);
    assert!(
        sample.err.contains("no sample called nothing"),
        "{}",
        sample.err
    );
    let status = stubprint(&server, &["printer", "status", "till", "colour=red"]).await;
    assert_eq!(status.code, 1);
    let loose = stubprint(&server, &["printer", "set", "till", "idle_timeout"]).await;
    assert_eq!(loose.code, 1);
    assert!(loose.err.contains("key=value"), "{}", loose.err);
    let missing = stubprint(&server, &["print", "/no/such/file"]).await;
    assert_eq!(missing.code, 1);
    assert!(missing.err.contains("cannot read"), "{}", missing.err);

    // No server there.
    let nobody = stubprint_at("http://127.0.0.1:1", &["info"], b"").await;
    assert_eq!(nobody.code, 1);
    assert!(
        nobody.err.contains("cannot reach the server"),
        "{}",
        nobody.err
    );
    server.running.stop().await;
}

#[tokio::test]
async fn an_option_that_starts_a_server_does_not_go_with_a_command() {
    let server = start(&["name=till"]).await;
    let run = {
        let address = server.address.clone();
        tokio::task::spawn_blocking(move || {
            Command::new(env!("CARGO_BIN_EXE_stubprint"))
                .args(["--ephemeral", "--server", &address, "info"])
                .env_remove("STUBPRINT_SERVER")
                .output()
                .unwrap()
        })
        .await
        .unwrap()
    };
    assert_eq!(run.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&run.stderr).contains("running server"));
    server.running.stop().await;
}
