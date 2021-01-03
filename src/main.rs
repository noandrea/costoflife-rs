use ::costoflife::{self, TxRecord};
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use clap::{App, Arg};
use dialoguer::{theme::ColorfulTheme, Confirm};
use directories::ProjectDirs;
use pad::{Alignment, PadStr};
use std::collections::HashMap;
use std::error;
use std::fs;
use std::fs::File;
use std::io::{self, BufRead, LineWriter, Write};
use std::path::Path;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DB_FILENAME: &str = "costoflife.data.txt";

fn main() -> Result<(), Box<dyn error::Error>> {
    //println!("Welcome to CostOf.Life!");

    let matches = App::new("costoflife")
        .version(VERSION)
        .author("Andrea G. <no.andrea@gmail.com>")
        .about("keep track of the cost of your daily life")
        .after_help("visit https://thecostof.life for more info")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .about("Sets a custom config file")
                .takes_value(true),
        )
        .arg(
            Arg::new("on_date")
                .short('o')
                .long("on")
                .value_name("DATE")
                .about("use this date to calculate the cost of life")
                .takes_value(true),
        )
        .subcommand(
            App::new("add")
                .about("add new expense")
                .author("<prez@adgb.me>")
                .arg(
                    Arg::new("EXP_STR")
                        .about("write the expense string")
                        .required(true)
                        .multiple(true)
                        .value_terminator("."),
                )
                .arg(
                    Arg::new("non_interactive")
                        .long("yes")
                        .short('y')
                        .takes_value(false)
                        .about("automatically reply yes"),
                ),
        )
        .subcommand(
            App::new("summary")
                .about("print th expenses summary")
                .author("<write@adgb.me>"),
        )
        .get_matches();

    // first, see if there is the config dir
    let path = match ProjectDirs::from("com", "FarcastTo", "CostOf.Life") {
        Some(p) => {
            if !p.data_dir().exists() {
                let authorized = Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("The CostOf.Life data dir does not exists, can I create it?")
                    .default(true)
                    .interact()
                    .unwrap();
                if !authorized {
                    println!("nevermind then :(");
                    return Ok(());
                }
                match fs::create_dir_all(p.data_dir()) {
                    Ok(_) => println!("data folder created at {:?}", p.data_dir()),
                    Err(e) => {
                        println!("error creating folder {:?}: {}", p.data_dir(), e);
                        panic!()
                    }
                }
            }
            p.data_dir().join(Path::new(DB_FILENAME))
        }
        None => panic!("cannot retrieve the config file dir"),
    };
    // load the datastores
    let mut ds = DataStore::new();
    ds.load(path.as_path())?;
    // get the date
    let target_date = match matches.value_of("on_date") {
        Some(v) => costoflife::date_from_str(v).expect("The date provided is not valid"),
        None => costoflife::today(),
    };
    // command line
    match matches.subcommand() {
        Some(("add", c)) => {
            if let Some(values) = c.values_of("EXP_STR") {
                let v = values.collect::<Vec<&str>>().join(" ");
                let tx = costoflife::TxRecord::from_str(&v).expect("Cannot parse the input string");
                // check the values for
                if c.is_present("non_interactive") {
                    ds.insert(&tx);
                    ds.save(path.as_path())?;
                    println!("done!");
                    return Ok(());
                }

                pretty_print(&tx);
                // save to the store
                match Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Do you want to add it?")
                    .default(true)
                    .interact()
                {
                    Ok(true) => {
                        ds.insert(&tx);
                        ds.save(path.as_path())?;
                        println!("done!")
                    }
                    _ => println!("ok, another time"),
                }
            }
        }
        Some(("summary", _c)) => {
            let sizes = (27, 12, 9, 100);
            // title
            println!(
                "{}|{}|{}|{}",
                "Item".pad(sizes.0, ' ', Alignment::Left, false),
                "Price".pad(sizes.1, ' ', Alignment::Left, false),
                "Diem".pad(sizes.2, ' ', Alignment::Left, false),
                "Progress".pad(sizes.3, ' ', Alignment::Left, false),
            );
            // separator
            println!(
                "{}|{}|{}|{}",
                "".pad(sizes.0, '-', Alignment::Right, false),
                "".pad(sizes.1, '-', Alignment::Right, false),
                "".pad(sizes.2, '-', Alignment::Right, false),
                "".pad(sizes.3, '-', Alignment::Right, false),
            );
            // data
            ds.summary(&target_date).iter().for_each(|v| {
                // ⧚ ░ ◼ ▪ this are characters that can be used for the bar
                let perc = v.3 * 100.0; // this is the percentage of completion
                println!(
                    "{}|{}|{}|{}",
                    v.0.pad(sizes.0, ' ', Alignment::Left, true),
                    format!("{}€", v.1).pad(sizes.1, ' ', Alignment::Right, false),
                    format!("{}€", v.2).pad(sizes.2, ' ', Alignment::Right, false),
                    format!("{:.2}", perc).pad(perc as usize, '▮', Alignment::Right, false),
                )
            });
            // separator
            println!(
                "{}|{}|{}|{}",
                "".pad(sizes.0, '-', Alignment::Right, false),
                "".pad(sizes.1, '-', Alignment::Right, false),
                "".pad(sizes.2, '-', Alignment::Right, false),
                "".pad(sizes.3, '-', Alignment::Right, false),
            );
        }
        Some((&_, _)) | None => {}
    }

    println!("Today CostOf.Life is: {}€", ds.cost_of_life(&target_date));
    Ok(())
}

#[derive(Debug)]
struct DataStore {
    data: HashMap<blake3::Hash, TxRecord>,
}

impl DataStore {
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

    pub fn save(&self, log_file: &Path) -> Result<(), std::io::Error> {
        let mut file = LineWriter::new(File::create(log_file)?);
        self.data.iter().for_each(|v| {
            file.write(v.1.to_string_record().as_bytes()).ok();
        });
        file.flush()?;
        Ok(())
    }

    fn cost_of_life(&self, d: &NaiveDate) -> BigDecimal {
        self.data
            .iter() // loop through data
            .filter(|(_k, v)| v.is_active_on(d)) // is still an active expense
            .map(|(_k, v)| {
                //println!("{} {}", v.get_name(), v.per_diem_raw());
                v.per_diem_raw() // get the amount
            })
            .sum::<BigDecimal>() // sum all the amount
            .with_scale(2) // apply the scale
    }

    /// compile a summary of the active costs, returning a tuple with
    /// (title, total amount, cost per day, percentage payed)
    fn summary(&self, d: &NaiveDate) -> Vec<(String, BigDecimal, BigDecimal, f64)> {
        let mut s = self
            .data
            .iter()
            .filter(|(_k, v)| v.is_active_on(d))
            .map(|(_k, v)| {
                (
                    String::from(v.get_name()),
                    v.get_amount_total(),
                    v.per_diem(),
                    v.get_progress(&Some(*d)),
                )
            })
            .collect::<Vec<(String, BigDecimal, BigDecimal, f64)>>();
        // sort the results descending by completion
        s.sort_by(|a, b| (b.3).partial_cmp(&a.3).unwrap());
        s
    }

    /// Insert a new tx record
    /// if the record exists returns the existing one
    fn insert(&mut self, tx: &TxRecord) -> Option<TxRecord> {
        self.data.insert(Self::hash(tx), tx.clone())
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

/// Pretty print to stdout a transaction
fn pretty_print(tx: &TxRecord) {
    println!("Name     : {}", tx.get_name());
    println!("Tags     : {}", tx.get_tag_list().join(", "));
    print!("Amount   : {}", tx.get_amount());
    if !tx.amount_is_total() {
        print!("(Total: {}€)", tx.get_amount_total());
    }
    println!("\nFrom - To: {} - {}", tx.get_starts_on(), tx.get_ends_on());
    println!("Per Diem : {}", tx.per_diem());
}

#[cfg(test)]
mod tests {
    use super::DataStore;
    use ::costoflife::{self, TxRecord};

    #[test]
    fn test_datastore() {
        let mut ds = DataStore::new();
        // insert one entry
        ds.insert(&TxRecord::new("Test#1", "10").unwrap());
        ds.insert(&TxRecord::new("Test#2", "10").unwrap());
        // simple insert
        assert_eq!(
            ds.cost_of_life(&costoflife::today()),
            costoflife::parse_amount("20").unwrap()
        );
        // summary test
        let summary = ds.summary(&costoflife::today());
        assert_eq!(summary.len(), 2);
    }
}
