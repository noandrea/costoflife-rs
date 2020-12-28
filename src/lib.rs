use anyhow::anyhow;
use bigdecimal::{BigDecimal, FromPrimitive, ParseBigDecimalError, ToPrimitive};
use chrono::{DateTime, Datelike, Duration, FixedOffset, Local, NaiveDate, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use slug::slugify;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::iter::FromIterator;
use std::str::FromStr;
use wasm_bindgen::prelude::*;

/// Purely for wasm
#[wasm_bindgen]
pub fn costoflife_per_diem(s: &str) -> f32 {
    match TxRecord::from_str(s) {
        Ok(v) => v.per_diem().to_f32().unwrap(),
        Err(_) => -1.0,
    }
}

#[wasm_bindgen]
pub fn costoflife_greetings() -> f32 {
    42.0
}

lazy_static! {
    static ref RE_CURRENCY: Regex = Regex::new(r"(\d+(\.\d{2})?)\p{Currency_Symbol}").unwrap();
    static ref RE_HASHTAG: Regex = Regex::new(r"[#\.]([a-zA-Z][0-9a-zA-Z_]*)").unwrap();
    static ref RE_LIFETIME: Regex =
        Regex::new(r"(([1-9]{1}[0-9]*)([dwmy]))(([1-9]{1}[0-9]*)x)?").unwrap();
    static ref RE_DATE: Regex = Regex::new(r"([0-3][0-9][0-1][0-9][1-9][0-9])").unwrap();
}

// ex "AKU Bellamont 3 Suede Low GTX 2020 #vestiti 129.95€ 3y"

fn extract_amount(input: &str) -> Option<&str> {
    RE_CURRENCY
        .captures(input)
        .and_then(|c| c.get(1).map(|m| m.as_str()))
}

fn extract_hashtag(text: &str) -> Option<&str> {
    RE_HASHTAG
        .captures(text)
        .and_then(|c| c.get(1).map(|m| m.as_str()))
}

fn extract_date(text: &str) -> Option<&str> {
    RE_DATE
        .captures(text)
        .and_then(|c| c.get(1).map(|m| m.as_str()))
}

fn extract_lifetime(text: &str) -> (&str, i64, i64) {
    match RE_LIFETIME.captures(text) {
        Some(c) => (
            c.get(3).map_or("d", |unit| unit.as_str()),
            c.get(2).map_or(1, |a| a.as_str().parse::<i64>().unwrap()),
            c.get(5).map_or(1, |r| r.as_str().parse::<i64>().unwrap()),
        ),
        None => ("d", 1, 1),
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Lifetime {
    // amount, times
    SingleDay,
    Year { amount: i64, times: i64 },
    Month { amount: i64, times: i64 },
    Week { amount: i64, times: i64 },
    Day { amount: i64, times: i64 },
}

impl Lifetime {
    /// Returns the number of days from a given date.
    ///
    /// This is significant con calculate the exact amount
    /// of days considering months and leap years
    pub fn get_days_since(&self, begin: &NaiveDate) -> i64 {
        match self {
            Self::Month { amount, times } => {
                // compute the total number of months (nm)
                let nm = begin.month() + u32::try_from(times * amount).unwrap();
                // match nm (number of months) and calculate the end year / month
                let ym = match nm {
                    12 => (begin.year_ce().1, 12),
                    nm => (begin.year_ce().1 + nm / 12, nm % 12),
                };
                // wrap the result with the correct type
                let eymd = (i32::try_from(ym.0).unwrap(), ym.1, begin.day());
                // calculate the end date
                let end = NaiveDate::from_ymd(eymd.0, eymd.1, eymd.2) - Duration::days(1);
                // count the days
                end.signed_duration_since(*begin).num_days()
            }
            Self::Year { amount, times } => {
                let ny = begin.year() + i32::try_from(times * amount).unwrap();
                let end = NaiveDate::from_ymd(ny, begin.month(), begin.day()) - Duration::days(1);
                // count the days
                end.signed_duration_since(*begin).num_days()
            }
            Self::Week { amount, times } => amount * 7 * times,
            Self::Day { amount, times } => amount * times,
            Self::SingleDay => 1,
        }
    }

    /// Approximates the size of the lifetime
    fn get_days_approx(&self) -> f64 {
        match self {
            Self::Year { amount, times } => 365.25 * f64::from_i64(amount * times).unwrap(),
            Self::Month { amount, times } => 30.44 * f64::from_i64(amount * times).unwrap(),
            Self::Week { amount, times } => 7.0 * f64::from_i64(amount * times).unwrap(),
            Self::Day { amount, times } => f64::from_i64(amount * times).unwrap(),
            Self::SingleDay => 1.0,
        }
    }

    pub fn get_repeats(&self) -> i64 {
        match self {
            Self::Year { times, .. } => *times,
            Self::Week { times, .. } => *times,
            Self::Day { times, .. } => *times,
            Self::Month { times, .. } => *times,
            Self::SingleDay => 1,
        }
    }
}

impl FromStr for Lifetime {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Lifetime, anyhow::Error> {
        let lifetime = extract_lifetime(s);
        match lifetime.0 {
            "d" => Ok(Lifetime::Day {
                amount: lifetime.1,
                times: lifetime.2,
            }),
            "w" => Ok(Lifetime::Week {
                amount: lifetime.1,
                times: lifetime.2,
            }),
            "y" => Ok(Lifetime::Year {
                amount: lifetime.1,
                times: lifetime.2,
            }),
            "m" => Ok(Lifetime::Month {
                amount: lifetime.1,
                times: lifetime.2,
            }),
            _ => Err(anyhow!("invalid value {}", s)),
        }
    }
}

impl PartialEq for Lifetime {
    fn eq(&self, other: &Self) -> bool {
        self.get_days_approx() == other.get_days_approx()
    }
}

impl fmt::Display for Lifetime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Year { amount, times } => write!(f, "{}y{}x", amount, times),
            Self::Month { amount, times } => write!(f, "{}m{}x", amount, times),
            Self::Week { amount, times } => write!(f, "{}w{}x", amount, times),
            Self::Day { amount, times } => write!(f, "{}d{}x", amount, times),
            Self::SingleDay => write!(f, "1d1x"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TxRecord {
    name: String,
    tags: HashMap<String, String>,
    amount: BigDecimal,
    starts_on: NaiveDate,
    lifetime: Lifetime, // in days
    recorded_at: DateTime<FixedOffset>,
    src: Option<String>,
}

impl TxRecord {
    // Getters
    pub fn get_name(&self) -> &str {
        &self.name[..]
    }
    pub fn get_tags(&self) -> Vec<String> {
        Vec::from_iter(self.tags.values().map(String::from))
    }
    pub fn get_amount(&self) -> BigDecimal {
        self.amount.with_scale(2)
    }
    pub fn get_lifetime(&self) -> &Lifetime {
        &self.lifetime
    }
    pub fn get_starts_on(&self) -> NaiveDate {
        self.starts_on
    }
    pub fn get_recorded_at(&self) -> &DateTime<FixedOffset> {
        &self.recorded_at
    }
    pub fn get_recorded_at_rfc3339(&self) -> String {
        self.recorded_at.to_rfc3339()
    }
    /// Returns true if the base amount is the same as the total
    ///
    /// That is, when there is no repetition on the lifetime
    pub fn amount_is_total(&self) -> bool {
        self.get_amount_total() == self.amount
    }
    /// Tells if the TxRecord as a tag
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains_key(&slugify(&tag))
    }
    /// Returns total amount for the transaction record
    pub fn get_amount_total(&self) -> BigDecimal {
        BigDecimal::from_i64(self.lifetime.get_repeats()).unwrap() * &self.amount
    }
    /// Returns the duration in days for this transaction
    pub fn get_duration_days(&self) -> i64 {
        self.lifetime.get_days_since(&self.starts_on)
    }
    /// Calculates and returns the per diem for the record
    /// and round it to the 2 decimals
    ///
    pub fn per_diem(&self) -> BigDecimal {
        self.per_diem_raw().with_scale(2)
    }
    /// Calculates and returns the per diem for the record
    ///
    /// The per diem is calculated as follow:
    /// END_DAY = START_DAY + (RECURRENCE_SIZE_DAYS * SEC_IN_DAYS  * RECURRENCE_TIMES)
    /// PER_DIEM = AMOUNT * RECURRENCE_TIMES) / (END_DAY - START_DAY )
    ///
    pub fn per_diem_raw(&self) -> BigDecimal {
        // TODO add inflation?
        let duration_days = BigDecimal::from_i64(self.get_duration_days()).unwrap();
        self.get_amount_total() / duration_days
    }

    /// Get the progress of the transaction at date
    ///
    /// None will use today as a data
    pub fn get_progress(&self, d: Option<&NaiveDate>) -> f64 {
        let d = match d {
            Some(d) => *d,
            None => today(),
        };
        // get the time range
        let (start, end) = (self.starts_on, self.get_ends_on());
        if d <= start {
            // if the tx period has not started
            return 0.0;
        }
        if d >= end {
            // tx period has expired
            return 100.0;
        }
        // total number of days
        let n = (end - start).num_days() as f64;
        // number of elapsed days
        let y = (d - start).num_days() as f64;
        // duration percentage
        y / n
    }

    /// Returns the end date (always computed)
    pub fn get_ends_on(&self) -> NaiveDate {
        self.starts_on + Duration::days(self.lifetime.get_days_since(&self.starts_on))
    }

    pub fn is_active_on(&self, target: &NaiveDate) -> bool {
        self.starts_on <= *target && *target <= self.get_ends_on()
    }

    /// Serialize the record to its string format
    pub fn to_string_record(&self) -> String {
        match &self.src {
            Some(s) => {
                format!(
                    "{}::{}::{}\n",
                    self.get_recorded_at_rfc3339(),
                    self.get_starts_on(),
                    s
                )
            }
            None => format!(
                "{}::{}::{} {}€ {} {}\n",
                self.get_recorded_at_rfc3339(),
                self.get_starts_on(),
                self.name,
                self.get_amount(),
                self.lifetime,
                self.get_tags()
                    .into_iter()
                    .map(|t| format!("#{}", t))
                    .collect::<Vec<String>>()
                    .join(" ")
            ),
        }
    }
    // Deserialize the record from
    pub fn from_string_record(s: &str) -> Result<TxRecord, Box<dyn std::error::Error>> {
        let abc = s.trim().splitn(3, "::").collect::<Vec<&str>>();
        let mut tx = Self::from_str(abc[2])?;
        tx.starts_on = NaiveDate::from_str(abc[1])?;
        tx.recorded_at = DateTime::parse_from_rfc3339(abc[0])?;
        Ok(tx)
    }

    pub fn new(name: &str, amount: &str) -> Result<TxRecord, anyhow::Error> {
        TxRecord::from(
            name,
            Vec::new(),
            amount,
            today(),
            Lifetime::SingleDay,
            now_local(),
            None,
        )
    }

    pub fn from(
        name: &str,
        tags: Vec<&str>,
        amount: &str,
        starts_on: NaiveDate,
        lifetime: Lifetime,
        recorded_at: DateTime<FixedOffset>,
        src: Option<&str>,
    ) -> Result<TxRecord, anyhow::Error> {
        Ok(TxRecord {
            name: String::from(name.trim()),
            tags: tags
                .iter()
                .map(|v| (slugify(v), String::from(*v)))
                .collect(),
            amount: BigDecimal::from_str(amount)?,
            lifetime,
            recorded_at,
            starts_on,
            src: match src {
                Some(s) => Some(String::from(s)),
                _ => None,
            },
        })
    }

    pub fn from_str(s: &str) -> Result<TxRecord, anyhow::Error> {
        // make an empty record
        let mut name: Vec<&str> = Vec::new();
        let mut amount = "0";
        let mut lifetime = Lifetime::SingleDay;
        let mut tags: Vec<&str> = Vec::new();
        let mut starts_on = today();
        // search for the stuff we need
        for t in s.split_whitespace() {
            if RE_CURRENCY.is_match(&t) {
                // read the currency
                if let Some(a) = extract_amount(t) {
                    amount = a
                }
            } else if RE_HASHTAG.is_match(&t) {
                // add tags
                if let Some(x) = extract_hashtag(&t) {
                    tags.push(x);
                }
            } else if RE_LIFETIME.is_match(t) {
                // add duration
                lifetime = t.parse::<Lifetime>()?;
            } else if RE_DATE.is_match(t) {
                starts_on = match extract_date(t) {
                    Some(d) => NaiveDate::parse_from_str(d, "%d%m%y")?,
                    None => today(),
                }
            } else {
                // catch all for the name
                name.push(&t)
            }
        }
        // build the tx record
        TxRecord::from(
            &name.join(" "),
            tags,
            amount,
            starts_on,
            lifetime,
            now_local(),
            Some(s),
        )
    }
}

impl FromStr for TxRecord {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<TxRecord, anyhow::Error> {
        TxRecord::from_str(s)
    }
}

impl fmt::Display for TxRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({})", self.name)
    }
}

impl PartialEq for TxRecord {
    fn eq(&self, other: &Self) -> bool {
        self.name.eq(&other.name)
            && self.tags.eq(&other.tags)
            && self.amount.eq(&other.amount)
            && self.starts_on.eq(&other.starts_on)
            && self.lifetime.eq(&other.lifetime)
    }
}

pub fn parse_amount(v: &str) -> Result<BigDecimal, ParseBigDecimalError> {
    BigDecimal::from_str(v)
}

/// Returns the current date
pub fn today() -> NaiveDate {
    Utc::today().naive_utc()
}

/// Returns the datetime with the local timezone
pub fn now_local() -> DateTime<FixedOffset> {
    DateTime::from(Local::now())
}

/// Parse a date
pub fn date(d: u32, m: u32, y: i32) -> NaiveDate {
    NaiveDate::from_ymd(y, m, d)
}

/// Parse a date from string, the string should be formatted
/// as dd/mm/yyyy
pub fn date_from_str(s: &str) -> Result<NaiveDate, chrono::ParseError> {
    NaiveDate::parse_from_str(s, "%d/%m/%Y")
}

#[cfg(test)]
mod tests {
    use super::Lifetime;
    use super::TxRecord;
    use chrono::Duration;

    #[test]
    fn test_getters() {
        let tests = vec![
            (
                // create by parsing
                TxRecord::from_str("Something we bought 1000€ #nice #living 100d").unwrap(),
                (
                    "Something we bought",                                  // title
                    super::today(),                                         // starts_on
                    (super::today() + Duration::days(100)),                 // ends_on
                    100,                                                    // duration days
                    vec![("nice", true), ("living", true), ("car", false)], // tags
                    (super::today(), true),                                 // is active
                    super::parse_amount("10").unwrap(),                     // per diem
                    (super::today(), 0.0 as f64),                           // progress
                ),
            ),
            // create using from
            (
                TxRecord::from(
                    "Car",
                    vec!["transportation", "lifestyle"],
                    "100000",
                    super::date(01, 01, 2010),
                    Lifetime::Year {
                        amount: 20,
                        times: 1,
                    },
                    super::now_local(),
                    None,
                )
                .unwrap(),
                (
                    "Car",
                    super::date(01, 01, 2010),
                    (super::date(01, 01, 2010) + Duration::days(7304)),
                    7304,
                    vec![
                        ("nice", false),
                        ("living", false),
                        ("car", false),
                        ("transportation", true),
                        ("lifestyle", true),
                    ],
                    (super::date(02, 01, 2030), false),
                    super::parse_amount("13.69").unwrap(),
                    (super::date(01, 10, 2020), 0.537513691128149 as f64),
                ),
            ),
            (
                // creae using new
                TxRecord::new("Building", "1000000").unwrap(),
                (
                    "Building",
                    super::today(),
                    (super::today() + Duration::days(1)),
                    1,
                    vec![
                        ("nice", false),
                        ("living", false),
                        ("car", false),
                        ("transportation", false),
                        ("lifestyle", false),
                    ],
                    (super::today(), true),
                    super::parse_amount("1000000").unwrap(),
                    (super::today(), 0.0 as f64),
                ),
            ),
        ];

        // run the test cases

        for (i, t) in tests.iter().enumerate() {
            println!("test_getters#{}", i);
            let (got, expected) = t;
            assert_eq!(got.get_name(), expected.0);
            assert_eq!(got.get_starts_on(), expected.1);
            assert_eq!(got.get_ends_on(), expected.2);
            assert_eq!(got.get_duration_days(), expected.3);
            // check the tags
            expected
                .4
                .iter()
                .for_each(|(tag, exists)| assert_eq!(got.has_tag(tag), *exists));
            // is active
            let (target_date, is_active) = expected.5;
            assert_eq!(got.is_active_on(&target_date), is_active);
            // per diem
            assert_eq!(got.per_diem(), expected.6);
            // progress
            let (on_date, progress) = expected.7;
            assert_eq!(got.get_progress(Some(&on_date)), progress);
        }
    }

    #[test]
    fn test_parse_transaction() {
        let tests = vec![
            (
                "Shoes #clothing 229.95€ 3y",
                TxRecord::from(
                    "Shoes",
                    vec!["clothing"],
                    "229.95",
                    super::today(),
                    Lifetime::Year {
                        amount: 3,
                        times: 1,
                    },
                    super::now_local(),
                    Some("Shoes #clothing 229.95€ 3y"),
                )
                .unwrap(),
                (super::parse_amount("0.21").unwrap(), 0.0 as f64),
            ),
            (
                "Rent 1729€ 1m12x 010118 #rent",
                TxRecord::from(
                    "Rent",
                    vec!["rent"],
                    "1729",
                    super::date(1, 1, 2018),
                    Lifetime::Month {
                        amount: 1,
                        times: 12,
                    },
                    super::now_local(),
                    None,
                )
                .unwrap(),
                (super::parse_amount("57.00").unwrap(), 100.0 as f64),
            ),
            (
                "Tea 20€ 2m1x 010120 #food",
                TxRecord::from(
                    "Tea",
                    vec!["food"],
                    "20",
                    super::date(1, 1, 2020),
                    Lifetime::Month {
                        amount: 2,
                        times: 1,
                    },
                    super::now_local(),
                    None,
                )
                .unwrap(),
                (super::parse_amount("0.33").unwrap(), 100.0 as f64),
            ),
            (
                "Tea 20€ 1m2x 010120 #food",
                TxRecord::from(
                    "Tea",
                    vec!["food"],
                    "20",
                    super::date(1, 1, 2020),
                    Lifetime::Month {
                        amount: 1,
                        times: 2,
                    },
                    super::now_local(),
                    None,
                )
                .unwrap(),
                (super::parse_amount("0.67").unwrap(), 100.0 as f64),
            ),
        ];

        for (i, t) in tests.iter().enumerate() {
            println!("test_parse_tx_record#{}", i);
            assert_eq!(
                t.0.parse::<TxRecord>().expect("test_parse_tx_record error"),
                t.1
            );
            let (per_diem, progress) = &t.2;
            assert_eq!(t.1.per_diem(), *per_diem);
            assert_eq!(t.1.get_progress(None), *progress);
        }
    }

    #[test]
    fn test_parse_lifetime() {
        let tests = vec![
            (
                "1d1x",
                Lifetime::Day {
                    amount: 1,
                    times: 1,
                },
                1,
            ),
            (
                "10d1x",
                Lifetime::Day {
                    amount: 10,
                    times: 1,
                },
                10,
            ),
            (
                "10w10x",
                Lifetime::Week {
                    amount: 10,
                    times: 10,
                },
                700,
            ),
            (
                "1y20x",
                Lifetime::Year {
                    amount: 1,
                    times: 20,
                },
                7304,
            ),
            (
                "1y",
                Lifetime::Year {
                    amount: 1,
                    times: 1,
                },
                365,
            ),
        ];

        for (i, t) in tests.iter().enumerate() {
            println!("test_parse_lifetime#{}", i);
            assert_eq!(
                t.0.parse::<Lifetime>().expect("test_parse_lifetime error"),
                t.1
            );
            assert_eq!(t.1.get_days_since(&super::date(1, 1, 2020)), t.2)
        }
    }
}
