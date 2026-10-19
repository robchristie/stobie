//! Format the shared fixtures (`fixtures.json`) on this platform, so that
//! the native unit tests and the browser test check one set of expectations
//! through one evaluator. Not part of the public API.

use serde_json::{Value, json};

use super::{Clock, DateOrder, HourCycle, LocalTime, TimeLocale, TimeZone, Timestamp};

/// The fixtures with every `expected` value and every `locales` entry
/// replaced by what this platform gives. Equal to the input when the
/// platform agrees.
pub fn report(fixtures: &str) -> Result<String, String> {
    let mut fixtures: Value = serde_json::from_str(fixtures).map_err(|error| error.to_string())?;
    let fixtures_object = fixtures
        .as_object_mut()
        .ok_or("fixtures are not an object")?;
    let cases = fixtures_object
        .get_mut("cases")
        .and_then(Value::as_array_mut);
    for case in cases.ok_or("no cases")? {
        let case = case.as_object_mut().ok_or("a case is not an object")?;
        let zone = case
            .get("zone")
            .and_then(Value::as_str)
            .ok_or("case without a zone")?;
        let zone = TimeZone::named(zone).map_err(|error| error.to_string())?;
        let locale = case.get("locale").and_then(Value::as_str);
        let locale = parse_locale(locale.ok_or("case without a locale")?)?;
        let now = timestamp(case.get("now").unwrap_or(&Value::Null))?;
        let local = LocalTime::new(zone, locale, Clock::Fixed(now));
        let instants = case.get_mut("instants").and_then(Value::as_array_mut);
        for instant in instants.ok_or("case without instants")? {
            let instant = instant
                .as_object_mut()
                .ok_or("an instant is not an object")?;
            let at = timestamp(instant.get("at").unwrap_or(&Value::Null))?;
            instant.insert("expected".into(), formats(&local, at));
        }
    }
    let locales = fixtures_object
        .get_mut("locales")
        .and_then(Value::as_object_mut);
    for (tag, resolved) in locales.ok_or("no locales")?.iter_mut() {
        *resolved = locale_name(TimeLocale::for_language_tag(tag)).into();
    }
    Ok(fixtures.to_string())
}

/// The viewer's zone and locale, and `at` formatted in them with the clock
/// fixed at `now`.
pub fn viewer_report(at: i64, now: i64) -> Result<String, String> {
    let (at, now) = (timestamp(&at.into())?, timestamp(&now.into())?);
    let local = LocalTime::viewer().with_clock(Clock::Fixed(now));
    Ok(json!({
        "zone": local.zone().name(),
        "locale": locale_name(local.locale()),
        "formats": formats(&local, at),
    })
    .to_string())
}

fn formats(local: &LocalTime, at: Timestamp) -> Value {
    json!({
        "time_of_day": local.time_of_day(at),
        "short_date": local.short_date(at),
        "long_date": local.long_date(at),
        "relative_day": local.relative_day(at),
        "relative_day_and_time": local.relative_day_and_time(at),
        "compact": local.compact(at),
        "utc_offset": local.utc_offset(at),
    })
}

fn timestamp(value: &Value) -> Result<Timestamp, String> {
    value
        .as_i64()
        .and_then(Timestamp::from_unix_millis)
        .ok_or_else(|| format!("not a supported instant: {value}"))
}

fn locale_name(locale: TimeLocale) -> String {
    let hours = match locale.hour_cycle() {
        HourCycle::Twelve => "12",
        HourCycle::TwentyFour => "24",
    };
    let order = match locale.date_order() {
        DateOrder::DayMonth => "day-month",
        DateOrder::MonthDay => "month-day",
    };
    format!("{hours} {order}")
}

fn parse_locale(name: &str) -> Result<TimeLocale, String> {
    let hour_cycle = match name.split(' ').next() {
        Some("12") => HourCycle::Twelve,
        Some("24") => HourCycle::TwentyFour,
        _ => return Err(format!("unknown locale {name:?}")),
    };
    let date_order = match name.split(' ').nth(1) {
        Some("day-month") => DateOrder::DayMonth,
        Some("month-day") => DateOrder::MonthDay,
        _ => return Err(format!("unknown locale {name:?}")),
    };
    Ok(TimeLocale::new(hour_cycle, date_order))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &str = include_str!("fixtures.json");

    #[test]
    fn native_formats_match_the_shared_fixtures() {
        let expected: Value = serde_json::from_str(FIXTURES).unwrap();
        let actual: Value = serde_json::from_str(&report(FIXTURES).unwrap()).unwrap();
        for (case, (expected, actual)) in expected["cases"]
            .as_array()
            .unwrap()
            .iter()
            .zip(actual["cases"].as_array().unwrap())
            .enumerate()
        {
            assert_eq!(actual, expected, "case {case} ({})", expected["zone"]);
        }
        assert_eq!(actual["locales"], expected["locales"]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn fixtures_list_every_tabled_locale_and_their_instants_are_labelled() {
        let fixtures: Value = serde_json::from_str(FIXTURES).unwrap();
        let listed: Vec<_> = fixtures["locales"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        let mut tabled: Vec<_> = super::super::LOCALE_TABLE
            .iter()
            .map(|(tag, _, _)| tag.to_string())
            .collect();
        tabled.sort();
        assert_eq!(listed, tabled);
        let label = |millis: i64| {
            let f = super::super::utc_fields(millis);
            let fraction = match millis.rem_euclid(1_000) {
                0 => String::new(),
                ms => format!(".{ms:03}"),
            };
            format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{fraction}Z",
                f.year, f.month, f.day, f.hour, f.minute, f.second
            )
        };
        for case in fixtures["cases"].as_array().unwrap() {
            assert_eq!(case["now_utc"], label(case["now"].as_i64().unwrap()));
            for instant in case["instants"].as_array().unwrap() {
                assert_eq!(instant["utc"], label(instant["at"].as_i64().unwrap()));
            }
        }
    }

    #[test]
    fn report_rejects_malformed_fixtures() {
        for malformed in [
            "[]",
            "{}",
            r#"{"cases": [1]}"#,
            r#"{"cases": [{"zone": "Mars/Base"}]}"#,
            r#"{"cases": [{"zone": "UTC", "locale": "12 day-month", "now": 0, "instants": [1]}]}"#,
            r#"{"cases": [], "locales": []}"#,
        ] {
            assert!(report(malformed).is_err(), "{malformed}");
        }
        let out_of_range = r#"{"cases": [{"zone": "UTC", "locale": "12 day-month", "now": 1e300, "instants": []}], "locales": {}}"#;
        assert!(report(out_of_range).is_err());
        assert!(parse_locale("13 day-month").is_err());
        assert!(viewer_report(0, i64::MAX).is_err());
        let viewer: Value = serde_json::from_str(&viewer_report(0, 0).unwrap()).unwrap();
        assert!(viewer["zone"].as_str().is_some_and(|zone| !zone.is_empty()));
    }
}
