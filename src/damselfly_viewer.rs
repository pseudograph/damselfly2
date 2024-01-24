pub mod instruction;
pub mod consts;

use std::cmp::{max, min};
use std::collections::HashMap;
use std::sync::{mpsc};
use std::time::Duration;
use log::debug;
use crate::damselfly_viewer::consts::{DEFAULT_BLOCK_SIZE, DEFAULT_TIMESPAN};
use crate::damselfly_viewer::instruction::Instruction;
use crate::memory::{MemoryStatus, MemoryUpdate};


#[derive(Debug, Default, Clone)]
pub struct MemoryUsage {
    pub memory_used_percentage: f64,
    pub memory_used_absolute: f64,
    pub total_memory: usize
}

#[derive(Debug)]
pub struct DamselflyViewer {
    instruction_rx: mpsc::Receiver<Instruction>,
    timespan: (usize, usize),
    timespan_is_unlocked: bool,
    memoryspan: (usize, usize),
    memoryspan_is_unlocked: bool,
    memory_usage_snapshots: Vec<MemoryUsage>,
    operation_history: Vec<MemoryUpdate>,
    memory_map: HashMap<usize, MemoryStatus>,
}

impl DamselflyViewer {
    pub fn new(instruction_rx: mpsc::Receiver<Instruction>) -> DamselflyViewer {
        DamselflyViewer {
            instruction_rx,
            timespan: (0, DEFAULT_TIMESPAN),
            timespan_is_unlocked: false,
            memoryspan: (0, consts::DEFAULT_MEMORYSPAN),
            memoryspan_is_unlocked: false,
            memory_usage_snapshots: Vec::new(),
            operation_history: Vec::new(),
            memory_map: HashMap::new(),
        }
    }

    /// Shifts timespan to the right.
    ///
    /// The absolute distance shifted is computed by multiplying units with the
    /// current timespan.
    ///
    pub fn shift_timespan_right(&mut self, units: usize) {
        let right = &mut self.timespan.1;
        let left = &mut self.timespan.0;
        debug_assert!(*right > *left);
        let span = *right - *left;
        if span < DEFAULT_TIMESPAN { return; }
        let absolute_shift = units * span;

        *right = min((*right).saturating_add(absolute_shift), self.memory_usage_snapshots.len() - 1);
        *left = min((*left).saturating_add(absolute_shift), *right - span);
        debug_assert!(right > left);
    }

    /// Shifts timespan to the left.
    ///
    /// The absolute distance shifted is computed by multiplying units with the
    /// current timespan.
    ///
    pub fn shift_timespan_left(&mut self, units: usize) {
        self.timespan_is_unlocked = true;
        let right = &mut self.timespan.1;
        let left = &mut self.timespan.0;
        debug_assert!(*right > *left);
        let span = *right - *left;
        let absolute_shift = units * span;

        *left = (*left).saturating_sub(absolute_shift);
        *right = max((*right).saturating_sub(absolute_shift), *left + span);
        debug_assert!(*right > *left);
    }

    pub fn shift_timespan_to_beginning(&mut self) {
        let span = self.get_timespan();
        self.timespan.0 = 0;
        self.timespan.1 = span.1 - span.0;
    }

    /// Shifts timespan to include the most recent data.
    pub fn shift_timespan_to_end(&mut self) {
        let span = self.get_timespan();
        self.timespan.1 = self.get_total_operations() - 1;
        self.timespan.0 = self.timespan.1 - (span.1 - span.0);
    }

    /// Locks the timespan, forcing it to automatically follow along as new data streams in.
    pub fn lock_timespan(&mut self) {
        let current_span = max(consts::DEFAULT_TIMESPAN, self.timespan.1 - self.timespan.0);
        self.timespan.1 = self.memory_usage_snapshots.len().saturating_sub(1);
        self.timespan.0 = self.timespan.1.saturating_sub(current_span);
        self.timespan_is_unlocked = false;
    }

    pub fn unlock_timespan(&mut self) {
        self.timespan_is_unlocked = true;
    }

    /// The main entry point to 
    pub fn update(&mut self) {
        let update = self.instruction_rx.recv();
        match update {
            Ok(instruction) => {
                self.update_memory_map(&instruction);
                self.calculate_memory_usage();
                self.log_operation(instruction);
            }
            Err(_) => {
                debug!("[damselfly_viewer::update]: Snapshot channel hung up.");
                return;
            }
        }


        if !self.timespan_is_unlocked {
            self.timespan.1 += 1;
            if self.timespan.1 > consts::DEFAULT_TIMESPAN {
                self.timespan.0 += 1;
            }
        }

        if !self.memoryspan_is_unlocked {
            // do nothing, memoryspan locking in tui
        }
    }

    pub fn gulp_channel(&mut self) {
        let mut counter = 0;
        while let Ok(instruction) = self.instruction_rx.recv_timeout(Duration::from_nanos(1)) {
            eprintln!("gulping {counter}");
            counter += 1;
            self.update_memory_map(&instruction);
            self.calculate_memory_usage();
            self.log_operation(instruction);
        }
    }

    pub fn calculate_memory_usage(&mut self) {
        let mut memory_used_absolute: f64 = 0.0;
        for (_, status) in self.memory_map.iter() {
            match status {
                MemoryStatus::Allocated(_) => memory_used_absolute += 1.0,
                MemoryStatus::PartiallyAllocated(_) => memory_used_absolute += 0.5,
                MemoryStatus::Free(_) => {}
            }
        }

        let memory_usage = MemoryUsage {
            memory_used_percentage: (memory_used_absolute / consts::DEFAULT_MEMORY_SIZE as f64) * 100.0,
            memory_used_absolute,
            total_memory: consts::DEFAULT_MEMORY_SIZE
        };

        self.memory_usage_snapshots.push(memory_usage);
    }

    fn update_memory_map(&mut self, instruction: &Instruction) {
        match instruction.get_operation() {
            MemoryUpdate::Allocation(address, size, callstack) => self.memory_map.insert(address, MemoryStatus::Allocated(callstack)),
            MemoryUpdate::Free(address, callstack) => self.memory_map.insert(address, MemoryStatus::Free(callstack)),
        };
    }

    fn log_operation(&mut self, instruction: Instruction) {
        self.operation_history.push(instruction.get_operation());
    }

    pub fn get_memory_usage(&self) -> MemoryUsage {
        let memory_usage = self.memory_usage_snapshots.last();
        match memory_usage {
            None => {
                MemoryUsage{
                    memory_used_percentage: 0.0,
                    memory_used_absolute: 0.0,
                    total_memory: consts::DEFAULT_MEMORY_SIZE,
                }
            }
            Some(memory_usage) => (*memory_usage).clone()
        }
    }

    pub fn get_memory_usage_view(&self) -> Vec<(f64, f64)> {
        let mut vector = Vec::new();
        for i in self.timespan.0..self.timespan.1 {
            vector.push(((i - self.timespan.0) as f64, self.memory_usage_snapshots.get(i)
                .expect("[DamselflyViewer::get_memory_usage_view]: Error getting timestamp {i} from memory_usage_snapshots")
                .memory_used_percentage));
        }
        vector
    }

    pub fn get_latest_map_state(&self) -> (HashMap<usize, MemoryStatus>, Option<&MemoryUpdate>) {
        (self.memory_map.clone(), self.operation_history.get(self.get_total_operations().saturating_sub(1)))
    }

    pub fn get_map_state(&self, time: usize) -> (HashMap<usize, MemoryStatus>, Option<&MemoryUpdate>) {
        let mut map: HashMap<usize, MemoryStatus> = HashMap::new();
        let mut iter = self.operation_history.iter();
        for _ in 0..=time {
            if let Some(operation) = iter.next() {
                match operation {
                    MemoryUpdate::Allocation(address, size, callstack) => {
                        map.insert(*address, MemoryStatus::Allocated(String::from(callstack)));
                    }
                    MemoryUpdate::Free(address, callstack) => {
                        map.insert(*address, MemoryStatus::Free(String::from(callstack)));
                    }
                }
            }
        }
        (map, self.operation_history.get(time))
    }

    fn allocate_memory(map: &mut HashMap<usize, MemoryStatus>, mut address: usize, mut bytes: usize, callstack: &str) {
        let full_blocks = bytes / DEFAULT_BLOCK_SIZE;
        for block_count in 0..full_blocks {
            map.insert(address + block_count, MemoryStatus::Allocated(String::from(callstack)));
        }

        if (full_blocks * DEFAULT_BLOCK_SIZE) < bytes {
            map.insert(address + full_blocks, MemoryStatus::PartiallyAllocated(String::from(callstack)));
        }
    }

    pub fn get_operation_address_at_time(&self, time: usize) -> Option<&MemoryUpdate> {
        self.operation_history.get(time)
    }

    pub fn get_timespan(&self) -> (usize, usize) {
        self.timespan
    }

    pub fn get_memoryspan(&self) -> (usize, usize) {
        self.memoryspan
    }

    pub fn get_total_operations(&self) -> usize {
        self.memory_usage_snapshots.len()
    }

    pub fn get_operation_log_span(&self, start: usize, end: usize) -> &[MemoryUpdate] {
        if self.operation_history.get(start).is_none() || self.operation_history.get(end - 1).is_none() {
            return &[];
        }
        &self.operation_history[start..end]
    }
    
    pub fn is_timespan_locked(&self) -> bool {
        !self.timespan_is_unlocked
    }

}

/*
#[cfg(test)]
mod tests {
    use crate::damselfly_viewer::{DamselflyViewer, consts::DEFAULT_MEMORY_SIZE, consts};
    use crate::memory::{MemoryStatus, MemorySysTraceParser, MemoryUpdate};

    fn initialise_viewer() -> (DamselflyViewer, MemorySysTraceParser) {
        let (memory_stub, instruction_rx) = MemorySysTraceParser::new();
        let damselfly_viewer = DamselflyViewer::new(instruction_rx);
        (damselfly_viewer, memory_stub)
    }

    #[test]
    fn shift_timespan() {
        let (mut damselfly_viewer, mut memory_stub) = initialise_viewer();
        for i in 0..100 {
            memory_stub.force_generate_event(MemoryUpdate::Allocation(i, String::from("force_generate_event_Allocation")));
        }
        for _ in 0..100 {
            damselfly_viewer.update();
        }
        damselfly_viewer.timespan.0 = 50;
        damselfly_viewer.timespan.1 = 75;
        assert_eq!(damselfly_viewer.timespan.0, 50);
        assert_eq!(damselfly_viewer.timespan.1, 75);
        damselfly_viewer.shift_timespan_left(1);
        assert_eq!(damselfly_viewer.timespan.0, 25);
        assert_eq!(damselfly_viewer.timespan.1, 50);
        damselfly_viewer.shift_timespan_right(1);
        assert_eq!(damselfly_viewer.timespan.0, 50);
        assert_eq!(damselfly_viewer.timespan.1, 75);
    }

    #[test]
    fn shift_timespan_left_cap() {
        let (mut damselfly_viewer, mut memory_stub) = initialise_viewer();
        for i in 0..250 {
            memory_stub.force_generate_event(MemoryUpdate::Allocation(i, String::from("force_generate_event_Allocation")));
        }

        for _ in 0..250 {
            damselfly_viewer.update();
        }
        damselfly_viewer.shift_timespan_left(3);
        assert_eq!(damselfly_viewer.timespan.0, 0);
        assert_eq!(damselfly_viewer.timespan.1, 100);
        damselfly_viewer.shift_timespan_right(1);
        assert_eq!(damselfly_viewer.timespan.0, 100);
        assert_eq!(damselfly_viewer.timespan.1, 200);
        damselfly_viewer.shift_timespan_left(2);
        assert_eq!(damselfly_viewer.timespan.0, 0);
        assert_eq!(damselfly_viewer.timespan.1, 100);
    }

    #[test]
    fn shift_timespan_right() {
        let (mut damselfly_viewer, mut memory_stub) = initialise_viewer();
        for i in 0..250 {
            memory_stub.force_generate_event(MemoryUpdate::Allocation(i, String::from("force_generate_event_Allocation")));
        }
        for _ in 0..250 {
            damselfly_viewer.update();
        }
        damselfly_viewer.shift_timespan_left(3);
        assert_eq!(damselfly_viewer.timespan.0, 0);
        assert_eq!(damselfly_viewer.timespan.1, 100);
        damselfly_viewer.shift_timespan_right(1);
        assert_eq!(damselfly_viewer.timespan.0, 100);
        assert_eq!(damselfly_viewer.timespan.1, 200);
        damselfly_viewer.shift_timespan_right(2);
        assert_eq!(damselfly_viewer.timespan.0, 149);
        assert_eq!(damselfly_viewer.timespan.1, 249);
    }

    #[test]
    fn memory_stub_channel_test() {
        let (mut memory_stub, instruction_rx) = MemoryStub::new();
        for i in 0..5 {
            memory_stub.force_generate_event(MemoryUpdate::Allocation(i, String::from("force_generate_event_Allocation")));
        }
        for i in 0..5 {
            let incoming_instruction = instruction_rx.recv().unwrap();
            assert_eq!(incoming_instruction.get_timestamp(), i);
        }
    }

    #[test]
    fn damselfly_channel_test() {
        let (mut memory_stub, instruction_rx) = MemoryStub::new();
        let mut damselfly_viewer = DamselflyViewer::new(instruction_rx);
        for i in 0..5 {
            memory_stub.force_generate_event(MemoryUpdate::Allocation(i, String::from("force_generate_event_Allocation")));
        }
        for _ in 0..5 {
            damselfly_viewer.update()
        }
    }

    #[test]
    fn lock_timespan() {
        let (mut damselfly_viewer, mut memory_stub) = initialise_viewer();
        for i in 0..250 {
            memory_stub.force_generate_event(MemoryUpdate::Allocation(i, String::from("force_generate_event_Allocation")));
        }
        for _ in 0..250 {
            damselfly_viewer.update();
        }
        damselfly_viewer.shift_timespan_left(1);
        assert_eq!(damselfly_viewer.timespan.0, 50);
        assert_eq!(damselfly_viewer.timespan.1, 150);
        assert!(damselfly_viewer.timespan_is_unlocked);
        damselfly_viewer.lock_timespan();
        assert_eq!(damselfly_viewer.timespan.0, 149);
        assert_eq!(damselfly_viewer.timespan.1, 249);
        assert!(!damselfly_viewer.timespan_is_unlocked);
    }
    #[test]

    #[allow(clippy::get_first)]
    #[test]
    fn stub_to_viewer_channel_test() {
        let (mut damselfly_viewer, mut memory_stub) = initialise_viewer();
        for i in 0..3 {
            memory_stub.force_generate_event(MemoryUpdate::Allocation(i, String::from("force_generate_event_Allocation")));
        }
        for i in 3..6 {
            memory_stub.force_generate_event(MemoryUpdate::PartialAllocation(i, String::from("force_generate_event_PartialAllocation")));
        }
        for i in 6..9 {
            memory_stub.force_generate_event(MemoryUpdate::Free(i - 4, String::from("force_generate_event_Free")));
        }
        for _ in 0..9 {
            damselfly_viewer.update();
        }
        for usage in &damselfly_viewer.memory_usage_snapshots {
            assert_eq!(usage.total_memory, consts::DEFAULT_MEMORY_SIZE);
        }
        assert_eq!(damselfly_viewer.memory_usage_snapshots.get(0).unwrap().memory_used_absolute, 1.0);
        assert_eq!(damselfly_viewer.memory_usage_snapshots.get(1).unwrap().memory_used_absolute, 2.0);
        assert_eq!(damselfly_viewer.memory_usage_snapshots.get(2).unwrap().memory_used_absolute, 3.0);
        assert_eq!(damselfly_viewer.memory_usage_snapshots.get(3).unwrap().memory_used_absolute, 3.5);
        assert_eq!(damselfly_viewer.memory_usage_snapshots.get(4).unwrap().memory_used_absolute, 4.0);
        assert_eq!(damselfly_viewer.memory_usage_snapshots.get(5).unwrap().memory_used_absolute, 4.5);
        assert_eq!(damselfly_viewer.memory_usage_snapshots.get(6).unwrap().memory_used_absolute, 3.5);
        assert_eq!(damselfly_viewer.memory_usage_snapshots.get(7).unwrap().memory_used_absolute, 3.0);
        assert_eq!(damselfly_viewer.memory_usage_snapshots.get(8).unwrap().memory_used_absolute, 2.5);

        let mut time = 0;
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&0).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        for i in 1..10 {
            assert!(!damselfly_viewer.get_map_state(time).0.contains_key(&i));
        }

        time = 1;
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&0).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&1).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        for i in 2..10 {
            assert!(!damselfly_viewer.get_map_state(time).0.contains_key(&i));
        }

        time = 2;
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&0).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&1).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&2).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        for i in 3..10 {
            assert!(!damselfly_viewer.get_map_state(time).0.contains_key(&i));
        }

        time = 3;
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&0).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&1).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&2).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&3).unwrap(), MemoryStatus::PartiallyAllocated(String::from("force_generate_event_PartialAllocation")));
        for i in 4..11 {
            assert!(!damselfly_viewer.get_map_state(time).0.contains_key(&i));
        }

        time = 4;
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&0).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&1).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&2).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&3).unwrap(), MemoryStatus::PartiallyAllocated(String::from("force_generate_event_PartialAllocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&4).unwrap(), MemoryStatus::PartiallyAllocated(String::from("force_generate_event_PartialAllocation")));
        for i in 5..11 {
            assert!(!damselfly_viewer.get_map_state(time).0.contains_key(&i));
        }

        time = 5;
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&0).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&1).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&2).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&3).unwrap(), MemoryStatus::PartiallyAllocated(String::from("force_generate_event_PartialAllocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&4).unwrap(), MemoryStatus::PartiallyAllocated(String::from("force_generate_event_PartialAllocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&5).unwrap(), MemoryStatus::PartiallyAllocated(String::from("force_generate_event_PartialAllocation")));
        for i in 6..11 {
            assert!(!damselfly_viewer.get_map_state(time).0.contains_key(&i));
        }

        time = 6;
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&0).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&1).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&2).unwrap(), MemoryStatus::Free(String::from("force_generate_event_Free")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&3).unwrap(), MemoryStatus::PartiallyAllocated(String::from("force_generate_event_PartialAllocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&4).unwrap(), MemoryStatus::PartiallyAllocated(String::from("force_generate_event_PartialAllocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&5).unwrap(), MemoryStatus::PartiallyAllocated(String::from("force_generate_event_PartialAllocation")));
        for i in 6..11 {
            assert!(!damselfly_viewer.get_map_state(time).0.contains_key(&i));
        }

        time = 7;
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&0).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&1).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&2).unwrap(), MemoryStatus::Free(String::from("force_generate_event_Free")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&3).unwrap(), MemoryStatus::Free(String::from("force_generate_event_Free")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&4).unwrap(), MemoryStatus::PartiallyAllocated(String::from("force_generate_event_PartialAllocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&5).unwrap(), MemoryStatus::PartiallyAllocated(String::from("force_generate_event_PartialAllocation")));
        for i in 6..11 {
            assert!(!damselfly_viewer.get_map_state(time).0.contains_key(&i));
        }

        time = 8;
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&0).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&1).unwrap(), MemoryStatus::Allocated(String::from("force_generate_event_Allocation")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&2).unwrap(), MemoryStatus::Free(String::from("force_generate_event_Free")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&3).unwrap(), MemoryStatus::Free(String::from("force_generate_event_Free")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&4).unwrap(), MemoryStatus::Free(String::from("force_generate_event_Free")));
        assert_eq!(*damselfly_viewer.get_map_state(time).0.get(&5).unwrap(), MemoryStatus::PartiallyAllocated(String::from("force_generate_event_PartialAllocation")));
        for i in 6..11 {
            assert!(!damselfly_viewer.get_map_state(time).0.contains_key(&i));
        }
    }
}
*/