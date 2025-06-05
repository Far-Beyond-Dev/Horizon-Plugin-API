// horizon-plugin-api/src/lib.rs

use std::{
    net::SocketAddr,
    sync::Arc,
    time::Duration,
    pin::Pin,
    future::Future,
};

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::sync::broadcast;
use uuid::Uuid;

// ============================================================================
// Core Types
// ============================================================================

pub type PluginId = Uuid;
pub type EventTypeId = u64;
pub type ConnectionId = Uuid;
pub type RegionId = (i64, i64, i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Protocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RegionCoordinate {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

impl From<(i64, i64, i64)> for RegionCoordinate {
    fn from((x, y, z): (i64, i64, i64)) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageFormat {
    Json,
    Binary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    pub id: Uuid,
    pub region: RegionCoordinate,
    pub address: SocketAddr,
    pub last_seen: std::time::SystemTime,
    pub load: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub region: RegionCoordinate,
    pub listen_address: SocketAddr,
    pub max_connections: usize,
    pub tick_rate: u32,
    pub cluster_discovery_port: u16,
    pub enable_clustering: bool,
    pub plugin_directories: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            region: RegionCoordinate { x: 0, y: 0, z: 0 },
            listen_address: "127.0.0.1:7777".parse().unwrap(),
            max_connections: 10000,
            tick_rate: 60,
            cluster_discovery_port: 7778,
            enable_clustering: true,
            plugin_directories: vec!["plugins/".to_string()],
        }
    }
}

// ============================================================================
// Event System Traits
// ============================================================================

/// Core trait that all events must implement
pub trait Event: Send + Sync + 'static {
    /// Get the unique type ID for this event type
    fn type_id() -> EventTypeId where Self: Sized;
    
    /// Get the plugin ID that owns this event
    fn plugin_id(&self) -> PluginId;
    
    /// Cast to Any for downcasting
    fn as_any(&self) -> &dyn std::any::Any;
    
    /// Clone this event into a Box
    fn clone_boxed(&self) -> Box<dyn Event>;
}

/// Trait for listening to specific event types
pub trait EventListener<T: Event>: Send + Sync {
    /// Handle an incoming event
    fn handle_event(&self, event: &T) -> impl std::future::Future<Output = Result<()>> + Send;
}

// ============================================================================
// Serialization Trait
// ============================================================================

/// Trait for types that can be serialized for network transmission
pub trait Serializable: Send + Sync {
    fn to_json(&self) -> Result<Vec<u8>>;
    fn to_binary(&self) -> Result<Vec<u8>>;
    fn from_json(data: &[u8]) -> Result<Self> where Self: Sized;
    fn from_binary(data: &[u8]) -> Result<Self> where Self: Sized;
}

impl<T> Serializable for T 
where 
    T: Serialize + DeserializeOwned + Send + Sync 
{
    fn to_json(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    fn to_binary(&self) -> Result<Vec<u8>> {
        // Fixed: Use the correct bincode API
        Ok(bincode::serialize(self).map_err(|e| anyhow::anyhow!("Bincode serialization error: {}", e))?)
    }

    fn from_json(data: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(data)?)
    }

    fn from_binary(data: &[u8]) -> Result<Self> {
        // Fixed: Use the correct bincode API
        Ok(bincode::deserialize(data).map_err(|e| anyhow::anyhow!("Bincode deserialization error: {}", e))?)
    }
}

// ============================================================================
// Plugin Context Interface
// ============================================================================

/// Context provided to plugins for interacting with the server
pub struct PluginContext {
    event_bus: Arc<dyn EventBusInterface>,
    network_manager: Arc<dyn NetworkManagerInterface>,
    cluster_manager: Arc<dyn ClusterManagerInterface>,
    pub region_id: RegionCoordinate,
    pub server_config: Arc<ServerConfig>,
}

impl PluginContext {
    /// Create a new plugin context (internal use only)
    pub fn new(
        event_bus: Arc<dyn EventBusInterface>,
        network_manager: Arc<dyn NetworkManagerInterface>,
        cluster_manager: Arc<dyn ClusterManagerInterface>,
        region_id: RegionCoordinate,
        server_config: Arc<ServerConfig>,
    ) -> Self {
        Self {
            event_bus,
            network_manager,
            cluster_manager,
            region_id,
            server_config,
        }
    }

    /// Register an event type for a plugin
    pub async fn register_event(&self, plugin_id: PluginId, event_type_id: EventTypeId) -> Result<()> {
        self.event_bus.register_event(plugin_id, event_type_id).await
    }

    /// Emit an event to the event bus
    pub async fn emit_event(&self, event: Box<dyn Event>) -> Result<()> {
        self.event_bus.emit(event).await
    }

    /// Subscribe to events from a specific plugin
    pub async fn subscribe_to_event(&self, plugin_id: PluginId, event_type_id: EventTypeId) -> Result<broadcast::Receiver<Arc<dyn Event>>> {
        self.event_bus.subscribe(plugin_id, event_type_id).await
    }

    /// Send data to a connected client
    pub async fn send_to_client(&self, 
        connection_id: ConnectionId, 
        data: Vec<u8>, 
        format: MessageFormat
    ) -> Result<()> {
        self.network_manager.send_to_connection(connection_id, data, format).await
    }

    /// Get information about the local cluster node
    pub fn get_local_node(&self) -> ClusterNode {
        self.cluster_manager.get_local_node()
    }

    /// Get the node responsible for a specific region
    pub fn get_node_for_region(&self, region: &RegionCoordinate) -> Option<ClusterNode> {
        self.cluster_manager.get_node_for_region(region)
    }

    /// Get neighboring regions within a certain distance
    pub fn get_neighboring_regions(&self, region: &RegionCoordinate, distance: i64) -> Vec<ClusterNode> {
        self.cluster_manager.get_neighboring_regions(region, distance)
    }

    /// Update the local node's load metric
    pub fn update_load(&self, load: f32) {
        self.cluster_manager.update_load(load);
    }
}

// ============================================================================
// Interface Traits (for dependency injection)
// ============================================================================

/// Interface for the event bus that plugins can use
#[async_trait]
pub trait EventBusInterface: Send + Sync {
    /// Register an event type for a plugin
    async fn register_event(&self, plugin_id: PluginId, event_type_id: EventTypeId) -> Result<()>;
    
    /// Subscribe to events from a plugin
    async fn subscribe(&self, plugin_id: PluginId, event_type_id: EventTypeId) -> Result<broadcast::Receiver<Arc<dyn Event>>>;
    
    /// Emit an event
    async fn emit(&self, event: Box<dyn Event>) -> Result<()>;
}

/// Interface for the network manager that plugins can use
#[async_trait]
pub trait NetworkManagerInterface: Send + Sync {
    /// Send data to a specific connection
    async fn send_to_connection(&self, 
        connection_id: ConnectionId, 
        data: Vec<u8>, 
        format: MessageFormat
    ) -> Result<()>;
    
    /// Get network statistics
    fn get_stats(&self) -> (u64, u64, u64, u64);
}

/// Interface for the cluster manager that plugins can use
pub trait ClusterManagerInterface: Send + Sync {
    /// Get the local cluster node
    fn get_local_node(&self) -> ClusterNode;
    
    /// Get the node for a specific region
    fn get_node_for_region(&self, region: &RegionCoordinate) -> Option<ClusterNode>;
    
    /// Get neighboring regions
    fn get_neighboring_regions(&self, region: &RegionCoordinate, distance: i64) -> Vec<ClusterNode>;
    
    /// Update the local node's load
    fn update_load(&self, load: f32);
}

// ============================================================================
// Plugin Trait
// ============================================================================

/// Main trait that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Get the unique ID of this plugin
    fn id(&self) -> PluginId;
    
    /// Get the human-readable name of this plugin
    fn name(&self) -> &str;
    
    /// Get the version of this plugin
    fn version(&self) -> &str;
    
    /// Initialize the plugin (called once during server startup)
    async fn initialize(&self, context: &mut PluginContext) -> Result<()>;
    
    /// Start the plugin (called after all plugins are initialized)
    async fn start(&self, context: &PluginContext) -> Result<()>;
    
    /// Stop the plugin (called during server shutdown)
    async fn stop(&self, context: &PluginContext) -> Result<()>;
    
    /// Called every server tick
    async fn tick(&self, context: &PluginContext, delta_time: Duration) -> Result<()>;
}

// ============================================================================
// Helper Functions and Type-Safe Wrappers
// ============================================================================

/// Helper function to emit a typed event
pub async fn emit_typed_event<T: Event + Clone>(
    context: &PluginContext,
    event: T,
) -> Result<()> {
    context.emit_event(Box::new(event)).await
}

/// Helper function to send typed data to a client
pub async fn send_typed_data<T: Serializable>(
    context: &PluginContext,
    connection_id: ConnectionId,
    data: &T,
    format: MessageFormat,
) -> Result<()> {
    let serialized = match format {
        MessageFormat::Json => data.to_json()?,
        MessageFormat::Binary => data.to_binary()?,
    };
    context.send_to_client(connection_id, serialized, format).await
}

// ============================================================================
// Utility Macros
// ============================================================================

/// Macro to generate a unique event type ID from a string
#[macro_export]
macro_rules! event_type_id {
    ($name:literal) => {{
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        $name.hash(&mut hasher);
        hasher.finish()
    }};
}

/// Macro to implement the Event trait for a type
#[macro_export]
macro_rules! impl_event {
    ($type:ty, $type_name:literal, $plugin_id_field:ident) => {
        impl Event for $type {
            fn type_id() -> EventTypeId {
                event_type_id!($type_name)
            }

            fn plugin_id(&self) -> PluginId {
                self.$plugin_id_field
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn clone_boxed(&self) -> Box<dyn Event> {
                Box::new(self.clone())
            }
        }
    };
}