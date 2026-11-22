//! UTC timestamps with the cache's existing serialized representation.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::fmt;
use std::str::FromStr;
use time::format_description::BorrowedFormatItem;
use time::macros::format_description;
use time::parsing::Parsed;
use time::{Date, SignedDuration, Time, UtcDateTime, UtcOffset};

const NANOS_PER_SECOND: u32 = 1_000_000_000;
const DATE: &[BorrowedFormatItem<'_>] = format_description!("[year range:extended]-[month]-[day]");
const TIME: &[BorrowedFormatItem<'_>] = format_description!(
    version = 2,
    "[hour]:[minute]:[second][optional [.[subsecond]]]"
);
const OFFSET: &[BorrowedFormatItem<'_>] = format_description!(
    version = 2,
    "[offset_hour sign:mandatory][optional [:]][offset_minute]"
);

/// UTC cache timestamp with nanosecond precision.
///
/// Serialized timestamps are UTC strings ending in `Z`. Parsing also accepts
/// numeric offsets, which are normalized to UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    seconds: i64,
    // A legacy leap second sorts after ordinary fractions of its nominal second.
    nanoseconds: u32,
}

impl Timestamp {
    /// Read the current UTC time.
    pub fn now() -> Self {
        UtcDateTime::now().into()
    }

    /// Construct a timestamp from nanoseconds since the Unix epoch.
    pub fn from_unix_timestamp_nanos(
        nanoseconds: i128,
    ) -> Result<Self, time::error::ComponentRange> {
        UtcDateTime::from_unix_timestamp_nanos(nanoseconds).map(Self::from)
    }

    /// Return the Unix timestamp rounded down to a whole second.
    ///
    /// A leap second shares the whole-second value of the second preceding it.
    pub const fn unix_timestamp(self) -> i64 {
        self.seconds
    }

    /// Return the fractional nanoseconds within the current second.
    pub const fn nanosecond(self) -> u32 {
        self.nanoseconds % NANOS_PER_SECOND
    }

    /// Return whether the timestamp represents a leap second.
    pub const fn is_leap_second(self) -> bool {
        self.nanoseconds >= NANOS_PER_SECOND
    }

    /// Add a duration, clamping to the supported timestamp range on overflow.
    pub fn saturating_add(self, duration: SignedDuration) -> Self {
        let nanos_per_second = i128::from(NANOS_PER_SECOND);
        let mut fraction = i128::from(self.nanoseconds) + duration.whole_nanoseconds();
        if self.is_leap_second() {
            if (nanos_per_second..2 * nanos_per_second).contains(&fraction) {
                return Self {
                    nanoseconds: fraction as u32,
                    ..self
                };
            }
            if fraction >= 2 * nanos_per_second {
                fraction -= nanos_per_second;
            }
        }
        let nanos = i128::from(self.seconds) * nanos_per_second + fraction;
        let min = UtcDateTime::MIN.unix_timestamp_nanos();
        let max = UtcDateTime::MAX.unix_timestamp_nanos();
        Self::from_unix_timestamp_nanos(nanos.clamp(min, max))
            .expect("timestamp is clamped to the valid range")
    }

    pub(crate) fn duration_since(self, earlier: Self) -> SignedDuration {
        let mut seconds = self.seconds - earlier.seconds;
        if self.seconds > earlier.seconds && earlier.is_leap_second() {
            seconds += 1;
        } else if self.seconds < earlier.seconds && self.is_leap_second() {
            seconds -= 1;
        }
        let fraction = self.nanoseconds as i32 - earlier.nanoseconds as i32;
        SignedDuration::new(seconds, fraction)
    }
}

impl From<UtcDateTime> for Timestamp {
    fn from(value: UtcDateTime) -> Self {
        Self {
            seconds: value.unix_timestamp(),
            nanoseconds: value.nanosecond(),
        }
    }
}

/// A timestamp string is malformed or outside the supported range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid UTC or offset timestamp")]
pub struct ParseTimestampError;

impl fmt::Display for Timestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let utc = UtcDateTime::from_unix_timestamp(self.seconds).map_err(|_| fmt::Error)?;
        let (year, month, day) = utc.to_calendar_date();
        if (0..=9999).contains(&year) {
            write!(formatter, "{year:04}")?;
        } else {
            write!(formatter, "{year:+05}")?;
        }
        let (hour, minute, second) = utc.as_hms();
        let second = second + u8::from(self.is_leap_second());
        write!(
            formatter,
            "-{:02}-{day:02}T{hour:02}:{minute:02}:{second:02}",
            u8::from(month)
        )?;

        let fractional = self.nanosecond();
        if fractional != 0 {
            if fractional.is_multiple_of(1_000_000) {
                write!(formatter, ".{:03}", fractional / 1_000_000)?;
            } else if fractional.is_multiple_of(1_000) {
                write!(formatter, ".{:06}", fractional / 1_000)?;
            } else {
                write!(formatter, ".{fractional:09}")?;
            }
        }
        formatter.write_str("Z")
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TimestampVisitor;

        impl de::Visitor<'_> for TimestampVisitor {
            type Value = Timestamp;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a timestamp string with a UTC or numeric offset")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Timestamp, E> {
                value.parse().map_err(E::custom)
            }
        }

        deserializer.deserialize_str(TimestampVisitor)
    }
}

impl FromStr for Timestamp {
    type Err = ParseTimestampError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (date, rest) = value
            .trim()
            .split_once(['T', 't', ' '])
            .ok_or(ParseTimestampError)?;
        let date = Date::parse(date, DATE).map_err(|_| ParseTimestampError)?;
        let mut parsed_time = Parsed::new();
        let offset = parsed_time
            .parse_items(rest.trim_start().as_bytes(), TIME)
            .map_err(|_| ParseTimestampError)?;
        let offset = parse_offset(offset)?;
        let leap_second = parsed_time.second() == Some(60);
        if leap_second {
            parsed_time.set_second(59).ok_or(ParseTimestampError)?;
        }
        let time = Time::try_from(parsed_time).map_err(|_| ParseTimestampError)?;
        let utc = UtcDateTime::new(date, time)
            .checked_sub(SignedDuration::seconds(i64::from(offset.whole_seconds())))
            .ok_or(ParseTimestampError)?;
        let mut timestamp = Self::from(utc);
        if leap_second {
            timestamp.nanoseconds += NANOS_PER_SECOND;
        }
        Ok(timestamp)
    }
}

fn parse_offset(value: &[u8]) -> Result<UtcOffset, ParseTimestampError> {
    let value = std::str::from_utf8(value)
        .map_err(|_| ParseTimestampError)?
        .trim_start();
    if value.eq_ignore_ascii_case("z") || value.eq_ignore_ascii_case("utc") {
        return Ok(UtcOffset::UTC);
    }
    let offset = UtcOffset::parse(value, OFFSET).map_err(|_| ParseTimestampError)?;
    if offset.whole_hours().unsigned_abs() >= 24 {
        return Err(ParseTimestampError);
    }
    Ok(offset)
}
