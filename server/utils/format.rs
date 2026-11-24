pub fn ago(secs: i64) -> String {
    let secs = secs.max(0);
    match secs {
        0..=59 => "just now".into(),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86399 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}

pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 || v >= 100.0 {
        format!("{v:.0} {}", UNITS[i])
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

pub fn rate(n: u64) -> String {
    format!("{}/s", bytes(n))
}

pub fn duration(secs: u64) -> String {
    let (d, h, m) = (secs / 86400, secs % 86400 / 3600, secs % 3600 / 60);
    match (d, h) {
        (0, 0) => format!("{m}m"),
        (0, _) => format!("{h}h {m}m"),
        _ => format!("{d}d {h}h"),
    }
}

pub fn bits(bytes_per_sec: u64) -> String {
    let b = bytes_per_sec as f64 * 8.0;
    let (v, unit) = match b {
        b if b >= 1e9 => (b / 1e9, "Gbps"),
        b if b >= 1e6 => (b / 1e6, "Mbps"),
        b if b >= 1e3 => (b / 1e3, "Kbps"),
        b => (b, "bps"),
    };
    if unit == "bps" || v >= 100.0 { format!("{v:.0} {unit}") } else { format!("{v:.1} {unit}") }
}

fn short(v: f64, base: f64) -> String {
    let units = ["", "K", "M", "G", "T"];
    let mut v = v;
    let mut i = 0;
    while v >= base && i < units.len() - 1 {
        v /= base;
        i += 1;
    }
    if i == 0 || v >= 100.0 {
        format!("{v:.0}{}", units[i])
    } else {
        format!("{v:.1}{}", units[i])
    }
}

pub fn short_bits(bytes_per_sec: u64) -> String {
    short(bytes_per_sec as f64 * 8.0, 1000.0)
}

pub fn short_bytes(n: u64) -> String {
    short(n as f64, 1024.0)
}

pub fn short_id(id: &str) -> &str {
    let id = id.strip_prefix("sha256:").unwrap_or(id);
    &id[..id.len().min(12)]
}
