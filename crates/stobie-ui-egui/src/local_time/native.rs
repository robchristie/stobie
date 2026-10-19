//! Native zone data: the system's time zone database through `jiff`
//! (`/usr/share/zoneinfo` on Unix, and `jiff`'s bundled copy on Windows).

use super::{Fields, TimeLocale, tabled_locale};

pub(super) type Zone = jiff::tz::TimeZone;

pub(super) fn now_millis() -> i64 {
    jiff::Timestamp::now().as_millisecond()
}

pub(super) fn utc() -> Zone {
    jiff::tz::TimeZone::UTC
}

pub(super) fn named(name: &str) -> Option<Zone> {
    jiff::tz::TimeZone::get(name).ok()
}

pub(super) fn viewer_zone() -> (String, Zone) {
    // `system` reads `TZ`, then the system's configured zone, and gives
    // `Etc/Unknown` (behaving as UTC) when neither is usable.
    let zone = jiff::tz::TimeZone::system();
    let name = zone.iana_name().unwrap_or("Etc/Unknown").to_owned();
    (name, zone)
}

pub(super) fn fields(zone: &Zone, millis: i64) -> Option<Fields> {
    let civil = zone.to_datetime(jiff::Timestamp::from_millisecond(millis).ok()?);
    Fields {
        year: i32::from(civil.year()),
        month: u8::try_from(civil.month()).ok()?,
        day: u8::try_from(civil.day()).ok()?,
        hour: u8::try_from(civil.hour()).ok()?,
        minute: u8::try_from(civil.minute()).ok()?,
        second: u8::try_from(civil.second()).ok()?,
    }
    .checked()
}

pub(super) fn viewer_locale() -> TimeLocale {
    let tag = ["LC_ALL", "LC_TIME", "LANG"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty())
        .unwrap_or_default();
    tabled_locale(&tag)
}

pub(super) fn locale_for_tag(tag: &str) -> TimeLocale {
    tabled_locale(tag)
}
