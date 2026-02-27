use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

fn first_value(s: &str) -> &str {
    s.split('\\').next().unwrap_or(s).trim()
}

pub fn parse_dicom_datetime_delta_ms(vr: &str, baseline: &str, modified: &str) -> Option<i64> {
    let base = first_value(baseline);
    let mod_ = first_value(modified);
    match vr {
        "DA" => {
            let d1 = NaiveDate::parse_from_str(base, "%Y%m%d").ok()?;
            let d2 = NaiveDate::parse_from_str(mod_, "%Y%m%d").ok()?;
            let delta_days = (d2 - d1).num_days();
            Some(delta_days * 24 * 3600 * 1000)
        }
        "TM" => {
            let t1 = parse_tm(base)?;
            let t2 = parse_tm(mod_)?;
            Some((t2 - t1).num_milliseconds())
        }
        "DT" => {
            let dt1 = parse_dt(base)?;
            let dt2 = parse_dt(mod_)?;
            Some((dt2 - dt1).num_milliseconds())
        }
        _ => None,
    }
}

fn parse_tm(s: &str) -> Option<NaiveTime> {
    if s.contains('.') {
        NaiveTime::parse_from_str(s, "%H%M%S%.f").ok()
    } else {
        NaiveTime::parse_from_str(s, "%H%M%S").ok()
    }
}

fn parse_dt(s: &str) -> Option<NaiveDateTime> {
    if s.contains('.') {
        NaiveDateTime::parse_from_str(s, "%Y%m%d%H%M%S%.f").ok()
    } else {
        NaiveDateTime::parse_from_str(s, "%Y%m%d%H%M%S").ok()
    }
}
