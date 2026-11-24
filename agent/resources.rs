use std::collections::HashMap;

use bollard::{
    Docker,
    query_parameters::{
        ListContainersOptionsBuilder, ListImagesOptions, ListNetworksOptions, ListVolumesOptions, PruneImagesOptionsBuilder,
        PruneNetworksOptions, PruneVolumesOptionsBuilder, RemoveImageOptionsBuilder, RemoveVolumeOptionsBuilder,
    },
};

use crate::proto::{ImageRow, NetworkRow, ResourceKind, VolumeRow};

fn err(e: bollard::errors::Error) -> String {
    e.to_string()
}

struct Usage {
    images: HashMap<String, Vec<String>>,
    volumes: HashMap<String, Vec<String>>,
    networks: HashMap<String, Vec<String>>,
}

async fn usage(docker: &Docker) -> Result<Usage, String> {
    let list = docker
        .list_containers(Some(ListContainersOptionsBuilder::default().all(true).build()))
        .await
        .map_err(err)?;
    let mut u = Usage { images: HashMap::new(), volumes: HashMap::new(), networks: HashMap::new() };
    for c in list {
        let name = c
            .names
            .as_ref()
            .and_then(|n| n.first())
            .map(|n| n.trim_start_matches('/').to_string())
            .unwrap_or_default();
        if let Some(img) = &c.image_id {
            u.images.entry(img.clone()).or_default().push(name.clone());
        }
        for m in c.mounts.unwrap_or_default() {
            if let Some(v) = m.name {
                u.volumes.entry(v).or_default().push(name.clone());
            }
        }
        if let Some(nets) = c.network_settings.and_then(|n| n.networks) {
            for (_, ep) in nets {
                if let Some(id) = ep.network_id {
                    u.networks.entry(id).or_default().push(name.clone());
                }
            }
        }
    }
    for list in u.images.values_mut().chain(u.volumes.values_mut()).chain(u.networks.values_mut()) {
        list.sort();
        list.dedup();
    }
    Ok(u)
}

pub async fn images(docker: &Docker) -> Result<Vec<ImageRow>, String> {
    let u = usage(docker).await?;
    let list = docker.list_images(None::<ListImagesOptions>).await.map_err(err)?;
    let mut out: Vec<ImageRow> = list
        .into_iter()
        .map(|i| {
            let tags: Vec<String> = i.repo_tags.into_iter().filter(|t| t != "<none>:<none>").collect();
            ImageRow {
                used_by: u.images.get(&i.id).cloned().unwrap_or_default(),
                id: i.id,
                tags,
                size: i.size.max(0) as u64,
                created: i.created,
            }
        })
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| a.tags.cmp(&b.tags)).then_with(|| a.id.cmp(&b.id)));
    Ok(out)
}

pub async fn volumes(docker: &Docker) -> Result<Vec<VolumeRow>, String> {
    let u = usage(docker).await?;
    let list = docker.list_volumes(None::<ListVolumesOptions>).await.map_err(err)?;
    let mut out: Vec<VolumeRow> = list
        .volumes
        .unwrap_or_default()
        .into_iter()
        .map(|v| VolumeRow {
            used_by: u.volumes.get(&v.name).cloned().unwrap_or_default(),
            stack: v.labels.get("com.docker.compose.project").cloned(),
            anonymous: v.labels.contains_key("com.docker.volume.anonymous") || (v.name.len() == 64 && v.name.chars().all(|c| c.is_ascii_hexdigit())),
            name: v.name,
            driver: v.driver,
            mountpoint: v.mountpoint,
        })
        .collect();
    out.sort_by(|a, b| (a.anonymous, &a.stack, &a.name).cmp(&(b.anonymous, &b.stack, &b.name)));
    Ok(out)
}

pub async fn networks(docker: &Docker) -> Result<Vec<NetworkRow>, String> {
    let u = usage(docker).await?;
    let list = docker.list_networks(None::<ListNetworksOptions>).await.map_err(err)?;
    let mut out: Vec<NetworkRow> = list
        .into_iter()
        .map(|n| {
            let id = n.id.unwrap_or_default();
            let name = n.name.unwrap_or_default();
            NetworkRow {
                used_by: u.networks.get(&id).cloned().unwrap_or_default(),
                builtin: matches!(name.as_str(), "bridge" | "host" | "none"),
                stack: n.labels.as_ref().and_then(|l| l.get("com.docker.compose.project").cloned()),
                subnet: n
                    .ipam
                    .and_then(|i| i.config)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|c| c.subnet)
                    .collect::<Vec<_>>()
                    .join(", "),
                driver: n.driver.unwrap_or_default(),
                scope: n.scope.unwrap_or_default(),
                internal: n.internal.unwrap_or(false),
                id,
                name,
            }
        })
        .collect();
    out.sort_by(|a, b| (!a.builtin, &a.stack, &a.name).cmp(&(!b.builtin, &b.stack, &b.name)));
    Ok(out)
}

pub async fn remove(docker: &Docker, kind: ResourceKind, id: &str, force: bool) -> Result<(), String> {
    match kind {
        ResourceKind::Image => docker
            .remove_image(id, Some(RemoveImageOptionsBuilder::default().force(force).build()), None)
            .await
            .map(|_| ())
            .map_err(err),
        ResourceKind::Volume => docker
            .remove_volume(id, Some(RemoveVolumeOptionsBuilder::default().force(force).build()))
            .await
            .map_err(err),
        ResourceKind::Network => docker.remove_network(id).await.map_err(err),
    }
}

pub async fn prune(docker: &Docker, kind: ResourceKind) -> Result<(u64, u64), String> {
    match kind {
        ResourceKind::Image => {
            let filters = HashMap::from([("dangling", vec!["false"])]);
            let r = docker
                .prune_images(Some(PruneImagesOptionsBuilder::default().filters(&filters).build()))
                .await
                .map_err(err)?;
            Ok((r.images_deleted.map(|d| d.iter().filter(|i| i.deleted.is_some()).count()).unwrap_or(0) as u64, r.space_reclaimed.unwrap_or(0).max(0) as u64))
        }
        ResourceKind::Volume => {
            let filters = HashMap::from([("all", vec!["true"])]);
            let r = docker
                .prune_volumes(Some(PruneVolumesOptionsBuilder::default().filters(&filters).build()))
                .await
                .map_err(err)?;
            Ok((r.volumes_deleted.map(|d| d.len()).unwrap_or(0) as u64, r.space_reclaimed.unwrap_or(0).max(0) as u64))
        }
        ResourceKind::Network => {
            let r = docker.prune_networks(None::<PruneNetworksOptions>).await.map_err(err)?;
            Ok((r.networks_deleted.map(|d| d.len()).unwrap_or(0) as u64, 0))
        }
    }
}

pub fn size(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 { format!("{v:.0} {}", UNITS[i]) } else { format!("{v:.1} {}", UNITS[i]) }
}
