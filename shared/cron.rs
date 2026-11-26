use jiff::{Timestamp, ToSpan, ZonedDateTime as Zoned, civil::DateTime, tz::TimeZone};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cron {
    minutes: u64,
    hours: u32,
    days: u32,
    months: u16,
    weekdays: u8,
    any_day: bool,
    any_weekday: bool,
}

fn field(text: &str, min: u32, max: u32, names: &[&str]) -> Result<(u64, bool), String> {
    let mut bits = 0u64;
    let any = text == "*" || text == "?";
    for part in text.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((r, s)) => (
                r,
                s.parse::<u32>()
                    .ok()
                    .filter(|s| *s > 0)
                    .ok_or_else(|| format!("bad step in {part}"))?,
            ),
            None => (part, 1),
        };
        let value = |v: &str| -> Result<u32, String> {
            let lower = v.to_ascii_lowercase();
            if let Some(i) = names.iter().position(|n| *n == lower) {
                return Ok(i as u32 + min);
            }
            v.parse::<u32>().map_err(|_| format!("bad value {v}"))
        };
        let (lo, hi) = if range == "*" || range == "?" {
            (min, max)
        } else if let Some((a, b)) = range.split_once('-') {
            (value(a)?, value(b)?)
        } else {
            let v = value(range)?;
            (v, if step > 1 { max } else { v })
        };
        let hi = if max == 7 && hi == 7 { 7 } else { hi };
        if lo < min || hi > max || lo > hi {
            return Err(format!("{part} is out of range {min}-{max}"));
        }
        let mut v = lo;
        while v <= hi {
            bits |= 1 << v;
            v += step;
        }
    }
    Ok((bits, any))
}

const MONTHS: &[&str] = &[
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];
const DAYS: &[&str] = &["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

impl Cron {
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = match text.trim() {
            "@hourly" => "0 * * * *",
            "@daily" | "@midnight" => "0 0 * * *",
            "@weekly" => "0 0 * * 0",
            "@monthly" => "0 0 1 * *",
            "@yearly" | "@annually" => "0 0 1 1 *",
            other => other,
        };
        let parts: Vec<&str> = text.split_whitespace().collect();
        let [m, h, dom, mon, dow] = parts.as_slice() else {
            return Err("a schedule needs 5 fields: minute hour day month weekday".into());
        };
        let (minutes, _) = field(m, 0, 59, &[])?;
        let (hours, _) = field(h, 0, 23, &[])?;
        let (days, any_day) = field(dom, 1, 31, &[])?;
        let (months, _) = field(mon, 1, 12, MONTHS)?;
        let (mut weekdays, any_weekday) = field(dow, 0, 7, DAYS)?;
        if weekdays & (1 << 7) != 0 {
            weekdays |= 1;
        }
        Ok(Self {
            minutes,
            hours: hours as u32,
            days: days as u32,
            months: months as u16,
            weekdays: (weekdays & 0x7f) as u8,
            any_day,
            any_weekday,
        })
    }

    fn day_matches(&self, dt: &DateTime) -> bool {
        let dom = self.days & (1 << dt.day()) != 0;
        let dow = self.weekdays & (1 << dt.weekday().to_sunday_zero_offset()) != 0;
        match (self.any_day, self.any_weekday) {
            (true, true) => true,
            (true, false) => dow,
            (false, true) => dom,
            (false, false) => dom || dow,
        }
    }

    pub fn next_after(&self, after: &Zoned) -> Option<Zoned> {
        let tz = after.time_zone().clone();
        let mut dt = after
            .datetime()
            .with()
            .second(0)
            .subsec_nanosecond(0)
            .build()
            .ok()?
            .checked_add(1.minute())
            .ok()?;
        let limit = dt.checked_add(400.days()).ok()?;
        while dt < limit {
            if self.months & (1 << dt.month()) == 0 || !self.day_matches(&dt) {
                dt = dt
                    .date()
                    .tomorrow()
                    .ok()?
                    .to_datetime(jiff::civil::Time::midnight());
                continue;
            }
            if self.hours & (1 << dt.hour()) == 0 {
                dt = dt
                    .with()
                    .minute(0)
                    .build()
                    .ok()?
                    .checked_add(1.hour())
                    .ok()?;
                continue;
            }
            if self.minutes & (1 << dt.minute()) == 0 {
                dt = dt.checked_add(1.minute()).ok()?;
                continue;
            }
            return tz.to_zoned_datetime(dt).ok();
        }
        None
    }

    pub fn due(&self, last: Timestamp, now: Timestamp) -> bool {
        let tz = TimeZone::system();
        self.next_after(&last.to_zoned_datetime(tz))
            .is_some_and(|next| next.timestamp() <= now)
    }
}

#[cfg(test)]
mod tests {
    use super::Cron;
    use jiff::{ZonedDateTime as Zoned, civil::date, tz::TimeZone};

    fn at(y: i16, mo: i8, d: i8, h: i8, mi: i8) -> Zoned {
        date(y, mo, d)
            .at(h, mi, 0, 0)
            .to_zoned_datetime(TimeZone::UTC)
            .unwrap()
    }

    #[test]
    fn next() {
        let c = Cron::parse("0 3 * * *").unwrap();
        assert_eq!(
            c.next_after(&at(2026, 11, 24, 21, 30)).unwrap(),
            at(2026, 11, 25, 3, 0)
        );
        let c = Cron::parse("*/15 * * * *").unwrap();
        assert_eq!(
            c.next_after(&at(2026, 11, 24, 21, 30)).unwrap(),
            at(2026, 11, 24, 21, 45)
        );
        let c = Cron::parse("30 4 * * mon").unwrap();
        assert_eq!(
            c.next_after(&at(2026, 11, 24, 21, 30)).unwrap(),
            at(2026, 11, 30, 4, 30)
        );
        let c = Cron::parse("@monthly").unwrap();
        assert_eq!(
            c.next_after(&at(2026, 11, 24, 21, 30)).unwrap(),
            at(2026, 12, 1, 0, 0)
        );
        assert!(Cron::parse("61 * * * *").is_err());
        assert!(Cron::parse("* * *").is_err());
    }
}
