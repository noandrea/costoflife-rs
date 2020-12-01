use clap::{App, Arg};
use costoflife::model::TxRecord;

fn main() {
    println!("Welcome to CostOf.Life!");

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
                        .index(1),
                ),
        )
        .get_matches();

    if let Some(c) = matches.subcommand_matches("new") {
        if let Some(tx) = c.value_of("EXP_STR") {
            let x = TxRecord::from_str(tx).expect("Cannot parse the input string");
            println!("Name     : {} #[{}]", x.get_name(), x.get_tags().join(","));
            match x.amount_is_total() {
                true => println!("Amount   : {}", x.get_amount()),
                _ => {
                    println!(
                        "Amount   : {} (Total: {})",
                        x.get_amount(),
                        x.get_amount_total()
                    )
                }
            }
            println!("From / To: {} / {}", x.get_starts_on(), x.get_ends_on());
            println!("Per Diem : {}", x.per_diem());
        };
    }
}
