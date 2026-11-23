use std::{cmp::Ordering, collections::BTreeMap};

use concentus_types::{FieldFilter, FieldValue, Lookup, PathSegment};
use serde_value::{Value, to_value};
use topcoat::Result;

use crate::records::DatabaseRecord;

/// Indexed SQL predicates narrow the candidates. Arbitrary nested field predicates
/// are evaluated on the hydrated record, so the lookup contract stays independent of table layout.
#[derive(Default)]
pub struct SqlLookup {
    predicates: Vec<String>,
    pub(crate) args: Vec<Option<String>>,
    filters: FilterNode,
    pub(crate) include_lyrics: bool,
}

impl SqlLookup {
    pub(crate) fn from_request<T: DatabaseRecord>(
        request: Lookup<T>,
    ) -> Result<Self, &'static str> {
        if request.options.lyrics && T::NAME != "tracks" {
            return Err("Lyrics can only be included in a track lookup.");
        }
        let mut lookup = Self {
            include_lyrics: request.options.lyrics,
            ..Self::default()
        };
        if request.fields.len() + request.external_ids.len() > 64 {
            return Err("Too many lookup fields.");
        }
        for FieldFilter { path, value } in request.fields {
            if !matches!(path.first(), Some(PathSegment::Field(_))) || path.len() > 8 {
                return Err(
                    "A lookup path must start with a field and have at most eight segments.",
                );
            }
            if matches!(value, FieldValue::Float(value) if !value.is_finite()) {
                return Err("Float filters must be finite.");
            }
            if let ([PathSegment::Field(key)], FieldValue::Text(value)) = (path.as_slice(), &value)
            {
                let column = match (T::NAME, key.as_str()) {
                    ("artists" | "tags", "name") => Some("name"),
                    ("albums" | "tracks", "title") => Some("title"),
                    _ => None,
                };
                if let Some(column) = column {
                    lookup.match_name(column, value.clone());
                }
            }
            lookup.filters.insert(&path, value)?;
        }
        lookup.filters.validate()?;
        if let Some(id) = request.id {
            let id = lookup.bind(Some(id.to_string()));
            lookup.predicates.push(format!("record.id = {id}"));
        }
        if let Some(name) = request.artist_name {
            if T::NAME == "artists" {
                lookup.match_name("name", name);
            } else if let Some(table) = T::CREDIT_TABLE {
                let name = lookup.bind(Some(name));
                lookup.predicates.push(format!(
                    "EXISTS (SELECT 1 FROM {table} AS credit
                        JOIN artists AS artist ON artist.id = credit.artist_id
                        WHERE credit.owner_id = record.id
                        AND credit.role IN ('Primary', 'Featured')
                        AND trim(artist.name) = trim({name}) COLLATE NOCASE)"
                ));
            } else {
                return Err("Artist names are available for artist, album and track lookups.");
            }
        }
        for id in request.external_ids {
            if let Some(table) = T::EXTERNAL_ID_TABLE {
                let [provider, text, number, name] =
                    external_parts(to_value(id).map_err(|_| "Invalid external ID.")?)?;
                let provider = lookup.bind(provider);
                let text = lookup.bind(text);
                let number = lookup.bind(number);
                let name = lookup.bind(name);
                lookup.predicates.push(format!(
                    "EXISTS (SELECT 1 FROM {table} AS external WHERE external.owner_id = record.id
                        AND external.provider = {provider} AND external.value_text IS {text}
                        AND external.value_number IS {number} AND external.other_name IS {name})"
                ));
            } else {
                lookup.predicates.push("0".into());
            }
        }
        Ok(lookup)
    }

    pub(crate) fn where_clause(&self) -> Option<String> {
        if self.predicates.is_empty() {
            self.has_field_filters().then(|| "1".into())
        } else {
            Some(self.predicates.join(" AND "))
        }
    }

    pub(crate) fn has_field_filters(&self) -> bool {
        !self.filters.children.is_empty()
    }

    pub(crate) fn has_lyrics_filters(&self) -> bool {
        self.filters
            .children
            .contains_key(&PathSegment::Field("lyrics".into()))
    }

    pub(crate) fn matches<T: DatabaseRecord>(&self, record: &T) -> Result<bool> {
        Ok(!self.has_field_filters() || self.filters.matches(&to_value(record)?, false))
    }

    fn bind(&mut self, value: Option<String>) -> String {
        self.args.push(value);
        format!("?{}", self.args.len())
    }

    fn match_name(&mut self, column: &'static str, value: String) {
        let value = self.bind(Some(value));
        self.predicates.push(format!(
            "trim(record.{column}) = trim({value}) COLLATE NOCASE"
        ));
    }
}

fn external_parts(value: Value) -> Result<[Option<String>; 4], &'static str> {
    const BAD: &str = "Invalid external ID.";
    let Value::Map(mut fields) = value else {
        return Err(BAD);
    };
    let Some((Value::String(provider), value)) = fields.pop_first() else {
        return Err(BAD);
    };
    let (text, number, name) = match value {
        Value::String(value) => (Some(value), None, None),
        Value::U64(value) => (None, Some(value.to_string()), None),
        Value::Map(mut fields) => {
            let Some(Value::String(name)) = fields.remove(&Value::String("name".into())) else {
                return Err(BAD);
            };
            let Some(Value::String(id)) = fields.remove(&Value::String("id".into())) else {
                return Err(BAD);
            };
            (Some(id), None, Some(name))
        }
        _ => return Err(BAD),
    };
    Ok([Some(provider), text, number, name])
}

#[derive(Default)]
struct FilterNode {
    value: Option<FieldValue>,
    children: BTreeMap<PathSegment, Self>,
}

impl FilterNode {
    fn insert(&mut self, path: &[PathSegment], value: FieldValue) -> Result<(), &'static str> {
        let mut node = self;
        for segment in path {
            node = node.children.entry(segment.clone()).or_default();
        }
        if node.value.is_some() {
            return Err("Duplicate lookup field.");
        }
        node.value = Some(value);
        Ok(())
    }

    fn validate(&self) -> Result<(), &'static str> {
        if let Some((first, _)) = self.children.first_key_value()
            && (self.value.is_some()
                || self
                    .children
                    .keys()
                    .any(|key| std::mem::discriminant(key) != std::mem::discriminant(first)))
        {
            return Err("Conflicting lookup paths.");
        }
        self.children.values().try_for_each(Self::validate)
    }

    fn matches(&self, record: &Value, ignore_case: bool) -> bool {
        let record = match record {
            Value::Option(Some(value)) | Value::Newtype(value) => {
                return self.matches(value, ignore_case);
            }
            _ => record,
        };
        if let Some(value) = &self.value {
            return match (value, record) {
                (FieldValue::Text(want), Value::String(actual)) if ignore_case => want
                    .trim_matches(' ')
                    .eq_ignore_ascii_case(actual.trim_matches(' ')),
                (FieldValue::Text(want), Value::String(actual)) => want == actual,
                (FieldValue::Unsigned(want), _) => integer(record) == Some(i128::from(*want)),
                (FieldValue::Signed(want), _) => integer(record) == Some(i128::from(*want)),
                (FieldValue::Float(want), Value::F32(actual)) => {
                    want.partial_cmp(actual) == Some(Ordering::Equal)
                }
                (FieldValue::Float(want), Value::F64(actual)) => {
                    f64::from(*want).partial_cmp(actual) == Some(Ordering::Equal)
                }
                (FieldValue::Float(want), _) => {
                    integer(record).is_some_and(|value| float_matches_integer(*want, value))
                }
                (FieldValue::Bool(want), Value::Bool(actual)) => want == actual,
                (FieldValue::Null, Value::Unit | Value::Option(None))
                | (FieldValue::Array, Value::Seq(_))
                | (FieldValue::Object, Value::Map(_)) => true,
                _ => false,
            };
        }
        self.children
            .iter()
            .all(|(segment, filter)| match (segment, record) {
                (PathSegment::Field(key), Value::Map(fields)) => fields
                    .get(&Value::String(key.clone()))
                    .is_some_and(|value| {
                        filter.matches(
                            value,
                            matches!(key.as_str(), "name" | "title" | "sort_name" | "sort_title"),
                        )
                    }),
                (PathSegment::Element(_), Value::Seq(items)) => {
                    items.iter().any(|item| filter.matches(item, false))
                }
                _ => false,
            })
    }
}

#[allow(clippy::cast_possible_truncation)]
fn float_matches_integer(want: f32, actual: i128) -> bool {
    // Values that saturate the cast cannot match a stored integer, which is at most 64 bits.
    want.fract() == 0.0 && (want as i128) == actual
}

fn integer(value: &Value) -> Option<i128> {
    Some(match value {
        Value::U8(value) => i128::from(*value),
        Value::U16(value) => i128::from(*value),
        Value::U32(value) => i128::from(*value),
        Value::U64(value) => i128::from(*value),
        Value::I8(value) => i128::from(*value),
        Value::I16(value) => i128::from(*value),
        Value::I32(value) => i128::from(*value),
        Value::I64(value) => i128::from(*value),
        _ => return None,
    })
}
