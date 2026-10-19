//! Browser zone data and locale preferences from `Intl.DateTimeFormat`.

use js_sys::{Array, Date, Function, Intl, Object, Reflect};
use wasm_bindgen::{JsCast, JsValue};

use super::{DateOrder, Fields, HourCycle, TimeLocale};

/// A formatter for the zone that reports every civil field as a number.
pub(super) type Zone = Intl::DateTimeFormat;

/// Gregorian dates and Latin digits, whatever the page's locale.
const FIELD_LOCALE: &str = "en-US-u-ca-gregory-nu-latn";

pub(super) fn now_millis() -> i64 {
    Date::now() as i64
}

pub(super) fn utc() -> Zone {
    named("UTC").expect("every browser knows UTC")
}

pub(super) fn named(name: &str) -> Option<Zone> {
    let options = Object::new();
    for (key, value) in [
        ("timeZone", name),
        ("year", "numeric"),
        ("month", "numeric"),
        ("day", "numeric"),
        ("hour", "numeric"),
        ("minute", "numeric"),
        ("second", "numeric"),
        ("hourCycle", "h23"),
    ] {
        Reflect::set(&options, &key.into(), &value.into()).ok()?;
    }
    date_time_format(&Array::of1(&FIELD_LOCALE.into()), &options)
}

pub(super) fn viewer_zone() -> (String, Zone) {
    let name = date_time_format(&Array::new(), &Object::new())
        .and_then(|format| string_option(&format.resolved_options(), "timeZone"));
    match name.and_then(|name| named(&name).map(|zone| (name, zone))) {
        Some(viewer) => viewer,
        None => ("Etc/Unknown".into(), utc()),
    }
}

pub(super) fn fields(zone: &Zone, millis: i64) -> Option<Fields> {
    let parts = zone.format_to_parts(&Date::new(&JsValue::from_f64(millis as f64)));
    let mut values = [None::<u32>; 6];
    for part in parts.iter() {
        let index = match string_option(&part, "type")?.as_str() {
            "year" => 0,
            "month" => 1,
            "day" => 2,
            "hour" => 3,
            "minute" => 4,
            "second" => 5,
            _ => continue,
        };
        values[index] = string_option(&part, "value")?.parse().ok();
    }
    let [year, month, day, hour, minute, second] = values;
    Fields {
        year: i32::try_from(year?).ok()?,
        month: u8::try_from(month?).ok()?,
        day: u8::try_from(day?).ok()?,
        // Some engines have written midnight as hour 24 under `hour12: false`.
        hour: u8::try_from(hour? % 24).ok()?,
        minute: u8::try_from(minute?).ok()?,
        second: u8::try_from(second?).ok()?,
    }
    .checked()
}

pub(super) fn viewer_locale() -> TimeLocale {
    intl_locale(&Array::new()).unwrap_or(TimeLocale::ENGLISH_US)
}

pub(super) fn locale_for_tag(tag: &str) -> TimeLocale {
    intl_locale(&Array::of1(&tag.into())).unwrap_or(TimeLocale::ENGLISH_US)
}

/// The locale's clock and the order of its numeric month and day.
fn intl_locale(locales: &Array) -> Option<TimeLocale> {
    let options = |pairs: &[(&str, &str)]| {
        let options = Object::new();
        for (key, value) in pairs {
            Reflect::set(&options, &(*key).into(), &(*value).into()).ok()?;
        }
        Some(options)
    };
    let clock = date_time_format(locales, &options(&[("hour", "numeric")])?)?;
    let hour_cycle = match string_option(&clock.resolved_options(), "hourCycle").as_deref() {
        Some("h11" | "h12") => HourCycle::Twelve,
        _ => HourCycle::TwentyFour,
    };
    let date = date_time_format(
        locales,
        &options(&[("month", "numeric"), ("day", "numeric")])?,
    )?;
    let mut order = None;
    for part in date
        .format_to_parts(&Date::new(&JsValue::from_f64(0.0)))
        .iter()
    {
        match string_option(&part, "type").as_deref() {
            Some("month") => order = order.or(Some(DateOrder::MonthDay)),
            Some("day") => order = order.or(Some(DateOrder::DayMonth)),
            _ => {}
        }
    }
    Some(TimeLocale::new(hour_cycle, order?))
}

/// `new Intl.DateTimeFormat(locales, options)`, or `None` where it throws,
/// as it does for an unknown zone or a malformed language tag.
fn date_time_format(locales: &Array, options: &Object) -> Option<Intl::DateTimeFormat> {
    let intl = Reflect::get(&js_sys::global(), &"Intl".into()).ok()?;
    let constructor: Function = Reflect::get(&intl, &"DateTimeFormat".into())
        .ok()?
        .dyn_into()
        .ok()?;
    let arguments = Array::of2(locales, options);
    Reflect::construct(&constructor, &arguments)
        .ok()
        .map(JsCast::unchecked_into)
}

fn string_option(object: &JsValue, key: &str) -> Option<String> {
    Reflect::get(object, &key.into()).ok()?.as_string()
}
