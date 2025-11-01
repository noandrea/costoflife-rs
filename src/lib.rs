//! The [`CostOf.Life`] calculator library.
//!
//! Provides functions to calculate the per diem cost
//! of an expense over a time range.
//!
//! [`CostOf.Life`]: http://thecostof.life
mod utils;
use bigdecimal::{BigDecimal, FromPrimitive, ToPrimitive, Zero};
use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveDate};
use lazy_static::lazy_static;
use regex::Regex;
use slug::slugify;
use std::collections::{BTreeSet, HashMap};
use std::fmt::{self};
use std::str::FromStr;
// export utils
pub use utils::*;
use wasm_bindgen::prelude::*;

/// Rounding factor for big decimals
const SCALE: i64 = 2;

/// Exposes the per diem calculation to wasm
///
/// # Arguments
///
/// * `s` - A string slice that holds the specs for the transaction
///
///
#[wasm_bindgen]
pub fn costoflife_per_diem(s: &str) -> f64 {
    match TxRecord::from_str(s) {
        Ok(v) => v.per_diem().to_f64().unwrap(),
        Err(_) => -1.0,
    }
}

/// A simple wasm function for testing
///
/// Always return 42.0
#[wasm_bindgen]
pub fn costoflife_greetings() -> f32 {
    42.0
}

// Let's use generic errors
type Result<T> = std::result::Result<T, CostOfLifeError>;

#[derive(Debug, Clone)]
pub enum CostOfLifeError {
    InvalidLifetimeFormat(String),
    InvalidDateFormat(String),
    InvalidAmount(String),
    GenericError(String),
}

impl From<chrono::ParseError> for CostOfLifeError {
    fn from(error: chrono::ParseError) -> Self {
        CostOfLifeError::InvalidDateFormat(error.to_string())
    }
}

// initialize regexp
lazy_static! {
    static ref RE_CURRENCY: Regex = Regex::new(r"(\d+(\.\d{2})?)\p{Currency_Symbol}").unwrap();
    static ref RE_HASHTAG: Regex = Regex::new(r"^[#\.]([a-zA-Z][0-9a-zA-Z_-]*)$").unwrap();
    static ref RE_LIFETIME: Regex =
        Regex::new(r"(([1-9]{1}[0-9]*)([dwmy]))(([1-9]{1}[0-9]*)x)?").unwrap();
    static ref RE_DATE: Regex = Regex::new(r"([0-3][0-9][0-1][0-9][1-9][0-9])").unwrap();
}

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

fn extract_date(text: &str) -> Option<NaiveDate> {
    let ds = RE_DATE
        .captures(text)
        .and_then(|c| c.get(1).map(|m| m.as_str()));
    match ds {
        Some(d) => utils::date_from_str(d),
        None => Some(utils::today()),
    }
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

/// A time range with duration and repetition
///
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
    pub fn get_days_since(&self, since: &NaiveDate) -> i64 {
        match self {
            Self::Month { amount, times } => {
                // compute the total number of months (nm)
                let nm = since.month() + (times * amount) as u32;
                // match nm (number of months) and calculate the end year / month
                let (y, m) = (since.year() as u32 + nm / 12, nm % 12);
                // wrap the result with the correct type
                let (y, m, d) = (y as i32, m, since.day());
                // calculate the end date
                let end = NaiveDate::from_ymd(y, m, d);
                // count the days
                end.signed_duration_since(*since).num_days()
            }
            Self::Year { amount, times } => {
                let ny = since.year() + (times * amount) as i32;
                let end = NaiveDate::from_ymd(ny, since.month(), since.day());
                // count the days
                end.signed_duration_since(*since).num_days()
            }
            Self::Week { amount, times } => amount * 7 * times,
            Self::Day { amount, times } => amount * times,
            Self::SingleDay => 1,
        }
    }

    /// Approximates the size of the lifetime
    ///
    /// this function differs from the `get_days_since` by the
    /// fact that the size of months and years is approximated:
    /// - A year is 365.25 days
    /// - A month is 30.44 days
    ///
    fn get_days_approx(&self) -> f64 {
        match self {
            Self::Year { amount, times } => 365.25 * (amount * times) as f64,
            Self::Month { amount, times } => 30.44 * (amount * times) as f64,
            Self::Week { amount, times } => 7.0 * (amount * times) as f64,
            Self::Day { amount, times } => (amount * times) as f64,
            Self::SingleDay => 1.0,
        }
    }

    /// Get the number of duration repeats for the current lifetime
    ///
    ///
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
    type Err = CostOfLifeError;

    fn from_str(s: &str) -> Result<Lifetime> {
        let (period, amount, times) = extract_lifetime(s);
        match period {
            "w" => Ok(Lifetime::Week { amount, times }),
            "y" => Ok(Lifetime::Year { amount, times }),
            "m" => Ok(Lifetime::Month { amount, times }),
            _ => Ok(Lifetime::Day { amount, times }),
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
            Self::Year { amount, times } => write!(f, "{amount}y{times}x"),
            Self::Month { amount, times } => write!(f, "{amount}m{times}x"),
            Self::Week { amount, times } => write!(f, "{amount}w{times}x"),
            Self::Day { amount, times } => write!(f, "{amount}d{times}x"),
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

/// Holds a transaction informations
///
///
impl TxRecord {
    // Getters
    pub fn get_name(&self) -> &str {
        &self.name[..]
    }
    /// Get the tags for the tx, sorted alphabetically
    pub fn get_tags(&self) -> Vec<String> {
        self.tags
            .values()
            .map(String::from)
            .collect::<BTreeSet<String>>()
            .into_iter()
            .collect()
    }
    /// Get the amount for the tx, rounded to 2 decimals
    pub fn get_amount(&self) -> BigDecimal {
        self.amount.with_scale(SCALE)
    }
    /// Get the lifetime for the tx
    pub fn get_lifetime(&self) -> &Lifetime {
        &self.lifetime
    }
    /// Get the start date for the tx
    pub fn get_starts_on(&self) -> NaiveDate {
        self.starts_on
    }
    /// Get the datetime when the tx was recorded
    pub fn get_recorded_at(&self) -> &DateTime<FixedOffset> {
        &self.recorded_at
    }
    /// Get the datetime when the tx was recorded as a rfc3339 string
    pub fn get_recorded_at_rfc3339(&self) -> String {
        self.recorded_at.to_rfc3339()
    }
    /// Returns true if the base amount is the same as the total
    ///
    /// That is, when there is no repetition on the lifetime
    pub fn amount_is_total(&self) -> bool {
        self.lifetime.get_repeats() > 1
    }
    /// Tells if the TxRecord as a tag
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains_key(&slugify(tag))
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
        self.per_diem_raw().with_scale(SCALE)
    }
    /// Calculates and returns the per diem for the record
    ///
    /// The per diem is calculated as follow:
    ///
    /// * END_DAY = START_DAY + (RECURRENCE_SIZE_DAYS * SEC_IN_DAYS  * RECURRENCE_TIMES)
    /// * PER_DIEM = AMOUNT * RECURRENCE_TIMES) / (END_DAY - START_DAY )
    ///
    pub fn per_diem_raw(&self) -> BigDecimal {
        let duration_days = BigDecimal::from_i64(self.get_duration_days()).unwrap();
        self.get_amount_total() / duration_days
    }

    /// Get the progress of the transaction at date
    ///
    /// None will use today as a data
    pub fn get_progress(&self, d: Option<NaiveDate>) -> f64 {
        let d = match d {
            Some(d) => d,
            None => utils::today(),
        };
        // get the time range
        let (start, end) = (self.starts_on, self.get_ends_on());
        if d <= start {
            // if the tx period has not started
            return 0.0;
        }
        if d >= end {
            // tx period has expired
            return 1.0;
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
        self.starts_on + Duration::days(self.lifetime.get_days_since(&self.starts_on) - 1)
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
                self.get_name(),
                self.get_amount(),
                self.get_lifetime(),
                self.get_tags()
                    .iter()
                    .map(|t| format!("#{t}"))
                    .collect::<Vec<String>>()
                    .join(" ")
            ),
        }
    }
    // Deserialize the record from
    pub fn from_string_record(s: &str) -> Result<TxRecord> {
        let abc = s.trim().splitn(3, "::").collect::<Vec<&str>>();
        let mut tx = Self::from_str(abc[2])?;
        tx.starts_on = NaiveDate::from_str(abc[1])?;
        tx.recorded_at = DateTime::parse_from_rfc3339(abc[0])?;
        Ok(tx)
    }

    pub fn new(name: &str, amount: &str) -> Result<TxRecord> {
        TxRecord::from(
            name,
            Vec::new(),
            amount,
            utils::today(),
            Lifetime::SingleDay,
            utils::now_local(),
            None,
        )
    }

    /// Builds a TxRecord using parameters
    ///
    /// # Arguments
    ///
    /// * `name` - A string slice that holds the name of the transaction
    /// * `tags` - A vector of string slices with the transaction's tags
    /// * `amount` - A string slice representing a monetary value
    /// * `starts_on` - The date of the start of the transaction
    /// * `lifetime` - The lifetime of transaction
    /// * `recorded_at` - The localized exact time when the tx was added
    /// * `src` - An option string slice with the original string used to submit the tx
    ///
    /// # Examples
    ///
    /// ```
    /// use costoflife::{self, TxRecord, Lifetime};
    ///
    /// let tx = TxRecord::from(
    ///     "Car",
    ///     vec!["transportation", "lifestyle"],
    ///     "100000",
    ///     costoflife::date(01, 01, 2010),
    ///     Lifetime::Year {
    ///         amount: 20,
    ///         times: 1,
    ///     },
    ///     costoflife::now_local(),
    ///     None,
    /// ).unwrap();
    ///
    /// ```
    pub fn from(
        name: &str,
        tags: Vec<&str>,
        amount: &str,
        starts_on: NaiveDate,
        lifetime: Lifetime,
        recorded_at: DateTime<FixedOffset>,
        src: Option<&str>,
    ) -> Result<TxRecord> {
        let tx = TxRecord {
            name: String::from(name.trim()),
            tags: tags
                .iter()
                .map(|v| (slugify(v), String::from(*v)))
                .collect(),
            amount: parse_amount(amount)
                .ok_or_else(|| CostOfLifeError::InvalidAmount("Invalid amount".to_string()))?,
            lifetime,
            recorded_at,
            starts_on,
            src: Some(src).map(|s| String::from(s.unwrap())),
        };
        // validate the amount
        if tx.get_amount() <= BigDecimal::zero() {
            return Err(CostOfLifeError::InvalidAmount(
                format! {"amount should be a positive number: {amount}"},
            ));
        }
        // all good
        Ok(tx)
    }
}

impl FromStr for TxRecord {
    type Err = CostOfLifeError;

    fn from_str(s: &str) -> Result<Self> {
        // make an empty record
        let mut name: Vec<&str> = Vec::new();
        let mut amount = "0";
        let mut lifetime = Lifetime::SingleDay;
        let mut tags: Vec<&str> = Vec::new();
        let mut starts_on = utils::today();
        // search for the stuff we need
        for t in s.split_whitespace() {
            if RE_CURRENCY.is_match(t) {
                // read the currency
                if let Some(a) = extract_amount(t) {
                    amount = a
                }
            } else if RE_HASHTAG.is_match(t) {
                // add tags
                if let Some(x) = extract_hashtag(t) {
                    tags.push(x);
                }
            } else if RE_LIFETIME.is_match(t) {
                // add duration
                lifetime = t.parse::<Lifetime>()?;
            } else if RE_DATE.is_match(t) {
                // start date
                starts_on = extract_date(t)
                    .ok_or_else(|| CostOfLifeError::GenericError(String::from(":")))?;
            } else {
                // catch all for the name
                name.push(t)
            }
        }
        // build the tx record
        TxRecord::from(
            &name.join(" "),
            tags,
            amount,
            starts_on,
            lifetime,
            utils::now_local(),
            Some(s),
        )
    }
}

impl fmt::Display for TxRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
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

/// Compute the cost of life for a set of transactions
///
pub fn cost_of_life<'a, I>(txs: I, on: &NaiveDate) -> BigDecimal
where
    I: Iterator<Item = &'a TxRecord>,
{
    txs.filter(|tx| tx.is_active_on(on)) // is still an active expense
        .map(|tx| tx.per_diem_raw())
        .sum::<BigDecimal>() // sum all the amount
        .with_scale(SCALE) // apply the scale
}

#[cfg(test)]
pub mod wasm_tests {
    use wasm_bindgen_test::*;
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[test]
    #[wasm_bindgen_test]
    fn test_greetings() {
        assert_eq!(super::costoflife_greetings(), 42.0);
    }

    #[test]
    #[wasm_bindgen_test]
    fn test_per_diem() {
        assert_eq!(super::costoflife_per_diem("20€ rent"), 20.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_tx() {
        let tests = vec![
            (
                // create by parsing
                TxRecord::from_str("Something we bought 1000€ #nice #living 100d"),
                (
                    Ok(()),                                                 // ok/error
                    "Something we bought",                                  // title
                    today(),                                                // starts_on
                    (today() + Duration::days(99)),                         // ends_on
                    100,                                                    // duration days
                    vec![("nice", true), ("living", true), ("car", false)], // tags
                    (today(), true),                                        // is active
                    parse_amount("10").unwrap(),                            // per diem
                    (Some(today()), 0.0_f64), // progress                                                 // PARSE ERROR
                ),
            ),
            (
                // create by parsing (same but with parse)
                "Something we bought 1000€ #nice #living 100d".parse::<TxRecord>(),
                (
                    Ok(()),                                                 // ok/error
                    "Something we bought",                                  // title
                    today(),                                                // starts_on
                    (today() + Duration::days(99)),                         // ends_on
                    100,                                                    // duration days
                    vec![("nice", true), ("living", true), ("car", false)], // tags
                    (today(), true),                                        // is active
                    parse_amount("10").unwrap(),                            // per diem
                    (Some(today()), 0.0_f64), // progress                                                 // PARSE ERROR
                ),
            ),
            (
                // create by parsing WITH ERROR / no amount
                TxRecord::from_str("we bought nothing #nice #living 100d"),
                (
                    Err(()),                                                // ok/error
                    "Something we bought",                                  // title
                    today(),                                                // starts_on
                    (today() + Duration::days(99)),                         // ends_on
                    100,                                                    // duration days
                    vec![("nice", true), ("living", true), ("car", false)], // tags
                    (today(), true),                                        // is active
                    parse_amount("10").unwrap(),                            // per diem
                    (Some(today()), 0.0_f64),                               // progress
                ),
            ),
            (
                // from string with date
                TxRecord::from_str("Rent 1729€ 1m12x 010118 #rent"),
                (
                    Ok(()),                                // ok/error
                    "Rent",                                // title
                    date(1, 1, 2018),                      // starts_on
                    (date(31, 12, 2018)),                  // ends_on
                    365,                                   // duration days
                    vec![("home", false), ("rent", true)], // tags
                    (today(), false),                      // is active
                    parse_amount("56.84").unwrap(),        // per diem
                    (None, 1.0_f64),                       // progress
                ),
            ),
            (
                // from string with WRONG date
                TxRecord::from_str("Rent#2018 1729€ 1m12x 320118 #rent"),
                (
                    Err(()),                                                 // ok/error
                    "Rent#2018",                                             // title
                    date(1, 1, 2018),                                        // starts_on
                    (date(31, 12, 2018)),                                    // ends_on
                    365,                                                     // duration days
                    vec![("home", false), ("rent", true), ("#2018", false)], // tags
                    (today(), false),                                        // is active
                    parse_amount("58.84").unwrap(),                          // per diem
                    (None, 1.0_f64),                                         // progress
                ),
            ),
            (
                // from string with week repeats (39,96)
                TxRecord::from_str("Mobile internet 9.99€ 210421 1w4x #internet"),
                (
                    Ok(()),                                           // ok/error
                    "Mobile internet",                                // title
                    date(21, 4, 2021),                                // starts_on
                    (date(18, 5, 2021)),                              // ends_on
                    28,                                               // duration days
                    vec![("internet", true)],                         // tags
                    (date(12, 5, 2021), true),                        // is active
                    parse_amount("1.42").unwrap(),                    // per diem
                    (Some(date(5, 5, 2021)), 0.5185185185185185_f64), // progress
                ),
            ),
            (
                // create using from
                TxRecord::from(
                    "Car",
                    vec!["transportation", "lifestyle"],
                    "100000",
                    date(1, 1, 2010),
                    Lifetime::Year {
                        amount: 20,
                        times: 1,
                    },
                    now_local(),
                    None,
                ),
                (
                    Ok(()),
                    "Car",
                    date(1, 1, 2010),
                    (date(31, 12, 2029)),
                    7305,
                    vec![
                        ("nice", false),
                        ("living", false),
                        ("car", false),
                        ("transportation", true),
                        ("lifestyle", true),
                    ],
                    (date(1, 1, 2030), false),
                    parse_amount("13.68").unwrap(),
                    (Some(date(1, 10, 2020)), 0.537513691128149_f64),
                ),
            ),
            (
                // create using new
                TxRecord::new("Building", "1000000"),
                (
                    Ok(()),
                    "Building",
                    today(),
                    today(),
                    1,
                    vec![
                        ("nice", false),
                        ("living", false),
                        ("car", false),
                        ("transportation", false),
                        ("lifestyle", false),
                    ],
                    (today(), true),
                    parse_amount("1000000").unwrap(),
                    (None, 0.0_f64),
                ),
            ),
            (
                // create using new / Invlaid amount
                TxRecord::new("Building", "not a number"),
                (
                    Err(()),
                    "Building",
                    today(),
                    today(),
                    1,
                    vec![
                        ("nice", false),
                        ("living", false),
                        ("car", false),
                        ("transportation", false),
                        ("lifestyle", false),
                    ],
                    (today(), true),
                    parse_amount("1000000").unwrap(),
                    (None, 0.0_f64),
                ),
            ),
            (
                // from string with week repeats (39,96)
                TxRecord::from_string_record(
                    "2021-01-03T19:36:43.976697738+00:00::2021-04-21::Mobile internet 9.99€ 210421 1w4x #internet",
                ),
                (
                    Ok(()),                                           // ok/error
                    "Mobile internet",                                // title
                    date(21, 4, 2021),                                // starts_on
                    (date(18, 5, 2021)),                              // ends_on
                    28,                                               // duration days
                    vec![("internet", true)],                         // tags
                    (date(12, 5, 2021), true),                        // is active
                    parse_amount("1.42").unwrap(),                    // per diem
                    (Some(date(5, 5, 2021)), 0.5185185185185185_f64), // progress
                ),
            ),
            (
                // from string with week repeats (39,96) // WITH WRONG DATE
                TxRecord::from_string_record(
                    "2021-01-03T19:36:43.976697738+00:00::2021-14-21::Mobile internet 9.99€ 210421 1w4x #internet",
                ),
                (
                    Err(()),                                          // ok/error
                    "Mobile internet",                                // title
                    date(21, 4, 2021),                                // starts_on
                    (date(18, 5, 2021)),                              // ends_on
                    28,                                               // duration days
                    vec![("internet", true)],                         // tags
                    (date(12, 5, 2021), true),                        // is active
                    parse_amount("1.42").unwrap(),                    // per diem
                    (Some(date(5, 5, 2021)), 0.5185185185185185_f64), // progress
                ),
            ),
            (
                // from string with week repeats (39,96) // WITH WRONG RECORDED DATE
                TxRecord::from_string_record(
                    "2021-01-32T19:36:43.976697738+00:00::2021-14-21::Mobile internet 9.99€ 210421 1w4x #internet",
                ),
                (
                    Err(()),                                          // ok/error
                    "Mobile internet",                                // title
                    date(21, 4, 2021),                                // starts_on
                    (date(18, 5, 2021)),                              // ends_on
                    28,                                               // duration days
                    vec![("internet", true)],                         // tags
                    (date(12, 5, 2021), true),                        // is active
                    parse_amount("1.42").unwrap(),                    // per diem
                    (Some(date(5, 5, 2021)), 0.5185185185185185_f64), // progress
                ),
            ),
        ];

        // run the test cases

        for (i, t) in tests.iter().enumerate() {
            println!("test_getters#{i}");
            let (res, expected) = t;
            let (result, name, starts_on, ends_on, duration, tags, status, per_diem, progress_test) =
                expected;
            // test for expected errors
            assert_eq!(res.is_err(), result.is_err());
            if res.is_err() {
                continue;
            }
            // test the parser
            let got = res.as_ref().unwrap();
            // test getters
            assert_eq!(got.get_name(), *name);
            assert_eq!(got.get_name(), got.to_string());
            assert_eq!(got.get_starts_on(), *starts_on);
            assert_eq!(got.get_ends_on(), *ends_on);
            assert_eq!(got.get_duration_days(), *duration);
            // check the tags
            tags.iter()
                .for_each(|(tag, exists)| assert_eq!(got.has_tag(tag), *exists));
            // is active
            let (target_date, is_active) = status;
            assert_eq!(got.is_active_on(target_date), *is_active);
            // per diem
            assert_eq!(got.per_diem(), *per_diem);
            // progress
            let (on_date, progress) = progress_test;
            assert_eq!(got.get_progress(*on_date), *progress);
            // test serializing deserializing
            let txs = got.to_string_record();
            let txr = TxRecord::from_string_record(&txs).unwrap();
            assert_eq!(*got, txr);
        }
    }

    #[test]
    fn test_lifetime() {
        let tests = vec![
            (
                ("1d1x", today(), 1, "1d1x"),
                Lifetime::Day {
                    amount: 1,
                    times: 1,
                },
            ),
            (
                ("10d1x", today(), 10, "10d1x"),
                Lifetime::Day {
                    amount: 10,
                    times: 1,
                },
            ),
            (
                ("10d10x", today(), 100, "10d10x"),
                Lifetime::Day {
                    amount: 10,
                    times: 10,
                },
            ),
            (
                ("1w", today(), 7, "1w1x"),
                Lifetime::Week {
                    amount: 1,
                    times: 1,
                },
            ),
            (
                ("7w", today(), 49, "7w1x"),
                Lifetime::Week {
                    amount: 7,
                    times: 1,
                },
            ),
            (
                ("10w10x", today(), 700, "10w10x"),
                Lifetime::Week {
                    amount: 10,
                    times: 10,
                },
            ),
            (
                ("20y", date(1, 1, 2020), 7305, "20y1x"),
                Lifetime::Year {
                    amount: 20,
                    times: 1,
                },
            ),
            (
                ("1y20x", date(1, 1, 2020), 7305, "1y20x"),
                Lifetime::Year {
                    amount: 1,
                    times: 20,
                },
            ),
            (
                ("20y", date(1, 1, 2021), 7305, "20y1x"),
                Lifetime::Year {
                    amount: 20,
                    times: 1,
                },
            ),
            (
                ("1y", date(1, 1, 2020), 366, "1y1x"),
                Lifetime::Year {
                    amount: 1,
                    times: 1,
                },
            ),
            (
                ("1y", date(1, 1, 2021), 365, "1y1x"),
                Lifetime::Year {
                    amount: 1,
                    times: 1,
                },
            ),
            (
                ("1m", date(1, 1, 2021), 31, "1m1x"),
                Lifetime::Month {
                    amount: 1,
                    times: 1,
                },
            ),
            (
                ("12m", date(1, 1, 2021), 365, "12m1x"),
                Lifetime::Month {
                    amount: 12,
                    times: 1,
                },
            ),
            (
                ("1m12x", date(1, 1, 2021), 365, "1m12x"),
                Lifetime::Month {
                    amount: 1,
                    times: 12,
                },
            ),
            (
                ("", today(), 1, "1d1x"),
                Lifetime::Day {
                    amount: 1,
                    times: 1,
                },
            ),
        ];

        for (i, t) in tests.iter().enumerate() {
            println!("test_parse_lifetime#{i}");

            let (lifetime_spec, lifetime_exp) = t;
            let (input_str, start_date, duration_days, to_str) = lifetime_spec;

            assert_eq!(
                input_str
                    .parse::<Lifetime>()
                    .expect("test_parse_lifetime error"),
                *lifetime_exp,
            );
            // this make sense only with the assertion above
            assert_eq!(lifetime_exp.get_days_since(start_date), *duration_days);
            // to string
            assert_eq!(lifetime_exp.to_string(), *to_str);
        }
    }

    #[test]
    fn test_extract() {
        // extract not matching date
        // this cannot happen but anyway
        let r = extract_date("invalid date");
        assert_eq!(r.unwrap(), today());
    }

    #[test]
    fn test_costoflife() {
        let txs = vec![
            // insert one entry
            TxRecord::new("Test#1", "10.2311321").unwrap(),
            TxRecord::new("Test#2", "10.5441231").unwrap(),
            TxRecord::new("Test#3", "70.199231321").unwrap(),
        ];
        // simple insert
        assert_eq!(
            cost_of_life(txs.iter(), &today()),
            parse_amount("90.97").unwrap()
        );
    }
}
