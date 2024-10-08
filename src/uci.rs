use std::io;
use std::io::BufRead;
use std::str::{FromStr, SplitAsciiWhitespace};

use crate::search;
use crate::searchinterface::{SearchInterface, StopConditions};

pub struct UciClient
{
    stdin: io::StdinLock<'static>,
    position: chess::Board,
    search_interface: SearchInterface,
}

impl UciClient
{
    pub fn new() -> UciClient
    {
        UciClient {
            stdin: io::stdin().lock(),
            position: chess::Board::default(),
            search_interface: SearchInterface::new(),
        }
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

            let mut command_words = input.trim_ascii().split_ascii_whitespace();
            let command =  command_words.next();
            if let Some(command) = command
            {
                match command
                {
                    "uci" => self.command_uci(),
                    "ucinewgame" => self.command_ucinewgame(),
                    "position" => self.command_position(command_words),
                    "d" => self.command_d(),
                    "isready" => self.command_isready(),

                    "go" => self.command_go(command_words),
                    "stop" => self.command_stop(),

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

    fn command_ucinewgame(&mut self)
    {
        // TODO: Clear the hash table
    }

    fn command_position(&mut self, mut arguments: SplitAsciiWhitespace)
    {
        let mut result_position;

        // Parse 'startpos' or 'fen <fen_str>'
        match arguments.next()
        {
            Some("startpos") => {
                result_position = chess::Board::default();
            }

            Some("fen") => {
                let arguments_str = arguments.remainder().unwrap_or("");
                let fen_str;
                let moves_str;
                if let Some(moves_idx) = arguments_str.find("moves") {
                    (fen_str, moves_str) = arguments_str.split_at(moves_idx);
                } else {
                    fen_str = arguments_str;
                    moves_str = "";
                }

                match chess::Board::from_str(fen_str)
                {
                    Ok(board) => {
                        result_position = board;
                    }
                    Err(e) => {
                        println!("ERROR: {e}");
                        return;
                    }
                }
                arguments = moves_str.split_ascii_whitespace();
            }

            _ => {
                println!("ERROR: Expected 'startpos' or 'fen'");
                return;
            }
        }

        // Optionally parse moves
        match arguments.next()
        {
            Some("moves") => {
                for move_str in arguments {
                    let next_move = match chess::ChessMove::from_str(move_str)
                    {
                        Ok(m) => m,
                        Err(e) => {
                            println!("ERROR: Invalid move \"{move_str}\": {e}");
                            return;
                        }
                    };

                    // Test if move is legal
                    let mut legal_move_found = false;
                    let movegen = chess::MoveGen::new_legal(&result_position);
                    for legal_move in movegen {
                        if next_move == legal_move {
                            legal_move_found = true;
                            let new_position = result_position.make_move_new(next_move);
                            result_position = new_position;
                            break;
                        }
                    }

                    if !legal_move_found {
                        println!("ERROR: Illegal move {move_str}");
                        return;
                    }
                }
            }

            Some(w) => {
                println!("ERROR: Unexpected word \"{w}\", expected \"moves\" or end of string");
                return;
            }

            None => {
                // ok, no moves specified
            }
        }

        assert!(result_position.is_sane());
        self.position = result_position;
    }

    fn command_d(&self)
    {
        use chess::Color::*;
        use chess::File::*;
        use chess::Piece::*;
        use chess::Rank::*;

        let mut display_str = String::new();
        display_str.push_str("info string ┌─────────────────┐\n");
        for rank in [Eighth, Seventh, Sixth, Fifth, Fourth, Third, Second, First] {
            display_str.push_str("info string │ ");
            for file in [A, B, C, D, E, F, G, H] {
                let square = chess::Square::make_square(rank, file);
                let sq_str = match (self.position.color_on(square), self.position.piece_on(square)) {
                    (None, None) => "  ",

                    (Some(White), Some(King)) => "K ",
                    (Some(White), Some(Queen)) => "Q ",
                    (Some(White), Some(Rook)) => "R ",
                    (Some(White), Some(Bishop)) => "B ",
                    (Some(White), Some(Knight)) => "N ",
                    (Some(White), Some(Pawn)) => "P ",

                    (Some(Black), Some(King)) => "k ",
                    (Some(Black), Some(Queen)) => "q ",
                    (Some(Black), Some(Rook)) => "r ",
                    (Some(Black), Some(Bishop)) => "b ",
                    (Some(Black), Some(Knight)) => "n ",
                    (Some(Black), Some(Pawn)) => "p ",

                    _ => unreachable!(),
                };
                display_str.push_str(sq_str);
            }
            display_str.push_str("│\n");
        }
        display_str.push_str("info string └─────────────────┘\n");

        display_str.push_str("info string ");
        display_str.push_str(match self.position.side_to_move() {
            Black => "Black",
            White => "White",
        });
        display_str.push_str(" to move\n");

        display_str.push_str("info string Checkers:");
        for checker in self.position.checkers().clone() {
            display_str.push_str(" ");
            display_str.push_str(&checker.to_string());
        }
        display_str.push_str("\n");

        display_str.push_str("info string FEN: ");
        display_str.push_str(&self.position.to_string());
        display_str.push_str("\n");


        print!("{}", display_str);
    }

    fn command_isready(&mut self)
    {
        println!("readyok");
    }

    fn command_go(&mut self, mut arguments: SplitAsciiWhitespace)
    {
        let mut stop_conditions = StopConditions::new();

        loop
        {
           match arguments.next()
           {
                Some("depth") => {
                    let depth_str = arguments.next().unwrap_or("");
                    let depth_parsed = search::Depth::from_str(depth_str);
                    match depth_parsed
                    {
                        Ok(d) => {
                            *stop_conditions.depth.get_mut() = d;
                        }
                        Err(e) => {
                            println!("ERROR: Invalid depth \"{depth_str}\": {e}");
                            return;
                        }
                    }
                }

                Some("movetime") => {
                    let movetime_str = arguments.next().unwrap_or("");
                    let movetime_parsed = u32::from_str(movetime_str);
                    match movetime_parsed
                    {
                        Ok(t) => {
                            *stop_conditions.movetime.get_mut() = t;
                        }
                        Err(e) => {
                            println!("ERROR: Invalid movetime \"{movetime_str}\": {e}");
                            return;
                        }
                    }
                }

                None => break,

                Some(other) => {
                    println!("ERROR: Unknown specifier \"{other}\"");
                    return;
                }
           }
        }
        self.search_interface.go(&self.position, stop_conditions);
    }

    fn command_stop(&mut self)
    {
        self.search_interface.stop();
    }

}
