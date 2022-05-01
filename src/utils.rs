use bigdecimal::BigDecimal;
use chrono::{DateTime, FixedOffset, Local, NaiveDate};
use std::str::FromStr;

pub fn parse_amount(s: &str) -> Option<BigDecimal> {
    BigDecimal::from_str(s).ok()
}

/// Returns the current date
pub fn today() -> NaiveDate {
    Local::today().naive_utc()
}

/// Returns the datetime with the local timezone
pub fn now_local() -> DateTime<FixedOffset> {
    DateTime::from(Local::now())
}

/// Builds a date from day/month/year numeric
///
/// # Examples
///
/// ```
/// use costoflife;
///
/// let d = costoflife::date(1, 1, 2001); // 2001-01-01
///
/// ```
pub fn date(d: u32, m: u32, y: i32) -> NaiveDate {
    NaiveDate::from_ymd(y, m, d)
}

/// Parse a date from string, it recognizes the formats
///
/// - dd/mm/yyyy
/// - dd.mm.yyyy
/// - ddmmyy
/// - dd.mm.yy
/// - dd/mm/yy
///
pub fn date_from_str(s: &str) -> Option<NaiveDate> {
    let formats = vec!["%d%m%y", "%d.%m.%y", "%d/%m/%y", "%d/%m/%Y", "%d.%m.%Y"];
    // check all the formats
    for f in formats {
        let r = NaiveDate::parse_from_str(s, f);
        if r.is_ok() {
            return r.ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsers() {
        // parse date
        let r = date_from_str("27/12/2020");
        assert_eq!(r.unwrap(), date(27, 12, 2020));
        // invalid date
        let r = date_from_str("30/02/2020");
        assert_eq!(r, None);
        // invalid format
        let r = date_from_str("30/02/20");
        assert_eq!(r, None);
        // dd.mm.yy
        let r = date_from_str("30.01.20");
        assert_eq!(r.unwrap(), date(30, 1, 2020));
        // dd/mm/yy
        let r = date_from_str("30/01/20");
        assert_eq!(r.unwrap(), date(30, 1, 2020));
        // dd.mm.yyyy
        let r = date_from_str("30/01/2020");
        assert_eq!(r.unwrap(), date(30, 1, 2020));
    }
}
