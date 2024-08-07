//! State machine. Push updates to it and query statistics after each push. Despite its name it 
//! computes statistics other than just no. of distinct blocks.
use std::cmp::{max, min};
use std::collections::{BTreeSet, HashSet};

use crate::damselfly::memory::memory_update::{MemoryUpdate, MemoryUpdateType};
use crate::damselfly::memory::NoHashMap;

#[derive(Default)]
pub struct DistinctBlockCounter {
    start: usize,
    stop: usize,
    left_padding: usize,
    right_padding: usize,
    manually_track_memory_bounds: bool,
    starts_set: HashSet<usize>,
    ends_set: HashSet<usize>,
    starts_tree: BTreeSet<usize>,
    ends_tree: BTreeSet<usize>,
    distinct_blocks: u128,
    free_blocks: Vec<(usize, usize)>,
    free_space: u128,
}

impl DistinctBlockCounter {
    /// Constructor.
    /// 
    /// # Arguments 
    /// 
    /// * `memory_updates`: Vec of memory updates.
    /// * `left_padding`: Padding to the left of each update (shifts the address).
    /// * `right_padding`: Padding to the right of each update (increases the size).
    /// * `memory_bounds`: Pool bounds, if known. Otherwise, the DistinctBlockCounter will compute 
    /// this on the fly based on the addresses it sees.
    /// 
    /// returns: DistinctBlockCounter 
    pub fn new(memory_updates: Vec<MemoryUpdateType>, left_padding: usize, right_padding: usize, memory_bounds: Option<(usize, usize)>) -> DistinctBlockCounter {
        let mut memory_updates_map: NoHashMap<usize, MemoryUpdateType> = NoHashMap::default();
        for memory_update in memory_updates {
            memory_updates_map.insert(memory_update.get_absolute_address(), memory_update);
        }
        
        let start;
        let stop;
        let mut manually_track_memory_bounds = true;
        match memory_bounds {
            None => {
                start = usize::MAX;
                stop = usize::MIN;
            }
            Some((bounds_start, bounds_stop)) => {
                start = bounds_start;
                stop = bounds_stop;
                manually_track_memory_bounds = true;
            }
        }
        let mut distinct_block_counter = DistinctBlockCounter {
            start, 
            stop,
            left_padding,
            right_padding,
            manually_track_memory_bounds,
            starts_set: HashSet::new(),
            ends_set: HashSet::new(),
            starts_tree: BTreeSet::new(),
            ends_tree: BTreeSet::new(),
            distinct_blocks: 0,
            free_blocks: Vec::new(),
            free_space: 0,
        };

        /*
        These are added so that free blocks starting from the start of memory bounds,
        or free blocks ending at the end of memory bounds,
        are still counted.
        */
        distinct_block_counter.starts_set.insert(stop);
        distinct_block_counter.ends_set.insert(start);
        distinct_block_counter.starts_tree.insert(stop);
        distinct_block_counter.ends_tree.insert(start);
        distinct_block_counter
    }

    /// Push an update into the state machine. You may query statistics after each push.
    /// 
    /// # Arguments 
    /// 
    /// * `update`: Memory update to push.
    /// 
    /// returns: () 
    pub fn push_update(&mut self, update: &MemoryUpdateType) {
        let start = update.get_start().saturating_sub(self.left_padding);
        let end = update.get_end().saturating_add(self.right_padding);
        let mut left_attached = false;
        let mut right_attached = false;
        let mut block_delta: i64 = 0;
        
        if self.ends_set.contains(&start) {
            left_attached = true;
        }
        if self.starts_set.contains(&end) {
            right_attached = true;
        }

        
        match update {
            MemoryUpdateType::Allocation(_) => {
                // glues together two blocks, reducing fragmentation
                if left_attached && right_attached {
                    block_delta = -1;
                }

                // island block with no blocks surrounding it, increasing fragmentation
                if !left_attached && !right_attached {
                    block_delta = 1;
                }

                // otherwise, glues onto an existing block, leaving fragmentation unchanged
                self.starts_set.insert(start);
                self.ends_set.insert(end);
                self.starts_tree.insert(start);
                self.ends_tree.insert(end);
            }
            MemoryUpdateType::Free(_) => {
                // breaks a block into two blocks, increasing fragmentation
                if left_attached && right_attached {
                    block_delta = 1;
                }
                
                // frees an island block, reducing fragmentation
                if !left_attached && !right_attached {
                    block_delta = -1;
                }
                
                // otherwise, frees a block glued onto another, leaving fragmentation unchanged
                self.starts_set.remove(&start);
                self.ends_set.remove(&end);
                self.starts_tree.remove(&start);
                self.ends_tree.remove(&end);
            }
        };
        
        if self.manually_track_memory_bounds {
            self.calculate_new_memory_bounds(update);
        }
        self.calculate_free_blocks();
        self.get_free_segment_fragmentation();
        self.distinct_blocks = self.distinct_blocks.saturating_add_signed(block_delta as i128);
    }

    /// Calculates free blocks and stores them within the struct.
    pub fn calculate_free_blocks(&mut self) {
        let mut starts_iter = self.starts_tree.iter();
        let mut ends_iter = self.ends_tree.iter();
        let mut cur_start = starts_iter.next();
        let mut cur_end = ends_iter.next();
        let mut free_blocks: Vec<(usize, usize)> = Vec::new();
        
        // free blocks start from the end of an alloc and last until the start of a new alloc.
        // exception: adjacent allocs, as they are not merged
            while let (Some(cur_start_val), Some(cur_end_val)) = (cur_start, cur_end) {
                // continue loop until start >= end
                if cur_start_val < cur_end_val {
                    cur_start = starts_iter.next();
                    continue;
                }

                // if start == end, there is an adjacent alloc with no space in between, so there is no free block
                // move on to the next end
                if cur_start_val == cur_end_val {
                    cur_end = ends_iter.next();
                    continue;
                }

                // if start > end, we have a free block spanning from [end..start)
                if cur_start_val > cur_end_val {
                    free_blocks.push((*cur_end_val, *cur_start_val));
                    self.free_space += (*cur_start_val - *cur_end_val) as u128;
                    cur_end = ends_iter.next();
                }
            } 
        
        self.free_blocks = free_blocks;
    }
    
    /// Gets the fragmentation of the total free area, which is equivalent to:
    /// 
    /// returns: ((total free bytes) / (largest free block)) - 1
    pub fn get_free_segment_fragmentation(&self) -> u128 {
        let largest_free_block = self.free_blocks.iter().max_by(|prev, next| {
            (prev.1 - prev.0).cmp(&(next.1 - next.0))
        });
        if let Some(largest_free_block) = largest_free_block {
            // Subtract 1 so that optimal usage of free space (one big block) gives us 0
            return (self.free_space / (largest_free_block.1 - largest_free_block.0) as u128).saturating_sub(1);
        }
        0
    }
    
    /// Gets the largest free block
    /// 
    /// returns: (start, end, size)
    pub fn get_largest_free_block(&self) -> (usize, usize, usize) {
        let mut largest_block = (0, 0, 0);
        for block in &self.free_blocks {
            let size = block.1 - block.0;
            if size > largest_block.1 - largest_block.0 {
                largest_block.0 = block.0;
                largest_block.1 = block.1;
                largest_block.2 = size;
            }
        }
        largest_block
    }
    
    /// Updates the tracked memory bounds within the DistinctBlockCounter based on the span of
    /// a new update.
    /// 
    /// # Arguments 
    /// 
    /// * `update`: The latest update.
    /// 
    /// returns: () 
    fn calculate_new_memory_bounds(&mut self, update: &MemoryUpdateType) {
        let new_start;
        let new_stop;
        match update {
            MemoryUpdateType::Allocation(allocation) => {
                new_start = allocation.get_absolute_address();
                new_stop = new_start + allocation.get_absolute_size()
            }
            MemoryUpdateType::Free(free) => {
                new_start = free.get_absolute_address();
                new_stop = new_start + free.get_absolute_size();
            }
        }
        self.start = min(self.start, new_start);
        self.stop = max(self.stop, new_stop);
    }
    
    pub fn get_distinct_blocks(&mut self) -> u128 {
        self.distinct_blocks
    }

    pub fn get_free_blocks(&self) -> Vec<(usize, usize)> {
        self.free_blocks.clone()
    }

    pub fn get_memory_bounds(&self) -> (usize, usize) {
        (self.start, self.stop)
    }

}

mod tests {
    use crate::damselfly::consts::{TEST_BINARY_PATH, TEST_LOG};
    use crate::damselfly::memory::memory_parsers::{MemoryParser, MemorySysTraceParser};
    use crate::damselfly::memory::memory_update::MemoryUpdateType;
    use crate::damselfly::update_interval::distinct_block_counter::DistinctBlockCounter;

    fn _initialise_test_log() -> (Vec<MemoryUpdateType>, DistinctBlockCounter) {
        let mst_parser = MemorySysTraceParser::new();
        let updates = mst_parser.parse_log_directly(TEST_LOG, TEST_BINARY_PATH).memory_updates;
        (updates, DistinctBlockCounter::default())
    }

    #[test]
    fn zero_distinct_blocks_test() {
        let (_, mut distinct_block_counter) = _initialise_test_log();
        assert_eq!(distinct_block_counter.get_distinct_blocks(), 0);
    }

    #[test]
    fn one_distinct_block_test() {
        let (updates, mut distinct_block_counter) = _initialise_test_log();
        distinct_block_counter.push_update(&updates[0]);
    }

    #[test]
    fn distinct_blocks_free_blocks_test() {
        let (updates, mut distinct_block_counter) = _initialise_test_log();
        for update in &updates {
            distinct_block_counter.push_update(update);
        }
        let distinct_blocks = distinct_block_counter.get_distinct_blocks();
        let free_blocks = distinct_block_counter.get_free_blocks();
        assert_eq!(distinct_blocks, 4);
        assert_eq!(free_blocks.len(), 3);
    }
}