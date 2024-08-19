#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct BoardScore
{
    // Ranges:
    // +32767 = checkmate
    // +32766 = mate in 1
    // +32512 = mate in 255 or more
    // + 9999 = 99.99 pawns in your favour
    //      0 = even
    // - 9999 = 99.99 pawns against you
    // -32512 = mated in 255 or more
    // -32766 = mated in 1
    // -32767 = mated
    // -32768 = not used

    // For mate scores, essentially only every other number gets used: Mated, mate in 1, mated in 2,
    // etc. Since the evaluation is for the side to move, and the side to move can never have checkmate,
    // mated in 1, mate in 2, etc. These numbers are only used for symmetry in the negamax algorithm.
    // This is reflected in that the number gets halved when reported in UCI:
    // Mate in 1  = score mate 1
    // Mated in 2 = score mate -1
    // Mate in 3  = score mate 2
    // Mated in 4 = score mate -2

    inner: i16,
}

impl BoardScore
{
    pub const MATE: Self = Self { inner: std::i16::MAX };
    const MATE_RANGE_BOTTOM: Self = Self { inner: Self::MATE.inner - 255 };
    pub const EVEN: Self = Self { inner: 0 };
    const MATED_RANGE_TOP: Self = Self { inner: Self::MATED.inner + 255 };
    pub const MATED: Self = Self::MATE.negate();

    pub const BEST_SCORE: Self = Self::MATE;
    pub const WORST_SCORE: Self = Self::MATED;

    const fn negate(self) -> Self
    {
        debug_assert!(self.inner != -32768);
        Self { inner: -self.inner }
    }

    pub fn increment_mate_plies(self) -> Self
    {
        if self > Self::MATE_RANGE_BOTTOM {
            Self { inner: self.inner - 1 }
        } else if self < Self::MATED_RANGE_TOP {
            Self { inner: self.inner + 1 }
        } else {
            self
        }
    }

    pub fn evaluation(evaluation: i16) -> BoardScore
    {
        BoardScore { inner: evaluation }
    }
}

impl std::ops::Neg for BoardScore
{
    type Output = Self;

    fn neg(self) -> Self
    {
        self.negate()
    }
}

impl std::fmt::Display for BoardScore
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.into() {
            BoardScoreDescription::Cp(cp) => write!(f, "cp {cp}")?,
            BoardScoreDescription::Mate(mate) => write!(f, "mate {mate}")?,
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum BoardScoreDescription
{
    Cp(i32),
    Mate(i32),
}

impl Into<BoardScoreDescription> for &BoardScore
{
    fn into(self) -> BoardScoreDescription {
        if self >= &BoardScore::MATE_RANGE_BOTTOM {
            // Positive values for mate
            // Mate in 1 = score mate 1
            // Mate in 3 = score mate 2
            // Mate in 5 = score mate 3
            BoardScoreDescription::Mate((BoardScore::MATE.inner as i32 - self.inner as i32 + 1) / 2)
        } else if self <= &BoardScore::MATED_RANGE_TOP {
            // Negative values if mated
            // Mated in 2 = score mate -1
            // Mated in 4 = score mate -2
            // Mated in 6 = score mate -3
            BoardScoreDescription::Mate((BoardScore::MATED.inner as i32 - self.inner as i32) / 2)
        } else {
            BoardScoreDescription::Cp(self.inner as i32)
        }
    }
}

