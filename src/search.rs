use std::collections::HashMap;

use chess::{Board, ChessMove, MoveGen};
use crate::evaluation;
use crate::score::{BoardScore, BoundedScore};

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
 * 1 byte spare (or used for "generation" counter?)
 * 16 bytes total, 8 byte alignment
 */
#[derive(Clone, Copy)]
pub struct HashEntry
{
    pub hash: u64,
    pub best_move: Option<ChessMove>,
    pub score: BoundedScore,
    pub depth: u8,
}

impl HashEntry
{
    pub fn new() -> Self
    {
        HashEntry {
            hash: 0,
            best_move: None,
            score: BoundedScore::Exact(BoardScore::NO_SCORE),
            depth: 0,
        }
    }
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

    /// Calculate the score for a position with alpha-beta search
    ///
    /// If the score is higher than `beta`, it may not calculate the exact score, but instead provide
    /// a `BoundedScore::LowerBound` of at least `beta`. Similarly, if the score is lower than `alpha`
    /// it may instead calculate a `BoundedScore::UpperBound` of at most `alpha`.
    pub fn alphabeta_search(&mut self, depth: usize, position: &Board, mut alpha: BoardScore, beta: BoardScore) -> BoundedScore
    {
        use BoundedScore::*;

        debug_assert!(depth < 256);
        debug_assert!(position.is_sane());
        debug_assert!(alpha != BoardScore::NO_SCORE);
        debug_assert!(beta != BoardScore::NO_SCORE);
        debug_assert!(alpha <= beta);

        // First, alpha and beta may be overdetermined, so no searching is necessary. This will happen
        // if, say, a mate-in-five has been found on another branch, and we are now six plies deep on
        // this branch. There is no way be can beat a mate-in-five at a depth of six, so we bail.
        if alpha >= BoardScore::BEST_SCORE {
            // BEST_SCORE can never be achieved on a real board, the best is MATE
            return UpperBound(BoardScore::MATE);
        }

        if beta == BoardScore::WORST_SCORE {
            // WORST_SCORE can never be achieved on a real board, the worst is MATED
            return LowerBound(BoardScore::MATED);
        }

        // Second, look up in hash table to see if this node has been searched already...
        if let Some(hash_entry) = self.hashmap.get(position)
        {
            // ... and to sufficient depth.
            if hash_entry.depth as usize >= depth
            {
                // If the previous score is compatible with our alpha-beta bounds, we can return it.
                match hash_entry.score
                {
                    Exact(s) => return Exact(s),
                    LowerBound(s) if s >= beta => return LowerBound(s),
                    UpperBound(s) if s <= alpha => return UpperBound(s),
                    _ => { },
                }
            }
            // TODO: Even if the score is not compatible, we can use the previous information in our current search.
        }

        if depth > 0
        {
            let legal_moves = MoveGen::new_legal(position);
            let mut best_score = Exact(BoardScore::NO_SCORE);
            let mut best_move = None;

            // TODO: Use better move ordering, e.g. test the best move first, then
            // all captures, and then the remaining moves.
            for next_move in legal_moves
            {
                let new_position = position.make_move_new(next_move);
                // println!("Trying move {next_move} {{");
                let search_score = -self.alphabeta_search(
                    depth - 1,
                    &new_position,
                    -beta.decrement_mate_plies(),
                    -alpha.decrement_mate_plies());

                if search_score.is_lowerbound() {
                    debug_assert!(search_score.unwrap() >= beta);
                }
                if search_score.is_upperbound() {
                    debug_assert!(search_score.unwrap() <= alpha);
                }

                if search_score.unwrap() > best_score.unwrap() {
                    // println!("New best move {next_move} with score {search_score}");
                    best_score = search_score;
                    best_move = Some(next_move);
                    if search_score.unwrap() > alpha
                    {
                        // Found a move better than alpha, so update alpha.
                        debug_assert!(!search_score.is_upperbound(), "UpperBound scores should not raise alpha");
                        alpha = search_score.unwrap();
                    }
                }
                // println!("}}")
                if best_score.unwrap() >= beta {
                    // If we found a move better than beta, we don't need to consider any other moves.
                    // This particular position is "too good" for us, and will therefore never be played
                    // by a minmaxing opponent anyway, so further search can be pruned. This score is
                    // now a LowerBound score: there could be even higher scores in the other moves.
                    debug_assert!(!best_score.is_upperbound(), "UpperBound scores should not cause beta termination"); // TODO: Is this correct?
                    // println!("beta bailing: {best_score:?} > {beta:?}");
                    best_score = LowerBound(best_score.unwrap());
                    break;
                }
            }

            if best_move.is_none()
            {
                // There were no legal moves!
                // This means checkmate or stalemate
                if *position.checkers() != chess::EMPTY
                {
                    best_score = Exact(BoardScore::MATED);
                }
                else
                {
                    best_score = Exact(BoardScore::EVEN);
                }

                // TODO: This evaluation is valid for any depth for purposes of hashtable lookup.
                //depth = 255;
            }
            else
            {
                // Increase mate distance, e.g. "mate in 4" becomes "mate in 5" since we are one step
                // above in the tree. Does nothing for centipawn evaluations.
                best_score = best_score.increment_mate_plies();
            }

            // println!("info string returning {best_score} at depth = {depth}");
            self.hashmap.insert(*position, HashEntry {
                hash: position.get_hash(),
                best_move,
                score: best_score,
                depth: depth as u8,
            });
            best_score
        }
        else
        {
            // Depth is zero, use leaf evaluation
            Exact(self.leaf_evaluation(position, alpha, beta))
        }
    }

    fn leaf_evaluation(&self, position: &Board, alpha: BoardScore, beta: BoardScore) -> BoardScore
    {
        debug_assert!(position.is_sane());
        debug_assert!(alpha != BoardScore::NO_SCORE);
        debug_assert!(beta != BoardScore::NO_SCORE);
        debug_assert!(alpha <= beta);

        // TODO: Quiescent search should go here
        let legal_moves = MoveGen::new_legal(position);
        if legal_moves.len() > 0
        {
            self.static_evaluation(position)
        }
        else
        {
            // This means checkmate or stalemate
            if *position.checkers() != chess::EMPTY
            {
                BoardScore::MATED
            }
            else
            {
                BoardScore::EVEN
            }
        }
    }

    fn static_evaluation(&self, position: &Board) -> BoardScore
    {
        evaluation::evaluate_always_zero(position)
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
