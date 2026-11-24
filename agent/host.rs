use std::time::Instant;

use std::collections::HashMap;

use crate::{
    docker::rate,
    proto::{HostExtra, HostStats},
};

#[derive(Default)]
pub struct Prev {
    at: Option<Instant>,
    disk: (u64, u64),
    disk_at: Option<Instant>,
    busy: u64,
    total: u64,
    rx: u64,
    tx: u64,
    read: u64,
    write: u64,
    cpu: [u64; 8],
    ctxt: u64,
    intr: u64,
    rx_packets: u64,
    tx_packets: u64,
    rx_errors: u64,
    tx_errors: u64,
    read_ops: u64,
    write_ops: u64,
    ticks: HashMap<String, u64>,
    swap_in: u64,
    swap_out: u64,
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn cpu_times() -> (u64, u64) {
    let stat = read("/proc/stat");
    let Some(line) = stat.lines().next() else { return (0, 0) };
    let v: Vec<u64> = line.split_whitespace().skip(1).take(8).filter_map(|x| x.parse().ok()).collect();
    let total: u64 = v.iter().sum();
    let idle = v.get(3).copied().unwrap_or(0) + v.get(4).copied().unwrap_or(0);
    (total.saturating_sub(idle), total)
}

fn meminfo() -> (u64, u64, u64, u64) {
    let text = read("/proc/meminfo");
    let get = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
            .unwrap_or(0)
            * 1024
    };
    let total = get("MemTotal:");
    let swap_total = get("SwapTotal:");
    (total, total.saturating_sub(get("MemAvailable:")), swap_total, swap_total.saturating_sub(get("SwapFree:")))
}

fn virtual_name(name: &str) -> bool {
    name == "lo"
        || name.contains('.')
        || ["veth", "docker", "br-", "virbr", "tun", "tap", "cni", "flannel", "tailscale", "wg", "zt", "ppp", "dummy", "ifb", "vxlan", "bond", "macvlan", "kube", "cali"]
            .iter()
            .any(|p| name.starts_with(p))
}

fn interfaces() -> Vec<(String, Vec<u64>)> {
    let rows: Vec<(String, Vec<u64>)> = read("/proc/net/dev")
        .lines()
        .skip(2)
        .filter_map(|l| {
            let (name, rest) = l.split_once(':')?;
            let v: Vec<u64> = rest.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            (v.len() >= 12).then(|| (name.trim().to_string(), v))
        })
        .collect();
    let physical: Vec<_> = rows
        .iter()
        .filter(|(n, _)| n != "lo" && std::path::Path::new(&format!("/sys/class/net/{n}/device")).exists())
        .cloned()
        .collect();
    if !physical.is_empty() {
        return physical;
    }
    rows.into_iter().filter(|(n, _)| !virtual_name(n)).collect()
}

fn net() -> (u64, u64) {
    interfaces().iter().fold((0, 0), |(r, t), (_, v)| (r + v[0], t + v[8]))
}

fn whole_disk(name: &str) -> bool {
    let digits_tail = |prefix: &str| name.strip_prefix(prefix).is_some_and(|r| !r.is_empty() && r.chars().all(|c| c.is_ascii_lowercase()));
    if digits_tail("sd") || digits_tail("vd") || digits_tail("xvd") || digits_tail("hd") {
        return true;
    }
    if let Some(r) = name.strip_prefix("nvme") {
        return r.contains('n') && !r.contains('p');
    }
    if let Some(r) = name.strip_prefix("mmcblk") {
        return !r.is_empty() && r.chars().all(|c| c.is_ascii_digit());
    }
    false
}

fn disk_io() -> (u64, u64) {
    read("/proc/diskstats")
        .lines()
        .filter_map(|l| {
            let v: Vec<&str> = l.split_whitespace().collect();
            if !whole_disk(v.get(2)?) {
                return None;
            }
            Some((v.get(5)?.parse::<u64>().ok()? * 512, v.get(9)?.parse::<u64>().ok()? * 512))
        })
        .fold((0, 0), |(r, w), (a, b)| (r + a, w + b))
}

async fn disk_usage(path: &str) -> (u64, u64) {
    let out = tokio::process::Command::new("df").args(["-B1", "--output=size,used", path]).output().await;
    let Ok(out) = out else { return (0, 0) };
    let text = String::from_utf8_lossy(&out.stdout);
    let v: Vec<u64> = text.lines().nth(1).unwrap_or("").split_whitespace().filter_map(|x| x.parse().ok()).collect();
    (v.first().copied().unwrap_or(0), v.get(1).copied().unwrap_or(0))
}

fn stat_line(stat: &str, key: &str) -> u64 {
    stat.lines()
        .find_map(|l| l.strip_prefix(key))
        .and_then(|v| v.split_whitespace().next())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

fn cpu_fields(stat: &str) -> [u64; 8] {
    let mut out = [0; 8];
    if let Some(line) = stat.lines().next() {
        for (i, v) in line.split_whitespace().skip(1).take(8).enumerate() {
            out[i] = v.parse().unwrap_or(0);
        }
    }
    out
}

fn net_extra() -> (u64, u64, u64, u64) {
    interfaces()
        .iter()
        .fold((0, 0, 0, 0), |a, (_, v)| (a.0 + v[1], a.1 + v[9], a.2 + v[2] + v[3], a.3 + v[10] + v[11]))
}

struct Disks {
    read_ops: u64,
    write_ops: u64,
    ticks: HashMap<String, u64>,
}

fn disk_ops() -> Disks {
    let mut out = Disks { read_ops: 0, write_ops: 0, ticks: HashMap::new() };
    for l in read("/proc/diskstats").lines() {
        let v: Vec<&str> = l.split_whitespace().collect();
        let Some(name) = v.get(2) else { continue };
        if !whole_disk(name) {
            continue;
        }
        let num = |i: usize| v.get(i).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
        out.read_ops += num(3);
        out.write_ops += num(7);
        out.ticks.insert((*name).to_string(), num(12));
    }
    out
}

fn vmstat(key: &str) -> u64 {
    read("/proc/vmstat")
        .lines()
        .find_map(|l| l.strip_prefix(key).and_then(|v| v.trim().parse::<u64>().ok()))
        .unwrap_or(0)
}

fn pressure(kind: &str) -> (Option<f32>, Option<f32>) {
    let text = read(&format!("/proc/pressure/{kind}"));
    let avg = |prefix: &str| {
        text.lines()
            .find(|l| l.starts_with(prefix))
            .and_then(|l| l.split_whitespace().find_map(|f| f.strip_prefix("avg10=")))
            .and_then(|v| v.parse::<f32>().ok())
    };
    (avg("some"), avg("full"))
}

fn temperature() -> Option<f32> {
    let mut best: Option<f32> = None;
    let mut consider = |path: std::path::PathBuf| {
        if let Some(milli) = std::fs::read_to_string(path).ok().and_then(|t| t.trim().parse::<f32>().ok()) {
            let c = milli / 1000.0;
            if c > 0.0 && c < 150.0 {
                best = Some(best.map_or(c, |b| b.max(c)));
            }
        }
    };
    for dir in std::fs::read_dir("/sys/class/thermal").into_iter().flatten().flatten() {
        if dir.file_name().to_string_lossy().starts_with("thermal_zone") {
            consider(dir.path().join("temp"));
        }
    }
    for dir in std::fs::read_dir("/sys/class/hwmon").into_iter().flatten().flatten() {
        for entry in std::fs::read_dir(dir.path()).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("temp") && name.ends_with("_input") {
                consider(entry.path());
            }
        }
    }
    best
}

fn mem_field(key: &str) -> u64 {
    read("/proc/meminfo")
        .lines()
        .find_map(|l| l.strip_prefix(key))
        .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
        .unwrap_or(0)
        * 1024
}

pub async fn sample(prev: &mut Prev, stacks_dir: &str) -> HostStats {
    let now = Instant::now();
    let (busy, total) = cpu_times();
    let (rx, tx) = net();
    let (rd, wr) = disk_io();
    let (mem_total, mem_used, swap_total, swap_used) = meminfo();
    if prev.disk_at.is_none_or(|t| t.elapsed().as_secs() >= 15) {
        prev.disk = disk_usage(stacks_dir).await;
        prev.disk_at = Some(now);
    }
    let (disk_total, disk_used) = prev.disk;
    let stat = read("/proc/stat");
    let cpu_now = cpu_fields(&stat);
    let ctxt = stat_line(&stat, "ctxt ");
    let intr = stat_line(&stat, "intr ");
    let (rxp, txp, rxe, txe) = net_extra();
    let disks = disk_ops();
    let swap_in = vmstat("pswpin ");
    let swap_out = vmstat("pswpout ");
    let load: Vec<f32> = read("/proc/loadavg").split_whitespace().take(3).filter_map(|x| x.parse().ok()).collect();
    let uptime = read("/proc/uptime").split_whitespace().next().and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0) as u64;
    let mut stats = HostStats {
        cores: std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1),
        load: [load.first().copied().unwrap_or(0.0), load.get(1).copied().unwrap_or(0.0), load.get(2).copied().unwrap_or(0.0)],
        mem_total,
        mem_used,
        swap_total,
        swap_used,
        disk_total,
        disk_used,
        uptime,
        kernel: read("/proc/sys/kernel/osrelease").trim().to_string(),
        ..Default::default()
    };
    if let Some(at) = prev.at {
        let secs = now.duration_since(at).as_secs_f64().max(0.001);
        let dt = total.saturating_sub(prev.total);
        stats.cpu = if dt > 0 { busy.saturating_sub(prev.busy) as f32 / dt as f32 * 100.0 } else { 0.0 };
        stats.net_rx = rate(rx, prev.rx, secs);
        stats.net_tx = rate(tx, prev.tx, secs);
        stats.io_read = rate(rd, prev.read, secs);
        stats.io_write = rate(wr, prev.write, secs);
        let dt_cpu = cpu_now.iter().sum::<u64>().saturating_sub(prev.cpu.iter().sum::<u64>()).max(1) as f32;
        let part = |i: usize| cpu_now[i].saturating_sub(prev.cpu[i]) as f32;
        let loadavg = read("/proc/loadavg");
        let procs_total = loadavg
            .split_whitespace()
            .nth(3)
            .and_then(|f| f.split_once('/'))
            .and_then(|(_, t)| t.parse().ok())
            .unwrap_or(0);
        let busy_max = disks
            .ticks
            .iter()
            .map(|(name, t)| t.saturating_sub(prev.ticks.get(name).copied().unwrap_or(*t)) as f64 / (secs * 1000.0) * 100.0)
            .fold(0.0_f64, f64::max)
            .min(100.0);
        let (cpu_p, _) = pressure("cpu");
        let (mem_p, mem_full) = pressure("memory");
        let (io_p, io_full) = pressure("io");
        stats.extra = Some(HostExtra {
            cpu_user: (part(0) + part(1)) / dt_cpu * 100.0,
            cpu_system: part(2) / dt_cpu * 100.0,
            cpu_iowait: part(4) / dt_cpu * 100.0,
            cpu_steal: part(7) / dt_cpu * 100.0,
            cpu_irq: (part(5) + part(6)) / dt_cpu * 100.0,
            mem_free: mem_field("MemFree:"),
            mem_cached: mem_field("Cached:"),
            mem_buffers: mem_field("Buffers:"),
            mem_available: mem_field("MemAvailable:"),
            swap_in: rate(swap_in, prev.swap_in, secs) * 4096,
            swap_out: rate(swap_out, prev.swap_out, secs) * 4096,
            io_read_ops: rate(disks.read_ops, prev.read_ops, secs),
            io_write_ops: rate(disks.write_ops, prev.write_ops, secs),
            io_busy: busy_max as f32,
            net_rx_packets: rate(rxp, prev.rx_packets, secs),
            net_tx_packets: rate(txp, prev.tx_packets, secs),
            net_rx_errors: rate(rxe, prev.rx_errors, secs),
            net_tx_errors: rate(txe, prev.tx_errors, secs),
            ctx_switches: rate(ctxt, prev.ctxt, secs),
            interrupts: rate(intr, prev.intr, secs),
            procs_running: stat_line(&stat, "procs_running "),
            procs_blocked: stat_line(&stat, "procs_blocked "),
            procs_total,
            tcp_inuse: read("/proc/net/sockstat")
                .lines()
                .find(|l| l.starts_with("TCP:"))
                .and_then(|l| l.split_whitespace().skip_while(|f| *f != "inuse").nth(1))
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            open_files: read("/proc/sys/fs/file-nr").split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0),
            pressure_cpu: cpu_p,
            pressure_mem: mem_p,
            pressure_mem_full: mem_full,
            pressure_io: io_p,
            pressure_io_full: io_full,
            temp: temperature(),
        });
    }
    *prev = Prev {
        at: Some(now),
        disk: prev.disk,
        disk_at: prev.disk_at,
        busy,
        total,
        rx,
        tx,
        read: rd,
        write: wr,
        cpu: cpu_now,
        ctxt,
        intr,
        rx_packets: rxp,
        tx_packets: txp,
        rx_errors: rxe,
        tx_errors: txe,
        read_ops: disks.read_ops,
        write_ops: disks.write_ops,
        ticks: disks.ticks,
        swap_in,
        swap_out,
    };
    stats
}

