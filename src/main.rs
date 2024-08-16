use std::io;

mod uci;

fn main()
{
    println!("Hello, world!");
    let stdin = io::stdin().lock();
    let stdout = io::stdout().lock();
    let mut uci = uci::UciClient::new(stdin, stdout);
    uci.main_loop().expect("IO error");
}
