use lazy_static::lazy_static;

use regex::Regex;

lazy_static! {
    static ref RE_CURRENCY: Regex =
        Regex::new(r"(([1-9]{1}[0-9]{0,2}(,[0-9]{3})*(\.[0-9]{0,2})?|[1-9]{1}[0-9]{0,}(\.[0-9]{0,2})?|0(\.[0-9]{0,2})?|(\.[0-9]{1,2})?))\p{Currency_Symbol}")
            .unwrap();
    static ref RE_HASHTAG: Regex = Regex::new(r"\#([a-zA-Z][0-9a-zA-Z_]*)").unwrap();
    static ref RE_LIFETIME: Regex = Regex::new(r"(([1-9]{1}[0-9]*)([dwy]))(([1-9]{1}[0-9]*)x)?").unwrap();
    static ref RE_DATE: Regex = Regex::new(r"([0-9]{6})").unwrap();
}

// ex "AKU Bellamont 3 Suede Low GTX 2020 #vestiti 129.95€ 3y"

fn extract_amount(input: &str) -> &str {
    RE_CURRENCY
        .captures(input)
        .and_then(|cap| cap.get(1).map(|m| m.as_str()))
        .unwrap()
}

fn extract_hashtag(text: &str) -> &str {
    RE_HASHTAG
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
    use bigdecimal::{BigDecimal, FromPrimitive};
    use chrono::{Date, DateTime, Duration, Local, TimeZone, Utc};
    use slug::slugify;
    use std::collections::HashMap;
    use std::fmt;
    use std::iter::FromIterator;
    use std::ops::Add;
    use std::str::FromStr;

    #[derive(Debug)]
    pub enum Lifetime {
        // amount, times
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

    #[derive(Debug, PartialEq)]
    pub struct TxRecord {
        name: String,
        tags: HashMap<String, String>,
        amount: BigDecimal,
        starts_on: i64,
        lifetime: Lifetime, // in days
        recorded_at: DateTime<Local>,
        // calculated
        per_diem: BigDecimal,
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
        pub fn get_per_diem(&self) -> &BigDecimal {
            &self.per_diem
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
        // end date is always computed
        pub fn get_end_date(&self) -> Date<Local> {
            let x = Utc.timestamp(self.starts_on, 0) + Duration::days(self.lifetime.get_days());
            Local.from_local_datetime(&x.naive_local()).unwrap().date()
        }
    }

    impl FromStr for TxRecord {
        type Err = anyhow::Error;
        fn from_str(s: &str) -> Result<TxRecord, anyhow::Error> {
            if s.len() == 0 {
                return Err(anyhow!("string is too short!"));
            }
            // local date time
            let now = Local::now();
            // utc time at begin of day
            let utc_sod = Utc::today().and_hms(0, 0, 0);

            let mut tr = TxRecord {
                name: String::from(""),
                tags: HashMap::new(),
                amount: BigDecimal::from(0),
                lifetime: Lifetime::Day {
                    amount: 1,
                    times: 1,
                },
                recorded_at: now,
                starts_on: utc_sod.timestamp(),
                per_diem: BigDecimal::from(0),
            };

            // search for the stuff we need
            for t in s.split_whitespace() {
                if super::RE_CURRENCY.is_match(&t) {
                    // read the currency
                    // TODO need to have a method to extract the amount
                    let a_str = super::extract_amount(t);
                    tr.amount = BigDecimal::from_str(a_str)?;
                } else if super::RE_HASHTAG.is_match(&t) {
                    // add tags
                    let l = super::extract_hashtag(&t);
                    tr.tags.insert(slugify(&l), String::from(l));
                } else if super::RE_LIFETIME.is_match(t) {
                    // add duration
                    tr.lifetime = t.parse::<Lifetime>()?;
                } else {
                    // catch all for the name
                    tr.name = tr.name.add(&t).add(" ")
                }
            }

            // now calculate the per diem amount with the following:
            // END_DAY = START_DAY + (RECURRENCE_SIZE_DAYS * SEC_IN_DAYS  * RECURRENCE_TIMES)
            // formula is (AMOUNT * RECURRENCE_TIMES) / (END_DAY - START_DAY )
            tr.per_diem = tr.get_amount_total() / tr.get_duration_days();
            Ok(tr)
        }
    }

    impl fmt::Display for TxRecord {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "({})", self.name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::model::Lifetime;

    #[test]
    fn test_parse_transaction() {
        assert_eq!(2 + 2, 4);
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
            ),
            (
                "10d1x",
                Lifetime::Day {
                    amount: 10,
                    times: 1,
                },
            ),
            (
                "10w10x",
                Lifetime::Week {
                    amount: 10,
                    times: 10,
                },
            ),
            (
                "1y20x",
                Lifetime::Year {
                    amount: 1,
                    times: 20,
                },
            ),
        ];

        for (i, t) in tests.iter().enumerate() {
            println!("test_parse_lifetime#{}", i);
            assert_eq!(
                t.0.parse::<Lifetime>().expect("test_parse_lifetime error"),
                t.1
            );
        }
    }
}
