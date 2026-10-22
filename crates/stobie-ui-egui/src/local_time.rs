//! Dates and times for people: a time of day, a relative day, short and
//! long dates and a duration, in the viewer's time zone and locale or in a
//! named IANA time zone.
//!
//! [`LocalTime`] combines a [`TimeZone`], a [`TimeLocale`] and a [`Clock`].
//! [`LocalTime::viewer`] reads all three from the platform; tests and
//! applications that show another zone, such as a schedule kept in a
//! person's home zone, choose them instead:
//!
//! ```
//! use stobie_ui_egui::local_time::{
//!     Clock, DateOrder, HourCycle, LocalTime, TimeLocale, TimeZone, Timestamp,
//! };
//!
//! let adelaide = TimeZone::named("Australia/Adelaide").unwrap();
//! let english_au = TimeLocale::new(HourCycle::Twelve, DateOrder::DayMonth);
//! // 2026-04-14T00:11:00Z, a Tuesday morning in Adelaide.
//! let at = Timestamp::from_unix_millis(1_776_125_460_000).unwrap();
//! let local = LocalTime::new(adelaide, english_au, Clock::Fixed(at));
//! assert_eq!(local.time_of_day(at), "9:41 AM");
//! assert_eq!(local.long_date(at), "Tuesday 14 April 2026");
//! assert_eq!(local.utc_offset(at), "UTC+9:30");
//! ```
//!
//! Zone arithmetic comes from the platform: the browser's `Intl` in a web
//! build and the system's time zone database (through `jiff`) natively.
//! Stobie assembles the text itself from the civil date and time, so a web
//! and a native build give the same text for the same zone, locale and
//! instant, and browser engines can differ only in their zone data and in
//! how they resolve a locale. The locale chooses the clock (12 or 24
//! hours) and the order of day and month; the words, such as "Yesterday"
//! and month names, are English. Natively, a language tag resolves through
//! a table that matches Chromium for the tags it lists; the
//! [guide](https://github.com/robchristie/stobie/blob/main/docs/ui-guides/components.md#local-date-and-time)
//! describes the differences.

use std::fmt;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
#[path = "local_time/native.rs"]
mod platform;
#[cfg(target_arch = "wasm32")]
#[path = "local_time/web.rs"]
mod platform;

// The fixtures shared by the native unit tests and the browser test.
#[path = "local_time/evidence.rs"]
pub(crate) mod evidence;

const MILLIS_PER_SECOND: i64 = 1_000;
const SECONDS_PER_DAY: i64 = 86_400;

/// An instant, as whole milliseconds since 1970-01-01T00:00:00Z.
///
/// The range keeps every civil date within the years 1 to 9999 in any zone.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Timestamp(i64);

impl Timestamp {
    /// The earliest instant, 0001-01-03T00:00:00Z.
    pub const MIN: Self = Self(-62_135_424_000_000);
    /// The latest instant, 9999-12-29T23:59:59.999Z.
    pub const MAX: Self = Self(253_402_127_999_999);

    /// The instant `millis` milliseconds after the Unix epoch, or `None`
    /// outside [`Timestamp::MIN`] to [`Timestamp::MAX`].
    pub const fn from_unix_millis(millis: i64) -> Option<Self> {
        if millis < Self::MIN.0 || millis > Self::MAX.0 {
            None
        } else {
            Some(Self(millis))
        }
    }

    /// The instant `seconds` seconds after the Unix epoch, or `None` outside
    /// the supported range.
    pub const fn from_unix_seconds(seconds: i64) -> Option<Self> {
        match seconds.checked_mul(MILLIS_PER_SECOND) {
            Some(millis) => Self::from_unix_millis(millis),
            None => None,
        }
    }

    /// Milliseconds since the Unix epoch.
    pub const fn unix_millis(self) -> i64 {
        self.0
    }

    /// The platform's current time: `Date.now()` in the browser and the
    /// system clock natively.
    pub fn now() -> Self {
        Self(platform::now_millis().clamp(Self::MIN.0, Self::MAX.0))
    }

    /// How long after `earlier` this instant is, or zero when it is not
    /// later, as a clock slightly behind a server's can give.
    pub fn saturating_duration_since(self, earlier: Self) -> Duration {
        Duration::from_millis(self.0.saturating_sub(earlier.0).max(0) as u64)
    }
}

/// Where [`LocalTime`] reads the current time, for "Today" and for whether
/// a date needs its year.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Clock {
    /// [`Timestamp::now`] at each call.
    System,
    /// Always this instant, for deterministic tests and evidence.
    Fixed(Timestamp),
}

impl Clock {
    pub fn now(self) -> Timestamp {
        match self {
            Self::System => Timestamp::now(),
            Self::Fixed(at) => at,
        }
    }
}

/// A time zone: the viewer's, UTC, or a named IANA zone such as
/// `Australia/Adelaide`.
#[derive(Clone)]
pub struct TimeZone {
    name: String,
    zone: platform::Zone,
}

impl TimeZone {
    /// The viewer's zone, read from the platform when called: the page's
    /// `Intl` zone in the browser and the system zone natively. A platform
    /// zone without an IANA name, such as a POSIX `TZ` rule, keeps its rules
    /// and is named `Etc/Unknown`; a platform that reports no usable zone
    /// gives UTC with that name.
    pub fn viewer() -> Self {
        let (name, zone) = platform::viewer_zone();
        Self { name, zone }
    }

    /// Coordinated Universal Time, named `UTC`.
    pub fn utc() -> Self {
        Self {
            name: "UTC".into(),
            zone: platform::utc(),
        }
    }

    /// The IANA zone `name`, such as `Australia/Adelaide`. Names are matched
    /// without regard to case, and [`TimeZone::name`] returns `name` as
    /// given. Offsets such as `+09:30` are not zone names.
    pub fn named(name: &str) -> Result<Self, UnknownTimeZone> {
        let unknown = || UnknownTimeZone {
            name: name.to_owned(),
        };
        let plausible = name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'_' | b'-' | b'+'))
            && name
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphabetic());
        if !plausible {
            return Err(unknown());
        }
        let zone = platform::named(name).ok_or_else(unknown)?;
        Ok(Self {
            name: name.to_owned(),
            zone,
        })
    }

    /// The zone's IANA name, such as `Australia/Adelaide`, for showing which
    /// zone is in use. For the viewer's zone it is the platform's name, which
    /// can be an older alias in the browser (`Asia/Calcutta` for
    /// `Asia/Kolkata` in Chromium).
    pub fn name(&self) -> &str {
        &self.name
    }

    fn civil(&self, at: Timestamp) -> CivilDateTime {
        let fields = platform::fields(&self.zone, at.0).unwrap_or_else(|| utc_fields(at.0));
        CivilDateTime::from_fields(fields, at.0)
    }
}

impl fmt::Debug for TimeZone {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("TimeZone").field(&self.name).finish()
    }
}

/// Zones are equal when their names are, as given: `Australia/Adelaide` and
/// `australia/adelaide` are the same zone but unequal values.
impl PartialEq for TimeZone {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for TimeZone {}

/// A name that is not a known IANA time zone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownTimeZone {
    pub name: String,
}

impl fmt::Display for UnknownTimeZone {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown time zone {:?}", self.name)
    }
}

impl std::error::Error for UnknownTimeZone {}

/// Whether a locale's clock runs to 12 hours with AM and PM, or to 24.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HourCycle {
    /// "9:41 AM", "12:05 PM", "12:00 AM" for midnight.
    Twelve,
    /// "09:41", "12:05", "00:00" for midnight.
    TwentyFour,
}

/// Whether a locale writes the day before the month ("14 April") or after
/// it ("April 14").
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DateOrder {
    DayMonth,
    MonthDay,
}

/// The parts of a locale that dates and times follow.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TimeLocale {
    hour_cycle: HourCycle,
    date_order: DateOrder,
}

impl TimeLocale {
    /// United States English: a 12-hour clock, month before day. Also the
    /// result for a malformed language tag.
    pub const ENGLISH_US: Self = Self::new(HourCycle::Twelve, DateOrder::MonthDay);

    pub const fn new(hour_cycle: HourCycle, date_order: DateOrder) -> Self {
        Self {
            hour_cycle,
            date_order,
        }
    }

    /// The viewer's locale, read from the platform when called: the page's
    /// default `Intl` locale in the browser, and natively the first of
    /// `LC_ALL`, `LC_TIME` and `LANG` that is set (`C` and `POSIX` read as
    /// United States English).
    pub fn viewer() -> Self {
        platform::viewer_locale()
    }

    /// The locale for a BCP 47 language tag such as `en-AU`. The browser
    /// resolves it with `Intl`. Natively, a table that matches Chromium
    /// resolves the tags it lists, then the tag's language, and gives a
    /// 24-hour clock with day before month for any other language.
    pub fn for_language_tag(tag: &str) -> Self {
        platform::locale_for_tag(tag)
    }

    pub const fn hour_cycle(self) -> HourCycle {
        self.hour_cycle
    }

    pub const fn date_order(self) -> DateOrder {
        self.date_order
    }

    /// The same locale with another clock, such as a person's own choice of
    /// a 24-hour clock.
    pub const fn with_hour_cycle(self, hour_cycle: HourCycle) -> Self {
        Self { hour_cycle, ..self }
    }
}

/// A day of the week.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    const ALL: [Self; 7] = [
        Self::Monday,
        Self::Tuesday,
        Self::Wednesday,
        Self::Thursday,
        Self::Friday,
        Self::Saturday,
        Self::Sunday,
    ];

    /// The English name, such as "Tuesday".
    pub const fn name(self) -> &'static str {
        match self {
            Self::Monday => "Monday",
            Self::Tuesday => "Tuesday",
            Self::Wednesday => "Wednesday",
            Self::Thursday => "Thursday",
            Self::Friday => "Friday",
            Self::Saturday => "Saturday",
            Self::Sunday => "Sunday",
        }
    }

    fn from_days(days: i64) -> Self {
        // 1970-01-01 was a Thursday.
        Self::ALL[(days + 3).rem_euclid(7) as usize]
    }
}

/// A wall-clock time of day without a date or zone, such as a schedule's
/// "7:00".
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ClockTime {
    hour: u8,
    minute: u8,
}

impl ClockTime {
    /// `hour` from 0 to 23 and `minute` from 0 to 59, or `None`.
    pub const fn new(hour: u8, minute: u8) -> Option<Self> {
        if hour < 24 && minute < 60 {
            Some(Self { hour, minute })
        } else {
            None
        }
    }

    pub const fn hour(self) -> u8 {
        self.hour
    }

    pub const fn minute(self) -> u8 {
        self.minute
    }
}

/// An instant's date and time on the wall clock of a zone.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CivilDateTime {
    /// 1 to 9999.
    pub year: i32,
    /// 1 to 12.
    pub month: u8,
    /// 1 to 31.
    pub day: u8,
    pub weekday: Weekday,
    /// 0 to 23.
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    /// The zone's offset from UTC at that instant, in seconds east.
    pub utc_offset_seconds: i32,
}

impl CivilDateTime {
    fn from_fields(fields: Fields, millis: i64) -> Self {
        let days = days_from_civil(fields.year, fields.month, fields.day);
        let local_seconds = days * SECONDS_PER_DAY
            + i64::from(fields.hour) * 3_600
            + i64::from(fields.minute) * 60
            + i64::from(fields.second);
        let utc_seconds = millis.div_euclid(MILLIS_PER_SECOND);
        Self {
            year: fields.year,
            month: fields.month,
            day: fields.day,
            weekday: Weekday::from_days(days),
            hour: fields.hour,
            minute: fields.minute,
            second: fields.second,
            utc_offset_seconds: (local_seconds - utc_seconds) as i32,
        }
    }

    /// Days since 1970-01-01 of this civil date, to compare days.
    fn day_number(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }
}

/// Formats instants for people in one zone and locale, reading the current
/// time from its clock.
///
/// Keep one and replace it when the viewer's zone or locale may have
/// changed, such as when the application returns to the foreground:
/// [`LocalTime::viewer`] reads the platform each time it is called, and in
/// the browser each format makes one `Intl` call, or two where the text
/// depends on today.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalTime {
    zone: TimeZone,
    locale: TimeLocale,
    clock: Clock,
}

impl LocalTime {
    /// The viewer's zone and locale with the system clock.
    pub fn viewer() -> Self {
        Self::new(TimeZone::viewer(), TimeLocale::viewer(), Clock::System)
    }

    pub fn new(zone: TimeZone, locale: TimeLocale, clock: Clock) -> Self {
        Self {
            zone,
            locale,
            clock,
        }
    }

    /// The same locale and clock in another zone, such as the zone a
    /// schedule runs in.
    pub fn with_zone(self, zone: TimeZone) -> Self {
        Self { zone, ..self }
    }

    pub fn with_locale(self, locale: TimeLocale) -> Self {
        Self { locale, ..self }
    }

    pub fn with_clock(self, clock: Clock) -> Self {
        Self { clock, ..self }
    }

    pub fn zone(&self) -> &TimeZone {
        &self.zone
    }

    pub fn locale(&self) -> TimeLocale {
        self.locale
    }

    pub fn clock(&self) -> Clock {
        self.clock
    }

    pub fn now(&self) -> Timestamp {
        self.clock.now()
    }

    /// `at` on this zone's wall clock.
    pub fn civil(&self, at: Timestamp) -> CivilDateTime {
        self.zone.civil(at)
    }

    /// The time of day: "9:41 AM", or "09:41" on a 24-hour clock.
    pub fn time_of_day(&self, at: Timestamp) -> String {
        self.time_of(&self.civil(at))
    }

    /// A wall-clock time in this locale, such as "7:00 AM" for a schedule.
    pub fn clock_time(&self, time: ClockTime) -> String {
        let ClockTime { hour, minute } = time;
        match self.locale.hour_cycle {
            HourCycle::Twelve => {
                let period = if hour < 12 { "AM" } else { "PM" };
                let hour = match hour % 12 {
                    0 => 12,
                    hour => hour,
                };
                format!("{hour}:{minute:02} {period}")
            }
            HourCycle::TwentyFour => format!("{hour:02}:{minute:02}"),
        }
    }

    /// A short date: "14 Apr", or "Apr 14" with month before day, with the
    /// year added ("14 Apr 2025", "Apr 14, 2025") when it is not the current
    /// year in this zone.
    pub fn short_date(&self, at: Timestamp) -> String {
        self.short_date_of(&self.civil(at), &self.civil(self.now()))
    }

    /// A long date: "Tuesday 14 April 2026", or "Tuesday, April 14, 2026"
    /// with month before day.
    pub fn long_date(&self, at: Timestamp) -> String {
        let civil = self.civil(at);
        let (weekday, month) = (civil.weekday.name(), MONTHS[usize::from(civil.month - 1)]);
        match self.locale.date_order {
            DateOrder::DayMonth => format!("{weekday} {} {month} {}", civil.day, civil.year),
            DateOrder::MonthDay => format!("{weekday}, {month} {}, {}", civil.day, civil.year),
        }
    }

    /// The day relative to today in this zone: "Today", "Yesterday",
    /// "Tomorrow", the weekday's name for the six days before today, and
    /// otherwise the [short date](LocalTime::short_date). Days are calendar
    /// days on the zone's wall clock, so a day with a daylight-saving change
    /// still counts as one.
    pub fn relative_day(&self, at: Timestamp) -> String {
        self.relative_day_of(&self.civil(at), &self.civil(self.now()))
    }

    /// The relative day and the time of day: "Yesterday at 7:00 AM",
    /// "14 Apr at 7:00 AM".
    pub fn relative_day_and_time(&self, at: Timestamp) -> String {
        let civil = self.civil(at);
        let day = self.relative_day_of(&civil, &self.civil(self.now()));
        format!("{day} at {}", self.time_of(&civil))
    }

    /// The [relative day](LocalTime::relative_day) as it reads inside a
    /// sentence: "today", "yesterday", "tomorrow", "on Tuesday" and "on
    /// 14 Apr", for text such as "Changed on Tuesday".
    pub fn relative_day_in_sentence(&self, at: Timestamp) -> String {
        self.relative_day_in_sentence_of(&self.civil(at), &self.civil(self.now()))
    }

    /// The relative day and the time of day as they read inside a sentence:
    /// "today at 3:30 PM", "on Saturday at 9:41 AM", "on 14 Apr at 7:00 AM",
    /// for text such as "Signed in today at 3:30 PM".
    pub fn relative_day_and_time_in_sentence(&self, at: Timestamp) -> String {
        let civil = self.civil(at);
        let day = self.relative_day_in_sentence_of(&civil, &self.civil(self.now()));
        format!("{day} at {}", self.time_of(&civil))
    }

    /// How long until the day in this zone changes, from the clock's current
    /// time: when "Today" becomes "Yesterday" and a time of day in
    /// [`compact`](LocalTime::compact) becomes a day. Schedule one pass then,
    /// such as with `Frame::request_repaint_after`, rather than repainting
    /// while waiting.
    ///
    /// The day changes at the next midnight on the zone's wall clock, or at
    /// the first instant of the next day where a daylight-saving change skips
    /// midnight, so a day with a change is 23 or 25 hours long. The duration
    /// is exact when the zone's offset changes at most once before the next
    /// day; where the zone's answers do not agree on when the day changes, it
    /// is at most an hour, so asking again then cannot be late by more.
    pub fn until_next_day(&self) -> Duration {
        let now = self.now();
        let today = self.civil(now);
        let day = today.day_number();
        let millis_into_day = (i64::from(today.hour) * 3_600
            + i64::from(today.minute) * 60
            + i64::from(today.second))
            * MILLIS_PER_SECOND
            + now.0.rem_euclid(MILLIS_PER_SECOND);
        // Midnight if the offset holds until then.
        let guess = now.0 + SECONDS_PER_DAY * MILLIS_PER_SECOND - millis_into_day;
        let mut next = guess;
        for _ in 0..3 {
            if next <= now.0 || next > Timestamp::MAX.0 {
                break;
            }
            let (at_next, before) = (
                self.zone.civil(Timestamp(next)),
                self.zone.civil(Timestamp(next - 1)),
            );
            if at_next.day_number() != day && before.day_number() == day {
                return Timestamp(next).saturating_duration_since(now);
            }
            // The offset changed before `next`: midnight on the offset that
            // `next` has, which also finds the first instant of a day whose
            // midnight a change skipped.
            next = ((day + 1) * SECONDS_PER_DAY - i64::from(at_next.utc_offset_seconds))
                * MILLIS_PER_SECOND;
        }
        Timestamp(guess)
            .saturating_duration_since(now)
            .min(UNVERIFIED_DAY_CHANGE)
            .max(Duration::from_millis(1))
    }

    /// The time of day for an instant today, and otherwise the
    /// [relative day](LocalTime::relative_day): what a list of
    /// conversations shows beside each one.
    pub fn compact(&self, at: Timestamp) -> String {
        let (civil, today) = (self.civil(at), self.civil(self.now()));
        if civil.day_number() == today.day_number() {
            self.time_of(&civil)
        } else {
            self.relative_day_of(&civil, &today)
        }
    }

    fn time_of(&self, civil: &CivilDateTime) -> String {
        self.clock_time(ClockTime {
            hour: civil.hour,
            minute: civil.minute,
        })
    }

    fn short_date_of(&self, civil: &CivilDateTime, today: &CivilDateTime) -> String {
        let current_year = today.year == civil.year;
        let month = &MONTHS[usize::from(civil.month - 1)][..3];
        match (self.locale.date_order, current_year) {
            (DateOrder::DayMonth, true) => format!("{} {month}", civil.day),
            (DateOrder::DayMonth, false) => format!("{} {month} {}", civil.day, civil.year),
            (DateOrder::MonthDay, true) => format!("{month} {}", civil.day),
            (DateOrder::MonthDay, false) => format!("{month} {}, {}", civil.day, civil.year),
        }
    }

    fn relative_day_of(&self, civil: &CivilDateTime, today: &CivilDateTime) -> String {
        match RelativeDay::of(civil, today) {
            RelativeDay::Today => "Today".into(),
            RelativeDay::Yesterday => "Yesterday".into(),
            RelativeDay::Tomorrow => "Tomorrow".into(),
            RelativeDay::Weekday(weekday) => weekday.name().into(),
            RelativeDay::Date => self.short_date_of(civil, today),
        }
    }

    fn relative_day_in_sentence_of(&self, civil: &CivilDateTime, today: &CivilDateTime) -> String {
        match RelativeDay::of(civil, today) {
            RelativeDay::Today => "today".into(),
            RelativeDay::Yesterday => "yesterday".into(),
            RelativeDay::Tomorrow => "tomorrow".into(),
            RelativeDay::Weekday(weekday) => format!("on {}", weekday.name()),
            RelativeDay::Date => format!("on {}", self.short_date_of(civil, today)),
        }
    }

    /// The zone's offset from UTC at `at`: "UTC", "UTC+10:30", "UTC-3:30".
    pub fn utc_offset(&self, at: Timestamp) -> String {
        let offset = self.civil(at).utc_offset_seconds;
        if offset == 0 {
            return "UTC".into();
        }
        let sign = if offset < 0 { '-' } else { '+' };
        let offset = offset.unsigned_abs();
        let (hours, minutes, seconds) = (offset / 3_600, offset / 60 % 60, offset % 60);
        if seconds != 0 {
            format!("UTC{sign}{hours}:{minutes:02}:{seconds:02}")
        } else {
            format!("UTC{sign}{hours}:{minutes:02}")
        }
    }
}

/// Which relative day a civil date is, shared by the sentence-initial and
/// in-sentence forms.
enum RelativeDay {
    Today,
    Yesterday,
    Tomorrow,
    /// Two to six days before today, by the weekday's name.
    Weekday(Weekday),
    /// Any other day, by its short date.
    Date,
}

impl RelativeDay {
    fn of(civil: &CivilDateTime, today: &CivilDateTime) -> Self {
        match civil.day_number() - today.day_number() {
            0 => Self::Today,
            -1 => Self::Yesterday,
            1 => Self::Tomorrow,
            -6..=-2 => Self::Weekday(civil.weekday),
            _ => Self::Date,
        }
    }
}

/// The longest [`LocalTime::until_next_day`] gives when the zone's answers do
/// not agree on when the day changes.
const UNVERIFIED_DAY_CHANGE: Duration = Duration::from_secs(3_600);

/// A duration for people, in its largest two units, rounded down: "45 s",
/// "12 min", "2 h", "2 h 10 min", "1 day 4 h", "3 days".
pub fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    let (days, hours, minutes) = (seconds / 86_400, seconds / 3_600 % 24, seconds / 60 % 60);
    let day_word = if days == 1 { "day" } else { "days" };
    match (days, hours, minutes) {
        (0, 0, 0) => format!("{seconds} s"),
        (0, 0, minutes) => format!("{minutes} min"),
        (0, hours, 0) => format!("{hours} h"),
        (0, hours, minutes) => format!("{hours} h {minutes} min"),
        (days, 0, _) => format!("{days} {day_word}"),
        (days, hours, _) => format!("{days} {day_word} {hours} h"),
    }
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// A civil date and time as a platform reports it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Fields {
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
}

impl Fields {
    /// Check the ranges a platform's answer must fall in.
    fn checked(self) -> Option<Self> {
        let valid = (1..=9999).contains(&self.year)
            && (1..=12).contains(&self.month)
            && (1..=31).contains(&self.day)
            && self.hour < 24
            && self.minute < 60
            && self.second < 60;
        valid.then_some(self)
    }
}

/// The civil date and time in UTC, without a platform.
fn utc_fields(millis: i64) -> Fields {
    let seconds = millis.div_euclid(MILLIS_PER_SECOND);
    let (days, second_of_day) = (
        seconds.div_euclid(SECONDS_PER_DAY),
        seconds.rem_euclid(SECONDS_PER_DAY),
    );
    let (year, month, day) = civil_from_days(days);
    Fields {
        year,
        month,
        day,
        hour: (second_of_day / 3_600) as u8,
        minute: (second_of_day / 60 % 60) as u8,
        second: (second_of_day % 60) as u8,
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let year = i64::from(year) - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let month = i64::from(month);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The proleptic Gregorian date `days` after 1970-01-01.
fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year as i32, month as u8, day as u8)
}

/// How a language tag's locale resolves where the platform has no `Intl`.
///
/// The entries match Chromium 151's `Intl.DateTimeFormat`: `resolvedOptions().hourCycle`
/// for `{hour: "numeric"}`, and whether `month` precedes `day` in the parts
/// of `{month: "numeric", day: "numeric"}`. The browser test checks every
/// entry against the page's `Intl`.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
const LOCALE_TABLE: &[(&str, HourCycle, DateOrder)] = {
    use DateOrder::{DayMonth as Dm, MonthDay as Md};
    use HourCycle::{Twelve as H12, TwentyFour as H24};
    &[
        ("ar", H12, Dm),
        ("cs", H24, Dm),
        ("da", H24, Dm),
        ("de", H24, Dm),
        ("el", H12, Dm),
        ("en", H12, Md),
        ("en-AU", H12, Dm),
        ("en-CA", H12, Md),
        ("en-GB", H24, Dm),
        ("en-IE", H24, Dm),
        ("en-IN", H12, Dm),
        ("en-NZ", H12, Dm),
        ("en-PH", H12, Md),
        ("en-SG", H12, Dm),
        ("en-US", H12, Md),
        ("en-ZA", H24, Md),
        ("es", H24, Dm),
        ("es-MX", H12, Dm),
        ("es-US", H12, Dm),
        ("fi", H24, Dm),
        ("fr", H24, Dm),
        ("fr-CA", H24, Md),
        ("he", H24, Dm),
        ("hi", H12, Dm),
        ("hu", H24, Md),
        ("id", H24, Dm),
        ("it", H24, Dm),
        ("ja", H24, Md),
        ("ko", H12, Md),
        ("ms", H12, Dm),
        ("nb", H24, Dm),
        ("nl", H24, Dm),
        ("pl", H24, Dm),
        ("pt", H24, Dm),
        ("pt-BR", H24, Dm),
        ("ro", H24, Dm),
        ("ru", H24, Dm),
        ("sv", H24, Dm),
        ("th", H24, Dm),
        ("tr", H24, Dm),
        ("uk", H24, Dm),
        ("vi", H24, Dm),
        ("zh", H24, Md),
        ("zh-HK", H12, Dm),
        ("zh-TW", H12, Md),
    ]
};

/// Resolve a language tag, or a POSIX locale name such as `en_AU.UTF-8`,
/// through [`LOCALE_TABLE`].
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn tabled_locale(tag: &str) -> TimeLocale {
    let tag = tag.split(['.', '@']).next().unwrap_or_default();
    let mut subtags = tag.split(['-', '_']);
    let language = subtags.next().unwrap_or_default().to_ascii_lowercase();
    if matches!(language.as_str(), "c" | "posix") {
        return TimeLocale::ENGLISH_US;
    }
    if !(2..=3).contains(&language.len()) || !language.bytes().all(|b| b.is_ascii_alphabetic()) {
        return TimeLocale::ENGLISH_US;
    }
    // The region is the first two-letter or three-digit subtag, after any script.
    let region = subtags
        .find(|subtag| {
            (subtag.len() == 2 && subtag.bytes().all(|b| b.is_ascii_alphabetic()))
                || (subtag.len() == 3 && subtag.bytes().all(|b| b.is_ascii_digit()))
        })
        .map(str::to_ascii_uppercase);
    let find = |key: &str| {
        LOCALE_TABLE
            .iter()
            .find(|(tag, _, _)| *tag == key)
            .map(|&(_, hour_cycle, date_order)| TimeLocale::new(hour_cycle, date_order))
    };
    region
        .and_then(|region| find(&format!("{language}-{region}")))
        .or_else(|| find(&language))
        .unwrap_or(TimeLocale::new(HourCycle::TwentyFour, DateOrder::DayMonth))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3_600_000;
    const AU: TimeLocale = TimeLocale::new(HourCycle::Twelve, DateOrder::DayMonth);
    const GB: TimeLocale = TimeLocale::new(HourCycle::TwentyFour, DateOrder::DayMonth);

    /// An instant from its UTC civil fields.
    fn utc(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Timestamp {
        let days = days_from_civil(year, month, day);
        let seconds = days * SECONDS_PER_DAY + i64::from(hour) * 3_600 + i64::from(minute) * 60;
        Timestamp::from_unix_seconds(seconds).unwrap()
    }

    fn adelaide(now: Timestamp) -> LocalTime {
        LocalTime::new(
            TimeZone::named("Australia/Adelaide").unwrap(),
            AU,
            Clock::Fixed(now),
        )
    }

    #[test]
    fn range_is_the_years_one_to_9999_with_a_day_to_spare() {
        assert_eq!(utc_fields(Timestamp::MIN.0).year, 1);
        assert_eq!(
            (
                utc_fields(Timestamp::MIN.0).month,
                utc_fields(Timestamp::MIN.0).day
            ),
            (1, 3)
        );
        let max = utc_fields(Timestamp::MAX.0);
        assert_eq!(
            (
                max.year, max.month, max.day, max.hour, max.minute, max.second
            ),
            (9999, 12, 29, 23, 59, 59)
        );
        assert!(Timestamp::from_unix_millis(Timestamp::MIN.0 - 1).is_none());
        assert!(Timestamp::from_unix_millis(Timestamp::MAX.0 + 1).is_none());
        assert!(Timestamp::from_unix_seconds(i64::MAX).is_none());
        // The furthest zones keep the civil year in range at both ends.
        for zone in ["Pacific/Kiritimati", "Etc/GMT+12"] {
            let local = LocalTime::new(
                TimeZone::named(zone).unwrap(),
                AU,
                Clock::Fixed(Timestamp::MIN),
            );
            assert!((1..=9999).contains(&local.civil(Timestamp::MIN).year));
            assert!((1..=9999).contains(&local.civil(Timestamp::MAX).year));
        }
    }

    #[test]
    fn civil_arithmetic_round_trips_and_names_weekdays() {
        for days in [-719_162, -1, 0, 1, 20_557, 2_932_896] {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(days_from_civil(year, month, day), days);
        }
        assert_eq!(Weekday::from_days(0), Weekday::Thursday);
        assert_eq!(Weekday::from_days(-1), Weekday::Wednesday);
        assert_eq!(
            Weekday::from_days(days_from_civil(2026, 4, 14)),
            Weekday::Tuesday
        );
        // Before the epoch, a millisecond belongs to the second before it.
        assert_eq!(utc_fields(-1).second, 59);
        assert_eq!(utc_fields(-1).year, 1969);
    }

    #[test]
    fn adelaide_leaves_daylight_saving_on_the_first_sunday_of_april() {
        // 2026-04-05 03:00 ACDT (+10:30) becomes 02:00 ACST (+9:30): 02:30 happens twice.
        let first = utc(2026, 4, 4, 16, 0);
        let second = Timestamp::from_unix_millis(first.0 + HOUR).unwrap();
        let local = adelaide(first);
        assert_eq!(local.time_of_day(first), "2:30 AM");
        assert_eq!(local.time_of_day(second), "2:30 AM");
        assert_eq!(local.utc_offset(first), "UTC+10:30");
        assert_eq!(local.utc_offset(second), "UTC+9:30");
        assert_eq!(local.long_date(second), "Sunday 5 April 2026");
        // The 25-hour day is still one day: 00:15 on the 5th is yesterday at
        // 23:50 on the 6th, and today from 23:50 on the 5th.
        let start = utc(2026, 4, 4, 13, 45);
        assert_eq!(local.civil(start).hour, 0);
        assert_eq!(
            adelaide(utc(2026, 4, 6, 14, 20)).relative_day(start),
            "Yesterday"
        );
        assert_eq!(
            adelaide(utc(2026, 4, 5, 14, 20)).relative_day(start),
            "Today"
        );
        assert_eq!(adelaide(utc(2026, 4, 5, 14, 20)).compact(start), "12:15 AM");
    }

    #[test]
    fn adelaide_enters_daylight_saving_on_the_first_sunday_of_october() {
        // 2026-10-04 02:00 ACST (+9:30) becomes 03:00 ACDT (+10:30): 02:30 never happens.
        let before = utc(2026, 10, 3, 16, 29);
        let after = utc(2026, 10, 3, 16, 30);
        let local = adelaide(after).with_locale(GB);
        assert_eq!(local.time_of_day(before), "01:59");
        assert_eq!(local.time_of_day(after), "03:00");
        assert_eq!(local.utc_offset(before), "UTC+9:30");
        assert_eq!(local.utc_offset(after), "UTC+10:30");
        // The 23-hour day: 00:30 on the 4th and 23:30 on the 4th are both today.
        let early = utc(2026, 10, 3, 15, 0);
        let late = utc(2026, 10, 4, 13, 0);
        assert_eq!(adelaide(late).relative_day(early), "Today");
        assert_eq!(adelaide(late).time_of_day(late), "11:30 PM");
    }

    #[test]
    fn relative_days_count_calendar_days_on_the_zone_clock() {
        // Thursday 16 April 2026, 10:00 in Adelaide.
        let now = utc(2026, 4, 16, 0, 30);
        let local = adelaide(now);
        let days_ago = |days: i64| Timestamp::from_unix_millis(now.0 - days * 24 * HOUR).unwrap();
        assert_eq!(local.relative_day(now), "Today");
        assert_eq!(local.relative_day(days_ago(1)), "Yesterday");
        assert_eq!(local.relative_day(days_ago(-1)), "Tomorrow");
        assert_eq!(local.relative_day(days_ago(2)), "Tuesday");
        assert_eq!(local.relative_day(days_ago(6)), "Friday");
        assert_eq!(local.relative_day(days_ago(7)), "9 Apr");
        assert_eq!(local.relative_day(days_ago(-2)), "18 Apr");
        assert_eq!(local.relative_day(days_ago(365)), "16 Apr 2025");
        assert_eq!(
            local.relative_day_and_time(days_ago(1)),
            "Yesterday at 10:00 AM"
        );
        assert_eq!(local.compact(days_ago(0)), "10:00 AM");
        assert_eq!(local.compact(days_ago(3)), "Monday");
        // 23:59 UTC on the 15th is already the 16th in Adelaide.
        assert_eq!(local.relative_day(utc(2026, 4, 15, 23, 59)), "Today");
        // In UTC the same instant is yesterday.
        let in_utc = local.clone().with_zone(TimeZone::utc());
        assert_eq!(in_utc.relative_day(utc(2026, 4, 15, 23, 59)), "Yesterday");
        assert_eq!(in_utc.relative_day(utc(2026, 4, 16, 0, 0)), "Today");
    }

    #[test]
    fn in_sentence_forms_share_the_relative_day() {
        // Thursday 16 April 2026, 10:00 in Adelaide.
        let now = utc(2026, 4, 16, 0, 30);
        let local = adelaide(now);
        let days_ago = |days: i64| Timestamp::from_unix_millis(now.0 - days * 24 * HOUR).unwrap();
        for (days, initial, in_sentence) in [
            (0, "Today", "today"),
            (1, "Yesterday", "yesterday"),
            (-1, "Tomorrow", "tomorrow"),
            (5, "Saturday", "on Saturday"),
            (7, "9 Apr", "on 9 Apr"),
            (-2, "18 Apr", "on 18 Apr"),
            (365, "16 Apr 2025", "on 16 Apr 2025"),
        ] {
            assert_eq!(local.relative_day(days_ago(days)), initial);
            assert_eq!(local.relative_day_in_sentence(days_ago(days)), in_sentence);
            assert_eq!(
                local.relative_day_and_time_in_sentence(days_ago(days)),
                format!("{in_sentence} at 10:00 AM")
            );
        }
        let us = local.clone().with_locale(TimeLocale::ENGLISH_US);
        assert_eq!(
            us.relative_day_and_time_in_sentence(days_ago(365)),
            "on Apr 16, 2025 at 10:00 AM"
        );
        assert_eq!(
            local
                .with_locale(GB)
                .relative_day_and_time_in_sentence(days_ago(5)),
            "on Saturday at 10:00"
        );
    }

    /// The first instant after the clock's time whose day differs, found a
    /// minute and then a second at a time: an oracle for `until_next_day`.
    fn next_day_by_scan(local: &LocalTime) -> Duration {
        let now = local.now();
        let day = local.civil(now).day_number();
        let same_day = |at: i64| local.civil(Timestamp(at)).day_number() == day;
        // Days change on whole seconds: every offset is whole seconds.
        let mut at = now.0 - now.0.rem_euclid(1_000) + 1_000;
        while same_day(at + 60_000) {
            at += 60_000;
        }
        while same_day(at) {
            at += 1_000;
        }
        Timestamp(at).saturating_duration_since(now)
    }

    #[test]
    fn the_next_day_starts_at_the_next_midnight_on_the_zone_clock() {
        // 00:00 on Thursday 16 April 2026 in Adelaide (+9:30).
        let midnight = utc(2026, 4, 15, 14, 30);
        let at = |millis: i64| adelaide(Timestamp::from_unix_millis(midnight.0 + millis).unwrap());
        assert_eq!(at(0).until_next_day(), Duration::from_secs(86_400));
        assert_eq!(at(1).until_next_day(), Duration::from_millis(86_399_999));
        assert_eq!(at(-1).until_next_day(), Duration::from_millis(1));
        assert_eq!(at(-500).until_next_day(), Duration::from_millis(500));
        assert_eq!(at(-60_000).until_next_day(), Duration::from_secs(60));
        assert_eq!(
            at(-HOUR * 14).until_next_day(),
            Duration::from_secs(14 * 3_600)
        );
        // In UTC the same instant is 14:30, so its day changes at 00:00 UTC.
        let in_utc = at(0).with_zone(TimeZone::utc());
        assert_eq!(
            in_utc.until_next_day(),
            Duration::from_secs(9 * 3_600 + 30 * 60)
        );
        // The day it gives is the day `compact` and `relative_day` change.
        let now = at(-90_000);
        let changes =
            Timestamp::from_unix_millis(now.now().0 + now.until_next_day().as_millis() as i64)
                .unwrap();
        assert_eq!(now.relative_day(midnight), "Tomorrow");
        assert_eq!(
            now.clone()
                .with_clock(Clock::Fixed(changes))
                .relative_day(midnight),
            "Today"
        );
        let just_before = Timestamp::from_unix_millis(changes.0 - 1).unwrap();
        assert_eq!(
            now.with_clock(Clock::Fixed(just_before))
                .relative_day(midnight),
            "Tomorrow"
        );
    }

    #[test]
    fn the_next_day_counts_daylight_saving_days_of_23_and_25_hours() {
        // Adelaide's 25-hour 5 April 2026, from its midnight (+10:30) and
        // from after 02:00 became 02:00 again (+9:30).
        assert_eq!(
            adelaide(utc(2026, 4, 4, 13, 30)).until_next_day(),
            Duration::from_secs(25 * 3_600)
        );
        assert_eq!(
            adelaide(utc(2026, 4, 4, 17, 30)).until_next_day(),
            Duration::from_secs(21 * 3_600)
        );
        // Its 23-hour 4 October 2026.
        assert_eq!(
            adelaide(utc(2026, 10, 3, 14, 30)).until_next_day(),
            Duration::from_secs(23 * 3_600)
        );
        // Santiago changes at midnight. On 4 April 2026, 24:00 (-3:00) becomes
        // 23:00 (-4:00): from 22:00 the day lasts three more hours.
        let santiago = |at| {
            LocalTime::new(
                TimeZone::named("America/Santiago").unwrap(),
                AU,
                Clock::Fixed(at),
            )
        };
        assert_eq!(
            santiago(utc(2026, 4, 5, 1, 0)).until_next_day(),
            Duration::from_secs(3 * 3_600)
        );
        // On 5 September 2026, 24:00 (-4:00) becomes 01:00 (-3:00): midnight
        // never happens, and from 23:00 the next day starts in an hour, at 01:00.
        let saturday = santiago(utc(2026, 9, 6, 3, 0));
        assert_eq!(saturday.until_next_day(), Duration::from_secs(3_600));
        let then = Timestamp::from_unix_millis(utc(2026, 9, 6, 3, 0).0 + HOUR).unwrap();
        assert_eq!(
            (saturday.civil(then).day, saturday.civil(then).hour),
            (6, 1)
        );
    }

    #[test]
    fn the_next_day_agrees_with_a_scan_around_daylight_saving_changes() {
        let starts = [
            ("Australia/Adelaide", utc(2026, 4, 3, 12, 0)),
            ("Australia/Adelaide", utc(2026, 10, 2, 12, 0)),
            ("America/Santiago", utc(2026, 4, 3, 12, 0)),
            ("America/Santiago", utc(2026, 9, 4, 12, 0)),
            ("America/New_York", utc(2026, 3, 7, 12, 0)),
            ("America/New_York", utc(2026, 10, 31, 12, 0)),
            ("Pacific/Chatham", utc(2026, 4, 3, 12, 0)),
            ("Australia/Lord_Howe", utc(2026, 4, 3, 12, 0)),
        ];
        for (zone, start) in starts {
            let zone = TimeZone::named(zone).unwrap();
            // Every 37 minutes and 13.5 seconds across three days.
            for step in 0..((3 * 24 * 60) / 37) {
                let now = Timestamp::from_unix_millis(start.0 + step * 2_233_500).unwrap();
                let local = LocalTime::new(zone.clone(), AU, Clock::Fixed(now));
                assert_eq!(
                    local.until_next_day(),
                    next_day_by_scan(&local),
                    "{zone:?} at {now:?}"
                );
            }
        }
    }

    #[test]
    fn locales_choose_the_clock_and_the_order_of_day_and_month() {
        let now = utc(2026, 4, 14, 0, 11);
        let us = adelaide(now).with_locale(TimeLocale::ENGLISH_US);
        let gb = adelaide(now).with_locale(GB);
        assert_eq!(us.time_of_day(now), "9:41 AM");
        assert_eq!(gb.time_of_day(now), "09:41");
        assert_eq!(us.short_date(now), "Apr 14");
        assert_eq!(gb.short_date(now), "14 Apr");
        let last_year = utc(2025, 12, 31, 0, 0);
        assert_eq!(us.short_date(last_year), "Dec 31, 2025");
        assert_eq!(gb.short_date(last_year), "31 Dec 2025");
        assert_eq!(us.long_date(now), "Tuesday, April 14, 2026");
        assert_eq!(gb.long_date(now), "Tuesday 14 April 2026");
        let clock = |hour, minute| ClockTime::new(hour, minute).unwrap();
        for (time, twelve, twenty_four) in [
            (clock(0, 0), "12:00 AM", "00:00"),
            (clock(0, 5), "12:05 AM", "00:05"),
            (clock(7, 0), "7:00 AM", "07:00"),
            (clock(12, 0), "12:00 PM", "12:00"),
            (clock(23, 59), "11:59 PM", "23:59"),
        ] {
            assert_eq!(us.clock_time(time), twelve);
            assert_eq!(gb.clock_time(time), twenty_four);
            assert_eq!(
                gb.clone()
                    .with_locale(GB.with_hour_cycle(HourCycle::Twelve))
                    .clock_time(time),
                twelve
            );
        }
        assert!(ClockTime::new(24, 0).is_none());
        assert!(ClockTime::new(0, 60).is_none());
    }

    #[test]
    fn offsets_name_hours_minutes_and_local_mean_time_seconds() {
        let at = utc(2026, 1, 15, 12, 0);
        let offset = |zone: &str| {
            LocalTime::new(TimeZone::named(zone).unwrap(), AU, Clock::Fixed(at)).utc_offset(at)
        };
        assert_eq!(offset("UTC"), "UTC");
        assert_eq!(offset("America/St_Johns"), "UTC-3:30");
        assert_eq!(offset("Asia/Kathmandu"), "UTC+5:45");
        assert_eq!(offset("Pacific/Chatham"), "UTC+13:45");
        // Adelaide kept local mean time until 1895.
        let local = adelaide(at);
        assert_eq!(local.utc_offset(utc(1890, 1, 1, 0, 0)), "UTC+9:14:20");
        assert_eq!(local.time_of_day(utc(1890, 1, 1, 0, 0)), "9:14 AM");
    }

    #[test]
    fn zones_are_named_as_given_and_unknown_names_are_refused() {
        let zone = TimeZone::named("australia/adelaide").unwrap();
        assert_eq!(zone.name(), "australia/adelaide");
        assert_eq!(zone, TimeZone::named("australia/adelaide").unwrap());
        assert_eq!(TimeZone::utc().name(), "UTC");
        for name in [
            "",
            "Mars/Olympus_Mons",
            "+09:30",
            "-03:00",
            "Australia/../Adelaide",
            " UTC",
        ] {
            assert_eq!(
                TimeZone::named(name).unwrap_err(),
                UnknownTimeZone { name: name.into() },
                "{name:?}"
            );
        }
        assert_eq!(
            UnknownTimeZone {
                name: "Mars".into()
            }
            .to_string(),
            "unknown time zone \"Mars\""
        );
        let viewer = TimeZone::viewer();
        assert!(!viewer.name().is_empty());
        let _ = LocalTime::viewer()
            .with_clock(Clock::Fixed(Timestamp::MIN))
            .compact(Timestamp::MAX);
    }

    #[test]
    fn civil_offsets_match_the_zone_database() {
        let zone = jiff::tz::TimeZone::get("Australia/Adelaide").unwrap();
        let local = adelaide(Timestamp::MIN);
        for millis in [
            Timestamp::MIN.0,
            -1,
            0,
            1_775_318_400_000,
            1_791_045_000_123,
            Timestamp::MAX.0,
        ] {
            let expected = zone
                .to_offset(jiff::Timestamp::from_millisecond(millis).unwrap())
                .seconds();
            let at = Timestamp::from_unix_millis(millis).unwrap();
            assert_eq!(local.civil(at).utc_offset_seconds, expected, "{millis}");
        }
    }

    #[test]
    fn language_tags_resolve_through_the_table() {
        let resolve = TimeLocale::for_language_tag;
        assert_eq!(resolve("en-AU"), AU);
        assert_eq!(resolve("en_AU.UTF-8"), AU);
        assert_eq!(resolve("EN-au"), AU);
        assert_eq!(resolve("en-GB"), GB);
        assert_eq!(resolve("en"), TimeLocale::ENGLISH_US);
        assert_eq!(resolve("en-DE"), TimeLocale::ENGLISH_US);
        assert_eq!(resolve("de-AT"), GB);
        assert_eq!(
            resolve("zh-Hant-TW"),
            TimeLocale::new(HourCycle::Twelve, DateOrder::MonthDay)
        );
        assert_eq!(
            resolve("sw"),
            TimeLocale::new(HourCycle::TwentyFour, DateOrder::DayMonth)
        );
        for tag in ["", "C", "POSIX", "C.UTF-8", "1234", "toolongtag"] {
            assert_eq!(resolve(tag), TimeLocale::ENGLISH_US, "{tag:?}");
        }
    }

    #[test]
    fn durations_use_their_largest_two_units() {
        let cases = [
            (0, "0 s"),
            (45, "45 s"),
            (60, "1 min"),
            (12 * 60 + 59, "12 min"),
            (2 * 3_600, "2 h"),
            (2 * 3_600 + 10 * 60 + 30, "2 h 10 min"),
            (86_400 + 4 * 3_600 + 59 * 60, "1 day 4 h"),
            (86_400 + 59 * 60, "1 day"),
            (3 * 86_400, "3 days"),
            (3 * 86_400 + 3_600, "3 days 1 h"),
        ];
        for (seconds, text) in cases {
            assert_eq!(format_duration(Duration::from_secs(seconds)), text);
        }
        assert_eq!(format_duration(Duration::from_millis(1_999)), "1 s");
        let (earlier, later) = (Timestamp(1_000), Timestamp(61_000));
        assert_eq!(
            later.saturating_duration_since(earlier),
            Duration::from_secs(60)
        );
        assert_eq!(earlier.saturating_duration_since(later), Duration::ZERO);
    }
}
