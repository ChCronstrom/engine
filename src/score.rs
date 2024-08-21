#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct BoardScore
{
    // Ranges:
    // +32767 = BEST_SCORE, better than any real score
    // +32766 = checkmate
    // +32765 = mate in 1
    // +32511 = mate in 255 or more
    // + 9999 = 99.99 pawns in your favour
    //      0 = even
    // - 9999 = 99.99 pawns against you
    // -32511 = mated in 255 or more
    // -32765 = mated in 1
    // -32766 = mated
    // -32767 = WORST_SCORE, worse than any real score
    // -32768 = NO_SCORE, placeholder for None

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
    pub const BEST_SCORE: Self = Self { inner: std::i16::MAX };
    pub const MATE: Self = Self { inner: std::i16::MAX - 1 };
    const MATE_RANGE_BOTTOM: Self = Self { inner: Self::MATE.inner - 255 };
    pub const EVEN: Self = Self { inner: 0 };
    const MATED_RANGE_TOP: Self = Self { inner: Self::MATED.inner + 255 };
    pub const MATED: Self = Self::MATE.neg();
    pub const WORST_SCORE: Self = Self::BEST_SCORE.neg();

    pub const NO_SCORE: Self = Self { inner: std::i16::MIN };

    const fn neg(self) -> Self
    {
        if self.inner != Self::NO_SCORE.inner {
            Self { inner: -self.inner }
        } else {
            Self::NO_SCORE
        }
    }

    pub fn increment_mate_plies(self) -> Self
    {
        if self > Self::MATE_RANGE_BOTTOM {
            Self { inner: self.inner - 1 }
        } else if self < Self::MATED_RANGE_TOP && self != Self::NO_SCORE {
            Self { inner: self.inner + 1 }
        } else {
            self
        }
    }

    pub fn decrement_mate_plies(self) -> Self
    {
        if self > Self::MATE_RANGE_BOTTOM {
            Self { inner: self.inner.saturating_add(1) }
        } else if self < Self::MATED_RANGE_TOP && self >= Self::MATED {
            Self { inner: self.inner - 1 }
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
        self.neg()
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoundedScore
{
    Exact(BoardScore),
    LowerBound(BoardScore),
    UpperBound(BoardScore),
}

impl BoundedScore
{
    pub const fn unwrap(self) -> BoardScore
    {
        use BoundedScore::*;

        match self
        {
            Exact(x) => x,
            LowerBound(x) => x,
            UpperBound(x) => x,
        }
    }

    pub const fn neg(self) -> BoundedScore
    {
        use BoundedScore::*;

        match self
        {
            Exact(x) => Exact(x.neg()),
            LowerBound(x) => UpperBound(x.neg()),
            UpperBound(x) => LowerBound(x.neg()),
        }
    }

    pub fn increment_mate_plies(self) -> BoundedScore
    {
        use BoundedScore::*;

        match self
        {
            Exact(x) => Exact(x.increment_mate_plies()),
            LowerBound(x) => LowerBound(x.increment_mate_plies()),
            UpperBound(x) => UpperBound(x.increment_mate_plies()),
        }
    }

    pub const fn is_exact(self) -> bool
    {
        match self
        {
            BoundedScore::Exact(_) => true,
            _ => false,
        }
    }

    pub const fn is_lowerbound(self) -> bool
    {
        match self
        {
            BoundedScore::LowerBound(_) => true,
            _ => false,
        }
    }

    pub const fn is_upperbound(self) -> bool
    {
        match self
        {
            BoundedScore::UpperBound(_) => true,
            _ => false,
        }
    }
}

impl std::ops::Neg for BoundedScore
{
    type Output = Self;

    fn neg(self) -> Self
    {
        self.neg()
    }
}

impl std::fmt::Display for BoundedScore
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
    {
        use BoundedScore::*;

        write!(f, "{}", self.unwrap())?;
        if let LowerBound(_) = self {
            write!(f, " lowerbound")?;
        } else if let UpperBound(_) = self {
            write!(f, " upperbound")?;
        }

        Ok(())
    }
}
