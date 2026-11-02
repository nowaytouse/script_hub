use std::{
    collections::BTreeMap,
    fmt,
    io::Read,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use url::Url;
use zeroize::{Zeroize, Zeroizing};

const MAX_CONTENT_BYTES: usize = 4 * 1024 * 1024;
const MAX_JSON_BYTES: usize = 8 * 1024 * 1024;
const API_ORIGIN: &str = "https://api.github.com";

pub struct GistFile {
    pub id: String,
    pub filename: String,
}

impl GistFile {
    pub fn parse(input: &str, filename: &str) -> Result<Self, GistError> {
        let file = Self::parse_source(input, filename)?;
        validate_filename(&file.filename)?;
        Ok(file)
    }

    pub fn parse_source(input: &str, filename: &str) -> Result<Self, GistError> {
        let (id, raw_filename) = if valid_hex(input) {
            (input.to_ascii_lowercase(), None)
        } else {
            let url = trusted_url(input)?;
            let parts: Vec<_> = url
                .path_segments()
                .ok_or(GistError::InvalidTarget)?
                .collect();
            match (url.host_str(), parts.as_slice()) {
                (Some("gist.github.com"), [id]) if valid_hex(id) => (id.to_ascii_lowercase(), None),
                (Some("gist.github.com"), [owner, id]) if valid_owner(owner) && valid_hex(id) => {
                    (id.to_ascii_lowercase(), None)
                }
                (Some("api.github.com"), ["gists", id]) if valid_hex(id) => {
                    (id.to_ascii_lowercase(), None)
                }
                (Some("gist.githubusercontent.com"), _) => {
                    let raw = raw_target(&url)?;
                    if raw.revision.is_some() {
                        return Err(GistError::PinnedTarget);
                    }
                    (raw.id, Some(raw.filename))
                }
                _ => return Err(GistError::InvalidTarget),
            }
        };
        let filename = match raw_filename {
            Some(raw_filename) if filename.is_empty() || filename == raw_filename => raw_filename,
            Some(_) => return Err(GistError::InvalidTarget),
            None => filename.to_owned(),
        };
        if !filename.is_empty() {
            validate_filename(&filename)?;
        }
        Ok(Self { id, filename })
    }
}

pub fn validate_name(name: &str) -> Result<(), GistError> {
    if name.chars().count() > 64 || name.chars().any(char::is_control) {
        return Err(GistError::InvalidName);
    }
    Ok(())
}

pub struct Snapshot {
    pub content: Option<Zeroizing<String>>,
    pub revision: String,
    pub description: String,
    pub public: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedGist {
    pub id: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GistError {
    InvalidTarget,
    PinnedTarget,
    InvalidName,
    InvalidToken,
    Cancelled,
    Transport,
    HttpStatus(u16),
    InvalidResponse,
    TooLarge,
    IncompleteFiles,
    SelectFile(Vec<String>),
    Conflict,
    PublicOutput,
    UncertainWrite,
    VerificationFailed,
    CreatedUnverified(CreatedGist),
}

impl fmt::Display for GistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidTarget => "Select a valid GitHub Gist and a distinct output Gist.",
            Self::PinnedTarget => {
                "Pinned revisions cannot be relay targets. Use a Gist ID or a latest raw-file URL without a revision."
            }
            Self::InvalidName => {
                "Names must be at most 64 characters, without controls or filename paths."
            }
            Self::InvalidToken => "A valid session-only GitHub token is required.",
            Self::Cancelled => "Gist operation cancelled before publication.",
            Self::Transport => "GitHub request failed; check the connection and try again.",
            Self::HttpStatus(status) => {
                return write!(f, "GitHub rejected the request (HTTP {status}).");
            }
            Self::InvalidResponse => "GitHub returned an invalid or incomplete response.",
            Self::TooLarge => "Gist content exceeds the bounded relay size limit.",
            Self::IncompleteFiles => {
                "GitHub returned a partial file list; the selected file cannot be confirmed absent."
            }
            Self::SelectFile(_) => "Choose a file from the source Gist.",
            Self::Conflict => "The output Gist changed; preview it again before publishing.",
            Self::PublicOutput => "Publishing to a public Gist requires explicit acknowledgement.",
            Self::UncertainWrite => {
                "The create or update may have been accepted; inspect your Gists before retrying. Creation is never automatically retried."
            }
            Self::VerificationFailed => {
                "The output readback did not match; inspect it before retrying."
            }
            Self::CreatedUnverified(created) => {
                return write!(
                    f,
                    "Gist created at {}, but verification failed or was cancelled. Inspect this Gist before retrying; reuse it instead of creating another.",
                    created.url
                );
            }
        };
        f.write_str(message)
    }
}

impl std::error::Error for GistError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishOutcome {
    Unchanged,
    Verified,
}

pub struct GistClient {
    agent: ureq::Agent,
    token: Zeroizing<String>,
    #[cfg(test)]
    origin: Option<String>,
}

impl GistClient {
    pub fn anonymous() -> Self {
        Self {
            agent: agent(true),
            token: Zeroizing::new(String::new()),
            #[cfg(test)]
            origin: None,
        }
    }

    pub fn new(token: &str) -> Result<Self, GistError> {
        if token.is_empty()
            || token.len() > 1024
            || !token.is_ascii()
            || token
                .bytes()
                .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
        {
            return Err(GistError::InvalidToken);
        }
        let mut client = Self::anonymous();
        client.token = Zeroizing::new(token.to_owned());
        Ok(client)
    }

    pub fn resolve_source(
        &self,
        input: &str,
        filename: &str,
        cancel: &AtomicBool,
    ) -> Result<GistFile, GistError> {
        check_cancel(cancel)?;
        let mut file = GistFile::parse_source(input, filename)?;
        if !file.filename.is_empty() {
            return Ok(file);
        }
        let bytes = self.get(&self.api_url(&file), true, MAX_JSON_BYTES, cancel)?;
        let doc: ApiGist =
            serde_json::from_slice(&bytes).map_err(|_| GistError::InvalidResponse)?;
        validate_metadata(&doc, &file.id)?;
        if doc.truncated {
            return Err(GistError::IncompleteFiles);
        }
        let mut names = Vec::with_capacity(doc.files.len());
        for (name, entry) in &doc.files {
            if validate_filename(name).is_err() || entry.filename != *name {
                return Err(GistError::InvalidResponse);
            }
            names.push(name.clone());
        }
        check_cancel(cancel)?;
        match names.len() {
            0 => Err(GistError::InvalidResponse),
            1 => {
                file.filename = names.pop().ok_or(GistError::InvalidResponse)?;
                Ok(file)
            }
            _ => Err(GistError::SelectFile(names)),
        }
    }

    pub fn read(&self, file: &GistFile, cancel: &AtomicBool) -> Result<Snapshot, GistError> {
        check_cancel(cancel)?;
        validate_file(file)?;
        let bytes = self.get(&self.api_url(file), true, MAX_JSON_BYTES, cancel)?;
        let mut doc: ApiGist =
            serde_json::from_slice(&bytes).map_err(|_| GistError::InvalidResponse)?;
        validate_metadata(&doc, &file.id)?;
        let version = doc
            .history
            .first()
            .ok_or(GistError::InvalidResponse)?
            .version
            .as_str();
        let content = match doc.files.get_mut(&file.filename) {
            None if doc.truncated => return Err(GistError::IncompleteFiles),
            None => None,
            Some(selected) => {
                if selected.filename != file.filename
                    || selected
                        .encoding
                        .as_deref()
                        .is_some_and(|encoding| encoding != "utf-8")
                {
                    return Err(GistError::InvalidResponse);
                }
                if selected.size > MAX_CONTENT_BYTES {
                    return Err(GistError::TooLarge);
                }
                let content = if selected.truncated {
                    let raw_url = trusted_url(
                        selected
                            .raw_url
                            .as_deref()
                            .ok_or(GistError::InvalidResponse)?,
                    )?;
                    let raw = raw_target(&raw_url)?;
                    if !raw.id.eq_ignore_ascii_case(&file.id)
                        || raw.filename != file.filename
                        || raw.revision.as_deref() != Some(version)
                    {
                        return Err(GistError::InvalidResponse);
                    }
                    let bytes =
                        self.get(&self.raw_url(&raw_url), false, MAX_CONTENT_BYTES, cancel)?;
                    Zeroizing::new(
                        String::from_utf8(bytes.to_vec())
                            .map_err(|_| GistError::InvalidResponse)?,
                    )
                } else {
                    Zeroizing::new(selected.content.take().ok_or(GistError::InvalidResponse)?)
                };
                if content.len() != selected.size {
                    return Err(GistError::InvalidResponse);
                }
                Some(content)
            }
        };
        check_cancel(cancel)?;
        Ok(Snapshot {
            content,
            revision: format!("{version}:{}", doc.updated_at),
            description: doc.description.unwrap_or_default(),
            public: doc.public,
        })
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "publication trust boundary is explicit"
    )]
    pub fn publish(
        &self,
        input: Option<&GistFile>,
        output: &GistFile,
        expected: &Snapshot,
        content: &str,
        description: &str,
        allow_public: bool,
        cancel: &AtomicBool,
    ) -> Result<PublishOutcome, GistError> {
        if self.token.is_empty() {
            return Err(GistError::InvalidToken);
        }
        check_cancel(cancel)?;
        if let Some(input) = input {
            validate_file(input)?;
            if input.id.eq_ignore_ascii_case(&output.id) {
                return Err(GistError::InvalidTarget);
            }
        }
        validate_file(output)?;
        validate_name(description)?;
        if content.len() > MAX_CONTENT_BYTES {
            return Err(GistError::TooLarge);
        }
        let payload = Zeroizing::new(
            serde_json::to_vec(&Patch {
                description,
                files: BTreeMap::from([(output.filename.as_str(), PatchFile { content })]),
            })
            .map_err(|_| GistError::InvalidResponse)?,
        );
        if payload.len() > MAX_JSON_BYTES {
            return Err(GistError::TooLarge);
        }
        let current = self.read(output, cancel)?;
        if current.revision != expected.revision
            || current.content != expected.content
            || current.description != expected.description
            || current.public != expected.public
        {
            return Err(GistError::Conflict);
        }
        if current.public && !allow_public {
            return Err(GistError::PublicOutput);
        }
        if current.content.as_deref().map(|value| value.as_str()) == Some(content)
            && current.description == description
        {
            return Ok(PublishOutcome::Unchanged);
        }
        check_cancel(cancel)?;
        // GitHub offers no documented atomic Gist CAS; this preflight only detects observed conflicts.
        let response = self
            .agent
            .patch(self.api_url(output))
            .header("Authorization", self.authorization()?)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2026-03-10")
            .content_type("application/json")
            .send(payload.as_slice())
            .map_err(|_| GistError::UncertainWrite)?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(if (400..500).contains(&status) {
                GistError::HttpStatus(status)
            } else {
                GistError::UncertainWrite
            });
        }
        drop(response);
        // After a possible write, cancellation or a lost readback cannot imply rollback.
        let verified = self
            .read(output, cancel)
            .map_err(|_| GistError::UncertainWrite)?;
        if verified.content.as_deref().map(|value| value.as_str()) != Some(content)
            || verified.description != description
            || verified.public != current.public
        {
            return Err(GistError::VerificationFailed);
        }
        Ok(PublishOutcome::Verified)
    }

    /// Creates one secret Gist. A known creation receipt survives failed or cancelled verification.
    pub fn create(
        &self,
        filename: &str,
        content: &str,
        description: &str,
        cancel: &AtomicBool,
    ) -> Result<CreatedGist, GistError> {
        if self.token.is_empty() {
            return Err(GistError::InvalidToken);
        }
        check_cancel(cancel)?;
        validate_filename(filename)?;
        validate_name(description)?;
        if content.len() > MAX_CONTENT_BYTES {
            return Err(GistError::TooLarge);
        }
        let payload = Zeroizing::new(
            serde_json::to_vec(&Create {
                public: false,
                description,
                files: BTreeMap::from([(filename, PatchFile { content })]),
            })
            .map_err(|_| GistError::InvalidResponse)?,
        );
        if payload.len() > MAX_JSON_BYTES {
            return Err(GistError::TooLarge);
        }
        #[cfg(test)]
        let origin = self.origin.as_deref().unwrap_or(API_ORIGIN);
        #[cfg(not(test))]
        let origin = API_ORIGIN;
        check_cancel(cancel)?;
        // Creation is not idempotent: never retry this POST after an uncertain outcome.
        let mut response = self
            .agent
            .post(format!("{origin}/gists"))
            .header("Authorization", self.authorization()?)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2026-03-10")
            .content_type("application/json")
            .send(payload.as_slice())
            .map_err(|_| GistError::UncertainWrite)?;
        let status = response.status().as_u16();
        if status != 201 {
            return Err(if (400..500).contains(&status) {
                GistError::HttpStatus(status)
            } else {
                GistError::UncertainWrite
            });
        }
        // Recover the ID even if cancellation arrived after GitHub accepted the POST.
        let mut bytes = Zeroizing::new(Vec::new());
        response
            .body_mut()
            .as_reader()
            .take((MAX_JSON_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| GistError::UncertainWrite)?;
        if bytes.len() > MAX_JSON_BYTES {
            return Err(GistError::UncertainWrite);
        }
        let receipt: CreateReceipt =
            serde_json::from_slice(&bytes).map_err(|_| GistError::UncertainWrite)?;
        if !valid_hex(&receipt.id) {
            return Err(GistError::UncertainWrite);
        }
        let id = receipt.id.to_ascii_lowercase();
        let created = CreatedGist {
            url: format!("https://gist.github.com/{id}"),
            id,
        };
        let output = GistFile {
            id: created.id.clone(),
            filename: filename.to_owned(),
        };
        let verified = self.read(&output, cancel);
        if !verified.is_ok_and(|snapshot| {
            snapshot.content.as_deref().map(|value| value.as_str()) == Some(content)
                && snapshot.description == description
                && !snapshot.public
        }) {
            return Err(GistError::CreatedUnverified(created));
        }
        Ok(created)
    }

    #[cfg(test)]
    pub(super) fn for_test(token: &str, origin: &str) -> Self {
        let url = Url::parse(origin).unwrap();
        assert_eq!(url.scheme(), "http");
        assert_eq!(url.host_str(), Some("127.0.0.1"));
        let mut client = Self::new(token).unwrap();
        client.agent = agent(false);
        client.origin = Some(origin.to_owned());
        client
    }

    fn api_url(&self, file: &GistFile) -> String {
        #[cfg(test)]
        if let Some(origin) = &self.origin {
            return format!("{origin}/gists/{}", file.id);
        }
        format!("{API_ORIGIN}/gists/{}", file.id)
    }

    fn raw_url(&self, url: &Url) -> String {
        #[cfg(test)]
        if let Some(origin) = &self.origin {
            return format!("{origin}{}", url.path());
        }
        url.as_str().to_owned()
    }

    fn authorization(&self) -> Result<ureq::http::HeaderValue, GistError> {
        let bearer = Zeroizing::new(format!("Bearer {}", self.token.as_str()));
        let mut header = ureq::http::HeaderValue::from_bytes(bearer.as_bytes())
            .map_err(|_| GistError::InvalidToken)?;
        header.set_sensitive(true);
        Ok(header)
    }

    fn get(
        &self,
        url: &str,
        authenticated: bool,
        limit: usize,
        cancel: &AtomicBool,
    ) -> Result<Zeroizing<Vec<u8>>, GistError> {
        check_cancel(cancel)?;
        let request = self.agent.get(url).header("Cache-Control", "no-cache");
        let request = if authenticated {
            request
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2026-03-10")
        } else {
            request
        };
        let request = if authenticated && !self.token.is_empty() {
            request.header("Authorization", self.authorization()?)
        } else {
            request
        };
        let mut response = request.call().map_err(|_| GistError::Transport)?;
        if response.status().as_u16() != 200 {
            return Err(GistError::HttpStatus(response.status().as_u16()));
        }
        let mut bytes = Zeroizing::new(Vec::new());
        let mut buffer = Zeroizing::new([0_u8; 8192]);
        let mut reader = response.body_mut().as_reader();
        loop {
            check_cancel(cancel)?;
            let count = reader
                .read(buffer.as_mut())
                .map_err(|_| GistError::Transport)?;
            if count == 0 {
                break;
            }
            if bytes.len() + count > limit {
                return Err(GistError::TooLarge);
            }
            bytes.extend_from_slice(&buffer[..count]);
        }
        check_cancel(cancel)?;
        Ok(bytes)
    }
}

fn agent(https_only: bool) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(https_only)
        .proxy(None)
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(20)))
        .timeout_connect(Some(Duration::from_secs(5)))
        .user_agent("script-hub-subscription-relay")
        .build()
        .into()
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), GistError> {
    if cancel.load(Ordering::SeqCst) {
        return Err(GistError::Cancelled);
    }
    Ok(())
}

fn validate_file(file: &GistFile) -> Result<(), GistError> {
    if !valid_hex(&file.id) {
        return Err(GistError::InvalidTarget);
    }
    validate_filename(&file.filename)
}

fn valid_hex(value: &str) -> bool {
    !value.is_empty() && value.len() <= 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_owner(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 39
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

pub(crate) fn validate_filename(filename: &str) -> Result<(), GistError> {
    validate_name(filename)?;
    if filename.is_empty() || matches!(filename, "." | "..") || filename.contains(['/', '\\']) {
        return Err(GistError::InvalidName);
    }
    Ok(())
}

fn trusted_url(value: &str) -> Result<Url, GistError> {
    if value.chars().any(char::is_control) {
        return Err(GistError::InvalidTarget);
    }
    let url = Url::parse(value).map_err(|_| GistError::InvalidTarget)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(GistError::InvalidTarget);
    }
    Ok(url)
}

struct RawTarget {
    id: String,
    revision: Option<String>,
    filename: String,
}

fn raw_target(url: &Url) -> Result<RawTarget, GistError> {
    if url.host_str() != Some("gist.githubusercontent.com") {
        return Err(GistError::InvalidTarget);
    }
    let parts: Vec<_> = url
        .path_segments()
        .ok_or(GistError::InvalidTarget)?
        .collect();
    let (owner, id, revision, filename) = match parts.as_slice() {
        [owner, id, "raw", revision, filename] if valid_hex(revision) => {
            (*owner, *id, Some((*revision).to_owned()), *filename)
        }
        [owner, id, "raw", filename] => (*owner, *id, None, *filename),
        _ => return Err(GistError::InvalidTarget),
    };
    if !valid_owner(owner) || !valid_hex(id) {
        return Err(GistError::InvalidTarget);
    }
    let filename = decode_filename(filename)?;
    validate_filename(&filename)?;
    Ok(RawTarget {
        id: id.to_ascii_lowercase(),
        revision,
        filename,
    })
}

fn decode_filename(value: &str) -> Result<String, GistError> {
    let mut bytes = Vec::with_capacity(value.len());
    let mut input = value.bytes();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let high = input
                .next()
                .and_then(|digit| char::from(digit).to_digit(16));
            let low = input
                .next()
                .and_then(|digit| char::from(digit).to_digit(16));
            let (Some(high), Some(low)) = (high, low) else {
                return Err(GistError::InvalidTarget);
            };
            bytes.push((high * 16 + low) as u8);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).map_err(|_| GistError::InvalidTarget)
}

#[derive(Deserialize)]
struct ApiGist {
    id: String,
    description: Option<String>,
    public: bool,
    truncated: bool,
    updated_at: String,
    history: Vec<ApiRevision>,
    files: BTreeMap<String, ApiFile>,
}

fn validate_metadata(doc: &ApiGist, id: &str) -> Result<(), GistError> {
    if !doc.id.eq_ignore_ascii_case(id)
        || !doc
            .history
            .first()
            .is_some_and(|revision| valid_hex(&revision.version))
        || doc.updated_at.is_empty()
        || doc.updated_at.len() > 64
        || doc.updated_at.chars().any(char::is_control)
    {
        return Err(GistError::InvalidResponse);
    }
    Ok(())
}

#[derive(Deserialize)]
struct ApiRevision {
    version: String,
}

#[derive(Deserialize)]
struct ApiFile {
    filename: String,
    content: Option<String>,
    raw_url: Option<String>,
    size: usize,
    truncated: bool,
    encoding: Option<String>,
}

impl Drop for ApiFile {
    fn drop(&mut self) {
        self.content.zeroize();
        self.raw_url.zeroize();
    }
}

#[derive(Serialize)]
struct Patch<'a> {
    description: &'a str,
    files: BTreeMap<&'a str, PatchFile<'a>>,
}

#[derive(Serialize)]
struct Create<'a> {
    public: bool,
    description: &'a str,
    files: BTreeMap<&'a str, PatchFile<'a>>,
}

#[derive(Deserialize)]
struct CreateReceipt {
    id: String,
}

#[derive(Serialize)]
struct PatchFile<'a> {
    content: &'a str,
}

#[cfg(test)]
#[path = "gist_tests.rs"]
pub(super) mod tests;
