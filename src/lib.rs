use lazy_static::lazy_static;

use regex::Regex;

lazy_static! {
    static ref RE_CURRENCY: Regex =
        Regex::new(r"(([1-9]{1}[0-9]{0,2}(,[0-9]{3})*(\.[0-9]{0,2})?|[1-9]{1}[0-9]{0,}(\.[0-9]{0,2})?|0(\.[0-9]{0,2})?|(\.[0-9]{1,2})?))\p{Currency_Symbol}")
            .unwrap();
    static ref RE_HASHTAG: Regex = Regex::new(r"\#([a-zA-Z][0-9a-zA-Z_]*)").unwrap();
    static ref RE_LIFETIME: Regex = Regex::new(r"(([1-9]{1}[0-9]*)([dwmy]))(([1-9]{1}[0-9]*)x)?").unwrap();
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

pub mod model {
    use anyhow::anyhow;
    use bigdecimal::{BigDecimal, FromPrimitive, ParseBigDecimalError};
    use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Utc};
    use slug::slugify;
    use std::collections::HashMap;
    use std::convert::TryFrom;
    use std::fmt;
    use std::iter::FromIterator;
    use std::str::FromStr;

    #[derive(Debug)]
    pub enum Lifetime {
        // amount, times
        SingleDay,
        Year { amount: i64, times: i64 },
        Month { amount: i64, times: i64 },
        Week { amount: i64, times: i64 },
        Day { amount: i64, times: i64 },
    }

    impl Lifetime {
        pub fn get_days(&self) -> i64 {
            match self {
                Self::Year { amount, times } => amount * 365 * times,
                Self::Month { amount, times } => amount * 30 * times, //approx
                Self::Week { amount, times } => amount * 7 * times,
                Self::Day { amount, times } => amount * times,
                Self::SingleDay => 1,
            }
        }

        /// Returns the number of days from a given date.
        ///
        /// This is only significant for months, that have a variable
        /// size and therefore is necessary to know the start date
        /// to calculate the number of days
        pub fn get_days_from(&self, begin: &NaiveDate) -> i64 {
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
                _ => self.get_days(),
            }
        }

        pub fn get_seconds(&self) -> i64 {
            self.get_days() * 86400
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
            let lifetime = super::extract_lifetime(s);
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
            self.get_seconds() == other.get_seconds()
        }
    }

    #[derive(Debug)]
    pub struct TxRecord {
        name: String,
        tags: HashMap<String, String>,
        amount: BigDecimal,
        starts_on: NaiveDate,
        lifetime: Lifetime, // in days
        recorded_at: DateTime<Local>,
        src: String,
    }

    impl TxRecord {
        // Getters
        pub fn get_name(&self) -> &str {
            &self.name[..]
        }
        pub fn get_tags(&self) -> Vec<String> {
            Vec::from_iter(self.tags.values().map(|v| String::from(v)))
        }
        pub fn get_amount(&self) -> &BigDecimal {
            &self.amount
        }
        pub fn get_duration(&self) -> &Lifetime {
            &self.lifetime
        }
        pub fn get_starts_on(&self) -> NaiveDate {
            self.starts_on
        }
        pub fn get_recorded_at(&self) -> &DateTime<Local> {
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
        pub fn get_duration_days(&self) -> BigDecimal {
            BigDecimal::from_i64(self.lifetime.get_days_from(&self.starts_on)).unwrap()
        }
        /// Calculates and returns the per diem for the record
        ///
        /// The per diem is calculated as follow:
        /// END_DAY = START_DAY + (RECURRENCE_SIZE_DAYS * SEC_IN_DAYS  * RECURRENCE_TIMES)
        /// PER_DIEM = AMOUNT * RECURRENCE_TIMES) / (END_DAY - START_DAY )
        ///
        pub fn per_diem(&self) -> BigDecimal {
            // TODO add inflation?
            (self.get_amount_total() / self.get_duration_days())
                .with_scale(100)
                .with_prec(2)
        }
        /// Returns the end date (always computed)
        pub fn get_ends_on(&self) -> NaiveDate {
            self.starts_on + Duration::days(self.lifetime.get_days_from(&self.starts_on))
        }

        pub fn new(name: &str, amount: &str) -> Result<TxRecord, anyhow::Error> {
            TxRecord::from(
                name,
                Vec::new(),
                amount,
                Utc::today().naive_utc(),
                Lifetime::SingleDay,
                Local::now(),
                &format!("{} {}", name, amount),
            )
        }

        pub fn from(
            name: &str,
            tags: Vec<&str>,
            amount: &str,
            starts_on: NaiveDate,
            lifetime: Lifetime,
            recorded_at: DateTime<Local>,
            src: &str,
        ) -> Result<TxRecord, anyhow::Error> {
            Ok(TxRecord {
                name: String::from(name.trim()),
                tags: tags
                    .iter()
                    .map(|v| (slugify(v), String::from(*v)))
                    .collect(),
                amount: BigDecimal::from_str(amount)?,
                lifetime: lifetime,
                recorded_at: recorded_at,
                starts_on: starts_on,
                src: String::from(src),
            })
        }

        pub fn from_str(s: &str) -> Result<TxRecord, anyhow::Error> {
            if s.len() == 0 {
                return Err(anyhow!("string is too short!"));
            }
            // make an empty record
            let mut name: Vec<&str> = Vec::new();
            let mut amount = "0";
            let mut lifetime = Lifetime::SingleDay;
            let mut tags: Vec<&str> = Vec::new();
            let mut starts_on = today();
            // search for the stuff we need
            for t in s.split_whitespace() {
                if super::RE_CURRENCY.is_match(&t) {
                    // read the currency
                    if let Some(a) = super::extract_amount(t) {
                        amount = a
                    }
                } else if super::RE_HASHTAG.is_match(&t) {
                    // add tags
                    if let Some(x) = super::extract_hashtag(&t) {
                        tags.push(x);
                    }
                } else if super::RE_LIFETIME.is_match(t) {
                    // add duration
                    lifetime = t.parse::<Lifetime>()?;
                } else if super::RE_DATE.is_match(t) {
                    starts_on = match super::extract_date(t) {
                        Some(d) => NaiveDate::parse_from_str(d, "%d%m%y")?,
                        None => today(),
                    }
                } else {
                    // catch all for the name
                    name.push(&t)
                }
            }

            TxRecord::from(
                &name.join(" "),
                tags,
                amount,
                starts_on,
                lifetime,
                Local::now(),
                s,
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

    pub fn today() -> NaiveDate {
        Utc::today().naive_utc()
    }

    pub fn now_local() -> DateTime<Local> {
        Local::now()
    }

    pub fn date(d: u32, m: u32, y: i32) -> NaiveDate {
        NaiveDate::from_ymd(y, m, d)
    }
}

#[cfg(test)]
mod tests {
    use super::model;
    use super::model::Lifetime;
    use super::model::TxRecord;

    #[test]
    fn test_parse_transaction() {
        let tests = vec![
            (
                "AKU Bellamont 3 Suede Low GTX 2020 #vestiti 129.95€ 3y",
                TxRecord::from(
                    "AKU Bellamont 3 Suede Low GTX 2020",
                    vec!["vestiti"],
                    "129.95",
                    model::today(),
                    Lifetime::Year {
                        amount: 3,
                        times: 1,
                    },
                    model::now_local(),
                    "AKU Bellamont 3 Suede Low GTX 2020 #vestiti 129.95€ 3y",
                )
                .unwrap(),
                model::parse_amount("0.12").unwrap(),
            ),
            (
                "Rent 729€ 1m12x 010120 #rent",
                TxRecord::from(
                    "Rent",
                    vec!["rent"],
                    "729",
                    model::date(1, 1, 2020),
                    Lifetime::Month {
                        amount: 1,
                        times: 12,
                    },
                    model::now_local(),
                    "-- not checked --",
                )
                .unwrap(),
                model::parse_amount("24").unwrap(),
            ),
            (
                "Tea 20€ 2m1x 010120 #food",
                TxRecord::from(
                    "Tea",
                    vec!["food"],
                    "20",
                    model::date(1, 1, 2020),
                    Lifetime::Month {
                        amount: 2,
                        times: 1,
                    },
                    model::now_local(),
                    "-- not checked --",
                )
                .unwrap(),
                model::parse_amount("0.34").unwrap(),
            ),
            (
                "Tea 20€ 1m2x 010120 #food",
                TxRecord::from(
                    "Tea",
                    vec!["food"],
                    "20",
                    model::date(1, 1, 2020),
                    Lifetime::Month {
                        amount: 1,
                        times: 2,
                    },
                    model::now_local(),
                    "-- not checked --",
                )
                .unwrap(),
                model::parse_amount("0.68").unwrap(),
            ),
        ];

        for (i, t) in tests.iter().enumerate() {
            println!("test_parse_tx_record#{}", i);
            assert_eq!(
                t.0.parse::<TxRecord>().expect("test_parse_tx_record error"),
                t.1
            );
            assert_eq!(t.1.per_diem(), t.2)
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
                7300,
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
            assert_eq!(t.1.get_days(), t.2)
        }
    }
}
