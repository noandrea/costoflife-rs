# CostOf.Life

[![Crates.io](https://img.shields.io/crates/v/costoflife)](https://crates.io/crates/costoflife)
[![Coverage Status](https://coveralls.io/repos/github/noandrea/costoflife-rs/badge.svg?branch=master)](https://coveralls.io/github/noandrea/costoflife-rs?branch=master)

This is the CostOf.Life library and command line client.

For a more general description about the project check website: [thecostof.life](https://thecostof.life) 


## Installation 

install it with `cargo install costoflife`


## Examples 

To add a single transaction use the command `costoflife add`:

```
|> costoflife add Netflix 7.99€ 100320 1m12x .movies .covid
```

See it in action:

[![asciicast](https://asciinema.org/a/382419.svg)](https://asciinema.org/a/382419)


## Transactions

The library interpret transactions and calculate for each transaction its _per diem_ amount. For a longer explanation check the [CostOf.life](https://thecostof.life) website.

A transaction as the properties:
- Name: a descriptive name
- Amount: the amount in monetary currency
- Lifetime: a duration that the transaction applies to
- Start date: the start date since when the lifetime should be computed
- Tags: for grouping transactions

As a simple interface the library interprets strings into transactions, the format of the string is shown in the examples and the details are listed below.
### Parsing rules 

The library tokenize the input string and looks for the patterns listed below. Anything that cannot be recognized as a pattern it will set as the title of the transaction. The title is **required**

#### Amount 

The monetary value of the transaction, **required**:

```EBNF
Amount ::= Natural ( '.' Digit Digit? )? "€" 

Natural ::= NaturalDigit Digit*
NaturalDigit ::= #'[1-9]'
Digit ::= "0" | NaturalDigit 
```

Currently the only available currency is `€`

Examples:
- `10€`
- `10000.99€`


#### Lifetime

The duration of transaction, optional, defaults to `1d`.
    
```EBNF
Lifetime ::= Duration Repeat?

Duration ::= Natural TimeUnit
Repeat ::= Natural "x"
TimeUnit ::= "d" | "w" | "m" | "y"
``` 

where the `TimeUnit` is:
- `d` days
- `w` weeks
- `m` months
- `y` years 

Examples:
- `1m12x` => one month for 12 times, for example for monthly expenses like monthly subscriptions (Netflix, etc)
- `12m` => twelve months for 1 time, same as `1y`
- `1w52x` => one week 52 times, for example weekly groceries expenses for all the year

> 💡 the number of repeats they influence the total amount of the transaction: `10€ 1m12x` will result of a transaction of total amount of `120€` while `12m1x` will result in a single transaction of `10€` over 12 months 


#### Start date

The transaction start date, optional, defaults to the current date, it uses the little endian format (day, month, year).

```EBNF
Date ::= Day Month Year

Month ::= "1" #'[0-2]' | "0" NaturalDigit
Day ::= '0' #'[1-9]' | #'[1-2]' Digit | '3' #'[0-1]'
Year ::= Digit Digit

Natural ::= NaturalDigit Digit*
NaturalDigit ::= #'[1-9]'
Digit ::= "0" | NaturalDigit 
```

Examples:
- `030521` => March the 3rd, 2021
- `312122` => December the 31st, 2022

#### Tags

To label transactions, optional. For convenience it uses the hashtag format.

```EBNF
HashTag ::=  ('#' | '.')  Word

EOL ::= '\r'? '\n' 
Word ::= AlphaNum+ [ (' ' | '\t')+ | EOL ]
AlphaNum  ::= #'[A-Za-z0-9_-]'
```

Examples:
- `#lifestile` 
- `.whatever`

## Appendix

Here the full grammar

```EBNF

Tx ::= ( Amount | Lifetime | StartDate | HashTag ) ( SEP ( Amount | Lifetime | StartDate | HashTag )  )+ EOL

EOL ::= '\r'? '\n' 
SEP ::= (' ' | '\t')+

Lifetime ::= Duration Repeat?

Duration ::= Natural TimeUnit
Repeat ::= Natural "x"
TimeUnit ::= "d" | "w" | "m" | "y"

StartDate ::= Day Month Year

Month ::= "1" #'[0-2]' | "0" NaturalDigit
Day ::= '0' #'[1-9]' | #'[1-2]' Digit | '3' #'[0-1]'
Year ::= Digit Digit

Amount ::= Natural ( '.' Digit Digit? )? "€" 

Natural ::= NaturalDigit Digit*
NaturalDigit ::= #'[1-9]'
Digit ::= "0" | NaturalDigit 
Int ::= "+" | "-" Digit+

HashTag ::=  ('#' | '.')  Word

Word ::= AlphaNum+
AlphaNum  ::= #'[A-Za-z0-9_-]'
```

---
Made by [adgb](https://adgb.me)



