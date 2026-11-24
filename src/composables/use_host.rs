use topcoat::{Result, context::Cx, router::error::RouterErrorExt};

use crate::{
    proto::{ContainerInfo, HostState, StackInfo, UpdateInfo},
    server::utils::{
        hub, params,
        state::{self, AgentRecord},
    },
};

pub struct Host {
    pub record: AgentRecord,
    pub online: bool,
    pub state: Option<HostState>,
    pub error: Option<String>,
}

pub async fn load(cx: &Cx) -> Result<Host> {
    let record = state::agent(params::agent(cx)).await.ok_or_not_found()?;
    let online = hub::online(&record.id);
    let (state, error) = if online {
        match hub::host_state(&record.id).await {
            Ok(s) => (Some(s), None),
            Err(e) => (None, Some(e)),
        }
    } else {
        (None, None)
    };
    Ok(Host {
        record,
        online,
        state,
        error,
    })
}

impl Host {
    pub fn stack_containers<'a>(&'a self, name: &str) -> Vec<&'a ContainerInfo> {
        self.state
            .as_ref()
            .map(|s| {
                s.containers
                    .iter()
                    .filter(|c| c.stack.as_deref() == Some(name))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn unmanaged(&self) -> Vec<&ContainerInfo> {
        self.state
            .as_ref()
            .map(|s| s.containers.iter().filter(|c| c.stack.is_none()).collect())
            .unwrap_or_default()
    }

    pub fn update_for(&self, image: &str) -> Option<&UpdateInfo> {
        self.state
            .as_ref()?
            .updates
            .iter()
            .find(|u| u.image == image)
    }

    pub fn stacks_dir(&self) -> String {
        self.state
            .as_ref()
            .map(|s| s.stacks_dir.clone())
            .unwrap_or_default()
            .trim_end_matches('/')
            .to_string()
    }

    pub fn stack_dir(&self, name: &str) -> String {
        match self.stack(name) {
            Some(s) if !s.dir.is_empty() => s.dir.clone(),
            _ => format!("{}/{name}", self.stacks_dir()),
        }
    }

    pub fn stack(&self, name: &str) -> Option<&StackInfo> {
        self.state.as_ref()?.stacks.iter().find(|s| s.name == name)
    }

    pub fn has_update(&self, c: &ContainerInfo) -> bool {
        c.update_policy != "off"
            && self
                .update_for(&c.image)
                .is_some_and(|u| u.status == "available")
    }

    pub fn stack_updates(&self, name: &str) -> usize {
        self.stack_containers(name)
            .iter()
            .filter(|c| self.has_update(c))
            .count()
    }

    pub fn updates_available(&self) -> usize {
        self.state
            .as_ref()
            .map(|s| s.pending_images().len())
            .unwrap_or(0)
    }
}

pub fn stack_summary(containers: &[&ContainerInfo]) -> (usize, usize) {
    let running = containers.iter().filter(|c| c.state == "running").count();
    (running, containers.len())
}

#[derive(Clone, Copy, Default)]
pub struct Usage {
    pub cpu: Option<f32>,
    pub mem: Option<u64>,
    pub net: Option<(u64, u64)>,
    pub io: Option<(u64, u64)>,
}

fn add2(a: Option<(u64, u64)>, b: Option<(u64, u64)>) -> Option<(u64, u64)> {
    match (a, b) {
        (Some(x), Some(y)) => Some((x.0 + y.0, x.1 + y.1)),
        (x, None) | (None, x) => x,
    }
}

impl Usage {
    pub fn of(c: &ContainerInfo) -> Self {
        Self {
            cpu: c.cpu,
            mem: c.mem,
            net: c.net_rx.zip(c.net_tx),
            io: c.io_read.zip(c.io_write),
        }
    }

    pub fn sum(containers: &[&ContainerInfo]) -> Self {
        containers.iter().fold(Self::default(), |acc, c| {
            let u = Self::of(c);
            Self {
                cpu: acc.cpu.map_or(u.cpu, |a| Some(a + u.cpu.unwrap_or(0.0))),
                mem: acc.mem.map_or(u.mem, |a| Some(a + u.mem.unwrap_or(0))),
                net: add2(acc.net, u.net),
                io: add2(acc.io, u.io),
            }
        })
    }
}

pub fn stack_issues(containers: &[&ContainerInfo]) -> usize {
    containers
        .iter()
        .filter(|c| c.state == "restarting" || c.health.as_deref() == Some("unhealthy"))
        .count()
}
