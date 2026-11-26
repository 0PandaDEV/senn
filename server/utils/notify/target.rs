use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

#[derive(Debug, Clone)]
pub struct Target {
    pub scheme: String,
    pub user: Option<String>,
    pub pass: Option<String>,
    pub host: String,
    pub port: Option<u16>,
    pub path: Vec<String>,
    pub params: Vec<(String, String)>,
}

fn decode(s: &str) -> String {
    percent_decode_str(s).decode_utf8_lossy().into_owned()
}

pub fn encode(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

pub fn split(s: &str) -> Vec<String> {
    s.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|x| !x.is_empty())
        .map(String::from)
        .collect()
}

pub fn parse(raw: &str) -> Result<Target, String> {
    let raw = raw.trim();
    let (scheme, rest) = raw
        .split_once("://")
        .ok_or_else(|| format!("not a notification URL: {raw}"))?;
    let (rest, query) = rest.split_once('?').unwrap_or((rest, ""));
    let params = query
        .split('&')
        .filter(|kv| !kv.is_empty())
        .map(|kv| {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            (decode(k), decode(v))
        })
        .collect();
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let (auth, hostport) = match authority.rsplit_once('@') {
        Some((a, h)) => (Some(a).filter(|a| !a.is_empty()), h),
        None => (None, authority),
    };
    let (user, pass) = match auth.map(|a| a.split_once(':')) {
        Some(Some((u, p))) => (Some(decode(u)), Some(decode(p))),
        Some(None) => (auth.map(decode), None),
        None => (None, None),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => {
            let port = p.parse().map_err(|_| format!("bad port in {raw}"))?;
            (h, Some(port))
        }
        _ => (hostport, None),
    };
    Ok(Target {
        scheme: scheme.to_ascii_lowercase(),
        user,
        pass,
        host: decode(host),
        port,
        path: path
            .split('/')
            .filter(|p| !p.is_empty())
            .map(decode)
            .collect(),
        params,
    })
}

impl Target {
    pub fn authority(&self) -> String {
        match self.port {
            Some(p) => format!("{}:{p}", self.host),
            None => self.host.clone(),
        }
    }

    pub fn secure(&self) -> bool {
        self.scheme.ends_with('s')
    }

    pub fn base(&self) -> String {
        format!(
            "{}://{}",
            if self.secure() { "https" } else { "http" },
            self.authority()
        )
    }

    pub fn q(&self, key: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
            .filter(|v| !v.is_empty())
    }

    pub fn list(&self, key: &str) -> Vec<String> {
        self.q(key).map(split).unwrap_or_default()
    }

    pub fn targets(&self) -> Vec<String> {
        let mut v = self.path.clone();
        v.extend(self.list("to"));
        v
    }

    pub fn prefixed(&self, prefix: char) -> impl Iterator<Item = (&str, &str)> {
        self.params
            .iter()
            .filter_map(move |(k, v)| k.strip_prefix(prefix).map(|k| (k, v.as_str())))
    }

    pub fn first(&self) -> Result<String, String> {
        self.path
            .first()
            .cloned()
            .ok_or_else(|| format!("{}:// URL is missing a path segment", self.scheme))
    }

    pub fn split_token(&self) -> (String, String) {
        let token = self.path.last().cloned().unwrap_or_default();
        let prefix = self.path[..self.path.len().saturating_sub(1)].join("/");
        let prefix = if prefix.is_empty() {
            String::new()
        } else {
            format!("/{prefix}")
        };
        (prefix, token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_parts() {
        let t = parse("matrixs://bot:p%40ss@matrix.org:8448/#ops:matrix.org/@me?Mode=x&+X-Key=1")
            .unwrap();
        assert_eq!(t.scheme, "matrixs");
        assert_eq!(t.user.as_deref(), Some("bot"));
        assert_eq!(t.pass.as_deref(), Some("p@ss"));
        assert_eq!(t.host, "matrix.org");
        assert_eq!(t.port, Some(8448));
        assert_eq!(t.path, ["#ops:matrix.org", "@me"]);
        assert_eq!(t.q("mode"), Some("x"));
        assert_eq!(t.prefixed('+').collect::<Vec<_>>(), [("X-Key", "1")]);
        assert_eq!(t.base(), "https://matrix.org:8448");
    }

    #[test]
    fn keeps_token_colons() {
        let t = parse("tgram://123456:ABC-def/-100200").unwrap();
        assert_eq!(t.authority(), "123456:ABC-def");
        assert_eq!(t.port, None);
        let t = parse("msteams://team/a@b/c/d/V2x").unwrap();
        assert_eq!(t.user, None);
        assert_eq!(t.path, ["a@b", "c", "d", "V2x"]);
    }
}
