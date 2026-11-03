use super::*;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex, atomic::Ordering},
    thread,
    time::{Duration, Instant},
};

pub(in crate::relay) const ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub(in crate::relay) const INPUT_ID: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const REV: &str = "cccccccccccccccccccccccccccccccccccccccc";
const TOKEN: &str = "synthetic_test_token";

struct Request {
    head: String,
    body: String,
}

pub(in crate::relay) struct Fixture {
    origin: String,
    requests: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Fixture {
    pub(in crate::relay) fn new(replies: Vec<Option<String>>) -> Self {
        Self::with_cancel(replies, None)
    }

    fn with_cancel(replies: Vec<Option<String>>, cancel: Option<Arc<AtomicBool>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let worker = thread::spawn(move || {
            for reply in replies {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    if stopped.load(Ordering::SeqCst) || Instant::now() >= deadline {
                        return;
                    }
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(error) => panic!("fixture accept: {error}"),
                    }
                };
                // Accepted sockets inherit nonblocking mode on macOS; request arrival can race accept.
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buf = [0; 4096];
                let (head_len, body_len) = loop {
                    let count = stream.read(&mut buf).unwrap();
                    assert!(count > 0, "request ended before headers");
                    bytes.extend_from_slice(&buf[..count]);
                    if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&bytes[..index]);
                        let length = head
                            .lines()
                            .find_map(|line| {
                                let (key, value) = line.split_once(':')?;
                                key.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        break (index + 4, length);
                    }
                };
                while bytes.len() < head_len + body_len {
                    let count = stream.read(&mut buf).unwrap();
                    assert!(count > 0, "request ended before body");
                    bytes.extend_from_slice(&buf[..count]);
                }
                recorded.lock().unwrap().push(Request {
                    head: String::from_utf8(bytes[..head_len].to_vec()).unwrap(),
                    body: String::from_utf8(bytes[head_len..head_len + body_len].to_vec()).unwrap(),
                });
                if let Some(cancel) = &cancel {
                    cancel.store(true, Ordering::SeqCst);
                }
                if let Some(reply) = reply {
                    let _ = stream.write_all(reply.as_bytes());
                }
            }
        });
        Self {
            origin,
            requests,
            stop,
            worker: Some(worker),
        }
    }

    pub(in crate::relay) fn client(&self) -> GistClient {
        GistClient::for_test(TOKEN, &self.origin)
    }

    pub(in crate::relay) fn request_methods(&self) -> Vec<String> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|request| request.head.split_whitespace().next().unwrap().to_owned())
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.worker.take().unwrap().join().unwrap();
    }
}

pub(in crate::relay) fn response(status: u16, headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
        body.len()
    )
}

#[test]
fn latest_raw_source_is_fetched_directly_without_credentials() {
    let fixture = Fixture::new(vec![Some(response(
        200,
        "Content-Type: text/plain\r\n",
        "hello",
    ))]);
    let input = format!("https://gist.githubusercontent.com/example/{ID}/raw/nodes.txt");
    let source = fixture
        .client()
        .read_source(&input, "", &AtomicBool::new(false))
        .unwrap();
    assert_eq!(source.content.as_str(), "hello");
    let requests = fixture.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(
        requests[0]
            .head
            .starts_with(&format!("GET /example/{ID}/raw/nodes.txt "))
    );
    assert!(
        !requests[0]
            .head
            .to_ascii_lowercase()
            .contains("authorization:")
    );
}

#[test]
fn raw_source_rejects_redirect_html_and_oversized_content() {
    let input = format!("https://gist.githubusercontent.com/example/{ID}/raw/nodes.txt");
    for (reply, expected) in [
        (
            response(302, "Location: https://example.com/\r\n", ""),
            GistError::HttpStatus(302),
        ),
        (
            response(200, "Content-Type: text/html; charset=utf-8\r\n", "<html>"),
            GistError::InvalidResponse,
        ),
        (
            response(200, "", &"x".repeat(MAX_CONTENT_BYTES + 1)),
            GistError::TooLarge,
        ),
    ] {
        let fixture = Fixture::new(vec![Some(reply)]);
        assert!(
            matches!(fixture.client().read_source(&input, "", &AtomicBool::new(false)), Err(error) if error == expected)
        );
        assert_eq!(fixture.request_methods(), vec!["GET"]);
    }
}

#[test]
fn rate_limits_are_typed_without_retry_and_plain_forbidden_is_not() {
    let input = format!("https://gist.githubusercontent.com/example/{ID}/raw/nodes.txt");
    for (status, headers, expected) in [
        (
            429,
            "Retry-After: 12\r\n",
            GistError::RateLimited {
                retry_after_seconds: Some(12),
            },
        ),
        (
            403,
            "X-RateLimit-Remaining: 0\r\n",
            GistError::RateLimited {
                retry_after_seconds: None,
            },
        ),
        (403, "", GistError::HttpStatus(403)),
        (
            403,
            "Retry-After: 12\r\n",
            GistError::RateLimited {
                retry_after_seconds: Some(12),
            },
        ),
    ] {
        let fixture = Fixture::new(vec![Some(response(status, headers, "secret response"))]);
        assert!(
            matches!(fixture.client().read_source(&input, "", &AtomicBool::new(false)), Err(error) if error == expected)
        );
        assert_eq!(fixture.request_methods(), vec!["GET"]);
    }
}

#[test]
fn rate_limit_uses_later_reset_without_shortening_wait() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let headers = format!(
        "Retry-After: 1\r\nX-RateLimit-Remaining: 0\r\nX-RateLimit-Reset: {}\r\n",
        now + 600
    );
    let fixture = Fixture::new(vec![Some(response(403, &headers, ""))]);
    let input = format!("https://gist.githubusercontent.com/example/{ID}/raw/nodes.txt");
    assert!(
        matches!(fixture.client().read_source(&input, "", &AtomicBool::new(false)), Err(GistError::RateLimited { retry_after_seconds: Some(seconds) }) if (590..=600).contains(&seconds))
    );
    assert_eq!(fixture.request_methods(), ["GET"]);
}

pub(in crate::relay) fn document(content: &str) -> Value {
    json!({
        "id": ID, "description": "relay", "public": false, "truncated": false,
        "updated_at": "2026-01-01T00:00:00Z", "history": [{ "version": REV }],
        "files": { "nodes.txt": { "filename": "nodes.txt", "truncated": false,
            "content": content, "size": content.len(),
            "raw_url": format!("https://gist.githubusercontent.com/example/{ID}/raw/{REV}/nodes.txt") }
        }
    })
}

pub(in crate::relay) fn api(value: Value) -> Option<String> {
    Some(response(
        200,
        "Content-Type: application/json\r\n",
        &value.to_string(),
    ))
}

pub(in crate::relay) fn created(value: Value) -> Option<String> {
    Some(response(
        201,
        "Content-Type: application/json\r\n",
        &value.to_string(),
    ))
}

#[test]
fn creation_posts_one_secret_gist_and_verifies_its_independent_target() {
    let fixture = Fixture::new(vec![
        created(document("new nodes")),
        api(document("new nodes")),
    ]);
    let (receipt, link) = fixture
        .client()
        .create("nodes.txt", "new nodes", "relay", &AtomicBool::new(false))
        .unwrap();
    assert_eq!(receipt.id, ID);
    assert_eq!(receipt.url, format!("https://gist.github.com/{ID}"));
    assert_policy_link(link);
    let requests = fixture.requests.lock().unwrap();
    assert!(requests[0].head.starts_with("POST /gists HTTP/1.1"));
    assert!(
        requests[1]
            .head
            .starts_with(&format!("GET /gists/{ID} HTTP/1.1"))
    );
    assert_eq!(
        serde_json::from_str::<Value>(&requests[0].body).unwrap(),
        json!({
            "public": false, "description": "relay", "files": {"nodes.txt": {"content": "new nodes"}}
        })
    );
}

#[test]
fn invalid_readback_raw_metadata_never_becomes_a_policy_link() {
    for raw in [
        Value::Null,
        json!(format!(
            "https://not-github.invalid/example/{ID}/raw/nodes.txt"
        )),
        json!(format!(
            "https://gist.githubusercontent.com/example/{INPUT_ID}/raw/nodes.txt"
        )),
        json!(format!(
            "https://gist.githubusercontent.com/example/{ID}/raw/other.txt"
        )),
        json!(format!(
            "https://gist.githubusercontent.com/example/{ID}/raw/nodes.txt?token=secret"
        )),
    ] {
        let mut readback = document("nodes");
        readback["files"]["nodes.txt"]["raw_url"] = raw;
        let fixture = Fixture::new(vec![created(document("nodes")), api(readback)]);
        let (created, link) = fixture
            .client()
            .create("nodes.txt", "nodes", "relay", &AtomicBool::new(false))
            .unwrap();
        assert_eq!(created.id, ID);
        assert!(link.is_none());
        assert_eq!(fixture.request_methods(), ["POST", "GET"]);
    }
}

#[test]
fn policy_link_encodes_profile_delimiters_and_keeps_receipt_out_of_debug() {
    let file = GistFile {
        id: ID.to_owned(),
        filename: "nodes, group=\"other\";#% .txt".to_owned(),
    };
    let raw = format!(
        "https://gist.githubusercontent.com/example/{ID}/raw/{REV}/nodes%2C%20group%3D%22other%22%3B%23%25%20.txt"
    );
    let link = PolicyLink::from_raw(&raw, &file).unwrap();
    assert!(link.matches(&file));
    assert!(!link.matches(&selected(ID)));
    let line = link.policy_group();
    assert_eq!(
        line.as_str(),
        format!(
            "Script Hub = select, policy-path=https://gist.githubusercontent.com/example/{ID}/raw/nodes%2C%20group%3D%22other%22%3B%23%25%20.txt"
        )
    );
    assert_eq!(line.lines().count(), 1);
    assert!(!line.contains(REV) && !line.contains("Authorization") && !line.contains(TOKEN));
    assert_eq!(format!("{link:?}"), "PolicyLink([redacted])");
}

#[test]
fn creation_never_retries_a_lost_or_ambiguous_post_response() {
    for reply in [
        None,
        Some(response(503, "", "")),
        Some(response(201, "", "{}")),
    ] {
        let fixture = Fixture::new(vec![reply]);
        let error = fixture
            .client()
            .create("nodes.txt", "nodes", "relay", &AtomicBool::new(false))
            .unwrap_err();
        assert_eq!(error, GistError::UncertainWrite);
        assert_eq!(fixture.request_methods(), ["POST"]);
    }
}

#[test]
fn creation_keeps_its_receipt_when_readback_fails_or_is_public_or_mismatched() {
    let mut public = document("nodes");
    public["public"] = json!(true);
    for readback in [None, api(public), api(document("different"))] {
        let fixture = Fixture::new(vec![created(document("nodes")), readback]);
        let error = fixture
            .client()
            .create("nodes.txt", "nodes", "relay", &AtomicBool::new(false))
            .unwrap_err();
        assert_eq!(
            error,
            GistError::CreatedUnverified(CreatedGist {
                id: ID.to_owned(),
                url: format!("https://gist.github.com/{ID}")
            })
        );
        assert_eq!(fixture.request_methods(), ["POST", "GET"]);
    }
}

#[test]
fn cancellation_after_creation_still_recovers_the_id_without_another_post() {
    let cancel = Arc::new(AtomicBool::new(false));
    let fixture = Fixture::with_cancel(vec![created(document("nodes"))], Some(Arc::clone(&cancel)));
    let error = fixture
        .client()
        .create("nodes.txt", "nodes", "relay", &cancel)
        .unwrap_err();
    assert!(matches!(error, GistError::CreatedUnverified(receipt) if receipt.id == ID));
    assert_eq!(fixture.request_methods(), ["POST"]);
}

#[test]
fn creation_rejects_invalid_inputs_and_preexisting_cancellation_before_io() {
    let fixture = Fixture::new(vec![]);
    let client = fixture.client();
    assert_eq!(
        client.create("../nodes", "nodes", "relay", &AtomicBool::new(false)),
        Err(GistError::InvalidName)
    );
    assert_eq!(
        client.create(
            "nodes.txt",
            "nodes",
            &"界".repeat(65),
            &AtomicBool::new(false)
        ),
        Err(GistError::InvalidName)
    );
    assert_eq!(
        client.create("nodes.txt", "nodes", "relay", &AtomicBool::new(true)),
        Err(GistError::Cancelled)
    );
    assert!(fixture.request_methods().is_empty());
}

fn selected(id: &str) -> GistFile {
    GistFile {
        id: id.to_owned(),
        filename: "nodes.txt".to_owned(),
    }
}

fn assert_policy_link(link: Option<PolicyLink>) {
    let link = link.unwrap();
    assert!(link.matches(&selected(ID)));
    assert!(!link.matches(&selected(INPUT_ID)));
    assert_eq!(
        link.policy_group().as_str(),
        format!(
            "Script Hub = select, policy-path=https://gist.githubusercontent.com/example/{ID}/raw/nodes.txt"
        )
    );
}

fn expected(content: &str) -> Snapshot {
    Snapshot {
        content: Some(Zeroizing::new(content.to_owned())),
        revision: format!("{REV}:2026-01-01T00:00:00Z"),
        description: "relay".to_owned(),
        public: false,
    }
}

#[test]
fn names_count_unicode_characters_and_reject_controls() {
    assert!(validate_name("").is_ok());
    assert!(validate_name(&"节".repeat(64)).is_ok());
    assert_eq!(validate_name(&"节".repeat(65)), Err(GistError::InvalidName));
    assert_eq!(validate_name("name\nprivate"), Err(GistError::InvalidName));
}

#[test]
fn filenames_include_extension_in_limit_and_reject_paths() {
    assert!(GistFile::parse(ID, &format!("{}.txt", "a".repeat(60))).is_ok());
    for filename in ["", ".", "..", "../nodes.txt", "a/b", "a\\b", "a\r\n"] {
        assert!(GistFile::parse(ID, filename).is_err());
    }
    assert!(GistFile::parse(ID, &format!("{}.txt", "a".repeat(61))).is_err());
}

#[test]
fn accepts_explicit_gist_and_raw_file_targets() {
    for target in [
        ID.to_owned(),
        format!("https://gist.github.com/example/{ID}"),
        format!("https://api.github.com/gists/{ID}"),
    ] {
        let file = GistFile::parse(&target, "nodes.txt").unwrap();
        assert_eq!(
            (file.id.as_str(), file.filename.as_str()),
            (ID, "nodes.txt")
        );
    }
    let file = GistFile::parse(
        &format!("https://gist.githubusercontent.com/example/{ID}/raw/%E8%8A%82.txt"),
        "",
    )
    .unwrap();
    assert_eq!(file.filename, "节.txt");
}

#[test]
fn source_resolution_infers_raw_and_single_file_without_changing_strict_parse() {
    assert!(GistFile::parse(ID, "").is_err());
    assert!(GistFile::parse_source(ID, "").unwrap().filename.is_empty());
    let fixture = Fixture::new(vec![api(document("private"))]);
    let cancel = AtomicBool::new(false);
    let raw = format!("https://gist.githubusercontent.com/example/{ID}/raw/nodes.txt");
    assert_eq!(
        fixture
            .client()
            .resolve_source(&raw, "", &cancel)
            .unwrap()
            .filename,
        "nodes.txt"
    );
    assert_eq!(
        fixture
            .client()
            .resolve_source(ID, "", &cancel)
            .unwrap()
            .filename,
        "nodes.txt"
    );
    assert_eq!(fixture.request_methods(), vec!["GET"]);
}

#[test]
fn source_resolution_requires_complete_valid_unambiguous_file_list() {
    let mut multiple = document("private");
    multiple["files"]["other.txt"] = multiple["files"]["nodes.txt"].clone();
    multiple["files"]["other.txt"]["filename"] = json!("other.txt");
    let mut empty = document("private");
    empty["files"] = json!({});
    let mut truncated = document("private");
    truncated["truncated"] = json!(true);
    let mut bad = document("private");
    bad["files"] = json!({"../bad": {"filename":"../bad", "size":0, "truncated":false}});
    let mut mismatch = document("private");
    mismatch["files"]["nodes.txt"]["filename"] = json!("other.txt");
    let mut invalid_metadata = document("private");
    invalid_metadata["history"] = json!([]);
    let fixture = Fixture::new(vec![
        api(multiple),
        api(empty),
        api(truncated),
        api(bad),
        api(mismatch),
        api(invalid_metadata),
    ]);
    let client = fixture.client();
    let cancel = AtomicBool::new(false);
    assert!(
        matches!(client.resolve_source(ID, "", &cancel), Err(GistError::SelectFile(names)) if names == ["nodes.txt", "other.txt"])
    );
    assert!(matches!(
        client.resolve_source(ID, "", &cancel),
        Err(GistError::InvalidResponse)
    ));
    assert!(matches!(
        client.resolve_source(ID, "", &cancel),
        Err(GistError::IncompleteFiles)
    ));
    for _ in 0..3 {
        assert!(matches!(
            client.resolve_source(ID, "", &cancel),
            Err(GistError::InvalidResponse)
        ));
    }
}

#[test]
fn revision_pinned_raw_urls_cannot_silently_select_the_latest_gist() {
    let target = format!("https://gist.githubusercontent.com/example/{ID}/raw/{REV}/nodes.txt");
    for filename in ["", "nodes.txt"] {
        assert!(matches!(
            GistFile::parse(&target, filename),
            Err(GistError::PinnedTarget)
        ));
    }
}

#[test]
fn rejects_untrusted_urls_and_ambiguous_file_selection() {
    for target in [
        format!("http://gist.github.com/example/{ID}"),
        format!("https://gist.github.com.evil.invalid/example/{ID}"),
        format!("https://token@gist.github.com/example/{ID}"),
        format!("https://gist.github.com:444/example/{ID}"),
        format!("https://gist.github.com/example/{ID}?token=private"),
        format!("https://gist.github.com/example/{ID}#file-secret"),
        format!("https://gist.githubusercontent.com/example/{ID}/raw/{REV}/%2Fsecret"),
    ] {
        assert!(GistFile::parse(&target, "nodes.txt").is_err());
    }
    assert!(
        GistFile::parse(
            &format!("https://gist.githubusercontent.com/example/{ID}/raw/{REV}/other.txt"),
            "nodes.txt"
        )
        .is_err()
    );
}

#[test]
fn token_validation_rejects_empty_and_header_injection() {
    for token in ["", " ", "secret\r\nX-Injected: yes", "a b", "非ascii"] {
        assert!(matches!(
            GistClient::new(token),
            Err(GistError::InvalidToken)
        ));
    }
}

#[test]
fn reads_selected_file_with_authentication_and_revision() {
    let fixture = Fixture::new(vec![api(document("synthetic node"))]);
    let snapshot = fixture
        .client()
        .read(&selected(ID), &AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        snapshot.content.as_deref().map(|s| s.as_str()),
        Some("synthetic node")
    );
    assert_eq!(snapshot.revision, expected("").revision);
    let requests = fixture.requests.lock().unwrap();
    assert!(
        requests[0]
            .head
            .starts_with(&format!("GET /gists/{ID} HTTP/1.1\r\n"))
    );
    assert!(
        requests[0]
            .head
            .to_ascii_lowercase()
            .contains(&format!("authorization: bearer {TOKEN}"))
    );
}

#[test]
fn api_redirect_never_forwards_credentials() {
    let destination = Fixture::new(vec![Some(response(200, "", "leaked"))]);
    let fixture = Fixture::new(vec![Some(response(
        302,
        &format!("Location: {}/stolen\r\n", destination.origin),
        "",
    ))]);
    let result = fixture
        .client()
        .read(&selected(ID), &AtomicBool::new(false));
    assert!(result.is_err());
    assert!(destination.requests.lock().unwrap().is_empty());
}

#[test]
fn truncated_file_uses_pinned_raw_without_authentication() {
    let mut doc = document("partial");
    doc["files"]["nodes.txt"]["truncated"] = json!(true);
    doc["files"]["nodes.txt"]["size"] = json!(8);
    let fixture = Fixture::new(vec![api(doc), Some(response(200, "", "complete"))]);
    let snapshot = fixture
        .client()
        .read(&selected(ID), &AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        snapshot.content.as_deref().map(|s| s.as_str()),
        Some("complete")
    );
    let requests = fixture.requests.lock().unwrap();
    assert!(
        requests[1]
            .head
            .starts_with(&format!("GET /example/{ID}/raw/{REV}/nodes.txt HTTP/1.1"))
    );
    assert!(
        !requests[1]
            .head
            .to_ascii_lowercase()
            .contains("authorization:")
    );
}

#[test]
fn truncated_file_rejects_foreign_or_unpinned_raw_resource() {
    for raw in [
        format!("https://evil.invalid/example/{ID}/raw/{REV}/nodes.txt"),
        format!("https://gist.githubusercontent.com/example/{INPUT_ID}/raw/{REV}/nodes.txt"),
        format!("https://gist.githubusercontent.com/example/{ID}/raw/nodes.txt"),
        format!("https://gist.githubusercontent.com/example/{ID}/raw/{REV}/other.txt"),
    ] {
        let mut doc = document("partial");
        doc["files"]["nodes.txt"]["truncated"] = json!(true);
        doc["files"]["nodes.txt"]["raw_url"] = json!(raw);
        let fixture = Fixture::new(vec![api(doc)]);
        assert!(
            fixture
                .client()
                .read(&selected(ID), &AtomicBool::new(false))
                .is_err()
        );
        assert_eq!(fixture.requests.lock().unwrap().len(), 1);
    }
}

#[test]
fn missing_file_requires_a_complete_file_list() {
    let mut complete = document("unused");
    complete["files"] = json!({});
    let mut incomplete = complete.clone();
    incomplete["truncated"] = json!(true);
    let fixture = Fixture::new(vec![api(complete), api(incomplete)]);
    let client = fixture.client();
    assert!(
        client
            .read(&selected(ID), &AtomicBool::new(false))
            .unwrap()
            .content
            .is_none()
    );
    assert!(matches!(
        client.read(&selected(ID), &AtomicBool::new(false)),
        Err(GistError::IncompleteFiles)
    ));
}

#[test]
fn malformed_or_oversized_response_is_not_accepted_as_a_snapshot() {
    let mut wrong = document("abc");
    wrong["id"] = json!(INPUT_ID);
    let mut short = document("abc");
    short["files"]["nodes.txt"]["size"] = json!(4);
    for reply in [
        api(wrong),
        api(short),
        Some(response(200, "", "{}")),
        Some(response(200, "", &" ".repeat(MAX_JSON_BYTES + 1))),
    ] {
        let fixture = Fixture::new(vec![reply]);
        assert!(
            fixture
                .client()
                .read(&selected(ID), &AtomicBool::new(false))
                .is_err()
        );
    }
}

#[test]
fn cancellation_before_read_sends_no_request() {
    let fixture = Fixture::new(vec![]);
    assert!(matches!(
        fixture.client().read(&selected(ID), &AtomicBool::new(true)),
        Err(GistError::Cancelled)
    ));
    assert!(fixture.requests.lock().unwrap().is_empty());
}

#[test]
fn identical_output_is_a_no_op() {
    let fixture = Fixture::new(vec![api(document("unchanged"))]);
    let result = fixture.client().publish(
        Some(&selected(INPUT_ID)),
        &selected(ID),
        &expected("unchanged"),
        "unchanged",
        "relay",
        false,
        &AtomicBool::new(false),
    );
    let PublishOutcome::Unchanged(link) = result.unwrap() else {
        panic!("identical content must not be written");
    };
    assert_policy_link(link);
    assert_eq!(fixture.requests.lock().unwrap().len(), 1);
}

#[test]
fn publication_updates_only_selected_file_and_verifies_readback() {
    let mut before = document("old");
    before["files"]["unrelated.txt"] =
        json!({"filename":"unrelated.txt", "content":"keep", "size":4, "truncated":false});
    let fixture = Fixture::new(vec![
        api(before),
        api(document("new")),
        api(document("new")),
    ]);
    let result = fixture.client().publish(
        Some(&selected(INPUT_ID)),
        &selected(ID),
        &expected("old"),
        "new",
        "relay",
        false,
        &AtomicBool::new(false),
    );
    let PublishOutcome::Verified(link) = result.unwrap() else {
        panic!("changed content must be verified after writing");
    };
    assert_policy_link(link);
    let requests = fixture.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests[1].head.starts_with("PATCH "));
    assert_eq!(
        serde_json::from_str::<Value>(&requests[1].body).unwrap(),
        json!({"description":"relay", "files":{"nodes.txt":{"content":"new"}}})
    );
    assert!(!requests[1].head.to_ascii_lowercase().contains("if-match:"));
}

#[test]
fn local_source_publication_still_validates_and_verifies_output() {
    let fixture = Fixture::new(vec![
        api(document("old")),
        api(document("new")),
        api(document("new")),
    ]);
    let client = fixture.client();
    let cancel = AtomicBool::new(false);
    let invalid = GistFile {
        id: "../invalid".to_owned(),
        filename: "nodes.txt".to_owned(),
    };
    assert_eq!(
        client.publish(
            None,
            &invalid,
            &expected("old"),
            "new",
            "relay",
            false,
            &cancel
        ),
        Err(GistError::InvalidTarget)
    );
    assert!(fixture.request_methods().is_empty());
    assert!(matches!(
        client.publish(
            None,
            &selected(ID),
            &expected("old"),
            "new",
            "relay",
            false,
            &cancel
        ),
        Ok(PublishOutcome::Verified(Some(_)))
    ));
    assert_eq!(fixture.request_methods(), vec!["GET", "PATCH", "GET"]);
}

#[test]
fn concurrent_output_change_prevents_patch() {
    let mut doc = document("old");
    doc["history"][0]["version"] = json!("dddddddddddddddddddddddddddddddddddddddd");
    let fixture = Fixture::new(vec![api(doc)]);
    assert_eq!(
        fixture.client().publish(
            Some(&selected(INPUT_ID)),
            &selected(ID),
            &expected("old"),
            "new",
            "relay",
            false,
            &AtomicBool::new(false)
        ),
        Err(GistError::Conflict)
    );
    assert_eq!(fixture.requests.lock().unwrap().len(), 1);
}

#[test]
fn public_output_requires_explicit_acknowledgement() {
    let mut doc = document("old");
    doc["public"] = json!(true);
    let fixture = Fixture::new(vec![api(doc)]);
    let mut snapshot = expected("old");
    snapshot.public = true;
    assert_eq!(
        fixture.client().publish(
            Some(&selected(INPUT_ID)),
            &selected(ID),
            &snapshot,
            "new",
            "relay",
            false,
            &AtomicBool::new(false)
        ),
        Err(GistError::PublicOutput)
    );
    assert_eq!(fixture.requests.lock().unwrap().len(), 1);
}

#[test]
fn identical_input_output_is_rejected_before_network() {
    let fixture = Fixture::new(vec![]);
    assert_eq!(
        fixture.client().publish(
            Some(&selected(ID)),
            &selected(ID),
            &expected("old"),
            "new",
            "relay",
            false,
            &AtomicBool::new(false)
        ),
        Err(GistError::InvalidTarget)
    );
    assert!(fixture.requests.lock().unwrap().is_empty());
}

#[test]
fn cancellation_after_preflight_prevents_patch() {
    let cancel = Arc::new(AtomicBool::new(false));
    let fixture = Fixture::with_cancel(vec![api(document("old"))], Some(cancel.clone()));
    assert_eq!(
        fixture.client().publish(
            Some(&selected(INPUT_ID)),
            &selected(ID),
            &expected("old"),
            "new",
            "relay",
            false,
            &cancel
        ),
        Err(GistError::Cancelled)
    );
    assert_eq!(fixture.requests.lock().unwrap().len(), 1);
}

#[test]
fn lost_patch_response_is_uncertain_and_never_retried() {
    let fixture = Fixture::new(vec![api(document("old")), None]);
    assert_eq!(
        fixture.client().publish(
            Some(&selected(INPUT_ID)),
            &selected(ID),
            &expected("old"),
            "new",
            "relay",
            false,
            &AtomicBool::new(false)
        ),
        Err(GistError::UncertainWrite)
    );
    assert_eq!(fixture.requests.lock().unwrap().len(), 2);
}

#[test]
fn wrong_readback_is_not_publication_success() {
    let fixture = Fixture::new(vec![
        api(document("old")),
        api(document("new")),
        api(document("different")),
    ]);
    assert_eq!(
        fixture.client().publish(
            Some(&selected(INPUT_ID)),
            &selected(ID),
            &expected("old"),
            "new",
            "relay",
            false,
            &AtomicBool::new(false)
        ),
        Err(GistError::VerificationFailed)
    );
    assert_eq!(fixture.requests.lock().unwrap().len(), 3);
}

#[test]
fn errors_do_not_expose_token_private_url_or_server_body() {
    let fixture = Fixture::new(vec![Some(response(
        403,
        "",
        &format!("{TOKEN} https://private.invalid/secret"),
    ))]);
    let result = fixture
        .client()
        .read(&selected(ID), &AtomicBool::new(false));
    let Err(error) = result else {
        panic!("server failure must fail");
    };
    let text = format!("{error} {error:?}");
    assert!(!text.contains(TOKEN));
    assert!(!text.contains("private.invalid"));
    assert!(!text.contains(ID));
}

#[test]
fn oversized_candidate_is_rejected_before_any_write() {
    let fixture = Fixture::new(vec![]);
    assert_eq!(
        fixture.client().publish(
            Some(&selected(INPUT_ID)),
            &selected(ID),
            &expected("old"),
            &"x".repeat(MAX_CONTENT_BYTES + 1),
            "relay",
            false,
            &AtomicBool::new(false)
        ),
        Err(GistError::TooLarge)
    );
    assert!(fixture.requests.lock().unwrap().is_empty());
}

#[test]
fn raw_redirect_and_oversized_body_are_rejected_without_credentials() {
    let destination = Fixture::new(vec![Some(response(200, "", "leaked"))]);
    for raw_reply in [
        response(
            302,
            &format!("Location: {}/stolen\r\n", destination.origin),
            "",
        ),
        response(200, "", &"x".repeat(MAX_CONTENT_BYTES + 1)),
    ] {
        let mut doc = document("partial");
        doc["files"]["nodes.txt"]["truncated"] = json!(true);
        let fixture = Fixture::new(vec![api(doc), Some(raw_reply)]);
        assert!(
            fixture
                .client()
                .read(&selected(ID), &AtomicBool::new(false))
                .is_err()
        );
        assert!(
            !fixture.requests.lock().unwrap()[1]
                .head
                .to_ascii_lowercase()
                .contains("authorization:")
        );
    }
    assert!(destination.requests.lock().unwrap().is_empty());
}

#[test]
fn unchanged_revision_cannot_hide_changed_file_or_description() {
    for mut doc in [document("concurrent edit"), document("old")] {
        if doc["files"]["nodes.txt"]["content"] == "old" {
            doc["description"] = json!("concurrent description");
        }
        let fixture = Fixture::new(vec![api(doc)]);
        assert_eq!(
            fixture.client().publish(
                Some(&selected(INPUT_ID)),
                &selected(ID),
                &expected("old"),
                "new",
                "relay",
                false,
                &AtomicBool::new(false)
            ),
            Err(GistError::Conflict)
        );
        assert_eq!(fixture.requests.lock().unwrap().len(), 1);
    }
}

#[test]
fn readback_transport_failure_remains_uncertain_after_patch() {
    let fixture = Fixture::new(vec![api(document("old")), api(document("new")), None]);
    assert_eq!(
        fixture.client().publish(
            Some(&selected(INPUT_ID)),
            &selected(ID),
            &expected("old"),
            "new",
            "relay",
            false,
            &AtomicBool::new(false)
        ),
        Err(GistError::UncertainWrite)
    );
    assert_eq!(fixture.requests.lock().unwrap().len(), 3);
}

#[test]
fn hand_constructed_invalid_target_cannot_bypass_validation() {
    let fixture = Fixture::new(vec![]);
    let file = GistFile {
        id: "../private".to_owned(),
        filename: "nodes.txt".to_owned(),
    };
    assert!(matches!(
        fixture.client().read(&file, &AtomicBool::new(false)),
        Err(GistError::InvalidTarget)
    ));
    assert!(fixture.requests.lock().unwrap().is_empty());
}

#[test]
fn anonymous_read_omits_authorization_entirely() {
    let fixture = Fixture::new(vec![api(document("public synthetic node"))]);
    let mut client = GistClient::anonymous();
    client.agent = agent(false);
    client.origin = Some(fixture.origin.clone());
    let snapshot = client.read(&selected(ID), &AtomicBool::new(false)).unwrap();
    assert_eq!(
        snapshot.content.as_deref().map(|value| value.as_str()),
        Some("public synthetic node")
    );
    assert!(
        !fixture.requests.lock().unwrap()[0]
            .head
            .to_ascii_lowercase()
            .contains("authorization:")
    );
}

#[test]
fn anonymous_publish_rejects_before_any_request() {
    let fixture = Fixture::new(vec![]);
    let mut client = GistClient::anonymous();
    client.agent = agent(false);
    client.origin = Some(fixture.origin.clone());
    assert_eq!(
        client.publish(
            Some(&selected(INPUT_ID)),
            &selected(ID),
            &expected("old"),
            "new",
            "relay",
            false,
            &AtomicBool::new(false)
        ),
        Err(GistError::InvalidToken)
    );
    assert!(fixture.requests.lock().unwrap().is_empty());
}
