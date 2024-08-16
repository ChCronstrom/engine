use std::io;
use std::io::BufRead;

pub struct UciClient
{
    stdin: io::StdinLock<'static>,
}

impl UciClient
{
    pub fn new() -> UciClient
    {
        UciClient { stdin: io::stdin().lock() }
    }

    pub fn main_loop(&mut self)
    {
        let mut input = String::new();
        loop
        {
            input.clear();
            if let Err(e) = self.stdin.read_line(&mut input)
            {
                println!("ERROR: IO error {e}");
                return;
            }
            if input.len() == 0 {
                // EOF on input, exit
                return;
            }

            let mut command_words = input.split_ascii_whitespace();
            let command =  command_words.next();
            if let Some(command) = command
            {
                match command
                {
                    "uci" => self.command_uci(),

                    "quit" => {
                        return;
                    }
                    _ => {
                        println!("Unknown command: {command}");
                    }
                }
            }
        }
    }

    fn command_uci(&mut self)
    {
        println!("id name Christoffer Engine 1.0");
        println!("id author Christoffer Cronström");
        println!("uciok");
    }
}
