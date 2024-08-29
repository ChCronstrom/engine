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

    generation: u8,
}

const NUM_SLOTS_PER_HASH: usize = 4;

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
        // SAFETY: Allocating raw memory
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
            generation: 0,
        }
    }

    pub fn get<'a>(&'a self, position: &Board) -> Option<&'a HashEntry>
    {
        let hash = position.get_hash();
        let slot_idx = self.get_slot_idx_for_hash(hash);
        let current_generation = self.generation;

        let mut slots = slot_idx
            .map(|idx| self.get_slot(idx))
            .into_iter()
            .filter(|e| e.hash == hash && e.entry_type.is_used());

        let result = slots.next();
        debug_assert!(slots.next().is_none(), "More than one entry for the same hash in table!");
        if let Some(entry) = result
        {
            if entry.generation != current_generation
            {
                // TODO: Upmark fetched entries to this generation. This will require that generation
                // numbers are wrapped in Cell. We also don't actually know if the entry was useful
                // at this point, so maybe this should happen in search.rs instead?
                // entry.generation = current_generation;
                // self.count += 1;
            }
        }
        result
    }

    pub fn insert(&mut self, position: &Board, entry: HashEntry)
    {
        let hash = position.get_hash();
        let current_generation = self.generation;
        let slot = self.get_mut_or_new_slot(hash);
        *slot = entry;
        slot.hash = hash;
        slot.generation = current_generation;
    }

    /// The capacity of the hash map, in number of entries
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

    pub fn new_generation(&mut self)
    {
        self.generation = self.generation.wrapping_add(1);
        self.count = 0;
    }



    /// Get a mutable reference to the entry for a specific hash, or insert a new one if necessary
    ///
    /// This method will use purging to create a new slot for this hash if the table is full.
    fn get_mut_or_new_slot(&mut self, hash: u64) -> &mut HashEntry
    {
        let slot_idx = self.get_slot_idx_for_hash(hash);

        let slot_to_use =
        // Find existing slot with this same hash
        if let Some(slot) = self.get_existing_slot(hash, slot_idx)
        {
            slot
        }

        // No existing slot, find empty slot to use instead
        else if let Some(slot) = self.get_empty_slot(slot_idx)
        {
            slot
        }

        // No existing slot, and no free slots. Time to purge!
        else
        {
            self.get_purgeable_slot(slot_idx)
        };

        self.get_slot_mut(slot_to_use)
    }

    fn get_existing_slot(&mut self, hash: u64, slot_idx: [usize; NUM_SLOTS_PER_HASH]) -> Option<usize>
    {
        let mut existing_slot = self.get_multi_slot_mut(slot_idx)
            .into_iter()
            .zip(slot_idx)
            .filter(|(e, _)| e.hash == hash);

        if let Some((_, i)) = existing_slot.next()
        {
            debug_assert!(existing_slot.next().is_none(), "More than one entry for the same hash in table!");
            return Some(i);
        }

        return None;
    }

    fn get_empty_slot(&mut self, slot_idx: [usize; NUM_SLOTS_PER_HASH]) -> Option<usize>
    {
        let mut empty_slots = self.get_multi_slot_mut(slot_idx)
            .into_iter()
            .zip(slot_idx)
            .filter(|(e, _)| !e.entry_type.is_used());

        let result =
        if let Some((_, i)) = empty_slots.next()
        {
            // self.count += 1;
            Some(i)
        }
        else
        {
            None
        };
        drop(empty_slots);

        if result.is_some() {
            self.count += 1;
        }

        return result;
    }

    fn get_purgeable_slot(&mut self, slot_idx: [usize; NUM_SLOTS_PER_HASH]) -> usize
    {
        // The full purging priority could go something like this:
        // 1. Purge any entry from more than one generation ago
        // 2. Purge any deficient entry from last generation
        // 3. Purge the full entry of lowest depth from last generation
        // 4. Purge the deficient entry of lowest depth from this generation
        // 5. Purge the full entry of lowest depth from this generation

        let current_generation = self.generation;

        // Entries older than 2 generations, i.e. not from this nor the previous one
        let mut old_entries = self.get_multi_slot_mut(slot_idx)
            .into_iter()
            .zip(slot_idx)
            // Use wrapping arithmetic: if we are in generation 2 and an entry is from generation 255,
            // then that entry is 3 generations old. 2u8.wrapping_sub(255u8) == 3u8
            .filter(|(e, _)| current_generation.wrapping_sub(e.generation) >= 2)
            .map(|(_, i)| i);

        // Purge the first one that comes up: it's unnecessary to sort them
        let first_entry = old_entries.next();
        drop(old_entries);

        if let Some(idx) = first_entry
        {
            self.count += 1;
            return idx;
        }

        // TODO: Handle deficient entries

        // Entry of lowest depth from last generation
        let mut entries = self.get_multi_slot_mut(slot_idx)
            .into_iter()
            .zip(slot_idx)
            .filter(|(e, _)| current_generation.wrapping_sub(e.generation) >= 1)
            .map(|(e, i)| (e.depth, i))
            .collect::<Vec<_>>(); // TODO: Collect onto stack instead of allocating

        if entries.len() > 0 {
            entries.sort_unstable_by_key(|(d, _)| *d);
            self.count += 1;
            return entries[0].1;
        }

        // Entry of lowest depth from this generation. This might be an entry that is actually useful
        // to us, so this will hurt search performance.
        let mut entries = self.get_multi_slot_mut(slot_idx)
            .into_iter()
            .zip(slot_idx)
            .map(|(e, i)| (e.depth, i))
            .collect::<Vec<_>>(); // TODO: Collect onto stack instead of allocating

        entries.sort_unstable_by_key(|(d, _)| *d);
        return entries[0].1;
    }

    /// Get the slots where this hash can be stored
    fn get_slot_idx_for_hash(&self, hash: u64) -> [usize; NUM_SLOTS_PER_HASH]
    {
        let mut hash = hash;
        let mut result = [0; NUM_SLOTS_PER_HASH];

        // Each slot is produced by (hash % capacity). The hash gets rotated by 11 bits to produce the
        // next candidate slot. Should this produce the same slot again, the rotation continues until
        // we have found NUM_SLOTS_PER_HASH distinct slots.
        //
        // The number 11 has been chosen with the following restrictions:
        // - It is coprime to 64, so that all 64 rotations of the hash eventually come up.
        // - The first 4 attempted slots will have used all the bits of the hash.
        // - If there is a slot collision with another hash, shifting 11 new bits into the low end means
        //   that the risk of the same hash colliding again is only 2^-11. If we only shifted 1 bit
        //   that risk would be 50%. If we shifted 17 bits the risk of collision between the first and
        //   fourth slot would be 12%, since only four new bits would have been brought in.
        //
        // If we go through all 64 rotations of the hash without finding 4 unique slots, we increment
        // the hash by 0x1000100010005. This number has been picked because is affects one bit in every
        // short of the hash, and is prime.
        let mut i = 0;
        loop {
            for _ in 0..u64::BITS {
                let next_slot = (hash % (self.capacity as u64)) as usize;
                if !&result[0..i].contains(&next_slot) {
                    result[i] = next_slot;
                    i += 1;
                    if i >= NUM_SLOTS_PER_HASH {
                        // Assert that we've done this correctly
                        debug_assert!(
                            result[0] != result[1] &&
                            result[0] != result[2] &&
                            result[0] != result[3] &&
                            result[1] != result[2] &&
                            result[1] != result[3] &&
                            result[2] != result[3]
                        );
                        return result;
                    }
                }
                hash = hash.rotate_left(11);
            }
            hash = hash.wrapping_add(0x1000100010005);
        }
    }

    /// Get the entry at a particular location
    fn get_slot(&self, idx: usize) -> &HashEntry
    {
        debug_assert!(idx < self.capacity);
        // SAFETY:
        // - offset() must only be used to produce pointers within the same allocation. This is upheld
        //   whenever slot is valid for our capacity
        // - as_ref() is only sound if the pointer is aligned and points to an initialized object. This
        //   is upheld since the allocation is aligned to begin with, offset() preserves alignment,
        //   the allocation was zeroing to begin with, and HashEntry is valid when zero-initialized.
        // - The resulting lifetime matches that of &self
        unsafe
        {
            self.pointer.offset(idx as isize).as_ref()
        }
    }

    /// Get a mutable reference to the entry at a particular location
    fn get_slot_mut(&self, idx: usize) -> &mut HashEntry
    {
        debug_assert!(idx < self.capacity);
        // SAFETY:
        // - offset() must only be used to produce pointers within the same allocation. This is upheld
        //   whenever slot is valid for our capacity
        // - as_ref() is only sound if the pointer is aligned and points to an initialized object. This
        //   is upheld since the allocation is aligned to begin with, offset() preserves alignment,
        //   the allocation was zeroing to begin with, and HashEntry is valid when zero-initialized.
        // - The resulting lifetime matches that of &self
        unsafe
        {
            self.pointer.offset(idx as isize).as_mut()
        }
    }

    /// Get the entries mutably at particular slots
    ///
    /// This method exists to be able to take mutable borrows to several slots simultaneously.
    ///
    /// Safety: It is the caller's responsibility to ensure that all four indexes provided refer to
    /// different slots. Otherwise you will get duplicate mut-references to the same slot.
    fn get_multi_slot_mut(&mut self, idx: [usize; NUM_SLOTS_PER_HASH]) -> [&mut HashEntry; NUM_SLOTS_PER_HASH]
    {
        debug_assert!(
            idx[0] < self.capacity &&
            idx[1] < self.capacity &&
            idx[2] < self.capacity &&
            idx[3] < self.capacity
        );
        debug_assert!(
            idx[0] != idx[1] &&
            idx[0] != idx[2] &&
            idx[0] != idx[3] &&
            idx[1] != idx[2] &&
            idx[1] != idx[3] &&
            idx[2] != idx[3]
        );
        // SAFETY:
        // - offset() must only be used to produce pointers within the same allocation. This is upheld
        //   whenever idx is valid for our capacity
        // - as_ref() is only sound if the pointer is aligned and points to an initialized object. This
        //   is upheld since the allocation is aligned to begin with, offset() preserves alignment,
        //   the allocation was zeroing to begin with, and HashEntry is valid when zero-initialized.
        // - The resulting lifetime matches that of &mut self, and all four entries are ensured to be
        //   distinct so that no two slots have simultaneous mut-references.
        unsafe
        {
            idx.map(|idx| self.pointer.offset(idx as isize).as_mut())
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
