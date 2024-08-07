//! Contains memory usage stats.
use crate::damselfly::memory::memory_usage::MemoryUsage;

#[derive(Clone)]
pub struct MemoryUsageStats {
    memory_usages: Vec<MemoryUsage>,
    max_usage: i128,
    max_free_blocks: u128,
    max_distinct_blocks: u128,
    max_free_segment_fragmentation: u128,
    max_largest_free_block: u128,
}

impl MemoryUsageStats {
    pub fn new(memory_usages: Vec<MemoryUsage>, max_usage: i128, max_free_blocks: u128, max_distinct_blocks: u128,
               max_free_segment_fragmentation: u128, max_largest_free_block: u128) -> Self {
        Self {
            memory_usages,
            max_usage,
            max_free_blocks,
            max_distinct_blocks,
            max_free_segment_fragmentation,
            max_largest_free_block,
        }
    }
    
    pub fn get_memory_usages(&self) -> &Vec<MemoryUsage> {
        &self.memory_usages
    }
    
    pub fn get_max_usage(&self) -> i128 {
        self.max_usage
    }
    
    pub fn get_max_free_blocks(&self) -> u128 {
        self.max_free_blocks
    }
    
    pub fn get_max_distinct_blocks(&self) -> u128 {
        self.max_distinct_blocks
    }
    
    pub fn get_max_free_segment_fragmentation(&self) -> u128 { self.max_free_segment_fragmentation }
    pub fn get_max_largest_free_block(&self) -> u128 { self.max_largest_free_block }
}