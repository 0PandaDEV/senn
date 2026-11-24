use std::{collections::BTreeMap, path::PathBuf, sync::Mutex};

use bollard::Docker;
use futures::{StreamExt, stream};
use serde::{Deserialize, Serialize};

use crate::{
    proto::{ContainerInfo, UpdateInfo},
    registry,
};

#[derive(Default, Serialize, Deserialize, Clone)]
struct Data {
    items: Vec<UpdateInfo>,
    checked_at: Option<i64>,
}

pub struct Updates {
    path: PathBuf,
    data: Mutex<Data>,
}

pub fn now() -> i64 {
    jiff::Timestamp::now().as_second()
}

impl Updates {
    pub fn load(state_dir: &std::path::Path) -> Self {
        let path = state_dir.join("updates.json");
        let data = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Self {
            path,
            data: Mutex::new(data),
        }
    }

    pub fn snapshot(&self) -> (Vec<UpdateInfo>, Option<i64>) {
        let d = self.data.lock().expect("updates lock");
        (d.items.clone(), d.checked_at)
    }

    fn store(&self, data: Data) {
        if let Ok(text) = serde_json::to_string(&data) {
            let _ = std::fs::write(&self.path, text);
        }
        *self.data.lock().expect("updates lock") = data;
    }

    pub async fn check(
        &self,
        docker: &Docker,
        containers: &[ContainerInfo],
        progress: impl FnMut(String),
    ) {
        let items = self.run(docker, containers, progress).await;
        self.store(Data {
            items,
            checked_at: Some(now()),
        });
    }

    pub async fn refresh(&self, docker: &Docker, containers: &[ContainerInfo], images: &[String]) {
        let subset: Vec<ContainerInfo> = containers
            .iter()
            .filter(|c| images.contains(&c.image))
            .cloned()
            .collect();
        let fresh = self.run(docker, &subset, |_| {}).await;
        let mut data = self.data.lock().expect("updates lock").clone();
        data.items.retain(|i| !fresh.iter().any(|f| f.image == i.image));
        data.items.extend(fresh);
        data.items.sort_by(|a, b| a.image.cmp(&b.image));
        self.store(data);
    }

    async fn run(
        &self,
        docker: &Docker,
        containers: &[ContainerInfo],
        mut progress: impl FnMut(String),
    ) -> Vec<UpdateInfo> {
        let mut by_image: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for c in containers {
            if c.image.is_empty() || c.image.starts_with("sha256:") {
                continue;
            }
            by_image
                .entry(c.image.clone())
                .or_default()
                .push(c.image_id.clone());
        }
        let client = registry::client();
        let total = by_image.len();
        progress(format!("checking {total} images"));
        let results: Vec<UpdateInfo> = stream::iter(by_image)
            .map(|(image, ids)| {
                let client = client.clone();
                let docker = docker.clone();
                async move { check_image(&client, &docker, image, ids).await }
            })
            .buffer_unordered(4)
            .collect()
            .await;
        let mut items = results;
        items.sort_by(|a, b| a.image.cmp(&b.image));
        for i in &items {
            progress(format!("{} {}", i.status, i.image));
        }
        items
    }
}

async fn check_image(
    client: &reqwest::Client,
    docker: &Docker,
    image: String,
    ids: Vec<String>,
) -> UpdateInfo {
    let mut info = UpdateInfo {
        image: image.clone(),
        status: "unknown".into(),
        local: None,
        remote: None,
        error: None,
        checked_at: now(),
    };
    let r = registry::parse(&image);
    if r.pinned {
        info.status = "pinned".into();
        return info;
    }
    let mut local_sets: Vec<Vec<String>> = Vec::new();
    for id in ids.iter().collect::<std::collections::BTreeSet<_>>() {
        if let Ok(inspect) = docker.inspect_image(id).await {
            let digests: Vec<String> = inspect
                .repo_digests
                .unwrap_or_default()
                .into_iter()
                .filter_map(|d| d.split_once('@').map(|(_, h)| h.to_string()))
                .collect();
            local_sets.push(digests);
        }
    }
    if local_sets.iter().all(|d| d.is_empty()) {
        info.status = "local".into();
        return info;
    }
    info.local = local_sets.iter().flatten().next().cloned();
    match registry::remote_digest(client, &r).await {
        Ok(remote) => {
            let behind = local_sets
                .iter()
                .any(|set| !set.is_empty() && !set.contains(&remote));
            info.status = if behind { "available" } else { "current" }.into();
            info.remote = Some(remote);
        }
        Err(e) => {
            info.status = "error".into();
            info.error = Some(e);
        }
    }
    info
}
