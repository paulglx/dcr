use dcr::dicom::parse_dicom_datetime_delta_ms;

#[test]
fn da_same_day_returns_zero() {
    let result = parse_dicom_datetime_delta_ms("DA", "20230101", "20230101");
    assert_eq!(result, Some(0));
}

#[test]
fn da_one_day_forward() {
    let result = parse_dicom_datetime_delta_ms("DA", "20230101", "20230102");
    assert_eq!(result, Some(86_400_000));
}

#[test]
fn da_one_day_backward_is_negative() {
    let result = parse_dicom_datetime_delta_ms("DA", "20230102", "20230101");
    assert_eq!(result, Some(-86_400_000));
}

#[test]
fn tm_same_time_returns_zero() {
    let result = parse_dicom_datetime_delta_ms("TM", "120000", "120000");
    assert_eq!(result, Some(0));
}

#[test]
fn tm_one_hour_diff() {
    let result = parse_dicom_datetime_delta_ms("TM", "110000", "120000");
    assert_eq!(result, Some(3_600_000));
}

#[test]
fn tm_fractional_seconds() {
    let result = parse_dicom_datetime_delta_ms("TM", "120000.000", "120000.500");
    assert_eq!(result, Some(500));
}

#[test]
fn dt_same_datetime_returns_zero() {
    let result = parse_dicom_datetime_delta_ms("DT", "20230101120000", "20230101120000");
    assert_eq!(result, Some(0));
}

#[test]
fn dt_cross_day_delta() {
    let result = parse_dicom_datetime_delta_ms("DT", "20230101235900", "20230102000100");
    assert_eq!(result, Some(120_000));
}

#[test]
fn dt_fractional_seconds() {
    let result = parse_dicom_datetime_delta_ms("DT", "20230101120000.000", "20230101120000.750");
    assert_eq!(result, Some(750));
}

#[test]
fn unknown_vr_returns_none() {
    let result = parse_dicom_datetime_delta_ms("PN", "Smith", "Jones");
    assert_eq!(result, None);
}

#[test]
fn multi_value_uses_first_value_only() {
    let result = parse_dicom_datetime_delta_ms("DA", "20230101\\20230615", "20230102\\20230715");
    assert_eq!(result, Some(86_400_000));
}

#[test]
fn malformed_da_returns_none() {
    let result = parse_dicom_datetime_delta_ms("DA", "not-a-date", "20230101");
    assert_eq!(result, None);
}

#[test]
fn malformed_tm_returns_none() {
    let result = parse_dicom_datetime_delta_ms("TM", "garbage", "120000");
    assert_eq!(result, None);
}

#[test]
fn malformed_dt_returns_none() {
    let result = parse_dicom_datetime_delta_ms("DT", "xyz", "20230101120000");
    assert_eq!(result, None);
}
