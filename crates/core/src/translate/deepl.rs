//! DeepL (<https://www.deepl.com/pro-api>), API v2.
//!
//! Checked against DeepL's `OpenAPI` spec 3.17.0 for `POST /v2/translate`:
//! - `https://api.deepl.com` for Pro keys and `https://api-free.deepl.com` for
//!   Free keys. Free keys end in `:fx`, which DeepL's own client libraries use
//!   to pick the host;
//! - `Authorization: DeepL-Auth-Key <key>` and a JSON body with `text` (an
//!   array), `target_lang`, and optional `source_lang` and `context`. The
//!   context guides the translation and is not billed;
//! - the answer is `{"translations": [{"detected_source_language", "text"}]}`;
//! - errors carry a JSON `message` (or `error.message` from DeepL's edge
//!   servers) and an `X-Trace-ID` header. 403 means a bad key and 456 means the
//!   quota is used up; 429, 529 and 5xx are retried with backoff, 456 and 400 never.

use std::thread::sleep;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};

use super::{http_agent, TranslateError, TranslationRequest, Translator};

/// How many times a busy or failing request is retried.
const RETRIES: u32 = 2;

/// The DeepL client.
pub struct DeepL {
    key: String,
    base_url: String,
    agent: ureq::Agent,
    /// The wait before the first retry; it doubles each time.
    first_retry_delay: Duration,
}

impl DeepL {
    /// A client for this key, using the Free or Pro endpoint as the key requires.
    #[must_use]
    pub fn new(key: &str) -> Self {
        let base_url = if key.trim().ends_with(":fx") {
            "https://api-free.deepl.com"
        } else {
            "https://api.deepl.com"
        };
        let mut client = Self::with_base_url(key, base_url);
        client.first_retry_delay = Duration::from_millis(500);
        client
    }

    /// A client that talks to `base_url`, for tests. It retries almost at once.
    #[must_use]
    pub fn with_base_url(key: &str, base_url: &str) -> Self {
        DeepL {
            key: key.trim().to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            agent: http_agent(),
            first_retry_delay: Duration::from_millis(5),
        }
    }

    /// The endpoint this client uses (never includes the key).
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// One HTTP attempt: the status and the body as text, plus the trace id.
    fn attempt(&self, body: &Value) -> Result<(u16, String, Option<String>), TranslateError> {
        let mut response = self
            .agent
            .post(format!("{}/v2/translate", self.base_url))
            .header("Authorization", format!("DeepL-Auth-Key {}", self.key))
            .send_json(body)
            .map_err(|e| TranslateError::Network(e.to_string()))?;
        let status = response.status().as_u16();
        let trace_id = response
            .headers()
            .get("X-Trace-ID")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| TranslateError::Network(e.to_string()))?;
        Ok((status, text, trace_id))
    }
}

/// DeepL's language code: upper case, and a regional variant for the targets
/// that have one.
fn deepl_code(code: &str, is_target: bool) -> String {
    match (code.to_lowercase().as_str(), is_target) {
        ("en", true) => "EN-US".to_string(),
        ("pt", true) => "PT-BR".to_string(),
        (other, _) => other.to_uppercase(),
    }
}

/// True for answers DeepL says to retry: too many requests, or a server error.
fn is_retryable(status: u16) -> bool {
    status == 429 || status == 529 || (500..600).contains(&status)
}

/// DeepL's error message: `message`, or `error.message` from its edge servers,
/// or the start of the raw body.
fn error_message(body: &str, trace_id: Option<&str>) -> String {
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    let message = parsed
        .as_ref()
        .and_then(|v| {
            v["message"]
                .as_str()
                .or_else(|| v["error"]["message"].as_str())
        })
        .map_or_else(|| body.chars().take(200).collect(), str::to_string);
    match trace_id {
        Some(id) => format!("{message} (trace id {id})"),
        None => message,
    }
}

/// How much of the monthly character quota a key has used, from `GET /v2/usage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Usage {
    pub character_count: u64,
    pub character_limit: u64,
}

impl DeepL {
    /// Checks the key and reads its usage. This costs no quota, so it is the
    /// way to test a newly entered key.
    ///
    /// # Errors
    /// Returns `Unauthorized` for a rejected key, `Network` when offline, and
    /// the other `TranslateError`s like a translation would.
    pub fn usage(&self) -> Result<Usage, TranslateError> {
        let mut response = self
            .agent
            .get(format!("{}/v2/usage", self.base_url))
            .header("Authorization", format!("DeepL-Auth-Key {}", self.key))
            .call()
            .map_err(|e| TranslateError::Network(e.to_string()))?;
        let status = response.status().as_u16();
        let trace_id = response
            .headers()
            .get("X-Trace-ID")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| TranslateError::Network(e.to_string()))?;
        check_status(status, &text, trace_id.as_deref())?;
        serde_json::from_str(&text).map_err(|e| TranslateError::BadResponse(e.to_string()))
    }
}

/// Maps DeepL's error statuses to `TranslateError`; `Ok` for 200.
fn check_status(status: u16, body: &str, trace_id: Option<&str>) -> Result<(), TranslateError> {
    match status {
        200 => Ok(()),
        401 | 403 => Err(TranslateError::Unauthorized),
        456 => Err(TranslateError::QuotaExceeded),
        429 | 529 => Err(TranslateError::RateLimited),
        _ => Err(TranslateError::Http {
            status,
            message: error_message(body, trace_id),
        }),
    }
}

#[derive(Deserialize)]
struct Response {
    translations: Vec<Translation>,
}

#[derive(Deserialize)]
struct Translation {
    text: String,
}

impl Translator for DeepL {
    fn name(&self) -> &'static str {
        "deepl"
    }

    fn translate(&self, request: &TranslationRequest) -> Result<String, TranslateError> {
        let mut body = json!({
            "text": [request.text],
            "source_lang": deepl_code(request.source, false),
            "target_lang": deepl_code(request.target, true),
        });
        if let Some(context) = request.context.filter(|c| !c.trim().is_empty()) {
            body["context"] = json!(context);
        }

        let mut delay = self.first_retry_delay;
        let mut attempt = 0;
        let (status, text, trace_id) = loop {
            let (status, text, trace_id) = self.attempt(&body)?;
            if is_retryable(status) && attempt < RETRIES {
                attempt += 1;
                sleep(delay);
                delay *= 2;
                continue;
            }
            break (status, text, trace_id);
        };

        check_status(status, &text, trace_id.as_deref())?;
        let parsed: Response =
            serde_json::from_str(&text).map_err(|e| TranslateError::BadResponse(e.to_string()))?;
        parsed
            .translations
            .into_iter()
            .next()
            .map(|t| t.text)
            .ok_or_else(|| TranslateError::BadResponse("no translations in the answer".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::super::mock_server::{serve, serve_once};
    use super::*;

    fn request<'a>(text: &'a str, context: Option<&'a str>) -> TranslationRequest<'a> {
        TranslationRequest {
            text,
            context,
            source: "fr",
            target: "en",
        }
    }

    fn body_of(raw: &str) -> Value {
        let (_, body) = raw.split_once("\r\n\r\n").expect("request has a body");
        serde_json::from_str(body).unwrap()
    }

    #[test]
    fn free_keys_use_the_free_endpoint() {
        assert_eq!(
            DeepL::new("abc:fx").base_url(),
            "https://api-free.deepl.com"
        );
        assert_eq!(DeepL::new("abc").base_url(), "https://api.deepl.com");
    }

    #[test]
    fn target_codes_use_the_regional_variant_where_needed() {
        assert_eq!(deepl_code("en", true), "EN-US");
        assert_eq!(deepl_code("en", false), "EN");
        assert_eq!(deepl_code("fr", true), "FR");
    }

    #[test]
    fn a_translation_is_returned_and_the_request_is_well_formed() {
        let (url, server) = serve_once(
            200,
            r#"{"translations":[{"detected_source_language":"FR","text":"The cat sleeps."}]}"#,
        );
        let deepl = DeepL::with_base_url("secret-key:fx", &url);
        assert_eq!(
            deepl.translate(&request("Le chat dort.", None)).unwrap(),
            "The cat sleeps."
        );
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /v2/translate "), "{raw}");
        assert!(raw.contains("DeepL-Auth-Key secret-key:fx"), "{raw}");
        assert_eq!(
            body_of(&raw),
            json!({"text": ["Le chat dort."], "source_lang": "FR", "target_lang": "EN-US"})
        );
    }

    #[test]
    fn context_is_sent_when_given() {
        let (url, server) = serve_once(200, r#"{"translations":[{"text":"reads"}]}"#);
        let deepl = DeepL::with_base_url("k", &url);
        assert_eq!(
            deepl
                .translate(&request("lit", Some("Il lit un livre.")))
                .unwrap(),
            "reads"
        );
        assert_eq!(
            body_of(&server.join().unwrap())["context"],
            json!("Il lit un livre.")
        );
    }

    #[test]
    fn busy_and_server_errors_are_retried_then_succeed() {
        let (url, server) = serve(&[
            (429, r#"{"message":"Too many requests"}"#),
            (503, r#"{"error":{"message":"Bad Gateway."}}"#),
            (200, r#"{"translations":[{"text":"cat"}]}"#),
        ]);
        let deepl = DeepL::with_base_url("k", &url);
        assert_eq!(deepl.translate(&request("chat", None)).unwrap(), "cat");
        assert_eq!(server.join().unwrap().len(), 3);
    }

    #[test]
    fn still_busy_after_retrying_is_rate_limited() {
        let busy = r#"{"message":"Too many requests"}"#;
        let (url, server) = serve(&[(429, busy), (429, busy), (429, busy)]);
        let error = DeepL::with_base_url("k", &url)
            .translate(&request("chat", None))
            .unwrap_err();
        assert_eq!(error, TranslateError::RateLimited);
        assert_eq!(server.join().unwrap().len(), 3);
    }

    #[test]
    fn quota_and_bad_requests_are_never_retried() {
        let (url, server) = serve_once(456, r#"{"message":"Quota exceeded"}"#);
        let error = DeepL::with_base_url("k", &url)
            .translate(&request("chat", None))
            .unwrap_err();
        server.join().unwrap();
        assert_eq!(error, TranslateError::QuotaExceeded);

        let (url, server) = serve_once(
            400,
            r#"{"message":"Value for 'target_lang' not supported."}"#,
        );
        let error = DeepL::with_base_url("k", &url)
            .translate(&request("chat", None))
            .unwrap_err();
        server.join().unwrap();
        assert_eq!(
            error,
            TranslateError::Http {
                status: 400,
                message: "Value for 'target_lang' not supported. (trace id trace-123)".into()
            }
        );
    }

    #[test]
    fn a_rejected_key_is_unauthorized_and_does_not_leak_the_key() {
        let (url, server) = serve_once(403, r#"{"message":"Forbidden"}"#);
        let error = DeepL::with_base_url("secret-key", &url)
            .translate(&request("chat", None))
            .unwrap_err();
        server.join().unwrap();
        assert_eq!(error, TranslateError::Unauthorized);
        assert!(!error.to_string().contains("secret-key"));
    }

    #[test]
    fn usage_checks_the_key_and_reads_the_quota() {
        let (url, server) =
            serve_once(200, r#"{"character_count":12345,"character_limit":500000}"#);
        let usage = DeepL::with_base_url("good-key:fx", &url).usage().unwrap();
        assert_eq!(
            usage,
            Usage {
                character_count: 12345,
                character_limit: 500_000
            }
        );
        let raw = server.join().unwrap();
        assert!(raw.starts_with("GET /v2/usage "), "{raw}");
        assert!(raw.contains("DeepL-Auth-Key good-key:fx"), "{raw}");

        let (url, server) = serve_once(403, r#"{"message":"Forbidden"}"#);
        let error = DeepL::with_base_url("bad-key", &url).usage().unwrap_err();
        server.join().unwrap();
        assert_eq!(error, TranslateError::Unauthorized);
    }

    #[test]
    fn edge_server_errors_show_their_nested_message() {
        assert_eq!(
            error_message(r#"{"error":{"message":"Bad Gateway."}}"#, None),
            "Bad Gateway."
        );
        assert_eq!(
            error_message("plain text", Some("t1")),
            "plain text (trace id t1)"
        );
    }

    #[test]
    fn a_malformed_answer_is_a_bad_response() {
        let (url, server) = serve_once(200, r#"{"translations":[]}"#);
        let error = DeepL::with_base_url("k", &url)
            .translate(&request("chat", None))
            .unwrap_err();
        server.join().unwrap();
        assert!(matches!(error, TranslateError::BadResponse(_)));
    }

    #[test]
    fn an_unreachable_service_is_a_network_error() {
        // Port 9 (discard) on localhost is closed on test machines.
        let error = DeepL::with_base_url("k", "http://127.0.0.1:9")
            .translate(&request("chat", None))
            .unwrap_err();
        assert!(matches!(error, TranslateError::Network(_)), "{error:?}");
    }
}
