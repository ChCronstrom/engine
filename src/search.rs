use std::collections::HashMap;

use chess::{Board, BoardStatus, ChessMove, MoveGen};
use crate::score::BoardScore;

/*
 * Optimal hash entry:
 * 8 bytes hash
 * 2 bytes Score
 * 1 byte depth
 * 3 bytes Option<ChessMove> (1 byte alignment)
 *      (could be compressed to 2 bytes if necessary:
 *          3 bits source file
 *          3 bits source rank
 *          3 bits target file
 *          3 bits target rank
 *          3 bits promotion (-, N, B, R, Q)
 *          1 bit Some/None)
 * 1 byte: is score exact, lower bound, or upper bound (later also quiescent, etc)
 * 1 byte spare
 * 16 bytes total, 8 byte alignment
 */
pub struct HashEntry
{
    pub hash: u64,
    pub best_move: ChessMove,
    pub score: BoardScore,
    pub depth: u8,
    pub node_type: NodeType,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeType
{
    Exact,
    LowerBound,
    UpperBound,
}

pub struct Searcher
{
    // TODO: Use a better, custom hashmap
    hashmap: HashMap<Board, HashEntry>,
}

impl Searcher
{
    pub fn new() -> Self
    {
        Searcher {
            hashmap: HashMap::new(),
        }
    }

    pub fn alphabeta_search(&mut self, depth: usize, position: &Board, mut alpha: BoardScore, beta: BoardScore) -> BoardScore
    {
        // TODO: Optimization: position.status() internally enumerates all legal moves to determine
        // stalemate and checkmate, and then throws away this information. Later we enumerate the
        // moves again to iterate over them. This should ideally only be done once.
        match position.status()
        {
            BoardStatus::Checkmate => {
                // println!("info string Checkmate found. depth = {depth}, fen = {position}");
                BoardScore::MATED
            }
            BoardStatus::Stalemate => BoardScore::EVEN,
            BoardStatus::Ongoing => {
                if depth > 0 {
                    let legal_moves = MoveGen::new_legal(position);
                    let mut best_score = BoardScore::WORST_SCORE;
                    let mut best_move = ChessMove::default();
                    // TODO: Use better move ordering, e.g. test the best move first, then
                    // all captures, and then the remaining moves.
                    for next_move in legal_moves
                    {
                        let new_position = position.make_move_new(next_move);
                        // println!("Trying move {next_move} {{");
                        let search_score = -self.alphabeta_search(depth - 1, &new_position, -beta, -alpha);
                        if search_score > best_score {
                            // println!("New best move {next_move} with score {search_score}");
                            best_score = search_score;
                            best_move = next_move;
                            if search_score > alpha {
                                alpha = search_score;
                            }
                        }
                        // println!("}}")
                        if search_score >= beta {
                            break;
                        }
                    }
                    best_score = best_score.increment_mate_plies();
                    // println!("info string returning {best_score} at depth = {depth}");
                    self.hashmap.insert(*position, HashEntry { 
                        hash: position.get_hash(),
                        best_move,
                        score: best_score,
                        depth: depth as u8,
                        node_type: NodeType::Exact,
                    });
                    best_score
                    
                } else {
                    BoardScore::EVEN
                }
            }
        }
    }

    pub fn hashmap(&self) -> &HashMap<Board, HashEntry>
    {
        &self.hashmap
    }
}

#[cfg(test)]
mod test
{
    use super::*;

    #[test]
    fn test_boardscore_compare()
    {
        assert!(BoardScore::MATE > BoardScore::EVEN);
        assert!(BoardScore::MATE >= BoardScore::EVEN);
        assert!(BoardScore::EVEN < BoardScore::MATE);
        assert!(BoardScore::EVEN <= BoardScore::MATE);

        assert!(BoardScore::MATE > BoardScore::MATED);
        assert!(BoardScore::MATE >= BoardScore::MATED);
        assert!(BoardScore::MATED < BoardScore::MATE);
        assert!(BoardScore::MATED <= BoardScore::MATE);
    }
}
