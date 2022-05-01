mod ledger;
use ledger::DataStore;

mod interaction;

use std::fmt;

use clap::{Arg, Command};
use dialoguer::{theme::ColorfulTheme, Confirm};
use directories_next::ProjectDirs;
use pad::{Alignment, PadStr};

use std::error;
use std::fs;
use std::path::Path;

use Alignment::*;
use Cell::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DB_FILENAME: &str = "costoflife.data.txt";

fn main() -> Result<(), Box<dyn error::Error>> {
    //println!("Welcome to CostOf.Life!");

    let matches = Command::new("costoflife")
        .version(VERSION)
        .author("Andrea G. <no.andrea@gmail.com>")
        .about("keep track of the cost of your daily life")
        .after_help("visit https://thecostof.life for more info")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Sets a custom config file")
                .takes_value(true),
        )
        .arg(
            Arg::new("on_date")
                .short('o')
                .long("on")
                .value_name("DATE")
                .help("use this date to calculate the cost of life")
                .takes_value(true),
        )
        .subcommand(
            Command::new("add")
                .about("add new expense")
                .arg(
                    Arg::new("EXP_STR")
                        .help("write the expense string")
                        .required(true)
                        .multiple_occurrences(true)
                        .value_terminator("."),
                )
                .arg(
                    Arg::new("non_interactive")
                        .long("yes")
                        .short('y')
                        .takes_value(false)
                        .help("automatically reply yes"),
                ),
        )
        .subcommand(Command::new("summary").about("print th expenses summary"))
        .subcommand(Command::new("tags").about("print th expenses tags summary"))
        .subcommand(
            Command::new("search")
                .about("search for a transaction")
                .arg(
                    Arg::new("SEARCH_PATTERN")
                        .help("pattern to match for tags and/or tx name")
                        .required(true)
                        .multiple_occurrences(true)
                        .value_terminator("."),
                ),
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
                // print the transaction
                println!("Name     : {}", tx.get_name());
                println!("Tags     : {}", tx.get_tags().join(", "));
                print!("Amount   : {}", tx.get_amount());
                if !tx.amount_is_total() {
                    print!("(Total: {}€)", tx.get_amount_total());
                }
                println!("\nFrom - To: {} - {}", tx.get_starts_on(), tx.get_ends_on());
                println!("Per Diem : {}", tx.per_diem());
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
            } else {
                println!("Tell me what to add, eg: Car 2000€ .transport 5y")
            }
        }
        Some(("summary", _c)) => {
            let mut p = Printer::new(vec![27, 12, 9, 100]);
            // title
            p.head(vec!["Item", "Price", "Diem", "Progress"]);
            p.sep();

            // data
            ds.summary(&target_date)
                .iter()
                .for_each(|(itm, total, per_diem, prog)| {
                    // ⧚ ░ ◼ ▪ this are characters that can be used for the bar
                    p.row(vec![
                        Str(itm.to_string()),
                        Amt(*total),
                        Amt(*per_diem),
                        Pcent(*prog), // completion percentage
                    ]);
                });
            // separator
            p.sep();
            p.render();
        }
        Some(("tags", _c)) => {
            let mut p = Printer::new(vec![27, 12, 9, 100]);

            p.head(vec!["Title", "Count", "Diem", "%"]);
            p.sep();

            // total per diem
            let total = ds.cost_of_life(&target_date);
            // data
            ds.tags(&target_date).iter().for_each(|(tag, count, cost)| {
                p.row(vec![
                    Str(tag.to_string()),
                    Cnt(*count),
                    Amt(*cost),
                    Pcent(cost / total), // tag amount over total
                ]);
            });
            // separator
            p.sep();
            p.render();
        }
        Some(("search", c)) => {
            let mut p = Printer::new(vec![40, 12, 8, 11, 11, 30, 40]);

            if let Some(values) = c.values_of("SEARCH_PATTERN") {
                let pattern = values.collect::<Vec<&str>>().join(" ");
                // no results
                let res = ds.search(&pattern);
                if res.is_empty() {
                    println!("No matches found ¯\\_(ツ)_/¯");
                    return Ok(());
                }
                // with results
                p.head(vec!["Item", "Price", "Diem", "Start", "End", "Tags", "%"]);
                p.sep();
                // compute the total
                let mut totals = (0.0, 0.0);
                // data
                res.iter()
                    .for_each(|(itm, price, diem, s, e, pcent, tags)| {
                        p.row(vec![
                            Str(itm.to_string()),
                            Amt(*price),
                            Amt(*diem),
                            Str(s.to_string()),
                            Str(e.to_string()),
                            Str(tags.to_string()),
                            Pcent(*pcent),
                        ]);
                        totals = (totals.0 + price, totals.1 + diem);
                    });
                // separator
                p.sep();
                // print the total as well
                p.row(vec![
                    Empty,
                    Amt(totals.0),
                    Amt(totals.1),
                    Empty,
                    Empty,
                    Empty,
                    Empty,
                ]);
                p.render();
            }
        }
        Some((&_, _)) | None => {}
    }
    println!("Today CostOf.Life is: {}€", ds.cost_of_life(&target_date));
    Ok(())
}

#[derive(Debug)]
enum Cell {
    Amt(f32),    // amount
    Pcent(f32),  // percent
    Str(String), // string
    Cnt(usize),  // counter
    Empty,
    Sep,
}

#[derive(Debug)]
struct Printer {
    sizes: Vec<usize>,
    data: Vec<Vec<Cell>>,
    col_sep: String,
    row_sep: char,
    progress: char,
}

impl fmt::Display for Printer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.data
                .iter()
                .map(|row| {
                    row.iter()
                        .enumerate()
                        .map(|(i, c)| {
                            let s = self.sizes[i];
                            match c {
                                Str(v) => v.pad(s, ' ', Left, true),
                                Amt(v) => format!("{}€", v).pad(s, ' ', Right, false),
                                Cnt(v) => format!("{}", v).pad(s, ' ', Right, false),
                                Empty => "".pad(s, ' ', Right, false),
                                Pcent(v) => {
                                    let p = v * 100.0;
                                    let b = (p as usize * s) / 100; // bar length
                                    format!("{:.2}", p).pad(b, self.progress, Right, false)
                                }
                                Sep => "".pad(s, self.row_sep, Alignment::Right, false),
                            }
                        })
                        .collect::<Vec<String>>()
                        .join(&self.col_sep)
                })
                .collect::<Vec<String>>()
                .join("\n")
        )
    }
}

impl Printer {
    pub fn new(col_sizes: Vec<usize>) -> Printer {
        Printer {
            sizes: col_sizes,
            data: Vec::new(),
            row_sep: '-',
            progress: '▮',
            col_sep: "|".to_string(),
        }
    }

    pub fn row(&mut self, row_data: Vec<Cell>) {
        self.data.push(row_data);
    }

    pub fn head(&mut self, head_data: Vec<&str>) {
        self.row(head_data.iter().map(|v| Str(v.to_string())).collect());
    }

    pub fn sep(&mut self) {
        self.row(self.sizes.iter().map(|_| Sep).collect());
    }

    pub fn render(&self) {
        println!("{}", self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_printer() {
        let mut p = Printer::new(vec![5, 10, 10, 50]);
        p.head(vec!["a", "b", "c", "d"]);
        p.sep();
        p.row(vec![
            Str("One".to_string()),
            Amt(80.0),
            Cnt(100),
            Pcent(0.1043), // completion percentage
        ]);
        p.row(vec![
            Str("Two".to_string()),
            Amt(59.0),
            Cnt(321),
            Pcent(0.0420123123), // completion percentage
        ]);
        p.row(vec![
            Str("Three".to_string()),
            Amt(220.0),
            Cnt(11),
            Pcent(0.309312321), // completion percentage
        ]);
        p.sep();

        let printed =
            "a    |b         |c         |d                                                 
-----|----------|----------|--------------------------------------------------
One  |       80€|       100|10.43
Two  |       59€|       321|4.20
Three|      220€|        11|▮▮▮▮▮▮▮▮▮▮30.93
-----|----------|----------|--------------------------------------------------";

        assert_eq!(p.data.len(), 6);
        assert_eq!(p.to_string(), printed);
    }
}
