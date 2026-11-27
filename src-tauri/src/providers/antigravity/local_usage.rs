use std::{
    collections::{HashMap, HashSet},
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{DateTime, Local, NaiveDate, Utc};
use rusqlite::{Connection, ErrorCode, OpenFlags, OptionalExtension, Statement};

use crate::{
    models::UsageHistory,
    pricing::{ModelPricing, TokenBreakdown},
    providers::daily_usage::DailyUsageAccumulator,
};

pub(super) const SOURCE_NOTE: &str = "From your Antigravity conversation databases (estimated)";
const MAX_BLOB_BYTES: i64 = 4 * 1024 * 1024;
const UNKNOWN_MODEL: &str = "Unknown Antigravity model";

pub(super) fn scan(
    now: DateTime<Utc>,
    pricing: &ModelPricing,
    warnings: &mut Vec<String>,
) -> Result<UsageHistory, Unavailable> {
    let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) else {
        return Ok(UsageHistory::default());
    };
    scan_stores(&PathBuf::from(home).join(".gemini"), now, pricing, warnings)
}

fn scan_stores(
    root: &Path,
    now: DateTime<Utc>,
    pricing: &ModelPricing,
    warnings: &mut Vec<String>,
) -> Result<UsageHistory, Unavailable> {
    let since = now
        .with_timezone(&Local)
        .date_naive()
        .checked_sub_days(crate::policy::USAGE_HISTORY_LOOKBACK)
        .unwrap_or(NaiveDate::MIN);
    let mut accumulator = DailyUsageAccumulator::default();
    let mut seen = HashMap::<String, HashSet<i64>>::new();
    let mut incompatible = false;
    for store in ["antigravity-cli", "antigravity", "antigravity-ide"] {
        let entries = match fs::read_dir(root.join(store).join("conversations")) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(_) => return Err(Unavailable),
        };
        let mut paths = entries
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|_| Unavailable)?;
        paths.sort_unstable();
        for path in paths
            .iter()
            .filter(|path| path.extension().is_some_and(|ext| ext == "db"))
        {
            let result = read_database(path, since, |conversation, index, event| {
                if !seen
                    .entry(conversation.to_owned())
                    .or_default()
                    .insert(index)
                {
                    return;
                }
                let (model, cost) = event.priced_model(pricing);
                let date = event.timestamp.with_timezone(&Local).date_naive();
                let tokens = event.tokens.total_tokens();
                match cost {
                    Some(cost) => accumulator.add(date, tokens, cost, model),
                    None => accumulator.add_unpriced(date, tokens, model),
                }
            });
            match result {
                Ok(DatabaseRead::Complete) => {}
                Ok(DatabaseRead::Incompatible) => incompatible = true,
                Err(error) if incompatible_error(&error) => incompatible = true,
                Err(_) => return Err(Unavailable),
            }
        }
    }
    if incompatible {
        warnings.push(
            "Some local Antigravity usage records use an unsupported format and were skipped."
                .into(),
        );
    }
    Ok(accumulator.build(now, SOURCE_NOTE))
}

#[derive(Debug)]
pub(super) struct Unavailable;

#[derive(Debug, PartialEq, Eq)]
enum DatabaseRead {
    Complete,
    Incompatible,
}

fn incompatible_error(error: &rusqlite::Error) -> bool {
    matches!(
        error.sqlite_error_code(),
        Some(ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt)
    )
}

fn read_database(
    path: &Path,
    since: NaiveDate,
    mut record: impl FnMut(&str, i64, Generation),
) -> rusqlite::Result<DatabaseRead> {
    let mut connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(Duration::from_millis(150))?;
    // A read transaction sees committed WAL frames and keeps schema and row reads on one snapshot.
    let database = connection.transaction()?;
    if !has_columns(&database, "gen_metadata", "idx", "data")? {
        return database.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema)", [], |row| {
            Ok(if row.get(0)? {
                DatabaseRead::Incompatible
            } else {
                DatabaseRead::Complete
            })
        });
    }
    let conversation = if has_columns(&database, "trajectory_meta", "cascade_id", "trajectory_id")?
    {
        database
            .query_row(
                "SELECT cascade_id FROM trajectory_meta WHERE typeof(cascade_id) = 'text'
                 AND length(cascade_id) BETWEEN 1 AND 128 LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
    } else {
        None
    };
    let conversation = conversation
        .as_deref()
        .or_else(|| path.file_stem().and_then(|name| name.to_str()));
    let Some(conversation) = conversation else {
        return Ok(DatabaseRead::Incompatible);
    };
    let mut steps = has_columns(&database, "steps", "idx", "metadata")?
        .then(|| {
            database.prepare(
                "SELECT metadata FROM steps WHERE idx = ?1
                 AND typeof(metadata) = 'blob' AND length(metadata) <= ?2",
            )
        })
        .transpose()?;
    let mut generations = database.prepare(
        "SELECT CASE WHEN typeof(idx) = 'integer' AND idx >= 0 THEN idx END,
                CASE WHEN typeof(data) = 'blob' AND length(data) <= ?1 THEN data END
         FROM gen_metadata WHERE data IS NOT NULL ORDER BY idx",
    )?;
    let mut rows = generations.query([MAX_BLOB_BYTES])?;
    let mut status = DatabaseRead::Complete;
    while let Some(row) = rows.next()? {
        let (Some(index), Some(blob)) = (
            row.get::<_, Option<i64>>(0)?,
            row.get_ref(1)?.as_blob().ok(),
        ) else {
            status = DatabaseRead::Incompatible;
            continue;
        };
        match generation(blob, |reference| {
            step_timestamp(steps.as_mut(), reference.unwrap_or(index))
        })? {
            Ok(Some(event)) if event.timestamp.with_timezone(&Local).date_naive() >= since => {
                record(conversation, index, event);
            }
            Ok(_) => {}
            Err(InvalidRecord) => status = DatabaseRead::Incompatible,
        }
    }
    Ok(status)
}

fn has_columns(
    database: &Connection,
    table: &str,
    first: &str,
    second: &str,
) -> rusqlite::Result<bool> {
    database.query_row(
        "SELECT count(*) = 2 FROM pragma_table_info(?1) WHERE name IN (?2, ?3)",
        [table, first, second],
        |row| row.get(0),
    )
}

fn step_timestamp(
    steps: Option<&mut Statement<'_>>,
    index: i64,
) -> rusqlite::Result<Option<DateTime<Utc>>> {
    let Some(steps) = steps else { return Ok(None) };
    steps
        .query_row([index, MAX_BLOB_BYTES], |row| {
            Ok(row.get_ref(0)?.as_blob().ok().and_then(|metadata| {
                Message(metadata)
                    .bytes(1)
                    .ok()
                    .flatten()
                    .and_then(timestamp)
            }))
        })
        .optional()
        .map(Option::flatten)
}

struct Generation {
    tokens: TokenBreakdown,
    model_id: Option<String>,
    display_name: Option<String>,
    timestamp: DateTime<Utc>,
}

impl Generation {
    fn priced_model(&self, pricing: &ModelPricing) -> (&str, Option<f64>) {
        let model = self.model_id.as_deref();
        let specific =
            model.filter(|id| !id.ends_with("-default") && !id.starts_with("MODEL_PLACEHOLDER_"));
        let display = self.display_name.as_deref();
        [specific, display]
            .into_iter()
            .flatten()
            .find_map(|name| {
                pricing
                    .estimated_cost_dollars(name, self.tokens, true)
                    .map(|cost| (name, Some(cost)))
            })
            .or_else(|| {
                let label = display?;
                let slug = label
                    .split(" (")
                    .next()?
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join("-")
                    .to_ascii_lowercase();
                pricing
                    .estimated_cost_dollars(&slug, self.tokens, true)
                    .map(|cost| (label, Some(cost)))
            })
            .unwrap_or((display.or(model).unwrap_or(UNKNOWN_MODEL), None))
    }
}

#[derive(Debug)]
struct InvalidRecord;

fn generation(
    blob: &[u8],
    mut read_step_timestamp: impl FnMut(Option<i64>) -> rusqlite::Result<Option<DateTime<Utc>>>,
) -> rusqlite::Result<Result<Option<Generation>, InvalidRecord>> {
    let (record, model, tokens) = match generation_tokens(blob) {
        Ok(Some(parts)) => parts,
        Ok(None) => return Ok(Ok(None)),
        Err(error) => return Ok(Err(error)),
    };
    let date = model
        .bytes(9)
        .ok()
        .flatten()
        .and_then(|timing| Message(timing).bytes(4).ok().flatten())
        .and_then(timestamp);
    let date = match date {
        Some(date) => Some(date),
        None => {
            let references = match record.step_references() {
                Ok(references) => references,
                Err(error) => return Ok(Err(error)),
            };
            let mut date = None;
            for reference in references {
                date = read_step_timestamp(reference)?;
                if date.is_some() {
                    break;
                }
            }
            date
        }
    };
    Ok(date.map_or(Err(InvalidRecord), |timestamp| {
        Ok(Some(Generation {
            tokens,
            model_id: model.text(19),
            display_name: model.text(21),
            timestamp,
        }))
    }))
}

fn generation_tokens(
    blob: &[u8],
) -> Result<Option<(Message<'_>, Message<'_>, TokenBreakdown)>, InvalidRecord> {
    let record = Message(blob);
    let model = Message(record.bytes(1)?.ok_or(InvalidRecord)?);
    let Some(usage) = model.bytes(4)?.map(Message) else {
        return Ok(None);
    };
    let input = usage.number(2)?;
    let output = usage.number(3)?;
    let cache_write = usage.number(4)?;
    let cache_read = usage.number(5)?;
    let thinking = usage.number(9)?;
    let response = usage.number(10)?;
    if [input, output, cache_write, cache_read, thinking, response]
        .iter()
        .all(Option::is_none)
    {
        return Err(InvalidRecord);
    }
    // The aggregate already includes thinking. The split is a fallback for records without it.
    let output = match output {
        Some(total) => total,
        None => thinking
            .unwrap_or(0)
            .checked_add(response.unwrap_or(0))
            .ok_or(InvalidRecord)?,
    };
    let tokens = TokenBreakdown {
        input: input.unwrap_or(0),
        output,
        cache_write_5m: cache_write.unwrap_or(0),
        cache_read: cache_read.unwrap_or(0),
        ..TokenBreakdown::default()
    };
    let total = [
        tokens.input,
        output,
        tokens.cache_write_5m,
        tokens.cache_read,
    ]
    .into_iter()
    .try_fold(0_u64, u64::checked_add)
    .ok_or(InvalidRecord)?;
    Ok((total > 0).then_some((record, model, tokens)))
}

fn timestamp(data: &[u8]) -> Option<DateTime<Utc>> {
    let message = Message(data);
    let seconds = i64::try_from(message.number(1).ok()??).ok()?;
    let nanos = u32::try_from(message.number(2).ok()?.unwrap_or(0)).ok()?;
    (seconds > 0 && nanos < 1_000_000_000)
        .then(|| DateTime::from_timestamp(seconds, nanos))
        .flatten()
}

#[derive(Clone, Copy)]
struct Message<'a>(&'a [u8]);

enum Field<'a> {
    Number(u64),
    Bytes(&'a [u8]),
    Fixed,
}

impl<'a> Message<'a> {
    fn bytes(self, number: u64) -> Result<Option<&'a [u8]>, InvalidRecord> {
        self.find(number, |field| match field {
            Field::Bytes(bytes) => Some(bytes),
            _ => None,
        })
    }

    fn number(self, number: u64) -> Result<Option<u64>, InvalidRecord> {
        self.find(number, |field| match field {
            Field::Number(value) => Some(value),
            _ => None,
        })
    }

    fn text(self, number: u64) -> Option<String> {
        let value = std::str::from_utf8(self.bytes(number).ok()??).ok()?.trim();
        (!value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control))
            .then(|| value.to_owned())
    }

    fn find<T>(
        self,
        number: u64,
        select: impl Fn(Field<'a>) -> Option<T>,
    ) -> Result<Option<T>, InvalidRecord> {
        let mut data = self.0;
        let mut value = None;
        while !data.is_empty() {
            let (field_number, field) = read_field(&mut data)?;
            if field_number == number {
                if let Some(next) = select(field) {
                    value = Some(next);
                }
            }
        }
        Ok(value)
    }

    fn step_references(self) -> Result<Vec<Option<i64>>, InvalidRecord> {
        let mut data = self.0;
        let mut indices = Vec::new();
        while !data.is_empty() {
            let (number, field) = read_field(&mut data)?;
            if number != 2 {
                continue;
            }
            match field {
                Field::Number(index) => {
                    indices.push(Some(i64::try_from(index).map_err(|_| InvalidRecord)?))
                }
                Field::Bytes(mut packed) => {
                    while !packed.is_empty() {
                        indices.push(Some(
                            i64::try_from(varint(&mut packed)?).map_err(|_| InvalidRecord)?,
                        ));
                    }
                }
                Field::Fixed => continue,
            }
        }
        if indices.is_empty() {
            indices.push(None);
        }
        Ok(indices)
    }
}

fn read_field<'a>(data: &mut &'a [u8]) -> Result<(u64, Field<'a>), InvalidRecord> {
    let tag = varint(data)?;
    let number = tag >> 3;
    if !(1..=0x1fff_ffff).contains(&number) {
        return Err(InvalidRecord);
    }
    let field = match tag & 7 {
        0 => Field::Number(varint(data)?),
        2 => {
            let length = usize::try_from(varint(data)?).map_err(|_| InvalidRecord)?;
            Field::Bytes(take(data, length)?)
        }
        wire @ (1 | 5) => {
            take(data, if wire == 1 { 8 } else { 4 })?;
            Field::Fixed
        }
        _ => return Err(InvalidRecord),
    };
    Ok((number, field))
}

fn varint(data: &mut &[u8]) -> Result<u64, InvalidRecord> {
    let mut value = 0;
    for shift in (0..64).step_by(7) {
        let byte = take(data, 1)?[0];
        if shift == 63 && byte > 1 {
            return Err(InvalidRecord);
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte < 0x80 {
            return Ok(value);
        }
    }
    Err(InvalidRecord)
}

fn take<'a>(data: &mut &'a [u8], length: usize) -> Result<&'a [u8], InvalidRecord> {
    let (value, rest) = data.split_at_checked(length).ok_or(InvalidRecord)?;
    *data = rest;
    Ok(value)
}

#[cfg(test)]
#[path = "local_usage_tests.rs"]
mod tests;
