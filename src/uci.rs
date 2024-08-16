use std::io;
use std::io::{BufRead, Write};

pub struct UciClient<'a>
{
    stdin: io::StdinLock<'a>,
    stdout: io::StdoutLock<'a>,
}

impl<'a> UciClient<'a>
{
    pub fn new(stdin: io::StdinLock<'a>, stdout: io::StdoutLock<'a>) -> UciClient<'a>
    {
        UciClient { stdin, stdout }
    }

    pub fn main_loop(&mut self) -> io::Result<()>
    {
        let mut input = String::new();
        loop
        {
            input.clear();
            self.stdin.read_line(&mut input)?;
            if input.len() == 0 {
                // EOF on input, exit
                return Ok(());
            }

            let mut command_words = input.split_ascii_whitespace();
            let command =  command_words.next();
            if let Some(command) = command
            {
                match command
                {
                    "uci" => self.command_uci()?,

                    "quit" => {
                        return Ok(());
                    }
                    _ => {
                        writeln!(self.stdout, "Unknown command: {command}")?;
                    }
                }
            }
        }
    }

    fn command_uci(&mut self) -> io::Result<()>
    {
        writeln!(self.stdout, "id name Christoffer Engine 1.0")?;
        writeln!(self.stdout, "id author Christoffer Cronström")?;
        writeln!(self.stdout, "uciok")?;

        Ok(())
    }
}
