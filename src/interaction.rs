use ::costoflife::{parse_amount, today, CostOfLifeError, TxRecord};
use dialoguer::console::Term;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Password, Select};

use Feat::*;
use PolarAnswer::*;

#[derive(PartialEq)]
pub enum PolarAnswer {
    Yes,
    No,
}

impl PolarAnswer {
    pub fn to_bool(&self) -> bool {
        match self {
            Self::Yes => true,
            Self::No => false,
        }
    }

    pub fn from_bool(v: bool) -> PolarAnswer {
        match v {
            true => Self::Yes,
            false => Self::No,
        }
    }
}

#[derive(PartialEq)]
pub enum Feat {
    NonEmpty,
    Empty,
}

impl Feat {
    fn to_bool(&self) -> bool {
        match self {
            Self::NonEmpty => false,
            Self::Empty => true,
        }
    }
}

/// shortcut for Confirm
pub fn confirm(q: &str, def: PolarAnswer) -> PolarAnswer {
    PolarAnswer::from_bool(
        Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(q)
            .default(def.to_bool())
            .interact()
            .unwrap(),
    )
}

/// shortcut for Input
pub fn input(q: &str, empty: Feat) -> String {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt(q)
        .allow_empty(empty.to_bool())
        .interact()
        .unwrap()
}

/// shortcut for Select optional input
pub fn select_opt<'a, T: ?Sized>(q: &str, opts: Vec<(&'a str, &'a T)>) -> Option<&'a T> {
    match Select::with_theme(&ColorfulTheme::default())
        .with_prompt(q)
        .items(
            &opts
                .iter()
                .map(|(l, _v)| l.to_string())
                .collect::<Vec<String>>(),
        )
        .default(0)
        .interact_on_opt(&Term::stdout())
        .unwrap()
    {
        Some(i) => Some(opts[i].1),
        _ => None,
    }
}

/// Select an item
pub fn select<'a, T: ?Sized>(q: &str, opts: Vec<(&'a str, &'a T)>) -> &'a T {
    opts[Select::with_theme(&ColorfulTheme::default())
        .with_prompt(q)
        .items(
            &opts
                .iter()
                .map(|(l, _v)| l.to_string())
                .collect::<Vec<String>>(),
        )
        .default(0)
        .interact_on(&Term::stdout())
        .unwrap()]
    .1
}

/// Show the options
pub fn menu() -> Option<String> {
    // ask for the quality
    match select_opt(
        "hello there, what's up? esc/q to quit",
        vec![
            ("Summary", "summary"),
            ("Tags", "agenda"),
            ("New Tx", "today"),
        ],
    ) {
        Some(x) => Some(x.to_string()),
        _ => None,
    }
}

pub fn new_tx() -> Result<TxRecord, CostOfLifeError> {
    let name = input("What it is it about?", NonEmpty);
    // amount
    let mut amount = parse_amount("0.0").unwrap();
    loop {
        let v = input("how much does it cost?", NonEmpty);
        amount = match parse_amount(&v) {
            Some(a) => a,
            None => continue,
        }
    }
    // tags
    let mut tags: Vec<String> = Vec::new();
    while Yes == confirm("add a tag?", No) {
        tags.push(input("tag label: ", NonEmpty));
    }
    let starts_on = today();

    //
    //TxRecord::from(name, tags, amount, starts_on, Lifetime::SingleDay, )
    Err(CostOfLifeError::GenericError("Not implemented".to_owned()))
}
