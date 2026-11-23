use super::ParseError;
use jiff::civil::Date;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{cmp::Ordering, fmt, str::FromStr};

/// A date known to the year, month or day (`YYYY[-MM[-DD]]`), ordered chronologically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SerializeDisplay, DeserializeFromStr)]
pub enum PartialDate {
    Year(u16),
    YearMonth { year: u16, month: u8 },
    YearMonthDay { year: u16, month: u8, day: u8 },
}

impl PartialDate {
    pub const fn year(year: u16) -> Self {
        Self::Year(year)
    }

    pub const fn ymd(year: u16, month: u8, day: u8) -> Self {
        Self::YearMonthDay { year, month, day }
    }

    /// The exact date, if the day is known.
    pub fn to_date(self) -> Option<Date> {
        let Self::YearMonthDay { year, month, day } = self else {
            return None;
        };
        Date::new(
            i16::try_from(year).ok()?,
            i8::try_from(month).ok()?,
            i8::try_from(day).ok()?,
        )
        .ok()
    }

    const fn sort_key(self) -> (u16, Option<u8>, Option<u8>) {
        match self {
            Self::Year(year) => (year, None, None),
            Self::YearMonth { year, month } => (year, Some(month), None),
            Self::YearMonthDay { year, month, day } => (year, Some(month), Some(day)),
        }
    }
}

impl Ord for PartialDate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.sort_key().cmp(&other.sort_key())
    }
}

impl PartialOrd for PartialDate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<Date> for PartialDate {
    fn from(date: Date) -> Self {
        Self::ymd(
            date.year().unsigned_abs(),
            date.month().unsigned_abs(),
            date.day().unsigned_abs(),
        )
    }
}

impl FromStr for PartialDate {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        const BAD: ParseError = ParseError("invalid date, expected YYYY[-MM[-DD]]");
        let mut parts = s.split('-');
        let year = parts
            .next()
            .filter(|y| y.len() == 4 && y.bytes().all(|byte| byte.is_ascii_digit()))
            .ok_or(BAD)?;
        let year = year.parse().map_err(|_| BAD)?;
        let mut field = |max: u8| -> Result<Option<u8>, ParseError> {
            parts
                .next()
                .map(|p| {
                    let n: u8 = p.parse().map_err(|_| BAD)?;
                    if p.len() == 2
                        && p.bytes().all(|byte| byte.is_ascii_digit())
                        && (1..=max).contains(&n)
                    {
                        Ok(n)
                    } else {
                        Err(BAD)
                    }
                })
                .transpose()
        };
        let Some(month) = field(12)? else {
            return Ok(Self::Year(year));
        };
        let Some(day) = field(31)? else {
            return Ok(Self::YearMonth { year, month });
        };
        let date = Self::YearMonthDay { year, month, day };
        (parts.next().is_none() && date.to_date().is_some())
            .then_some(date)
            .ok_or(BAD)
    }
}

impl fmt::Display for PartialDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Year(year) => write!(f, "{year:04}"),
            Self::YearMonth { year, month } => write!(f, "{year:04}-{month:02}"),
            Self::YearMonthDay { year, month, day } => write!(f, "{year:04}-{month:02}-{day:02}"),
        }
    }
}
