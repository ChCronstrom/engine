#![feature(str_split_whitespace_remainder)]

mod evaluation;
mod uci;
mod score;
mod search;
mod searchinterface;

fn main()
{
    println!("Hello, world!");
    let mut uci = uci::UciClient::new();
    uci.main_loop();
}
