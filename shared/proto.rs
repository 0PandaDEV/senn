#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentConfig {
    pub update_interval_secs: u64,
    #[serde(default)]
    pub auto_update: bool,
    #[serde(default)]
    pub default_policy: String,
    #[serde(default)]
    pub policies: std::collections::BTreeMap<String, String>,
    #[serde(default = "default_channel")]
    pub channel: String,
    #[serde(default)]
    pub self_update: bool,
    #[serde(default)]
    pub update_cron: String,
    #[serde(default)]
    pub self_update_cron: String,
}

pub const DEFAULT_UPDATE_CRON: &str = "0 */6 * * *";
pub const DEFAULT_SELF_UPDATE_CRON: &str = "0 4 * * *";

fn default_channel() -> String {
    "stable".into()
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            update_interval_secs: 6 * 60 * 60,
            auto_update: false,
            default_policy: "notify".into(),
            policies: Default::default(),
            channel: default_channel(),
            self_update: true,
            update_cron: DEFAULT_UPDATE_CRON.into(),
            self_update_cron: DEFAULT_SELF_UPDATE_CRON.into(),
        }
    }
}

impl AgentConfig {
    pub fn policy_default(&self) -> &str {
        match self.default_policy.as_str() {
            "auto" => "auto",
            "off" => "off",
            "notify" => "notify",
            _ if self.auto_update => "auto",
            _ => "notify",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ToAgent {
    Welcome { config: AgentConfig },
    Config { config: AgentConfig },
    Req { id: u64, op: Op },
    Cancel { id: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum FromAgent {
    Hello {
        version: String,
        hostname: String,
        os: String,
        arch: String,
        docker: Option<String>,
        #[serde(default)]
        address: Option<String>,
    },
    Res {
        id: u64,
        ok: bool,
        data: Value,
        error: Option<String>,
    },
    Chunk {
        id: u64,
        data: String,
    },
    End {
        id: u64,
        ok: bool,
        error: Option<String>,
    },
    Event {
        kind: String,
        data: Value,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StackAction {
    Up,
    Down,
    Start,
    Stop,
    Restart,
    Pull,
    Update,
    Delete,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContainerAction {
    Start,
    Stop,
    Restart,
    Kill,
    Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    State,
    SelfUpdate {
        tag: String,
    },
    Stack {
        stack: String,
        action: StackAction,
        service: Option<String>,
    },
    Container {
        id: String,
        action: ContainerAction,
    },
    Logs {
        container: Option<String>,
        stack: Option<String>,
        #[serde(default)]
        containers: Vec<String>,
        tail: u32,
        follow: bool,
    },
    CheckUpdates,
    UpdateAll,
    ListDir {
        path: String,
    },
    ReadFile {
        path: String,
    },
    ReadTree {
        path: String,
    },
    WriteFile {
        path: String,
        content: String,
        validate: bool,
    },
    MakeDir {
        path: String,
    },
    Remove {
        path: String,
    },
    Rename {
        from: String,
        to: String,
    },
    CreateStack {
        name: String,
        compose: String,
    },
    Resources {
        kind: ResourceKind,
    },
    RemoveResource {
        kind: ResourceKind,
        id: String,
        force: bool,
    },
    Prune {
        kind: ResourceKind,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Image,
    Volume,
    Network,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRow {
    pub id: String,
    pub tags: Vec<String>,
    pub size: u64,
    pub created: i64,
    pub used_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeRow {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub stack: Option<String>,
    pub anonymous: bool,
    pub used_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkRow {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub subnet: String,
    pub internal: bool,
    pub builtin: bool,
    pub stack: Option<String>,
    pub used_by: Vec<String>,
}

impl HostState {
    pub fn pending_images(&self) -> Vec<String> {
        self.updates
            .iter()
            .filter(|u| u.status == "available")
            .filter(|u| {
                self.containers
                    .iter()
                    .any(|c| c.image == u.image && c.update_policy != "off")
            })
            .map(|u| u.image.clone())
            .collect()
    }
}

impl Op {
    pub fn streams(&self) -> bool {
        matches!(
            self,
            Op::Stack { .. }
                | Op::Logs { .. }
                | Op::CheckUpdates
                | Op::UpdateAll
                | Op::Prune { .. }
                | Op::SelfUpdate { .. }
        )
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostState {
    pub stacks: Vec<StackInfo>,
    pub containers: Vec<ContainerInfo>,
    pub updates: Vec<UpdateInfo>,
    pub updates_checked_at: Option<i64>,
    pub stacks_dir: String,
    pub stacks_dir_ok: bool,
    #[serde(default)]
    pub host: Option<HostStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackInfo {
    pub name: String,
    #[serde(default)]
    pub dir: String,
    pub file: String,
    pub services: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    pub private: u16,
    pub public: Option<u16>,
    pub proto: String,
    pub ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub image_id: String,
    pub state: String,
    pub status: String,
    pub health: Option<String>,
    pub created: i64,
    pub ports: Vec<PortInfo>,
    pub stack: Option<String>,
    pub service: Option<String>,
    pub update_policy: String,
    #[serde(default)]
    pub cpu: Option<f32>,
    #[serde(default)]
    pub mem: Option<u64>,
    #[serde(default)]
    pub mem_limit: Option<u64>,
    #[serde(default)]
    pub net_rx: Option<u64>,
    #[serde(default)]
    pub net_tx: Option<u64>,
    #[serde(default)]
    pub io_read: Option<u64>,
    #[serde(default)]
    pub io_write: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DockerStats {
    pub images: u64,
    pub images_size: u64,
    pub volumes: u64,
    pub networks: u64,
    pub driver: String,
    pub root_dir: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostStats {
    pub cpu: f32,
    pub cores: u32,
    pub load: [f32; 3],
    pub mem_total: u64,
    pub mem_used: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub disk_total: u64,
    pub disk_used: u64,
    pub net_rx: u64,
    pub net_tx: u64,
    pub io_read: u64,
    pub io_write: u64,
    pub uptime: u64,
    pub kernel: String,
    #[serde(default)]
    pub docker: Option<DockerStats>,
    #[serde(default)]
    pub extra: Option<HostExtra>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostExtra {
    pub cpu_user: f32,
    pub cpu_system: f32,
    pub cpu_iowait: f32,
    pub cpu_steal: f32,
    pub cpu_irq: f32,
    pub mem_free: u64,
    pub mem_cached: u64,
    pub mem_buffers: u64,
    pub mem_available: u64,
    pub swap_in: u64,
    pub swap_out: u64,
    pub io_read_ops: u64,
    pub io_write_ops: u64,
    pub io_busy: f32,
    pub net_rx_packets: u64,
    pub net_tx_packets: u64,
    pub net_rx_errors: u64,
    pub net_tx_errors: u64,
    pub ctx_switches: u64,
    pub interrupts: u64,
    pub procs_running: u64,
    pub procs_blocked: u64,
    pub procs_total: u64,
    pub tcp_inuse: u64,
    pub open_files: u64,
    pub pressure_cpu: Option<f32>,
    pub pressure_mem: Option<f32>,
    pub pressure_mem_full: Option<f32>,
    pub pressure_io: Option<f32>,
    pub pressure_io_full: Option<f32>,
    pub temp: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub image: String,
    pub status: String,
    pub local: Option<String>,
    pub remote: Option<String>,
    pub error: Option<String>,
    pub checked_at: i64,
}
