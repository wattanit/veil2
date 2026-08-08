//! Turning results into something to read or something to parse
//! (Design §3.4, §7).
//!
//! Results go to standard output; progress and failures go to standard error,
//! so a pipeline is not polluted by either.

use std::io::Write;

use crate::cli::Format;
use crate::failure::Run;

/// One file, as the table and the JSON both see it.
#[derive(Debug, serde::Serialize)]
pub struct FileRow {
    /// The file's name.
    pub name: String,
    /// The folder recorded with it. Empty for a file at the vault's root.
    pub folder: String,
    /// Size in bytes — exact here, human-readable only in the table.
    pub size: u64,
    /// When it was added, in seconds since the Unix epoch.
    pub added: u64,
}

/// Writes a value as JSON to standard output.
pub fn json(value: &impl serde::Serialize) -> Run<()> {
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, value)
        .map_err(|e| anyhow::Error::new(e).context("cannot write the result"))?;
    out.write_all(b"\n")?;
    Ok(())
}

/// Writes the file listing as a table, in the column order Design §3.2 fixes:
/// name, folder, size, added.
///
/// Widths are counted in characters. For scripts whose characters are drawn
/// double-width — Han, Hangul, Kana — the column edge will not line up. The
/// name is still printed exactly as stored, which is the half that matters:
/// padding a name to straighten a column would alter what the vault reports it
/// holds, and HC-8 makes the stored name authoritative.
pub fn table(rows: &[FileRow]) -> Run<()> {
    let mut out = std::io::stdout().lock();
    if rows.is_empty() {
        writeln!(out, "No files.")?;
        return Ok(());
    }

    let sizes: Vec<String> = rows.iter().map(|r| human_size(r.size)).collect();
    let stamps: Vec<String> = rows.iter().map(|r| stamp(r.added)).collect();

    let name_w = width("Name", rows.iter().map(|r| r.name.as_str()));
    let folder_w = width("Folder", rows.iter().map(|r| r.folder.as_str()));
    let size_w = width("Size", sizes.iter().map(String::as_str));

    writeln!(
        out,
        "{}  {}  {:>size_w$}  Added (UTC)",
        pad("Name", name_w),
        pad("Folder", folder_w),
        "Size"
    )?;
    for ((row, size), stamp) in rows.iter().zip(&sizes).zip(&stamps) {
        writeln!(
            out,
            "{:<name_w$}  {:<folder_w$}  {:>size_w$}  {}",
            pad(&row.name, name_w),
            pad(&row.folder, folder_w),
            size,
            stamp
        )?;
    }
    writeln!(
        out,
        "\n{} file{}",
        count(rows.len() as u64),
        plural(rows.len())
    )?;
    Ok(())
}

/// The same listing, grouped by folder.
pub fn grouped(rows: &[FileRow]) -> Run<()> {
    let mut out = std::io::stdout().lock();
    if rows.is_empty() {
        writeln!(out, "No files.")?;
        return Ok(());
    }

    let mut folders: Vec<&str> = rows.iter().map(|r| r.folder.as_str()).collect();
    folders.sort_unstable();
    folders.dedup();

    for folder in folders {
        writeln!(
            out,
            "\n{}",
            if folder.is_empty() {
                "(no folder)"
            } else {
                folder
            }
        )?;
        for row in rows.iter().filter(|r| r.folder == folder) {
            writeln!(
                out,
                "  {}  {}  {}",
                row.name,
                human_size(row.size),
                stamp(row.added)
            )?;
        }
    }
    writeln!(
        out,
        "\n{} file{}",
        count(rows.len() as u64),
        plural(rows.len())
    )?;
    Ok(())
}

/// Writes a line of prose to standard output, for a command whose result is a
/// sentence rather than a table. Suppressed in JSON mode, where prose on
/// standard output would break whatever is parsing it.
pub fn say(format: Format, message: &str) -> Run<()> {
    if format == Format::Table {
        writeln!(std::io::stdout().lock(), "{message}")?;
    }
    Ok(())
}

/// Writes an advisory to standard error, in both output modes.
///
/// Standard output carries results, and a script reading it must find valid
/// output or nothing — never a sentence where a listing was expected. But some
/// things have to be said whatever the caller asked for: FR-32 requires the
/// space reconciliation recovered to be reported rather than absorbed, and a
/// read-only vault to say so. Neither is a result of the command that was
/// asked for, so neither belongs on standard output.
pub fn note(message: &str) {
    let _ = writeln!(std::io::stderr().lock(), "{message}");
}

/// A size in the units a person reads, one decimal place, powers of 1000.
#[must_use]
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["kB", "MB", "GB", "TB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1000.0;
    let mut unit = 0;
    while value >= 1000.0 && unit + 1 < UNITS.len() {
        value /= 1000.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// A count with thousands separators. Exact, never rounded (Design §7).
#[must_use]
pub fn count(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `""` or `"s"`, so a count and its noun agree.
#[must_use]
pub fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// A Unix timestamp as `YYYY-MM-DD HH:MM` in UTC.
///
/// UTC rather than local time, because converting to local time needs a
/// timezone database this project does not carry, and a time silently wrong by
/// hours is worse than one labelled with its zone.
#[must_use]
pub fn stamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        rest / 3600,
        (rest % 3600) / 60
    )
}

/// Days since the Unix epoch to a calendar date.
///
/// Howard Hinnant's `civil_from_days`, which is the standard published solution
/// to this and is exact for every date the format can hold. Written out rather
/// than pulled in as a dependency: it is fifteen lines, and its output is
/// checked against known dates in this module's tests.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m as u32, d as u32)
}

/// Column width: the header or the widest value, in characters.
fn width<'a>(header: &str, values: impl Iterator<Item = &'a str>) -> usize {
    values
        .map(|v| v.chars().count())
        .chain(std::iter::once(header.chars().count()))
        .max()
        .unwrap_or(0)
}

/// Pads to a character count. `{:<w$}` pads to a *byte* count, which turns
/// every non-ASCII name into a short column.
fn pad(value: &str, width: usize) -> String {
    let mut out = value.to_owned();
    for _ in value.chars().count()..width {
        out.push(' ');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_the_way_a_person_reads_them() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(999), "999 B");
        assert_eq!(human_size(1000), "1.0 kB");
        assert_eq!(human_size(48_200_000), "48.2 MB");
        assert_eq!(human_size(312_400_000_000), "312.4 GB");
    }

    #[test]
    fn counts_are_exact() {
        assert_eq!(count(0), "0");
        assert_eq!(count(999), "999");
        assert_eq!(count(1284), "1,284");
        assert_eq!(count(1_000_000), "1,000,000");
    }

    #[test]
    fn dates_are_the_dates_they_claim_to_be() {
        assert_eq!(stamp(0), "1970-01-01 00:00");
        assert_eq!(stamp(1_000_000_000), "2001-09-09 01:46");
        // A leap day, which is where a hand-written conversion goes wrong.
        assert_eq!(stamp(1_709_208_000), "2024-02-29 12:00");
        assert_eq!(stamp(1_754_640_000), "2025-08-08 08:00");
    }

    #[test]
    fn padding_counts_characters_not_bytes() {
        assert_eq!(pad("ก", 3).chars().count(), 3);
        assert_eq!(pad("abc", 3), "abc");
    }
}
