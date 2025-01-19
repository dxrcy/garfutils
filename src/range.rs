use std::str::FromStr;

use chrono::{Datelike as _, NaiveDate};

/// Always inclusive
#[derive(Clone, Copy, Debug)]
pub struct DateRange {
    from: MonthDay,
    to: MonthDay,
}

// Invariant: date.year is always constant: `MonthDay::YEAR`
#[derive(Clone, Copy, Debug)]
struct MonthDay {
    date: NaiveDate,
}

impl DateRange {
    pub fn all() -> Self {
        Self {
            from: MonthDay::first(),
            to: MonthDay::last(),
        }
    }

    pub fn contains(&self, date: NaiveDate) -> bool {
        let date = MonthDay::from(date);
        date >= self.from && date <= self.to
    }
}

impl MonthDay {
    const YEAR: i32 = 0;

    pub fn from_ymd_opt(month: u32, date: u32) -> Option<Self> {
        Some(Self {
            date: NaiveDate::from_ymd_opt(Self::YEAR, month, date)?,
        })
    }

    pub fn first() -> Self {
        Self::from_ymd_opt(1, 1).expect("constant date is valid")
    }
    pub fn last() -> Self {
        Self::from_ymd_opt(12, 31).expect("constant date is valid")
    }
}

impl From<NaiveDate> for MonthDay {
    fn from(date: NaiveDate) -> Self {
        Self {
            date: date.with_year(Self::YEAR).expect("year is valid"),
        }
    }
}

impl PartialEq for MonthDay {
    fn eq(&self, other: &Self) -> bool {
        self.date == other.date
    }
}

impl PartialOrd for MonthDay {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.date.partial_cmp(&other.date)
    }
}

impl FromStr for DateRange {
    type Err = String;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let mut parts = string.split("..");

        let from = parts.next().unwrap_or(string);
        let from: MonthDay = from
            .try_into()
            .map_err(|_| format!("Invalid start date: '{}'", from))?;

        let to = match parts.next() {
            Some(to) => to
                .try_into()
                .map_err(|_| format!("Invalid end date: '{}'", to))?,
            None => from,
        };

        if from > to {
            return Err("End date must be after start date".to_string());
        }

        Ok(Self { from, to })
    }
}

impl TryFrom<&str> for MonthDay {
    type Error = ();

    fn try_from(string: &str) -> Result<Self, Self::Error> {
        let mut parts = string.split('-');

        let month = parts.next().unwrap_or(string);
        let month: u32 = month.parse().map_err(|_| ())?;

        let day = parts.next().ok_or(())?;
        let day: u32 = day.parse().map_err(|_| ())?;

        Self::from_ymd_opt(month, day).ok_or(())
    }
}
