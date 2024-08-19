use chess::{Board, Color, Piece};
use crate::score::BoardScore;

pub fn evaluation(board: &Board) -> BoardScore
{
    evaluate_always_zero(board)
}

fn evaluate_always_zero(_: &Board) -> BoardScore
{
    BoardScore::EVEN
}

fn evaluate_piece_values(board: &Board) -> BoardScore
{
    let mut evaluation = 0;

    let white = board.color_combined(Color::White);
    let black = board.color_combined(Color::Black);

    let piece_balance = |piece: Piece| {
        let pieces = board.pieces(piece);
        let nbr_white_pieces = (pieces & white).popcnt();
        let nbr_black_pieces = (pieces & black).popcnt();
        (nbr_white_pieces as i16) - (nbr_black_pieces as i16)
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
