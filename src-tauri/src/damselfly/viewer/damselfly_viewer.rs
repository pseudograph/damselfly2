//! The main struct. Instantiate a DamselflyViewer with your log file and binary, and it will
//! create DamselflyInstances for each memory pool.
//!
//! DamselflyViewer also exposes methods for querying each DamselflyInstance to generate memory maps,
//! get graphs etc.
use std::cmp::min;
use crate::damselfly::memory::memory_parsers::{MemoryParser};
use crate::damselfly::memory::memory_pool::MemoryPool;
use crate::damselfly::memory::memory_update::MemoryUpdateType;
use crate::damselfly::memory::memory_usage_factory::MemoryUsageFactory;
use crate::damselfly::memory::memory_usage_stats::MemoryUsageStats;
use crate::damselfly::viewer::damselfly_instance::DamselflyInstance;

pub struct DamselflyViewer {
    pub damselflies: Vec<DamselflyInstance>,
}

impl DamselflyViewer {
    /// Constructor.
    ///
    /// # Arguments
    ///
    /// * `log_path`: Path to log file.
    /// * `binary_path`: Path to threadxApp binary for debuginfo.
    /// * `cache_size`: Interval between cached maps.
    /// * `distinct_block_left_padding`: Padding to the left of each memory update (shifts the address).
    /// * `distinct_block_right_padding`: Padding to the right of each memory update (increases the size.
    /// * `parser`: The parser used to parse the log file. You can implement your own if you like.
    ///
    /// returns: DamselflyViewer
    pub fn new(
        log_path: &str,
        binary_path: &str,
        cache_size: u64,
        distinct_block_left_padding: usize,
        distinct_block_right_padding: usize,
        parser: impl MemoryParser
    ) -> Self {
        let mut damselfly_viewer = DamselflyViewer {
            damselflies: Vec::new(),
        };
        let pool_restricted_parse_results = parser.parse_log_contents_split_by_pools(log_path, binary_path, distinct_block_left_padding, distinct_block_right_padding);
        for parse_results in &pool_restricted_parse_results {
            let (memory_updates, max_timestamp) = (parse_results.memory_updates.clone(), parse_results.max_timestamp);
            let (pool_start, pool_stop) = (parse_results.pool.get_start(), parse_results.pool.get_start() + parse_results.pool.get_size());
            let mut resampled_memory_updates = Vec::new();
            // This should really be iter_mut, but I don't want to break anything
            for (index, memory_update) in memory_updates.iter().enumerate() {
                let mut resampled_memory_update = memory_update.clone();
                resampled_memory_update.set_timestamp(index);
                resampled_memory_updates.push(resampled_memory_update);
            }

            // Compensate for padding
            for memory_update in resampled_memory_updates.iter_mut() {
                memory_update.set_absolute_address(memory_update.get_absolute_address() - distinct_block_left_padding);
                memory_update.set_absolute_size(memory_update.get_absolute_size() + distinct_block_right_padding);
            }
            
            let cache_size = min(cache_size, resampled_memory_updates.len() as u64);
            let memory_usage_stats = MemoryUsageFactory::new(resampled_memory_updates.clone(), 
                                                             distinct_block_left_padding,
                                                             distinct_block_right_padding,
                                                             pool_start,
                                                             pool_stop,
                                                            ).calculate_usage_stats();
            damselfly_viewer.spawn_damselfly(resampled_memory_updates, memory_usage_stats, parse_results.pool.clone(), max_timestamp, cache_size);
        }

        damselfly_viewer
    }

    /// Spawns a DamselflyInstance. Each DamselflyInstance manages a single memory pool, encapsulating
    /// the graph and memory map for each.
    ///
    /// # Arguments
    ///
    /// * `memory_updates`: Vec of memory updates.
    /// * `memory_usage_stats`: Memory usage stats.
    /// * `pool`: Pool to associate with this instance.
    /// * `max_timestamp`: Max timestamp in this instance.
    /// * `cache_size`: Cache size for this instance.
    ///
    /// returns: ()
    fn spawn_damselfly(&mut self, memory_updates: Vec<MemoryUpdateType>, memory_usage_stats: MemoryUsageStats, pool: MemoryPool, max_timestamp: u64, cache_size: u64) {
        self.damselflies.push(
            DamselflyInstance::new(
                pool.get_name().to_string(),
                memory_updates,
                memory_usage_stats,
                pool.get_start(),
                pool.get_start() + pool.get_size(),
                cache_size as usize,
                max_timestamp,
            )
        );
    }
}
