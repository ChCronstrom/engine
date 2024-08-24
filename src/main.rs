#![feature(str_split_whitespace_remainder)]

mod evaluation;
mod hash;
mod score;
mod search;
mod searchinterface;
mod uci;

fn main()
{
    println!("Hello, world!");
    let mut uci = uci::UciClient::new();
    uci.main_loop();
}
