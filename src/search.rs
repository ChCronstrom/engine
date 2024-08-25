use std::fmt::Write;
use std::sync::atomic::Ordering;
use std::time;

use chess::{Board, MoveGen};
use crate::evaluation;
use crate::hash::{HashEntry, HashMap};
use crate::moveorder::MoveGenerator;
use crate::score::{BoardScore, BoundedScore};
use crate::searchinterface::StopConditions;

pub type Depth = u8;

pub struct Searcher<'a>
{
    // TODO: Use a better, custom hashmap
    hashmap: HashMap,
    stop_conditions: &'a StopConditions,
    nodes: u64,
    starttime: time::Instant,
}

impl<'a> Searcher<'a>
{
    pub fn new(stop_conditions: &'a StopConditions) -> Self
    {
        Searcher {
            hashmap: HashMap::new(128),
            stop_conditions,
            nodes: 0,
            starttime: time::Instant::now(),
        }
    }

    pub fn search(&mut self, position: Board)
    {
        self.nodes = 0;
        self.starttime = time::Instant::now();

        // TODO: Loop from the latest depth in the hash table instead of 1?
        for depth in 1..=Depth::MAX
        {
            if self.should_stop_search() {
                break;
            }

            if depth > self.stop_conditions.depth.load(Ordering::Relaxed) {
                break;
            }

            let score = self.alphabeta_search(depth, &position, BoardScore::WORST_SCORE, BoardScore::BEST_SCORE);

            let nodes = self.nodes;
            let time = self.starttime.elapsed().as_millis() as u64;
            let nps = if time != 0 { (1000 * nodes) / time } else { 0 };
            let hashfull = (1000 * self.hashmap.filled()) / self.hashmap.capacity();
            let pv = self.trace_pv(&position);
            println!("info depth {depth} multipv 1 score {score} nodes {nodes} nps {nps} hashfull {hashfull} time {time} pv{pv}");
        }
        let best_move = self.hashmap.get(&position)
            .expect("Root node has been purged from hash map")
            .best_move
            .expect("root node had no best move?");
        println!("bestmove {best_move}");
    }

    /// Calculate the score for a position with alpha-beta search
    ///
    /// If the score is higher than `beta`, it may not calculate the exact score, but instead provide
    /// a `BoundedScore::LowerBound` of at least `beta`. Similarly, if the score is lower than `alpha`
    /// it may instead calculate a `BoundedScore::UpperBound` of at most `alpha`.
    ///
    /// If the search gets stopped partway, it may also return `LowerBound` and `UpperBound` scores that
    /// lie inside the range of `alpha` and `beta`.
    fn alphabeta_search(&mut self, mut depth: Depth, position: &Board, mut alpha: BoardScore, beta: BoardScore) -> BoundedScore
    {
        use BoundedScore::*;

        debug_assert!(position.is_sane());
        debug_assert!(alpha != BoardScore::NO_SCORE);
        debug_assert!(beta != BoardScore::NO_SCORE);
        debug_assert!(alpha <= beta);
        self.nodes += 1;

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

        let mut previous_best_move = None;

        // Second, look up in hash table to see if this node has been searched already...
        if let Some(hash_entry) = self.hashmap.get(position)
        {
            debug_assert!(hash_entry.hash == position.get_hash());
            debug_assert!(hash_entry.score.unwrap() != BoardScore::NO_SCORE);
            // ... and to sufficient depth.
            if hash_entry.depth >= depth
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
            // Even if the score is not compatible, we can use the previous information in our current
            // search.
            previous_best_move = hash_entry.best_move;
        }

        if self.should_stop_search() {
            depth = 0;
        }

        if depth > 0
        {
            let mut best_score = UpperBound(BoardScore::NO_SCORE);
            let mut best_move = None;
            let mut any_moves = false;
            let move_gen = MoveGenerator::new(position, previous_best_move);

            for next_move in move_gen
            {
                any_moves = true;

                if self.should_stop_search() {
                    // We are terminating the search early! We now proceed with a depth 0 search for
                    // all remaining nodes. This will still look up in the hash table any positions
                    // already searched, but use leaf evaluation for those nodes that have not previously
                    // been searched.
                    depth = 1;
                }

                let new_position = position.make_move_new(next_move);
                // println!("Trying move {next_move} {{");
                let search_score = -self.alphabeta_search(
                    depth - 1,
                    &new_position,
                    -beta.decrement_mate_plies(),
                    -alpha.decrement_mate_plies())
                    .increment_mate_plies();

                // Assertions for testing that alphabeta has returned a reasonable result. These are
                // not necessarily true if search was aborted partway.
                if !self.should_stop_search() {
                    debug_assert!(search_score.unwrap() != BoardScore::NO_SCORE);
                    if search_score.is_lowerbound() {
                        debug_assert!(search_score.unwrap() >= beta);
                    }
                    if search_score.is_upperbound() {
                        debug_assert!(search_score.unwrap() <= alpha);
                    }
                }

                if search_score > best_score {
                    // println!("New best move {next_move} with score {search_score}");
                    best_score = search_score;
                    best_move = Some(next_move);
                    if !search_score.is_upperbound() && search_score.unwrap() > alpha
                    {
                        // Found a move better than alpha, so update alpha.
                        debug_assert!(!search_score.is_upperbound(), "UpperBound scores should not raise alpha");
                        alpha = search_score.unwrap();
                    }
                }
                // println!("}}");
                if !best_score.is_upperbound() && best_score.unwrap() >= beta {
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

            if !any_moves
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
                // This evaluation is valid for any depth for purposes of hashtable lookup.
                depth = Depth::MAX;
            }

            if best_score.unwrap() != BoardScore::NO_SCORE
            {
                let hash_entry = HashEntry::with_contents(
                    position.get_hash(),
                    best_move,
                    best_score,
                    if best_score.is_exact() && best_score.unwrap().is_mate_score() {
                        // This evaluation is valid for any depth for purposes of hashtable lookup.
                        Depth::MAX
                    } else {
                        depth
                    }
                );

                // println!("info string returning {best_score} at depth = {depth}");
                self.hashmap.insert(position, hash_entry);
            }

            best_score
        }
        else
        {
            // Depth is zero, use leaf evaluation
            Exact(self.leaf_evaluation(position, alpha, beta))
        }
    }

    fn leaf_evaluation(&mut self, position: &Board, alpha: BoardScore, beta: BoardScore) -> BoardScore
    {
        debug_assert!(position.is_sane());
        debug_assert!(alpha != BoardScore::NO_SCORE);
        debug_assert!(beta != BoardScore::NO_SCORE);
        debug_assert!(alpha <= beta);

        self.nodes += 1;

        // TODO: Quiescent search should go here
        // TODO: This method should also store its scores in the hash map
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
                // TODO: This evaluation is valid for any depth for purposes of hashtable lookup.
                BoardScore::EVEN
            }
        }
    }

    fn static_evaluation(&self, position: &Board) -> BoardScore
    {
        use chess::Color::*;

        let wpov_eval = evaluation::evaluate_piece_values(position);
        match position.side_to_move()
        {
            White => wpov_eval,
            Black => -wpov_eval,
        }
    }

    fn should_stop_search(&mut self) -> bool
    {
        self.stop_conditions.stop_now.load(Ordering::Relaxed)
    }

    fn trace_pv(&self, position: &Board) -> String
    {
        let mut result = String::new();

        let mut position = *position;
        while let Some(hash_entry) = self.hashmap.get(&position)
        {
            if let Some(best_move) = hash_entry.best_move
            {
                write!(result, " {}", best_move).expect("string write always succeeds");
                position = position.make_move_new(best_move);
            }
            else
            {
                break
            }
        }

        result
    }
}
