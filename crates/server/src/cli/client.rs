//! The HTTP client of the commands: it asks a running server and hands back what it said.

use std::io::{BufRead, BufReader};
use std::time::Duration;

use serde_json::Value;

/// What a command can fail with, in words for the person who typed it.
pub type Failure = String;

/// How long the client waits for a connection to open.
const CONNECT: Duration = Duration::from_secs(5);

/// What the server answered with a status that is not an error.
pub struct Answer {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Answer {
    pub fn json(&self) -> Result<Value, Failure> {
        serde_json::from_slice(&self.body)
            .map_err(|error| format!("the server did not answer with JSON: {error}"))
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// A running server, by its address.
pub struct Client {
    agent: ureq::Agent,
    base: String,
}

impl Client {
    pub fn new(server: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            // An error is a status like any other: the answer says what went wrong.
            .http_status_as_error(false)
            .timeout_connect(Some(CONNECT))
            .build()
            .into();
        Self {
            agent,
            base: server.trim_end_matches('/').to_string(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    /// Turns what came back into an answer, or into what went wrong in words.
    fn finish(
        &self,
        result: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    ) -> Result<Answer, Failure> {
        let mut response = result.map_err(|error| {
            format!(
                "cannot reach the server at {}: {error}. Is it running? Start it with `stubprint`, or say where it is with --server.",
                self.base
            )
        })?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .with_config()
            .limit(u64::MAX)
            .read_to_vec()
            .map_err(|error| format!("the answer of the server was cut: {error}"))?;
        if status >= 400 {
            let said = serde_json::from_slice::<Value>(&body)
                .ok()
                .and_then(|value| value["error"].as_str().map(str::to_string));
            return Err(said.unwrap_or_else(|| format!("the server answered {status}")));
        }
        Ok(Answer { status, body })
    }

    pub fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Answer, Failure> {
        let request = self
            .agent
            .get(self.url(path))
            .query_pairs(query.iter().map(|(key, value)| (*key, value.as_str())));
        self.finish(request.call())
    }

    pub fn delete(&self, path: &str, query: &[(&str, String)]) -> Result<Answer, Failure> {
        let request = self
            .agent
            .delete(self.url(path))
            .query_pairs(query.iter().map(|(key, value)| (*key, value.as_str())));
        self.finish(request.call())
    }

    pub fn post_bytes(
        &self,
        path: &str,
        query: &[(&str, String)],
        bytes: &[u8],
    ) -> Result<Answer, Failure> {
        let request = self
            .agent
            .post(self.url(path))
            .query_pairs(query.iter().map(|(key, value)| (*key, value.as_str())))
            .header("content-type", "application/octet-stream");
        self.finish(request.send(bytes))
    }

    pub fn post_empty(&self, path: &str, query: &[(&str, String)]) -> Result<Answer, Failure> {
        let request = self
            .agent
            .post(self.url(path))
            .query_pairs(query.iter().map(|(key, value)| (*key, value.as_str())));
        self.finish(request.send_empty())
    }

    pub fn post_json(&self, path: &str, body: &Value) -> Result<Answer, Failure> {
        self.finish(self.agent.post(self.url(path)).send_json(body))
    }

    pub fn patch_json(&self, path: &str, body: &Value) -> Result<Answer, Failure> {
        self.finish(self.agent.patch(self.url(path)).send_json(body))
    }

    /// Opens a stream that does not end, line by line.
    pub fn stream(&self, path: &str) -> Result<impl BufRead, Failure> {
        let response = self
            .agent
            .get(self.url(path))
            .call()
            .map_err(|error| format!("cannot reach the server at {}: {error}", self.base))?;
        Ok(BufReader::new(response.into_body().into_reader()))
    }
}
