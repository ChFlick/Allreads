//! Machine translation behind one small trait, so services can be swapped.
//!
//! Only DeepL is implemented. Its free tier (an account and a key, no cost
//! within the monthly quota) is enough for reading. The API key is passed in
//! per call from the Android Keystore; it is never stored or logged.

pub mod deepl;

use std::time::Duration;

use thiserror::Error;

/// Errors from a translation service. None of them contain the API key.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TranslateError {
    #[error("could not reach the translation service: {0}")]
    Network(String),
    #[error("the translation service rejected the API key")]
    Unauthorized,
    #[error("the translation quota is used up")]
    QuotaExceeded,
    /// Still too many requests after retrying; try again in a moment.
    #[error("the translation service is busy; try again in a moment")]
    RateLimited,
    #[error("the translation service answered HTTP {status}: {message}")]
    Http { status: u16, message: String },
    #[error("unexpected answer from the translation service: {0}")]
    BadResponse(String),
}

/// What to translate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranslationRequest<'a> {
    pub text: &'a str,
    /// Text that guides the translation without being translated, e.g. the
    /// sentence around a tapped word.
    pub context: Option<&'a str>,
    /// Lower-case language codes, e.g. `fr`.
    pub source: &'a str,
    pub target: &'a str,
}

/// A translation service.
pub trait Translator {
    /// A stable name, used in the translation cache key.
    fn name(&self) -> &'static str;

    /// Translates one text.
    ///
    /// # Errors
    /// Returns a `TranslateError` if the service cannot be reached or refuses.
    fn translate(&self, request: &TranslationRequest) -> Result<String, TranslateError>;
}

/// The HTTP client every service uses: short timeouts, status codes handled by
/// the caller, and no proxy from environment variables, so behaviour is the
/// same on a phone and in tests.
pub(crate) fn http_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .timeout_connect(Some(Duration::from_secs(5)))
        .http_status_as_error(false)
        .proxy(None)
        .build()
        .into()
}

#[cfg(any(test, feature = "test-support"))]
pub mod mock_server {
    //! A local HTTP server for tests: it answers a fixed list of requests, in order.
    //! Other crates' tests get it through the `test-support` feature.

    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    /// Serves one request with `status` and a JSON `body`. Returns the base URL
    /// and a handle that yields the raw request (head and body) once served.
    ///
    /// # Panics
    /// Panics if the test server cannot bind or the request is malformed.
    #[must_use]
    pub fn serve_once(status: u16, body: &str) -> (String, JoinHandle<String>) {
        let (url, handle) = serve(&[(status, body)]);
        (
            url,
            std::thread::spawn(move || handle.join().unwrap().remove(0)),
        )
    }

    /// Serves one request per response, in order. The handle yields every raw request.
    ///
    /// # Panics
    /// Panics if the test server cannot bind.
    #[must_use]
    pub fn serve(responses: &[(u16, &str)]) -> (String, JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let responses: Vec<(u16, String)> = responses
            .iter()
            .map(|(s, b)| (*s, (*b).to_string()))
            .collect();
        let handle = std::thread::spawn(move || {
            responses
                .into_iter()
                .map(|(status, body)| answer(&listener, status, &body))
                .collect()
        });
        (url, handle)
    }

    fn answer(listener: &TcpListener, status: u16, body: &str) -> String {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut request = String::new();
        let mut content_length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                content_length = value.trim().parse().unwrap();
            }
            request.push_str(&line);
            if line == "\r\n" || line.is_empty() {
                break;
            }
        }
        let mut request_body = vec![0; content_length];
        reader.read_exact(&mut request_body).unwrap();
        request.push_str(&String::from_utf8(request_body).unwrap());
        let response = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n\
             X-Trace-ID: trace-123\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        reader.get_mut().write_all(response.as_bytes()).unwrap();
        request
    }
}
