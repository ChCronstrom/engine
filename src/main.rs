mod uci;

fn main()
{
    println!("Hello, world!");
    let mut uci = uci::UciClient::new();
    uci.main_loop();
}
