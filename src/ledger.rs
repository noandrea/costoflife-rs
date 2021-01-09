use ::costoflife::{self, TxRecord};
use bigdecimal::{BigDecimal, ToPrimitive};
use chrono::NaiveDate;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, LineWriter, Write};
use std::path::Path;

/// A simple datastore that can persist data on file
///
#[derive(Debug)]
pub struct DataStore {
    data: HashMap<blake3::Hash, TxRecord>,
}
impl DataStore {
    /// Initialize an empty datastore
    ///
    pub fn new() -> DataStore {
        DataStore {
            data: HashMap::new(),
        }
    }
    /// Load the datastore with the records found
    /// at log_file path
    pub fn load(&mut self, log_file: &Path) -> Result<(), std::io::Error> {
        // read path
        if let Ok(lines) = DataStore::read_lines(log_file) {
            for line in lines {
                let record = line?;
                if let Ok(tx) = TxRecord::from_string_record(&record) {
                    self.data.insert(Self::hash(&tx), tx);
                }
            }
        }
        Ok(())
    }
    /// Persist the datastore to disk, overwriting existing files
    ///
    /// The order of the item saved is random
    pub fn save(&self, log_file: &Path) -> Result<(), std::io::Error> {
        let mut file = LineWriter::new(File::create(log_file)?);
        self.data.iter().for_each(|v| {
            file.write(v.1.to_string_record().as_bytes()).ok();
        });
        file.flush()?;
        Ok(())
    }
    /// Retrieve the cost of life for a date
    ///
    pub fn cost_of_life(&self, d: &NaiveDate) -> f32 {
        costoflife::cost_of_life(self.data.values(), d)
            .to_f32()
            .unwrap()
    }
    /// Compile a summary of the active costs, returning a tuple with
    /// (title, total amount, cost per day, percentage payed)
    pub fn summary(&self, d: &NaiveDate) -> Vec<(String, f32, f32, f32)> {
        let mut s = self
            .data
            .iter()
            .filter(|(_k, v)| v.is_active_on(d))
            .map(|(_k, v)| {
                (
                    String::from(v.get_name()),
                    v.get_amount_total().to_f32().unwrap(),
                    v.per_diem().to_f32().unwrap(),
                    v.get_progress(&Some(*d)),
                )
            })
            .collect::<Vec<(String, f32, f32, f32)>>();
        // sort the results descending by completion
        s.sort_by(|a, b| (b.3).partial_cmp(&a.3).unwrap());
        s
    }
    /// Return aggregation summary for tags
    ///
    pub fn tags(&self, d: &NaiveDate) -> Vec<(String, usize, f32)> {
        // counters here
        let mut agg: HashMap<String, (usize, BigDecimal)> = HashMap::new();
        // aggregate tags
        self.data
            .iter()
            .filter(|(_h, tx)| tx.is_active_on(d))
            .for_each(|(_h, tx)| {
                tx.get_tags().iter().for_each(|tg| {
                    let (n, a) = match agg.get(tg) {
                        Some((n, a)) => (n + 1, a + tx.per_diem()),
                        None => (1, tx.per_diem()),
                    };
                    agg.insert(tg.to_string(), (n, a));
                    // * agg.entry(*tg).or_insert((1, tx.per_diem())) +=(1, tx.per_diem());
                });
            });
        // return
        let mut s = agg
            .iter()
            .map(|(tag, v)| (tag.to_string(), v.0, v.1.clone().to_f32().unwrap()))
            .collect::<Vec<(String, usize, f32)>>();
        // sort the results descending by count
        s.sort_by(|a, b| (b.2).partial_cmp(&a.2).unwrap());
        s
    }
    /// Insert a new tx record
    /// if the record exists returns the existing one
    ///
    /// TODO: handle duplicates more gracefully
    pub fn insert(&mut self, tx: &TxRecord) -> Option<TxRecord> {
        self.data.insert(Self::hash(tx), tx.clone())
    }
    /// Get the size of the datastore
    ///
    /// # Arguments
    ///
    /// * `on` - A Option<chrono:NaiveDate> to filter for active transactions
    ///
    /// if the Option is None then the full size is returned
    ///
    pub fn size(&self, on: Option<NaiveDate>) -> usize {
        match on {
            Some(date) => self.summary(&date).len(),
            None => self.data.len(),
        }
    }
    // The output is wrapped in a Result to allow matching on errors
    // Returns an Iterator to the Reader of the lines of the file.
    fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
    where
        P: AsRef<Path>,
    {
        let file = File::open(filename)?;
        Ok(io::BufReader::new(file).lines())
    }
    /// Compute the blake3 has for a TxRecord
    ///
    /// The hash is calculated on
    /// - name
    /// - lifetime
    /// - starts_on
    /// - amount
    ///
    fn hash(tx: &TxRecord) -> blake3::Hash {
        let fields = format!(
            "{}:{}:{}:{}",
            tx.get_name(),
            tx.get_amount(),
            tx.get_lifetime(),
            tx.get_starts_on(),
        );
        blake3::hash(fields.as_bytes())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ::costoflife::{self, TxRecord};
    #[test]
    fn test_datastore() {
        let mut ds = DataStore::new();
        // insert one entry
        ds.insert(&TxRecord::new("Test#1", "10").unwrap());
        ds.insert(&TxRecord::new("Test#2", "10").unwrap());
        // simple insert
        assert_eq!(ds.cost_of_life(&costoflife::today()), 20.0);
        // summary test
        let summary = ds.summary(&costoflife::today());
        assert_eq!(summary.len(), 2);
        // test tags
        let mut ds = DataStore::new();
        // insert one entry
        ds.insert(&TxRecord::from_str("Test#1 10€ #tag1").unwrap());
        ds.insert(&TxRecord::from_str("Test#2 20€ #tag2").unwrap());
        ds.insert(&TxRecord::from_str("Test#3 50€ #tag3").unwrap());
        ds.insert(&TxRecord::from_str("Test#4 40€ #tag2").unwrap());
        let tags = ds.tags(&costoflife::today());
        assert_eq!(tags.len(), 3);
        // tag2
        let got = &tags[0];
        let exp = (String::from("tag2"), 2 as usize, 60.0);
        assert_eq!(*got, exp);
        // tag3
        let got = &tags[1];
        let exp = (String::from("tag3"), 1 as usize, 50.0);
        assert_eq!(*got, exp);
        // test load
        let mut ds = DataStore::new();
        // db path
        let p = Path::new("./testdata/costoflife.data.txt");
        // load
        let r = ds.load(p);
        assert_eq!(r.is_err(), false);
        assert_eq!(ds.size(None), 5 as usize);
    }
}
