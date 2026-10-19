# Local date and time

Status: active
Next action: open the pull request, then review it.

An application needs to show people when things happen: a message's time
("9:41 AM"), a list row's day ("Yesterday", "Tuesday", "14 Apr"), a
schedule's wall-clock time ("7:00 AM"), a long date, a duration, and which
zone is in use, in the viewer's zone and locale or in a named zone such as
the one a person's schedule runs in. Stobie offered nothing beyond
`web-time`: no time zone access and no local date or time formatting, so
applications fell back to relative times ("3 h ago") redrawn each minute.

## Decisions

- **`stobie_ui_egui::local_time`, not a new crate or `stobie-core`.**
  Formatting for people is presentation, and the browser path needs `Intl`
  through `js-sys`, which `stobie-core` must not depend on. `js-sys` and
  `wasm-bindgen`, already in every hosted browser build through the
  `eframe` feature, become ordinary `wasm32` dependencies so that the module
  works without that feature.
- **The platform converts; Stobie writes the text.** Each platform supplies
  only an instant's wall-clock fields in a zone (`Intl.DateTimeFormat`'s
  `formatToParts`, in `en-US-u-ca-gregory-nu-latn` with `hourCycle: "h23"`,
  in the browser; `jiff` and the system's time zone database natively) and
  a locale's two preferences. Shared Rust computes weekdays, offsets and
  calendar-day differences and assembles every string. Formatting the whole
  string with `Intl` would have followed each engine's CLDR version (Chromium
  writes "9:41 am" for `en-AU`, "9:41 a.m." for `en-CA`), so the web and
  native builds, and Chromium and Safari, could not agree, and host tests
  could not predict browser output.
- **A locale is a clock and a date order.** `TimeLocale` holds `HourCycle`
  (12 or 24 hours) and `DateOrder` (day before month, or after it). In the
  browser both come from `Intl` (`resolvedOptions().hourCycle` and the order
  of `month` and `day` parts); natively from `LC_ALL`, `LC_TIME` or `LANG`
  through a 45-entry table probed from Chromium 151. Words are English, like
  the rest of Stobie's text; translation is later work.
- **Formats.** `time_of_day` ("9:41 AM", "09:41"), `clock_time` for a
  zone-less wall-clock time, `short_date` ("14 Apr", with the year outside
  the current one), `long_date` ("Tuesday 14 April 2026", "Tuesday, April 14,
  2026"), `relative_day` (Today, Yesterday, Tomorrow, a weekday for the six
  days before today, else the short date), `relative_day_and_time`,
  `compact` (the time today, else the relative day, for list rows),
  `utc_offset` ("UTC+10:30", with seconds for local mean time), `civil`
  fields, and the free `format_duration` ("2 h 10 min", "1 day 4 h").
  Relative days count calendar days on the zone's wall clock, so 23- and
  25-hour days count as one.
- **Injectable zone, locale and clock.** `LocalTime::new(zone, locale,
  clock)` with `Clock::Fixed` makes every format deterministic;
  `LocalTime::viewer()` reads all three from the platform, and `with_zone`
  shows the same instants in a named zone.
- **Instants are `Timestamp`, whole milliseconds since the Unix epoch**,
  from 0001-01-03T00:00:00Z to 9999-12-29T23:59:59.999Z, so every zone's
  civil year stays within 1 to 9999 where `Intl` and `jiff` both work.
- **Zone names are kept as given.** `TimeZone::named` checks the name's
  characters (refusing offsets such as `+09:30`, which newer `Intl`
  accepts and `jiff` does not) and then the platform; `name()` returns the
  name as given, because Chromium canonicalises some zones to older aliases
  (`Asia/Calcutta`). The viewer's zone uses the platform's name, or
  `Etc/Unknown` without one.
- **One evaluator, two platforms.** `local_time/fixtures.json` lists
  instants in ten zones with their expected formats, and every table tag
  with its resolution. A hidden evaluator (`__private::local_time_report`)
  replaces each expectation with what the platform gives; the native unit
  test and `tools/local-time-browser.mjs` both require the result to equal
  the fixtures. The browser test loads the Gallery's WebAssembly module (two
  exported hooks) in Chromium pages with emulated zones and locales.
- **New dependency: `jiff` 0.2.38, native only** (Unlicense OR MIT; with
  `jiff-core`, `jiff-static`, `jiff-tzdb` and `jiff-tzdb-platform` under the
  same licences, `portable-atomic` and `portable-atomic-util` under
  Apache-2.0 OR MIT, and `windows-link` under MIT OR Apache-2.0). The
  browser build has no new dependency.

## Acceptance

- Unit tests (`stobie-ui-egui`, `local_time`): `Australia/Adelaide` leaving
  daylight saving on 5 April 2026 (02:30 twice, at +10:30 then +9:30) and
  entering it on 4 October 2026 (01:59 then 03:00), "Today" and "Yesterday"
  across the 25- and 23-hour days, relative days and weekdays, both clocks
  and date orders, midnight and noon, offsets with half and quarter hours
  and local mean time, offsets equal to `jiff`'s at the range's ends, the
  range, refused zone names, language-tag and POSIX locale resolution, and
  durations.
- Shared fixtures natively (`native_formats_match_the_shared_fixtures`): 21
  instants in Adelaide (both changes), New York (its March change), Kolkata,
  UTC, Chatham, St John's, Adelaide in 1890, Kiritimati and `Etc/GMT+12` at
  the range's ends, and the 45 table tags.
- Browser test (`tools/local-time-browser.mjs`, gallery-browser stage): the
  same fixtures through the page's `Intl` equal the expectations in four
  pages emulating Adelaide (`en-AU`), New York (`en-US`), London (`en-GB`)
  and Tokyo (`ar-EG`); in each, the viewer's zone is the page's, its locale
  resolves as expected, and a fixed instant formats in the page's zone.
- The guide's [Local date and time](../ui-guides/components.md#local-date-and-time)
  section documents the API and the locale differences, with a runnable
  rustdoc example on the module.
- `cargo xtask verify` passes.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| `local_time` module, native zone data and the guide | In progress | Unit tests |
| Browser path and the shared fixtures in Chromium | In progress | Browser test |

## Observations

- Chromium 151 probes (`Intl.DateTimeFormat`, 52 tags): the hour cycle and
  date order in the native table; `en-AU` and `en-NZ` use 12 hours with "am";
  `en-GB` and `en-IE` 24 hours; `en-ZA`, `fr-CA`, `ja`, `zh` and `hu` put
  the month first in numeric dates. `Asia/Kolkata` resolves to
  `Asia/Calcutta`. Dates before 1582 are proleptic Gregorian, as in `jiff`,
  and local mean time keeps its seconds (Adelaide +9:14:20 before 1895,
  Kiritimati -10:29:20 in year 1).
- Playwright's WebKit does not start on this host (missing system
  libraries), so Safari was not run.

## Later work

- Translating the words, and following locale conventions a `TimeLocale`
  does not model ("am", "a.m.", year-first dates), when Stobie gains
  localisation.
- A run in Safari or WebKit, for its zone data and locale resolution.
