use serde::{Deserialize, Serialize};
pub const MAX_LINE_BYTES: usize = 1024 * 1024;
pub const STARTUP_TIMEOUT_SECONDS: u64 = 60;
pub const TRANSCRIPTION_TIMEOUT_SECONDS: u64 = 120;
pub const CANCEL_TIMEOUT_SECONDS: u64 = 3;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolError;
impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Message du moteur invalide")
    }
}
impl std::error::Error for ProtocolError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    MicUnavailable,
    NoSpeech,
    ModelNotFound,
    ModelLoadFailed,
    ApiConfigError,
    ApiAuthError,
    ApiRateLimit,
    ApiUnavailable,
    EngineBusy,
    EngineTimeout,
    EngineCrashed,
    FocusChanged,
    InjectionFailed,
    ClipboardFailed,
    Cancelled,
    ProtocolError,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptionProvider {
    #[default]
    Local,
    Groq,
}
impl TranscriptionProvider {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Groq => "groq",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Fr,
    En,
    Auto,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Transcribe {
        protocol_version: u8,
        request_id: String,
        audio_path: String,
        language: Language,
        model_path: String,
        vad: bool,
    },
    Cancel {
        protocol_version: u8,
        request_id: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Response {
    Ready {
        protocol_version: u8,
        request_id: String,
    },
    Result {
        protocol_version: u8,
        request_id: String,
        text: String,
        language: Language,
        duration_ms: u64,
    },
    Error {
        protocol_version: u8,
        request_id: String,
        code: ErrorCode,
        message: String,
    },
    Cancelled {
        protocol_version: u8,
        request_id: String,
    },
}
pub type EngineRequest = Request;
pub type EngineResponse = Response;
fn envelope(version: u8, id: &str, startup: bool) -> Result<(), ProtocolError> {
    if version != 1 || id.is_empty() || (!startup && id == "startup") {
        Err(ProtocolError)
    } else {
        Ok(())
    }
}
fn bytes(raw: &[u8]) -> Result<&[u8], ProtocolError> {
    let raw = raw.strip_suffix(b"\n").unwrap_or(raw);
    if raw.len() > MAX_LINE_BYTES
        || raw.contains(&b'\n')
        || raw.contains(&b'\r')
        || std::str::from_utf8(raw).is_err()
    {
        return Err(ProtocolError);
    }
    Ok(raw)
}
impl Request {
    pub fn validate_model_path(&self, loaded_path: &str) -> Result<(), ProtocolError> {
        self.validate()?;
        match self {
            Self::Transcribe { model_path, .. } if model_path != loaded_path => Err(ProtocolError),
            _ => Ok(()),
        }
    }
    pub fn request_id(&self) -> &str {
        match self {
            Self::Transcribe { request_id, .. } | Self::Cancel { request_id, .. } => request_id,
        }
    }
    pub fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::Transcribe {
                protocol_version,
                request_id,
                audio_path,
                model_path,
                vad,
                ..
            } => {
                envelope(*protocol_version, request_id, false)?;
                if !vad
                    || [audio_path, model_path]
                        .iter()
                        .any(|p| !p.starts_with('/') || p.contains('\0'))
                {
                    return Err(ProtocolError);
                }
                Ok(())
            }
            Self::Cancel {
                protocol_version,
                request_id,
            } => envelope(*protocol_version, request_id, false),
        }
    }
}
impl Response {
    pub fn request_id(&self) -> &str {
        match self {
            Self::Ready { request_id, .. }
            | Self::Result { request_id, .. }
            | Self::Error { request_id, .. }
            | Self::Cancelled { request_id, .. } => request_id,
        }
    }
    pub fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::Ready {
                protocol_version,
                request_id,
            } => {
                envelope(*protocol_version, request_id, true)?;
                if request_id != "startup" {
                    Err(ProtocolError)
                } else {
                    Ok(())
                }
            }
            Self::Result {
                protocol_version,
                request_id,
                text,
                language,
                duration_ms,
            } => {
                envelope(*protocol_version, request_id, false)?;
                if text.is_empty()
                    || *language == Language::Auto
                    || *duration_ms > 9_007_199_254_740_991
                {
                    Err(ProtocolError)
                } else {
                    Ok(())
                }
            }
            Self::Error {
                protocol_version,
                request_id,
                message,
                ..
            } => {
                envelope(*protocol_version, request_id, true)?;
                if message.is_empty() {
                    Err(ProtocolError)
                } else {
                    Ok(())
                }
            }
            Self::Cancelled {
                protocol_version,
                request_id,
            } => envelope(*protocol_version, request_id, false),
        }
    }
}
pub fn parse_request(raw: &[u8]) -> Result<Request, ProtocolError> {
    let value: Request = serde_json::from_slice(bytes(raw)?).map_err(|_| ProtocolError)?;
    value.validate()?;
    Ok(value)
}
pub fn parse_response(raw: &[u8]) -> Result<Response, ProtocolError> {
    let value: Response = serde_json::from_slice(bytes(raw)?).map_err(|_| ProtocolError)?;
    value.validate()?;
    Ok(value)
}
pub fn parse_engine_line(raw: &[u8]) -> Result<EngineResponse, ProtocolError> {
    parse_response(raw)
}
fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, ProtocolError> {
    let mut raw = serde_json::to_vec(value).map_err(|_| ProtocolError)?;
    if raw.len() > MAX_LINE_BYTES {
        return Err(ProtocolError);
    }
    raw.push(b'\n');
    Ok(raw)
}
pub fn encode_request(value: &Request) -> Result<Vec<u8>, ProtocolError> {
    value.validate()?;
    encode(value)
}
pub fn encode_response(value: &Response) -> Result<Vec<u8>, ProtocolError> {
    value.validate()?;
    encode(value)
}
/// Buffer never grows beyond the line limit; overflow discards until next LF.
#[derive(Default)]
pub struct JsonlDecoder {
    buffer: Vec<u8>,
    discarding: bool,
}
impl JsonlDecoder {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Result<Vec<u8>, ProtocolError>> {
        let mut output = Vec::new();
        for &byte in chunk {
            if self.discarding {
                if byte == b'\n' {
                    self.discarding = false;
                }
                continue;
            }
            if byte == b'\n' {
                output.push(Ok(std::mem::take(&mut self.buffer)));
            } else if self.buffer.len() == MAX_LINE_BYTES {
                self.buffer.clear();
                self.discarding = true;
                output.push(Err(ProtocolError));
            } else {
                self.buffer.push(byte);
            }
        }
        output
    }
    pub fn finish(&mut self) -> Result<(), ProtocolError> {
        let valid = self.buffer.is_empty() && !self.discarding;
        self.buffer.clear();
        self.discarding = false;
        if valid {
            Ok(())
        } else {
            Err(ProtocolError)
        }
    }
}

impl ProtocolError {
    pub const fn code(&self) -> ErrorCode {
        ErrorCode::ProtocolError
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_fixtures() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../../contracts/fixtures/messages.json")).unwrap();
        for case in cases.as_array().unwrap() {
            let raw = case["json"].as_str().unwrap().as_bytes();
            let valid = if case["direction"] == "request" {
                parse_request(raw).is_ok()
            } else {
                parse_response(raw).is_ok()
            };
            assert_eq!(valid, case["valid"].as_bool().unwrap(), "{}", case["name"]);
        }
    }
    #[test]
    fn stream_fragmentation_and_overflow_recovery() {
        let mut decoder = JsonlDecoder::default();
        assert!(decoder.push(b"ab").is_empty());
        let lines = decoder.push(b"c\ndef\n");
        assert_eq!(lines, vec![Ok(b"abc".to_vec()), Ok(b"def".to_vec())]);
        assert!(decoder.push(&vec![b'x'; MAX_LINE_BYTES]).is_empty());
        assert_eq!(decoder.push(b"x"), vec![Err(ProtocolError)]);
        assert_eq!(decoder.push(b"discard\nok\n"), vec![Ok(b"ok".to_vec())]);
        assert!(decoder.finish().is_ok());
        decoder.push(b"partial");
        assert!(decoder.finish().is_err());
    }
    #[test]
    fn outbound_validation_and_loaded_model_identity() {
        let request = Request::Transcribe {
            protocol_version: 1,
            request_id: "d-1".into(),
            audio_path: "/tmp/a.wav".into(),
            language: Language::Auto,
            model_path: "/models/a".into(),
            vad: true,
        };
        assert!(request.validate_model_path("/models/a").is_ok());
        assert!(request.validate_model_path("/models/b").is_err());
        assert_eq!(
            parse_request(&encode_request(&request).unwrap()).unwrap(),
            request
        );
        let response = Response::Result {
            protocol_version: 1,
            request_id: "d-1".into(),
            text: "Bonjour".into(),
            language: Language::Fr,
            duration_ms: 0,
        };
        assert_eq!(
            parse_response(&encode_response(&response).unwrap()).unwrap(),
            response
        );
        let invalid = Response::Ready {
            protocol_version: 2,
            request_id: "startup".into(),
        };
        assert!(encode_response(&invalid).is_err());
    }
    #[test]
    fn strict_bytes_and_exact_limit() {
        for bytes in [b"\xff".as_slice(), b"{}\n{}", b"{\"type\":NaN}"] {
            assert!(parse_response(bytes).is_err());
        }
        let prefix = r#"{"type":"error","protocol_version":1,"request_id":"startup","code":"PROTOCOL_ERROR","message":""#;
        let mut raw = prefix.as_bytes().to_vec();
        raw.resize(MAX_LINE_BYTES - 2, b'x');
        raw.extend_from_slice(b"\"}");
        assert!(parse_response(&raw).is_ok());
        raw.push(b' ');
        assert!(parse_response(&raw).is_err());
    }
}
