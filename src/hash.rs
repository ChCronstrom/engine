use std::alloc;
use std::alloc::Layout;
use std::mem;
use std::ptr;
use chess::{Board, ChessMove};

use crate::score::{BoardScore, BoundedScore};

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
    entry_type: HashEntryType,
    pub hash: u64,
    pub best_move: Option<ChessMove>,
    pub score: BoundedScore,
    pub depth: u8,
    // pub generation: u8,
}

#[derive(Clone, PartialEq, Eq)]
enum HashEntryType
{
    Unused = 0, // must be zero so that zeroed memory from the allocator comes out as unused entries
    Full,
}

impl HashEntry
{
    pub fn new() -> Self
    {
        HashEntry {
            entry_type: HashEntryType::Full,
            hash: 0,
            best_move: None,
            score: BoundedScore::Exact(BoardScore::NO_SCORE),
            depth: 0,
        }
    }

    pub fn with_contents(hash: u64, best_move: Option<ChessMove>, score: BoundedScore, depth: u8) -> Self
    {
        HashEntry {
            entry_type: HashEntryType::Full,
            hash,
            best_move,
            score,
            depth,
        }
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
    capacity: usize,
    layout: Layout,
    phantom_data: std::marker::PhantomData<[HashEntry]>,
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

        // TODO: Fix so that entries are packed better
        // assert!(layout.size() == nbr_bytes, "HashMap had unexpected size, was {} bytes, requested {}", layout.size(), nbr_bytes);

        let allocation =
        // SAFETY: Allocating raw memory and transmuting it into a static mut slice.
        unsafe {
            let pointer = alloc::alloc_zeroed(layout) as *mut HashEntry;
            ptr::NonNull::new(pointer).expect("alloc returned null")
        };

        HashMap {
            pointer: allocation,
            capacity: nbr_entries,
            layout,
            phantom_data: std::marker::PhantomData,
        }
    }

    pub fn get<'a>(&'a self, position: &Board) -> Option<&'a HashEntry>
    {
        let hash = position.get_hash();
        let slot_idx = self.get_slot_idx_for_hash(hash);
        let slot = self.get_slot(slot_idx);

        if slot.entry_type != HashEntryType::Unused && slot.hash == hash
        {
            Some(slot)
        }
        else
        {
            None
        }
    }

    pub fn get_mut<'a>(&'a self, position: &Board) -> Option<&'a mut HashEntry>
    {
        unimplemented!()
    }

    pub fn insert(&mut self, position: &Board, entry: HashEntry)
    {
        let hash = position.get_hash();
        let slot_idx = self.get_slot_idx_for_hash(hash);
        let slot = self.get_slot_mut(slot_idx);

        // TODO: Implement a more serious purging strategy
        *slot = entry;
        slot.entry_type = HashEntryType::Full;
    }

    pub fn capacity(&self) -> usize
    {
        unimplemented!()
    }

    pub fn filled(&self) -> usize
    {
        0
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
