#![feature(str_split_whitespace_remainder)]

mod uci;
mod search;

fn main()
{
    println!("Hello, world!");
    let mut uci = uci::UciClient::new();
    uci.main_loop();
}
