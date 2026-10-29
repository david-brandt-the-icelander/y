//! A small stand-in for Y's `standard.http`, written by hand: the `http<service>` capability (Y § 3.7), which reads
//! from one web service, and a fake of it for tests.
//!
//! It is a crate of its own, apart from `y-runtime`, so that the examples that do not reach the network do not build
//! an HTTP client. It is only as large as the examples need: it gets text.
//!
//! A call to the network may suspend (Y § 3.7), so `get` is an `async fn`. Under Macroquad's executor, which drives
//! no I/O, it does its work at once, blocking the frame it runs in; the examples call it while setting up, not while
//! drawing. Every wait is bounded (Y § 3.8): `get` takes the `Deadline` of the `within` it runs in, and fails with
//! `Within::Expired` when the deadline passes first.

use std::time::Duration;
use y_runtime::{Deadline, Expired, System, Within};

/// Y's `http_error`: why a service did not give what was asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpError {
    /// The HTTP status the service answered with, or none when no answer came.
    pub status: Option<u16>,
    /// What went wrong, for a person to read.
    pub message: String,
}

impl HttpError {
    /// The failure a fake service gives for every path when it answers with `status`: for a test to compare with.
    pub fn fake_status(status: u16) -> HttpError {
        HttpError { status: Some(status), message: format!("status {status}") }
    }
}

/// What a fake service answers.
enum Fake {
    /// Every path gives this text.
    Answer(String),
    /// Every path gives this status.
    Failing(u16),
    /// Every wait expires.
    Expiring,
}

/// The capability to read from one web service, or a fake of it for tests: Y's `http<service>`.
pub struct Http {
    /// The service's address, such as `https://openmoji.org`, to which each path is joined.
    address: String,
    fake: Option<Fake>,
}

/// `system.http(service)`: the capability for one service, from its address.
pub trait SystemHttp {
    fn http(&self, address: &str) -> Http;
}

impl SystemHttp for System {
    fn http(&self, address: &str) -> Http {
        Http { address: address.to_string(), fake: None }
    }
}

impl Http {
    /// A fake service for a test, which gives `text` for every path.
    pub fn fake(text: &str) -> Http {
        Http { address: String::new(), fake: Some(Fake::Answer(text.to_string())) }
    }

    /// A fake service for a test, which answers every path with an HTTP status, such as 404.
    pub fn fake_failing(status: u16) -> Http {
        Http { address: String::new(), fake: Some(Fake::Failing(status)) }
    }

    /// A fake service for a test, so slow that every wait expires.
    pub fn fake_expiring() -> Http {
        Http { address: String::new(), fake: Some(Fake::Expiring) }
    }

    /// Gets the text at a path of the service, such as `/data/color/svg/1F600.svg`, before a deadline.
    pub async fn get(&mut self, path: &str, deadline: &Deadline) -> Result<String, Within<HttpError>> {
        match &self.fake {
            Some(Fake::Answer(text)) => Ok(text.clone()),
            Some(Fake::Failing(status)) => Err(Within::Failed(HttpError::fake_status(*status))),
            Some(Fake::Expiring) => Err(Within::Expired(Expired::fake())),
            None => self.fetch(path, deadline),
        }
    }

    fn fetch(&self, path: &str, deadline: &Deadline) -> Result<String, Within<HttpError>> {
        let left: Duration = deadline.remaining().map_err(Within::Expired)?;
        let agent = ureq::AgentBuilder::new().try_proxy_from_env(true).timeout(left).build();
        let url = format!("{}{path}", self.address);
        let failed = |status: Option<u16>, message: String| {
            // a wait cut short by the deadline is an expiry, whatever the transport called it
            match deadline.remaining() {
                Err(expired) => Within::Expired(expired),
                Ok(_) => Within::Failed(HttpError { status, message }),
            }
        };
        match agent.get(&url).call() {
            Ok(answer) => answer.into_string().map_err(|e| failed(None, format!("{url}: {e}"))),
            Err(ureq::Error::Status(status, _)) => Err(failed(Some(status), format!("{url}: status {status}"))),
            // a transport error names the address itself
            Err(e) => Err(failed(None, e.to_string())),
        }
    }
}
