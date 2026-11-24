//! Shared HTTP request and response types and their Postcard encoding.

pub use postcard::Error as CodecError;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const CONTENT_TYPE: &str = "application/vnd.concentus.postcard";

pub fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, CodecError> {
    postcard::to_allocvec(value)
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CodecError> {
    let (value, rest) = postcard::take_from_bytes(bytes)?;
    if rest.is_empty() {
        Ok(value)
    } else {
        Err(CodecError::DeserializeBadEncoding)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageQuery {
    pub after: Option<String>,
    pub limit: Option<u32>,
}
