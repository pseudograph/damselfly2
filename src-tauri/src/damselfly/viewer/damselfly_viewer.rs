use std::cmp::max;
use rust_lapper::Lapper;
use crate::damselfly::consts::DEFAULT_OPERATION_LOG_SIZE;
use crate::damselfly::memory::memory_parsers::MemorySysTraceParser;
use crate::damselfly::memory::memory_status::MemoryStatus;
use crate::damselfly::memory::memory_update::MemoryUpdateType;
use crate::damselfly::memory::memory_usage_factory::MemoryUsageFactory;
use crate::damselfly::update_interval::update_interval_factory::UpdateIntervalFactory;
use crate::damselfly::update_interval::update_queue_compressor::UpdateQueueCompressor;
use crate::damselfly::update_interval::UpdateInterval;
use crate::damselfly::viewer::graph_viewer::GraphViewer;
use crate::damselfly::viewer::map_viewer::MapViewer;

pub struct DamselflyViewer {
    graph_viewer: GraphViewer,
    map_viewer: MapViewer,
}

impl DamselflyViewer {
    pub fn new(log_path: &str, binary_path: &str) -> DamselflyViewer {
        let mem_sys_trace_parser = MemorySysTraceParser::new();
        let memory_updates = mem_sys_trace_parser.parse_log(log_path, binary_path);
        let (memory_usages, max_usage, max_distinct_blocks) =
            MemoryUsageFactory::new(memory_updates.clone()).calculate_usage_stats();
        let graph_viewer = GraphViewer::new(memory_usages, max_usage, max_distinct_blocks);
        let update_intervals = UpdateIntervalFactory::new(memory_updates).construct_enum_vector();
        let map_viewer = MapViewer::new(update_intervals);
        DamselflyViewer {
            graph_viewer,
            map_viewer
        }
    }

    pub fn get_map(&mut self) -> Vec<MemoryStatus> {
        self.sync_viewers();
        self.map_viewer.snap_and_paint_map()
    }

    pub fn get_map_full(&mut self) -> Vec<MemoryStatus> {
        self.sync_viewers();
        self.map_viewer.paint_map_full()
    }

    pub fn get_map_full_at(&mut self, timestamp: usize) -> Vec<MemoryStatus> {
        self.set_graph_saved_highlight(timestamp);
        self.map_viewer.paint_map_full()
    }

    pub fn get_usage_graph(&self) -> Vec<[f64; 2]> {
        self.graph_viewer.get_usage_plot_points()
    }

    pub fn get_distinct_blocks_graph(&self) -> Vec<[f64; 2]> {
        self.graph_viewer.get_distinct_blocks_plot_points()
    }

    pub fn get_free_blocks_stats(&self) -> (usize, usize) {
        eprintln!("getting updates");
        let updates_till_now = self.map_viewer.get_updates_from(0, self.get_graph_highlight());
        let updates_till_now: Vec<MemoryUpdateType> = updates_till_now.iter()
            .map(|update| update.val.clone())
            .collect();
        let compressed_allocs = UpdateQueueCompressor::compress_to_allocs(&updates_till_now);
        let compressed_intervals = UpdateIntervalFactory::new(compressed_allocs).construct_enum_vector();
        eprintln!("initialising lapper");
        let mut lapper = Lapper::new(compressed_intervals);
        lapper.merge_overlaps();

        let mut largest_free_block_size: usize = 0;
        let mut free_blocks: usize = 0;
        let mut lapper_iter = lapper.iter().peekable();

        while let Some(current_block) = lapper_iter.next() {
            if let Some(next_block) = lapper_iter.peek() {
                let current_free_block_size = next_block.val.get_start() - current_block.val.get_start();
                largest_free_block_size = max(largest_free_block_size, current_free_block_size);
                free_blocks += 1;
            }
        }

        let mut largest_free_block_size = 0;
        let mut free_blocks = 0;
        let mut left = self.map_viewer.get_lowest_address();
        let mut right = left + 1;
        let highest_address = self.map_viewer.get_highest_address();

        eprintln!("looping {left} {highest_address}");
        while right < highest_address {
            while lapper.find(left, right).count() == 0{
                right += 1;
            }
            largest_free_block_size = max(largest_free_block_size, right - left);
            free_blocks += 1;
            left = right;
            right = left + 1;
        }
        eprintln!("exit loop");
        (largest_free_block_size, free_blocks)
    }

    pub fn get_total_operations(&self) -> usize {
        self.graph_viewer.get_total_operations()
    }

    pub fn get_current_operation(&self) -> MemoryUpdateType {
        self.map_viewer.get_current_operation()
    }

    pub fn get_operation_history(&self) -> Vec<MemoryUpdateType> {
        self.map_viewer.get_update_history(DEFAULT_OPERATION_LOG_SIZE)
    }

    pub fn get_graph_highlight(&self) -> usize {
        self.graph_viewer.get_highlight()
    }

    pub fn get_all_intervals(&self) -> &Vec<UpdateInterval> {
        self.map_viewer.get_update_intervals()
    }

    pub fn set_graph_current_highlight(&mut self, new_highlight: usize) {
        self.graph_viewer.set_current_highlight(new_highlight);
    }

    pub fn set_graph_saved_highlight(&mut self, new_highlight: usize) {
        self.graph_viewer.set_saved_highlight(new_highlight);
    }

    pub fn clear_graph_current_highlight(&mut self) {
        self.graph_viewer.clear_current_highlight();
    }


    pub fn set_map_block_size(&mut self, new_size: usize) {
        self.map_viewer.set_block_size(new_size);
    }

    pub fn set_map_span(&mut self, new_span: usize) {
        self.map_viewer.set_map_span(new_span);
    }

    pub fn sync_viewers(&mut self) {
        let current_timestamp = self.graph_viewer.get_highlight();
        self.map_viewer.set_timestamp(current_timestamp);
    }
}