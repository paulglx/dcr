//! Millisecond deltas between DICOM DA / TM / DT values, without a date-time library.
//!
//! Only the fixed-width forms are accepted: `YYYYMMDD`, `HHMMSS[.f+]` and
//! `YYYYMMDDHHMMSS[.f+]`. Anything else yields `None`.

const MS_PER_DAY: i64 = 24 * 3600 * 1000;
const NS_PER_MS: i64 = 1_000_000;

fn first_value(s: &str) -> &str {
    s.split('\\').next().unwrap_or(s).trim()
}

pub fn parse_dicom_datetime_delta_ms(vr: &str, baseline: &str, modified: &str) -> Option<i64> {
    let base = first_value(baseline);
    let mod_ = first_value(modified);
    match vr {
        "DA" => Some((parse_da(mod_)? - parse_da(base)?) * MS_PER_DAY),
        "TM" => Some((parse_tm(mod_)? - parse_tm(base)?) / NS_PER_MS),
        "DT" => Some((parse_dt(mod_)? - parse_dt(base)?) / NS_PER_MS),
        _ => None,
    }
}

/// Parses exactly `n` ASCII digits from the front of `s`, returning the value and the rest.
fn digits(s: &str, n: usize) -> Option<(i64, &str)> {
    let (head, rest) = s.split_at_checked(n)?;
    if head.len() != n || !head.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((head.parse().ok()?, rest))
}

/// `YYYYMMDD` → days since 1970-01-01.
fn parse_da(s: &str) -> Option<i64> {
    let (y, rest) = digits(s, 4)?;
    let (m, rest) = digits(rest, 2)?;
    let (d, rest) = digits(rest, 2)?;
    if !rest.is_empty() {
        return None;
    }
    days_from_civil(y, m, d)
}

/// `HHMMSS[.f+]` → nanoseconds since midnight.
fn parse_tm(s: &str) -> Option<i64> {
    let (ns, rest) = parse_time_prefix(s)?;
    rest.is_empty().then_some(ns)
}

/// `YYYYMMDDHHMMSS[.f+]` → nanoseconds since 1970-01-01T00:00:00.
fn parse_dt(s: &str) -> Option<i64> {
    let (date, time) = s.split_at_checked(8)?;
    let days = parse_da(date)?;
    let ns = parse_tm(time)?;
    Some(days * MS_PER_DAY * NS_PER_MS + ns)
}

/// Parses `HHMMSS` plus an optional `.f` fraction (1–9 digits). Returns nanoseconds and the rest.
fn parse_time_prefix(s: &str) -> Option<(i64, &str)> {
    let (h, rest) = digits(s, 2)?;
    let (m, rest) = digits(rest, 2)?;
    let (sec, rest) = digits(rest, 2)?;
    if h >= 24 || m >= 60 || sec >= 60 {
        return None;
    }
    let mut ns = ((h * 3600 + m * 60 + sec) * 1000) * NS_PER_MS;
    let rest = match rest.strip_prefix('.') {
        Some(frac) => {
            let len = frac.bytes().take_while(|b| b.is_ascii_digit()).count();
            if len == 0 || len > 9 {
                return None;
            }
            let (frac_digits, rest) = frac.split_at(len);
            let value: i64 = frac_digits.parse().ok()?;
            ns += value * 10_i64.pow(9 - len as u32);
            rest
        }
        None => rest,
    };
    Some((ns, rest))
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> Option<i64> {
    if !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_is_day_zero() {
        assert_eq!(days_from_civil(1970, 1, 1), Some(0));
    }

    #[test]
    fn leap_years_are_handled() {
        assert_eq!(parse_da("20240229").is_some(), true);
        assert_eq!(parse_da("20230229"), None);
        assert_eq!(parse_da("19000229"), None);
        assert_eq!(parse_da("20000229").is_some(), true);
    }

    #[test]
    fn year_boundary_delta() {
        assert_eq!(
            parse_dicom_datetime_delta_ms("DA", "20231231", "20240101"),
            Some(MS_PER_DAY)
        );
    }

    #[test]
    fn sub_millisecond_fraction_truncates_toward_zero() {
        assert_eq!(parse_dicom_datetime_delta_ms("TM", "120000", "120000.0009"), Some(0));
        assert_eq!(parse_dicom_datetime_delta_ms("TM", "120000.0009", "120000"), Some(0));
    }

    #[test]
    fn out_of_range_fields_rejected() {
        assert_eq!(parse_tm("240000"), None);
        assert_eq!(parse_tm("126000"), None);
        assert_eq!(parse_da("20231301"), None);
        assert_eq!(parse_tm("1200"), None);
        assert_eq!(parse_tm("120000."), None);
    }
}
