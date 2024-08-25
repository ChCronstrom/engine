use chess::{Board, ChessMove, MoveGen};

/// Wrapper around [chess::MoveGen] with the best-move-first optimization
// TODO: After best_move has been yielded, it might make sense to yield captures and checks before
// quiet moves.
pub struct MoveGenerator
{
    inner: MoveGen,
    best_move: Option<ChessMove>,
    generator_state: GeneratorState,
}

impl MoveGenerator
{
    pub fn new(position: &Board, best_move: Option<ChessMove>) -> Self
    {
        let inner = MoveGen::new_legal(position);
        let generator_state =
        if best_move.is_some() {
            // Notice: MoveGen::remove_move doesn't work as expected
            // Previous implementation would call inner.remove_move(best_move) to make sure that inner
            // never iterated that move again. This didn't always work, and sometimes the move came
            // out twice.
            GeneratorState::BestMove
        }
        else
        {
            GeneratorState::Rest
        };
        
        MoveGenerator {
            inner,
            best_move,
            generator_state,
        }
    }
}

impl Iterator for MoveGenerator
{
    type Item = ChessMove;

    fn next(&mut self) -> Option<Self::Item>
    {
        match self.generator_state
        {
            GeneratorState::BestMove => {
                self.generator_state = GeneratorState::Rest;
                debug_assert!(self.best_move.is_some());
                self.best_move
            }

            GeneratorState::Rest => {
                // Make sure that we don't yield best_move again
                while let Some(m) = self.inner.next()
                {
                    if let Some(best_move) = self.best_move
                    {
                        if m != best_move
                        {
                            return Some(m)
                        }
                    }
                    else
                    {
                        return Some(m);
                    }
                }
                None
            }
        }
    }
}

#[derive(Clone, Copy)]
enum GeneratorState
{
    BestMove,
    Rest,
}
