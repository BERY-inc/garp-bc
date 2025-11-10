use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::cmp::Ordering;
use std::fmt;
use garp_common::{GarpResult, ParticipantId};

/// Vector clock for distributed transaction ordering
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorClock {
    /// Clock values for each node
    pub clocks: BTreeMap<String, u64>,
    
    /// Physical timestamp for tie-breaking
    pub physical_time: DateTime<Utc>,
    
    /// Node ID that owns this clock
    pub node_id: String,
}

/// Lamport timestamp for simple ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LamportTimestamp {
    /// Logical timestamp
    pub logical_time: u64,
    
    /// Physical timestamp
    pub physical_time: DateTime<Utc>,
    
    /// Node ID for tie-breaking
    pub node_id_hash: u64,
}

/// Hybrid logical clock combining logical and physical time
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridLogicalClock {
    /// Logical timestamp
    pub logical_time: u64,
    
    /// Physical timestamp
    pub physical_time: DateTime<Utc>,
    
    /// Logical counter for events at same physical time
    pub logical_counter: u64,
    
    /// Node ID
    pub node_id: String,
}

/// Clock comparison result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClockOrdering {
    /// This clock happens before the other
    Before,
    
    /// This clock happens after the other
    After,
    
    /// Clocks are concurrent (no causal relationship)
    Concurrent,
    
    /// Clocks are identical
    Equal,
}

/// Event for vector clock updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockEvent {
    /// Event ID
    pub event_id: String,
    
    /// Node that generated the event
    pub node_id: String,
    
    /// Vector clock at event time
    pub vector_clock: VectorClock,
    
    /// Event type
    pub event_type: EventType,
    
    /// Event data
    pub data: serde_json::Value,
}

/// Event type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    /// Transaction submitted
    TransactionSubmitted,
    
    /// Transaction sequenced
    TransactionSequenced,
    
    /// Consensus vote
    ConsensusVote,
    
    /// Consensus result
    ConsensusResult,
    
    /// Participant joined
    ParticipantJoined,
    
    /// Participant left
    ParticipantLeft,
    
    /// Domain event
    DomainEvent,
    
    /// Heartbeat
    Heartbeat,
}

/// Clock manager for maintaining vector clocks
pub struct ClockManager {
    /// Current vector clock
    vector_clock: VectorClock,
    
    /// Node ID
    node_id: String,
    
    /// Known nodes
    known_nodes: HashMap<String, NodeInfo>,
    
    /// Event history (for debugging)
    event_history: Vec<ClockEvent>,
    
    /// Maximum history size
    max_history_size: usize,
}

/// Node information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Node ID
    pub node_id: String,
    
    /// Last known clock value
    pub last_clock_value: u64,
    
    /// Last seen timestamp
    pub last_seen: DateTime<Utc>,
    
    /// Node status
    pub status: NodeStatus,
}

/// Node status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeStatus {
    Active,
    Inactive,
    Suspected,
    Failed,
}

impl VectorClock {
    /// Create new vector clock
    pub fn new(node_id: String) -> Self {
        let mut clocks = BTreeMap::new();
        clocks.insert(node_id.clone(), 0);
        
        Self {
            clocks,
            physical_time: Utc::now(),
            node_id,
        }
    }
    
    /// Create vector clock with initial nodes
    pub fn with_nodes(node_id: String, nodes: Vec<String>) -> Self {
        let mut clocks = BTreeMap::new();
        
        for node in nodes {
            clocks.insert(node, 0);
        }
        
        // Ensure our node is included
        clocks.insert(node_id.clone(), 0);
        
        Self {
            clocks,
            physical_time: Utc::now(),
            node_id,
        }
    }
    
    /// Increment local clock
    pub fn tick(&mut self) {
        if let Some(value) = self.clocks.get_mut(&self.node_id) {
            *value += 1;
        }
        self.physical_time = Utc::now();
    }
    
    /// Update clock on receiving message
    pub fn update(&mut self, other: &VectorClock) {
        // Update physical time
        self.physical_time = Utc::now();
        
        // Merge clocks (take maximum of each component)
        for (node_id, &other_value) in &other.clocks {
            let current_value = self.clocks.get(node_id).copied().unwrap_or(0);
            self.clocks.insert(node_id.clone(), current_value.max(other_value));
        }
        
        // Increment our own clock
        if let Some(value) = self.clocks.get_mut(&self.node_id) {
            *value += 1;
        }
    }
    
    /// Compare with another vector clock
    pub fn compare(&self, other: &VectorClock) -> ClockOrdering {
        if self == other {
            return ClockOrdering::Equal;
        }
        
        let mut self_less = false;
        let mut self_greater = false;
        
        // Get all node IDs from both clocks
        let all_nodes: std::collections::BTreeSet<_> = self.clocks.keys()
            .chain(other.clocks.keys())
            .collect();
        
        for node_id in all_nodes {
            let self_value = self.clocks.get(node_id).copied().unwrap_or(0);
            let other_value = other.clocks.get(node_id).copied().unwrap_or(0);
            
            match self_value.cmp(&other_value) {
                Ordering::Less => self_less = true,
                Ordering::Greater => self_greater = true,
                Ordering::Equal => {}
            }
        }
        
        match (self_less, self_greater) {
            (true, false) => ClockOrdering::Before,
            (false, true) => ClockOrdering::After,
            (false, false) => ClockOrdering::Equal, // Should not happen due to earlier check
            (true, true) => ClockOrdering::Concurrent,
        }
    }
    
    /// Check if this clock happens before another
    pub fn happens_before(&self, other: &VectorClock) -> bool {
        matches!(self.compare(other), ClockOrdering::Before)
    }
    
    /// Check if this clock happens after another
    pub fn happens_after(&self, other: &VectorClock) -> bool {
        matches!(self.compare(other), ClockOrdering::After)
    }
    
    /// Check if clocks are concurrent
    pub fn is_concurrent(&self, other: &VectorClock) -> bool {
        matches!(self.compare(other), ClockOrdering::Concurrent)
    }
    
    /// Get clock value for a node
    pub fn get_clock(&self, node_id: &str) -> u64 {
        self.clocks.get(node_id).copied().unwrap_or(0)
    }
    
    /// Set clock value for a node
    pub fn set_clock(&mut self, node_id: String, value: u64) {
        self.clocks.insert(node_id, value);
    }
    
    /// Add new node to clock
    pub fn add_node(&mut self, node_id: String) {
        self.clocks.entry(node_id).or_insert(0);
    }
    
    /// Remove node from clock
    pub fn remove_node(&mut self, node_id: &str) {
        self.clocks.remove(node_id);
    }
    
    /// Get all nodes in clock
    pub fn get_nodes(&self) -> Vec<String> {
        self.clocks.keys().cloned().collect()
    }
    
    /// Create a copy for sending
    pub fn for_sending(&self) -> Self {
        let mut copy = self.clone();
        copy.physical_time = Utc::now();
        copy
    }
    
    /// Merge with another vector clock
    pub fn merge(&mut self, other: &VectorClock) {
        for (node_id, &other_value) in &other.clocks {
            let current_value = self.clocks.get(node_id).copied().unwrap_or(0);
            self.clocks.insert(node_id.clone(), current_value.max(other_value));
        }
        
        // Update physical time to latest
        if other.physical_time > self.physical_time {
            self.physical_time = other.physical_time;
        }
    }
    
    /// Get total ordering key for deterministic sorting
    pub fn total_order_key(&self) -> (u64, DateTime<Utc>, String) {
        let sum: u64 = self.clocks.values().sum();
        (sum, self.physical_time, self.node_id.clone())
    }
}

impl LamportTimestamp {
    /// Create new Lamport timestamp
    pub fn new(node_id: &str) -> Self {
        Self {
            logical_time: 0,
            physical_time: Utc::now(),
            node_id_hash: Self::hash_node_id(node_id),
        }
    }
    
    /// Increment timestamp
    pub fn tick(&mut self) {
        self.logical_time += 1;
        self.physical_time = Utc::now();
    }
    
    /// Update timestamp on receiving message
    pub fn update(&mut self, other: &LamportTimestamp) {
        self.logical_time = self.logical_time.max(other.logical_time) + 1;
        self.physical_time = Utc::now();
    }
    
    /// Hash node ID for tie-breaking
    fn hash_node_id(node_id: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        node_id.hash(&mut hasher);
        hasher.finish()
    }
}

impl HybridLogicalClock {
    /// Create new hybrid logical clock
    pub fn new(node_id: String) -> Self {
        Self {
            logical_time: 0,
            physical_time: Utc::now(),
            logical_counter: 0,
            node_id,
        }
    }
    
    /// Increment clock
    pub fn tick(&mut self) {
        let now = Utc::now();
        
        if now > self.physical_time {
            self.physical_time = now;
            self.logical_counter = 0;
        } else {
            self.logical_counter += 1;
        }
        
        self.logical_time += 1;
    }
    
    /// Update clock on receiving message
    pub fn update(&mut self, other: &HybridLogicalClock) {
        let now = Utc::now();
        let max_physical = self.physical_time.max(other.physical_time).max(now);
        
        if max_physical == self.physical_time && max_physical == other.physical_time {
            self.logical_counter = self.logical_counter.max(other.logical_counter) + 1;
        } else if max_physical == self.physical_time {
            self.logical_counter += 1;
        } else if max_physical == other.physical_time {
            self.logical_counter = other.logical_counter + 1;
        } else {
            self.logical_counter = 0;
        }
        
        self.physical_time = max_physical;
        self.logical_time = self.logical_time.max(other.logical_time) + 1;
    }
    
    /// Compare with another HLC
    pub fn compare(&self, other: &HybridLogicalClock) -> ClockOrdering {
        match self.physical_time.cmp(&other.physical_time) {
            Ordering::Less => ClockOrdering::Before,
            Ordering::Greater => ClockOrdering::After,
            Ordering::Equal => {
                match self.logical_counter.cmp(&other.logical_counter) {
                    Ordering::Less => ClockOrdering::Before,
                    Ordering::Greater => ClockOrdering::After,
                    Ordering::Equal => {
                        match self.node_id.cmp(&other.node_id) {
                            Ordering::Less => ClockOrdering::Before,
                            Ordering::Greater => ClockOrdering::After,
                            Ordering::Equal => ClockOrdering::Equal,
                        }
                    }
                }
            }
        }
    }
}

impl ClockManager {
    /// Create new clock manager
    pub fn new(node_id: String) -> Self {
        Self {
            vector_clock: VectorClock::new(node_id.clone()),
            node_id,
            known_nodes: HashMap::new(),
            event_history: Vec::new(),
            max_history_size: 1000,
        }
    }
    
    /// Create clock manager with known nodes
    pub fn with_nodes(node_id: String, nodes: Vec<String>) -> Self {
        let mut manager = Self::new(node_id.clone());
        manager.vector_clock = VectorClock::with_nodes(node_id, nodes.clone());
        
        // Initialize known nodes
        for node in nodes {
            if node != manager.node_id {
                manager.known_nodes.insert(node.clone(), NodeInfo {
                    node_id: node,
                    last_clock_value: 0,
                    last_seen: Utc::now(),
                    status: NodeStatus::Active,
                });
            }
        }
        
        manager
    }
    
    /// Generate event with current clock
    pub fn generate_event(&mut self, event_type: EventType, data: serde_json::Value) -> ClockEvent {
        self.vector_clock.tick();
        
        let event = ClockEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            node_id: self.node_id.clone(),
            vector_clock: self.vector_clock.clone(),
            event_type,
            data,
        };
        
        self.add_to_history(event.clone());
        event
    }
    
    /// Process received event
    pub fn process_event(&mut self, event: ClockEvent) -> GarpResult<()> {
        // Update vector clock
        self.vector_clock.update(&event.vector_clock);
        
        // Update known node info
        if let Some(node_info) = self.known_nodes.get_mut(&event.node_id) {
            node_info.last_clock_value = event.vector_clock.get_clock(&event.node_id);
            node_info.last_seen = Utc::now();
            node_info.status = NodeStatus::Active;
        } else {
            // Add new node
            self.known_nodes.insert(event.node_id.clone(), NodeInfo {
                node_id: event.node_id.clone(),
                last_clock_value: event.vector_clock.get_clock(&event.node_id),
                last_seen: Utc::now(),
                status: NodeStatus::Active,
            });
            
            // Add to vector clock
            self.vector_clock.add_node(event.node_id.clone());
        }
        
        self.add_to_history(event);
        Ok(())
    }
    
    /// Get current vector clock
    pub fn get_clock(&self) -> &VectorClock {
        &self.vector_clock
    }
    
    /// Get current clock for sending
    pub fn get_clock_for_sending(&self) -> VectorClock {
        self.vector_clock.for_sending()
    }
    
    /// Add node to clock
    pub fn add_node(&mut self, node_id: String) {
        if node_id != self.node_id && !self.known_nodes.contains_key(&node_id) {
            self.vector_clock.add_node(node_id.clone());
            self.known_nodes.insert(node_id.clone(), NodeInfo {
                node_id,
                last_clock_value: 0,
                last_seen: Utc::now(),
                status: NodeStatus::Active,
            });
        }
    }
    
    /// Remove node from clock
    pub fn remove_node(&mut self, node_id: &str) {
        if node_id != self.node_id {
            self.vector_clock.remove_node(node_id);
            self.known_nodes.remove(node_id);
        }
    }
    
    /// Get known nodes
    pub fn get_known_nodes(&self) -> Vec<NodeInfo> {
        self.known_nodes.values().cloned().collect()
    }
    
    /// Check for suspected nodes (haven't been seen recently)
    pub fn check_suspected_nodes(&mut self, timeout: chrono::Duration) {
        let now = Utc::now();
        
        for node_info in self.known_nodes.values_mut() {
            if now - node_info.last_seen > timeout {
                match node_info.status {
                    NodeStatus::Active => {
                        node_info.status = NodeStatus::Suspected;
                        tracing::warn!("Node {} is now suspected", node_info.node_id);
                    }
                    NodeStatus::Suspected => {
                        node_info.status = NodeStatus::Failed;
                        tracing::error!("Node {} is now considered failed", node_info.node_id);
                    }
                    _ => {}
                }
            }
        }
    }
    
    /// Get event history
    pub fn get_event_history(&self) -> &[ClockEvent] {
        &self.event_history
    }
    
    /// Clear event history
    pub fn clear_history(&mut self) {
        self.event_history.clear();
    }
    
    /// Add event to history
    fn add_to_history(&mut self, event: ClockEvent) {
        self.event_history.push(event);
        
        // Trim history if too large
        if self.event_history.len() > self.max_history_size {
            self.event_history.remove(0);
        }
    }
    
    /// Order events by vector clock
    pub fn order_events(&self, events: &mut [ClockEvent]) {
        events.sort_by(|a, b| {
            match a.vector_clock.compare(&b.vector_clock) {
                ClockOrdering::Before => Ordering::Less,
                ClockOrdering::After => Ordering::Greater,
                ClockOrdering::Equal => Ordering::Equal,
                ClockOrdering::Concurrent => {
                    // Use total order key for deterministic ordering
                    a.vector_clock.total_order_key().cmp(&b.vector_clock.total_order_key())
                }
            }
        });
    }
    
    /// Check if two events are causally related
    pub fn are_causally_related(&self, event1: &ClockEvent, event2: &ClockEvent) -> bool {
        !event1.vector_clock.is_concurrent(&event2.vector_clock)
    }
    
    /// Get causal dependencies for an event
    pub fn get_causal_dependencies(&self, event: &ClockEvent) -> Vec<String> {
        self.event_history.iter()
            .filter(|e| e.vector_clock.happens_before(&event.vector_clock))
            .map(|e| e.event_id.clone())
            .collect()
    }
}

impl fmt::Display for VectorClock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VC[")?;
        let mut first = true;
        for (node_id, value) in &self.clocks {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{}:{}", node_id, value)?;
            first = false;
        }
        write!(f, "]@{}", self.physical_time.format("%H:%M:%S%.3f"))
    }
}

impl fmt::Display for LamportTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LT[{}]@{}", self.logical_time, self.physical_time.format("%H:%M:%S%.3f"))
    }
}

impl fmt::Display for HybridLogicalClock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HLC[{}:{}:{}]@{}", 
               self.logical_time, 
               self.physical_time.format("%H:%M:%S%.3f"),
               self.logical_counter,
               self.node_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_vector_clock_creation() {
        let clock = VectorClock::new("node1".to_string());
        assert_eq!(clock.get_clock("node1"), 0);
        assert_eq!(clock.node_id, "node1");
    }
    
    #[test]
    fn test_vector_clock_tick() {
        let mut clock = VectorClock::new("node1".to_string());
        clock.tick();
        assert_eq!(clock.get_clock("node1"), 1);
        
        clock.tick();
        assert_eq!(clock.get_clock("node1"), 2);
    }
    
    #[test]
    fn test_vector_clock_update() {
        let mut clock1 = VectorClock::new("node1".to_string());
        let mut clock2 = VectorClock::new("node2".to_string());
        
        clock1.tick(); // node1: 1, node2: 0
        clock2.tick(); // node1: 0, node2: 1
        clock2.tick(); // node1: 0, node2: 2
        
        clock1.update(&clock2); // node1: 2, node2: 2
        
        assert_eq!(clock1.get_clock("node1"), 2);
        assert_eq!(clock1.get_clock("node2"), 2);
    }
    
    #[test]
    fn test_vector_clock_comparison() {
        let mut clock1 = VectorClock::with_nodes("node1".to_string(), vec!["node1".to_string(), "node2".to_string()]);
        let mut clock2 = VectorClock::with_nodes("node2".to_string(), vec!["node1".to_string(), "node2".to_string()]);
        
        // Initial state: both clocks are [0, 0]
        assert_eq!(clock1.compare(&clock2), ClockOrdering::Equal);
        
        // clock1 ticks: [1, 0] vs [0, 0]
        clock1.tick();
        assert_eq!(clock1.compare(&clock2), ClockOrdering::After);
        assert_eq!(clock2.compare(&clock1), ClockOrdering::Before);
        
        // clock2 ticks: [1, 0] vs [0, 1]
        clock2.tick();
        assert_eq!(clock1.compare(&clock2), ClockOrdering::Concurrent);
        assert_eq!(clock2.compare(&clock1), ClockOrdering::Concurrent);
    }
    
    #[test]
    fn test_lamport_timestamp() {
        let mut ts1 = LamportTimestamp::new("node1");
        let mut ts2 = LamportTimestamp::new("node2");
        
        ts1.tick();
        assert_eq!(ts1.logical_time, 1);
        
        ts2.update(&ts1);
        assert_eq!(ts2.logical_time, 2);
        
        ts1.update(&ts2);
        assert_eq!(ts1.logical_time, 3);
    }
    
    #[test]
    fn test_hybrid_logical_clock() {
        let mut hlc1 = HybridLogicalClock::new("node1".to_string());
        let mut hlc2 = HybridLogicalClock::new("node2".to_string());
        
        hlc1.tick();
        assert_eq!(hlc1.logical_time, 1);
        
        hlc2.update(&hlc1);
        assert_eq!(hlc2.logical_time, 2);
        
        assert_eq!(hlc1.compare(&hlc2), ClockOrdering::Before);
    }
    
    #[test]
    fn test_clock_manager() {
        let mut manager = ClockManager::new("node1".to_string());
        
        let event = manager.generate_event(
            EventType::TransactionSubmitted,
            serde_json::json!({"tx_id": "tx1"})
        );
        
        assert_eq!(event.node_id, "node1");
        assert_eq!(event.vector_clock.get_clock("node1"), 1);
        
        // Simulate receiving event from another node
        let mut other_clock = VectorClock::new("node2".to_string());
        other_clock.tick();
        other_clock.tick();
        
        let received_event = ClockEvent {
            event_id: "event2".to_string(),
            node_id: "node2".to_string(),
            vector_clock: other_clock,
            event_type: EventType::ConsensusVote,
            data: serde_json::json!({"vote": true}),
        };
        
        manager.process_event(received_event).unwrap();
        
        assert_eq!(manager.get_clock().get_clock("node1"), 2);
        assert_eq!(manager.get_clock().get_clock("node2"), 2);
        assert_eq!(manager.known_nodes.len(), 1);
    }
    
    #[test]
    fn test_event_ordering() {
        let mut manager = ClockManager::with_nodes(
            "node1".to_string(),
            vec!["node1".to_string(), "node2".to_string()]
        );
        
        let event1 = manager.generate_event(
            EventType::TransactionSubmitted,
            serde_json::json!({"tx_id": "tx1"})
        );
        
        let event2 = manager.generate_event(
            EventType::TransactionSequenced,
            serde_json::json!({"tx_id": "tx1"})
        );
        
        let mut events = vec![event2.clone(), event1.clone()];
        manager.order_events(&mut events);
        
        assert_eq!(events[0].event_id, event1.event_id);
        assert_eq!(events[1].event_id, event2.event_id);
    }
}