use serde::{Deserialize, Serialize};

/// A predicate on a field within a record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldFilter {
    pub path: Vec<PathSegment>,
    pub value: FieldValue,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PathSegment {
    Field(String),
    /// Groups predicates that must match the same array element. This is a label,
    /// independent of the element's actual position in the stored array.
    Element(u8),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldValue {
    Text(String),
    Unsigned(u64),
    Signed(i64),
    Float(f32),
    Bool(bool),
    Null,
    Array,
    Object,
}

impl From<String> for FieldValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for FieldValue {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}

impl From<u64> for FieldValue {
    fn from(value: u64) -> Self {
        Self::Unsigned(value)
    }
}

impl From<bool> for FieldValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}
