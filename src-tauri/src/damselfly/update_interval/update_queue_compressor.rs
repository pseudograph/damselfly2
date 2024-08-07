//! Utility struct that compresses updates. It does this by deleting allocs that have a corresponding free.
//! Use this when you only care about the result of a collection of updates.
use crate::damselfly::memory::memory_update::{MemoryUpdate, MemoryUpdateType};
use crate::damselfly::update_interval::UpdateInterval;

pub struct UpdateQueueCompressor { }

impl UpdateQueueCompressor {
    /// Compresses updates by removing allocs with corresponding frees.
    /// 
    /// # Arguments 
    /// 
    /// * `updates`: Updates to compress.
    /// 
    /// returns: Compressed updates.
    pub fn compress_to_allocs(updates: &Vec<MemoryUpdateType>) -> Vec<MemoryUpdateType> {
        let mut compressed_updates = Vec::new();
        for update in updates {
            match update {
                MemoryUpdateType::Allocation(allocation) => compressed_updates.push(allocation.clone().wrap_in_enum()),
                MemoryUpdateType::Free(free) => {
                    let alloc_to_remove = compressed_updates
                        .iter()
                        .position(|update| {
                            match update {
                                MemoryUpdateType::Allocation(allocation) =>
                                    allocation.get_absolute_address() == free.get_absolute_address(),
                                MemoryUpdateType::Free(_) => panic!("[UpdateQueueCompressor::compress_to_allocs_only]: Free found in compressed_updates"),
                            }
                        })
                        .or(None);
                    if let Some(alloc_to_remove) = alloc_to_remove {
                        compressed_updates.remove(alloc_to_remove);
                    }
                }
            };
        }
        compressed_updates
    }

    /// I don't remember how this differs from compress_to_allocs...
    pub fn compress_ref_to_allocs(updates: &Vec<&MemoryUpdateType>) -> Vec<MemoryUpdateType> {
        let mut compressed_updates = Vec::new();
        for update in updates {
            match update {
                MemoryUpdateType::Allocation(allocation) => compressed_updates.push(allocation.clone().wrap_in_enum()),
                MemoryUpdateType::Free(free) => {
                    compressed_updates.remove(
                        compressed_updates
                            .iter()
                            .position(|update| {
                                match update {
                                    MemoryUpdateType::Allocation(allocation) =>
                                        allocation.get_absolute_address() == free.get_absolute_address(),
                                    MemoryUpdateType::Free(_) => panic!("[UpdateQueueCompressor::compress_to_allocs_only]: Free found in compressed_updates"),
                                }
                            })
                            .expect("[UpdateQueueCompressor::strip_frees_and_corresponding_allocs]: Cannot find alloc corresponding to free"));
                }
            };
        }
        compressed_updates
    }
    
    /// Compresses a list of Intervals by deleting allocations that have corresponding frees.
    /// 
    /// # Arguments 
    /// 
    /// * `updates`: Intervals to compress.
    /// 
    /// returns: Compressed intervals.
    pub fn compress_intervals(updates: Vec<&UpdateInterval>) -> Vec<MemoryUpdateType> {
        let mut compressed_updates = Vec::new();
        for update in updates {
            match &update.val {
                MemoryUpdateType::Allocation(allocation) => compressed_updates.push(allocation.clone().wrap_in_enum()),
                MemoryUpdateType::Free(free) => {
                    compressed_updates.remove(
                        compressed_updates
                            .iter()
                            .position(|update| {
                                match update {
                                    MemoryUpdateType::Allocation(allocation) => 
                                        allocation.get_absolute_address() == free.get_absolute_address(),
                                    MemoryUpdateType::Free(_) => panic!("[UpdateQueueCompressor::compress_intervals]: Free found in compressed_updates"),
                                }
                            })
                            .expect("[UpdateQueueCompressor::compress_intervals]: Cannot find alloc corresponding to free"));
                }
            }
        }
        compressed_updates
    }
}

#[cfg(test)]
mod tests {
    use crate::damselfly::consts::{OVERLAP_FINDER_TEST_LOG, TEST_BINARY_PATH};
    use crate::damselfly::memory::memory_parsers::{MemoryParser, MemorySysTraceParser};
    use crate::damselfly::memory::memory_update::{MemoryUpdate, MemoryUpdateType};
    use crate::damselfly::update_interval::overlap_finder::OverlapFinder;
    use crate::damselfly::update_interval::update_interval_factory::UpdateIntervalFactory;
    use crate::damselfly::update_interval::update_queue_compressor::UpdateQueueCompressor;
    use crate::damselfly::update_interval::utility::Utility;

    fn initialise_test_log() -> OverlapFinder {
        let mst_parser = MemorySysTraceParser::new();
        let updates = mst_parser.parse_log_directly(OVERLAP_FINDER_TEST_LOG, TEST_BINARY_PATH).memory_updates;
        let update_intervals = UpdateIntervalFactory::new(updates).construct_enum_vector();
        OverlapFinder::new(update_intervals)
    }

    #[test]
    fn compress_updates_test() {
        let overlap_finder = initialise_test_log();
        let overlaps = overlap_finder.find_overlaps(0, 400);
        let updates = Utility::convert_intervals_to_updates(&overlaps);
        let compressed_updates = UpdateQueueCompressor::compress_ref_to_allocs(&updates);

        assert_eq!(compressed_updates.len(), 5);
        for update in &compressed_updates {
            assert!(matches!(*update, MemoryUpdateType::Allocation(_)));
        }

        if let MemoryUpdateType::Allocation(allocation) = &compressed_updates[0] {
            assert_eq!(allocation.get_absolute_address(), 0);
            assert_eq!(allocation.get_absolute_size(), 20);
        }

        if let MemoryUpdateType::Allocation(allocation) = &compressed_updates[1] {
            assert_eq!(allocation.get_absolute_address(), 32);
            assert_eq!(allocation.get_absolute_size(), 20);
        }

        if let MemoryUpdateType::Allocation(allocation) = &compressed_updates[2] {
            assert_eq!(allocation.get_absolute_address(), 64);
            assert_eq!(allocation.get_absolute_size(), 276);
        }

        if let MemoryUpdateType::Allocation(allocation) = &compressed_updates[3] {
            assert_eq!(allocation.get_absolute_address(), 344);
            assert_eq!(allocation.get_absolute_size(), 20);
        }

        if let MemoryUpdateType::Allocation(allocation) = &compressed_updates[4] {
            assert_eq!(allocation.get_absolute_address(), 364);
            assert_eq!(allocation.get_absolute_size(), 20);
        }
    }
}

