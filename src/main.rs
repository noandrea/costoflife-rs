use costoflife::model::TxRecord;

fn main() {
    println!("Welcome to CostOf.Life!");

    let r = "car 22000€ 10y #transport".parse::<TxRecord>().unwrap();

    println!("{}", r);
}
