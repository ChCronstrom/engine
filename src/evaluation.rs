use chess::{Board, Piece};
use crate::score::BoardScore;

pub fn _evaluate_always_zero(_: &Board) -> BoardScore
{
    BoardScore::EVEN
}

pub fn evaluate_piece_values(board: &Board) -> BoardScore
{
    let mut evaluation = 0;

    let up = board.side_to_move();
    let red = board.color_combined(up);
    let blue = board.color_combined(!up);

    let piece_balance = |piece: Piece| {
        let pieces = board.pieces(piece);
        let nbr_red_pieces = (pieces & red).popcnt();
        let nbr_blue_pieces = (pieces & blue).popcnt();
        (nbr_red_pieces as i16) - (nbr_blue_pieces as i16)
    };

    // Queens are worth 900 centipawns
    evaluation += 900 * piece_balance(Piece::Queen);

    // Rooks are worth 500 centipawns
    evaluation += 500 * piece_balance(Piece::Rook);

    // Knights and bishops are worth 300 centipawns
    evaluation += 300 * piece_balance(Piece::Knight);
    evaluation += 300 * piece_balance(Piece::Bishop);

    // Pawns are worth 100 centipawns
    evaluation += 100 * piece_balance(Piece::Pawn);

    BoardScore::evaluation(evaluation)
}
