use ::costoflife::{self, TxRecord};
use bigdecimal::BigDecimal;
use blake3;
use chrono::NaiveDate;
use clap::{App, Arg};
use dialoguer::{theme::ColorfulTheme, Confirm};
use directories::ProjectDirs;
use std::collections::HashMap;
use std::error;
use std::fs;
use std::fs::File;
use std::io::{self, BufRead, LineWriter, Write};
use std::path::Path;

fn main() -> Result<(), Box<dyn error::Error>> {
    //println!("Welcome to CostOf.Life!");

    let matches = App::new("My Super Program")
        .version("1.0")
        .author("Kevin K. <kbknapp@gmail.com>")
        .about("Does awesome things")
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
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .about("suppress verbose logging"),
        )
        .arg(
            Arg::new("v")
                .short('v')
                .multiple(true)
                .about("Sets the level of verbosity"),
        )
        .arg(
            Arg::new("debug")
                .short('d')
                .about("print debug information verbosely"),
        )
        .subcommand(
            App::new("new")
                .about("add new expense")
                .version("1.3")
                .author("<prez@adgb.me>")
                .arg(
                    Arg::new("EXP_STR")
                        .about("write the expense string")
                        .required(true)
                        .multiple(true)
                        .value_terminator("."),
                ),
        )
        .get_matches();

    // first, see if there is the config dir
    let path = match ProjectDirs::from("com", "FarcastTo", "CostOf.Life") {
        Some(p) => {
            // println!("config dir is {:?}", p.config_dir());
            // println!("data   dir is {:?}", p.data_dir());

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
            p.data_dir().join(Path::new("cost.of.life.data.txt"))
        }
        None => panic!("cannot retrieve the config file dir"),
    };
    // load the datastores
    let mut ds = DataStore::new();
    ds.load(path.as_path())?;
    // command line
    if let Some(c) = matches.subcommand_matches("new") {
        if let Some(values) = c.values_of("EXP_STR") {
            let v = values.collect::<Vec<&str>>().join(" ");
            let tx = costoflife::TxRecord::from_str(&v).expect("Cannot parse the input string");
            tx.pretty_print();
            // save to the store
            match Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("Do you want to save it?")
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
        };
    }
    println!(
        "Today CostOf.Life is: {}€",
        ds.cost_of_life(&costoflife::today())
    );
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
            .iter()
            .filter(|(_k, v)| v.is_active_on(d))
            .map(|(_k, v)| {
                //println!("{} {}", v.get_name(), v.per_diem_raw());
                v.per_diem_raw()
            })
            .sum::<BigDecimal>()
            .with_scale(2)
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
