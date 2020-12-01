use lazy_static::lazy_static;

use regex::Regex;

lazy_static! {
    static ref RE_CURRENCY: Regex =
        Regex::new(r"(([1-9]{1}[0-9]{0,2}(,[0-9]{3})*(\.[0-9]{0,2})?|[1-9]{1}[0-9]{0,}(\.[0-9]{0,2})?|0(\.[0-9]{0,2})?|(\.[0-9]{1,2})?))\p{Currency_Symbol}")
            .unwrap();
    static ref RE_HASHTAG: Regex = Regex::new(r"\#([a-zA-Z][0-9a-zA-Z_]*)").unwrap();
    static ref RE_LIFETIME: Regex = Regex::new(r"(([1-9]{1}[0-9]*)([dwy]))(([1-9]{1}[0-9]*)x)?").unwrap();
    static ref RE_DATE: Regex = Regex::new(r"([0-3][0-9][0-1][0-9][1-9][0-9])").unwrap();
}

// ex "AKU Bellamont 3 Suede Low GTX 2020 #vestiti 129.95€ 3y"

fn extract_amount(input: &str) -> &str {
    RE_CURRENCY
        .captures(input)
        .and_then(|c| c.get(1).map(|m| m.as_str()))
        .unwrap()
}

fn extract_hashtag(text: &str) -> &str {
    RE_HASHTAG
        .captures(text)
        .and_then(|c| c.get(1).map(|m| m.as_str()))
        .unwrap()
}

fn extract_date(text: &str) -> &str {
    RE_DATE
        .captures(text)
        .and_then(|c| c.get(1).map(|m| m.as_str()))
        .unwrap()
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
    pub use bigdecimal::{BigDecimal, FromPrimitive, ParseBigDecimalError};
    pub use chrono::{Date, DateTime, Duration, Local, NaiveDate, TimeZone, Utc};
    use slug::slugify;
    use std::collections::HashMap;
    use std::fmt;
    use std::iter::FromIterator;
    use std::str::FromStr;

    #[derive(Debug)]
    pub enum Lifetime {
        // amount, times
        SingleDay,
        Year { amount: i64, times: i64 },
        Week { amount: i64, times: i64 },
        Day { amount: i64, times: i64 },
    }

    impl Lifetime {
        pub fn get_days(&self) -> i64 {
            match self {
                Self::Year { amount, times } => amount * 365 * times,
                Self::Week { amount, times } => amount * 7 * times,
                Self::Day { amount, times } => amount * times,
                Self::SingleDay => 1,
            }
        }

        pub fn get_seconds(&self) -> i64 {
            self.get_days() * 86400
        }

        // calculate the amount
        pub fn get_amount(&self, base: &BigDecimal) -> BigDecimal {
            match self {
                Self::Year { times, .. } => BigDecimal::from_i64(*times).unwrap() * base,
                Self::Week { times, .. } => BigDecimal::from_i64(*times).unwrap() * base,
                Self::Day { times, .. } => BigDecimal::from_i64(*times).unwrap() * base,
                Self::SingleDay => BigDecimal::from(1) * base,
            }
        }
    }

    impl FromStr for Lifetime {
        type Err = anyhow::Error;
        fn from_str(s: &str) -> Result<Lifetime, anyhow::Error> {
            let l = super::extract_lifetime(s);
            match l.0 {
                "d" => Ok(Lifetime::Day {
                    amount: l.1,
                    times: l.2,
                }),
                "w" => Ok(Lifetime::Week {
                    amount: l.1,
                    times: l.2,
                }),
                "y" => Ok(Lifetime::Year {
                    amount: l.1,
                    times: l.2,
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
        starts_on: i64,
        lifetime: Lifetime, // in days
        recorded_at: DateTime<Local>,
        src: String,
    }

    impl TxRecord {
        // Getters
        pub fn get_name(&self) -> &str {
            &self.name[..]
        }
        pub fn get_tags(&self) -> Vec<&String> {
            Vec::from_iter(self.tags.values())
        }
        pub fn get_amount(&self) -> &BigDecimal {
            &self.amount
        }
        pub fn get_duration(&self) -> &Lifetime {
            &self.lifetime
        }
        pub fn get_starts_on(&self) -> i64 {
            self.starts_on
        }
        pub fn get_recorded_at(&self) -> &DateTime<Local> {
            &self.recorded_at
        }
        pub fn get_recorded_at_rfc3339(&self) -> String {
            self.recorded_at.to_rfc3339()
        }
        // tells if the TxRecord as a tag
        pub fn has_tag(&self, tag: &str) -> bool {
            self.tags.contains_key(&slugify(&tag))
        }
        // get total amount for the transaction records
        pub fn get_amount_total(&self) -> BigDecimal {
            self.lifetime.get_amount(&self.amount)
        }
        // return the duration in days for this transaction
        pub fn get_duration_days(&self) -> BigDecimal {
            BigDecimal::from_i64(self.lifetime.get_days()).unwrap()
        }
        // calculate the per_diem
        pub fn per_diem(&self) -> BigDecimal {
            // TODO add inflation?
            // now calculate the per diem amount with the following:
            // END_DAY = START_DAY + (RECURRENCE_SIZE_DAYS * SEC_IN_DAYS  * RECURRENCE_TIMES)
            // formula is (AMOUNT * RECURRENCE_TIMES) / (END_DAY - START_DAY )
            (self.get_amount_total() / self.get_duration_days())
                .with_scale(100)
                .with_prec(2)
        }
        // end date is always computed
        pub fn get_end_date(&self) -> Date<Local> {
            let x = Utc.timestamp(self.starts_on, 0) + Duration::days(self.lifetime.get_days());
            Local.from_local_datetime(&x.naive_local()).unwrap().date()
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
                starts_on: starts_on.and_hms(0, 0, 0).timestamp(),
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
            let mut starts_on = Utc::today().naive_utc();
            // search for the stuff we need
            for t in s.split_whitespace() {
                if super::RE_CURRENCY.is_match(&t) {
                    // read the currency
                    amount = super::extract_amount(t);
                } else if super::RE_HASHTAG.is_match(&t) {
                    // add tags
                    tags.push(super::extract_hashtag(&t));
                } else if super::RE_LIFETIME.is_match(t) {
                    // add duration
                    lifetime = t.parse::<Lifetime>()?;
                } else if super::RE_DATE.is_match(t) {
                    starts_on = NaiveDate::parse_from_str(super::extract_date(t), "%d%m%y")?;
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
                "Rent 729€ 1y #rent",
                TxRecord::from(
                    "Rent",
                    vec!["rent"],
                    "729",
                    model::today(),
                    Lifetime::Year {
                        amount: 1,
                        times: 1,
                    },
                    model::now_local(),
                    "Rent 729€ 1y #rent",
                )
                .unwrap(),
                model::parse_amount("23.97").unwrap(),
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
                70,
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
