use std::alloc;
use std::alloc::Layout;
use std::mem;
use std::ptr;
use chess::{Board, ChessMove};

use crate::score::{BoardScore, BoundedScore};
use crate::search;

/*
 * Optimal hash entry in 16 bytes (could be packed to 15)
 * 8 bytes hash
 * 2 bytes BoardScore
 * 1 byte Exact, UpperBound, LowerBound packed with
 *        is node in use, full, half, quiescent, etc
 * 1 byte depth
 * 3 bytes Option<ChessMove> (1 byte alignment)
 *      (could be packed to 2 bytes if necessary:
 *          3 bits source file
 *          3 bits source rank
 *          3 bits target file
 *          3 bits target rank
 *          3 bits promotion (-, N, B, R, Q)
 *          1 bit Some/None)
 * 1 byte generation counter
 * 16 bytes total, 8 byte alignment
 */
#[derive(Clone)]
pub struct HashEntry
{
    entry_type: HashEntryInfo,
    hash: u64,
    best_move: Option<ChessMove>,
    score: BoardScore,
    depth: u8,
    generation: u8,
}

/// Stores various information about the hash entry in packed form, so that the total size of a hash entry
/// doesn't exceed 16 bytes.
///
/// Semantically equivalent to this struct, but stores all of it in a single byte:
///
/// ```
/// struct HashEntryInfo
/// {
///     entry_type: HashEntryKind,
///     score_type: BoardScoreType,
/// }
/// ```
#[derive(Clone, PartialEq, Eq)]
struct HashEntryInfo(u8);

impl HashEntryInfo
{
    fn new(entry_kind: HashEntryKind, score_type: BoardScoreType) -> Self
    {
        let mut result = HashEntryInfo(0);
        result.set_entry_kind(entry_kind);
        result.set_score_type(score_type);
        result
    }

    fn entry_kind(&self) -> HashEntryKind
    {
        use HashEntryKind::*;

        // Lower two bits store entry kind
        match self.0 & 0b11
        {
            0 => Unused,
            1 => Deficient,
            2 => Full,
            _ => unreachable!(),
        }
    }

    fn set_entry_kind(&mut self, entry_kind: HashEntryKind)
    {
        use HashEntryKind::*;

        self.0 &= !0b11;
        self.0 |=
        match entry_kind
        {
            Unused => 0,
            Deficient => 1,
            Full => 2,
        }
    }

    fn score_type(&self) -> BoardScoreType
    {
        use BoardScoreType::*;

        // Bits 2-3 store score type
        match self.0 & 0b1100
        {
            0b0000 => Exact,
            0b0100 => LowerBound,
            0b1000 => UpperBound,
            _ => unreachable!(),
        }
    }

    fn set_score_type(&mut self, score_type: BoardScoreType)
    {
        use BoardScoreType::*;

        self.0 &= !0b1100;
        self.0 |=
        match score_type
        {
            Exact => 0b0000,
            LowerBound => 0b0100,
            UpperBound => 0b1000,
        }
    }

    fn is_used(&self) -> bool
    {
        self.entry_kind() != HashEntryKind::Unused
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HashEntryKind
{
    Unused, Deficient, Full,
}

#[derive(Clone, Copy)]
enum BoardScoreType
{
    Exact, LowerBound, UpperBound,
}

impl BoardScoreType
{
    fn from_score(score: BoundedScore) -> Self
    {
        use BoardScoreType::*;

        match score
        {
            BoundedScore::Exact(_) => Exact,
            BoundedScore::LowerBound(_) => LowerBound,
            BoundedScore::UpperBound(_) => UpperBound,
        }
    }
}

impl HashEntry
{
    // pub fn new() -> Self
    // {
    //     HashEntry {
    //         entry_type: HashEntryInfo::new(),
    //         hash: 0,
    //         best_move: None,
    //         score: BoardScore::NO_SCORE,
    //         depth: 0,
    //         generation: 0,
    //     }
    // }

    pub fn with_contents(hash: u64, best_move: Option<ChessMove>, score: BoundedScore, depth: u8) -> Self
    {
        HashEntry {
            entry_type: HashEntryInfo::new(HashEntryKind::Full, BoardScoreType::from_score(score)),
            hash,
            best_move,
            score: score.unwrap(),
            depth,
            generation: 0,
        }
    }

    pub fn hash(&self) -> u64
    {
        self.hash
    }

    pub fn score(&self) -> BoundedScore
    {
        match self.entry_type.score_type()
        {
            BoardScoreType::Exact => BoundedScore::Exact(self.score),
            BoardScoreType::LowerBound => BoundedScore::LowerBound(self.score),
            BoardScoreType::UpperBound => BoundedScore::UpperBound(self.score),
        }
    }

    pub fn depth(&self) -> search::Depth
    {
        self.depth
    }

    pub fn best_move(&self) -> Option<ChessMove>
    {
        self.best_move
    }
}

/// A special purpose hash map for storing chess positions
///
/// Every entry is mapped from a Zobrist hash to a `HashEntry`. The map has a fixed size specified at
/// creation time. When new entries are inserted, old entries will be purged.
///
/// ## Purging strategy
///
/// Evey hash has a fixed number of locations in the map where it can be stored. If all of these locations
/// are filled, some form of purging is necessary. This purge primarily happens using the generation
/// number: entries from older generations are purged in favor of newer ones. It also uses the depth
/// number: entries of low depth are easier to recompute if necessary, so they are also candidates for
/// purging.
///
/// Hash collisions are not handled gracefully: should two positions have the same Zobrist
/// hash, the wrong entry may be returned.
pub struct HashMap
{
    pointer: ptr::NonNull<HashEntry>,
    layout: Layout,
    phantom_data: std::marker::PhantomData<[HashEntry]>,

    count: usize,
    capacity: usize,
}

impl HashMap
{
    /// Create a new hash map of a specific size
    pub fn new(megabytes: usize) -> Self
    {
        assert!(megabytes > 0);
        // TODO: Maybe allocate megabyte-aligned memory using megapage mapping, for better performance?
        let nbr_bytes = megabytes.checked_mul(1024*1024).expect("overflow");
        let nbr_entries = nbr_bytes / mem::size_of::<HashEntry>();
        let layout = alloc::Layout::array::<HashEntry>(nbr_entries)
            .expect("layout error");

        assert!(layout.size() == nbr_bytes, "HashMap had unexpected size, was {} bytes, requested {}", layout.size(), nbr_bytes);

        let allocation =
        // SAFETY: Allocating raw memory and transmuting it into a static mut slice.
        unsafe {
            let pointer = alloc::alloc_zeroed(layout) as *mut HashEntry;
            ptr::NonNull::new(pointer).expect("alloc returned null")
        };

        HashMap {
            pointer: allocation,
            layout,
            phantom_data: std::marker::PhantomData,
            count: 0,
            capacity: nbr_entries,
        }
    }

    pub fn get<'a>(&'a self, position: &Board) -> Option<&'a HashEntry>
    {
        let hash = position.get_hash();
        let slot_idx = self.get_slot_idx_for_hash(hash);
        let slot = self.get_slot(slot_idx);

        if slot.entry_type.is_used() && slot.hash == hash
        {
            Some(slot)
        }
        else
        {
            None
        }
    }

    pub fn insert(&mut self, position: &Board, entry: HashEntry)
    {
        let hash = position.get_hash();
        let slot_idx = self.get_slot_idx_for_hash(hash);
        let slot = self.get_slot_mut(slot_idx);

        // TODO: Implement a more serious purging strategy
        let old_entry = mem::replace(slot, entry);

        if !old_entry.entry_type.is_used() {
            self.count += 1
        }
    }

    /// The capcity of the hash map, in number of entries
    pub fn capacity(&self) -> usize
    {
        self.capacity
    }

    /// The number of entries that are filled in the hash map in this generation
    ///
    /// If it gets too high, nodes from this generation will start being purged.
    pub fn filled(&self) -> usize
    {
        self.count
    }

    /// Get the slot where this hash can be stored
    fn get_slot_idx_for_hash(&self, hash: u64) -> usize
    {
        // TODO: Every hash should have like 4 locations
        (hash % (self.capacity as u64)) as usize
    }

    /// Get the entry at a particular location
    fn get_slot(&self, slot: usize) -> &HashEntry
    {
        debug_assert!(slot < self.capacity);
        // SAFETY:
        // - offset() must only be used to produce pointers within the same allocation. This is upheld
        //   whenever slot is valid for our capacity
        // - as_ref() is only sound if the pointer is aligned and points to an initialized object. This
        //   is upheld since the allocation is aligned to begin with, offset() preserves alignment,
        //   the allocation was zeroing to begin with, and HashEntry is valid when zero-initialized.
        // - The resulting lifetime matches that of &self
        unsafe
        {
            self.pointer.offset(slot as isize).as_ref()
        }
    }

    /// Get the entry mutable at a particular location
    fn get_slot_mut(&mut self, slot: usize) -> &mut HashEntry
    {
        debug_assert!(slot < self.capacity);
        // SAFETY:
        // - offset() must only be used to produce pointers within the same allocation. This is upheld
        //   whenever slot is valid for our capacity
        // - as_ref() is only sound if the pointer is aligned and points to an initialized object. This
        //   is upheld since the allocation is aligned to begin with, offset() preserves alignment,
        //   the allocation was zeroing to begin with, and HashEntry is valid when zero-initialized.
        // - The resulting lifetime matches that of &mut self
        unsafe
        {
            self.pointer.offset(slot as isize).as_mut()
        }
    }
}

impl Drop for HashMap
{
    fn drop(&mut self)
    {
        // SAFETY: Deallocating using the same layout object that was used to allocate. The pointer
        // is not dereferenced again.
        unsafe {
            alloc::dealloc(self.pointer.as_ptr() as *mut _, self.layout);
        }
    }
}
