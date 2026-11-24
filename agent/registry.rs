use std::time::Duration;

use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::{Client, StatusCode, header};

#[derive(Debug, Clone)]
pub struct ImageRef {
    pub registry: String,
    pub repo: String,
    pub tag: String,
    pub pinned: bool,
}

pub fn parse(image: &str) -> ImageRef {
    let (name, pinned) = match image.split_once('@') {
        Some((name, _)) => (name, true),
        None => (image, false),
    };
    let (path, tag) = match name.rsplit_once(':') {
        Some((p, t)) if !t.contains('/') => (p, t.to_string()),
        _ => (name, "latest".to_string()),
    };
    let (registry, repo) = match path.split_once('/') {
        Some((first, rest))
            if first.contains('.') || first.contains(':') || first == "localhost" =>
        {
            (first.to_string(), rest.to_string())
        }
        _ => ("registry-1.docker.io".to_string(), path.to_string()),
    };
    let repo = if registry == "registry-1.docker.io" && !repo.contains('/') {
        format!("library/{repo}")
    } else {
        repo
    };
    ImageRef {
        registry,
        repo,
        tag,
        pinned,
    }
}

pub fn client() -> Client {
    Client::builder()
        .user_agent("senn-agent")
        .timeout(Duration::from_secs(20))
        .build()
        .expect("http client")
}

fn credentials(registry: &str) -> Option<String> {
    let dir = std::env::var_os("DOCKER_CONFIG")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".docker")))?;
    let text = std::fs::read_to_string(dir.join("config.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let auths = json.get("auths")?.as_object()?;
    let keys: Vec<&str> = if registry == "registry-1.docker.io" {
        vec![
            "https://index.docker.io/v1/",
            "index.docker.io",
            "docker.io",
            "registry-1.docker.io",
        ]
    } else {
        vec![registry]
    };
    keys.iter()
        .find_map(|k| auths.get(*k))
        .and_then(|v| v.get("auth"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn challenge_param(header: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = header.find(&needle)? + needle.len();
    let end = header[start..].find('"')? + start;
    Some(header[start..end].to_string())
}

const ACCEPT: &str = "application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json";

pub async fn remote_digest(client: &Client, r: &ImageRef) -> Result<String, String> {
    let scheme = if r.registry.starts_with("localhost") || r.registry.starts_with("127.") {
        "http"
    } else {
        "https"
    };
    let url = format!(
        "{scheme}://{}/v2/{}/manifests/{}",
        r.registry, r.repo, r.tag
    );
    let head = |token: Option<String>| {
        let mut req = client.head(&url).header(header::ACCEPT, ACCEPT);
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        req
    };
    let mut resp = head(None).send().await.map_err(|e| e.to_string())?;
    if resp.status() == StatusCode::UNAUTHORIZED {
        let challenge = resp
            .headers()
            .get(header::WWW_AUTHENTICATE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        if challenge.to_lowercase().starts_with("bearer") {
            let realm = challenge_param(&challenge, "realm").ok_or("no auth realm")?;
            let mut req = client.get(&realm);
            if let Some(service) = challenge_param(&challenge, "service") {
                req = req.query(&[("service", service)]);
            }
            let scope = challenge_param(&challenge, "scope")
                .unwrap_or_else(|| format!("repository:{}:pull", r.repo));
            req = req.query(&[("scope", scope)]);
            if let Some(auth) = credentials(&r.registry) {
                req = req.header(header::AUTHORIZATION, format!("Basic {auth}"));
            }
            let body: serde_json::Value = req
                .send()
                .await
                .map_err(|e| e.to_string())?
                .json()
                .await
                .map_err(|e| e.to_string())?;
            let token = body
                .get("token")
                .or_else(|| body.get("access_token"))
                .and_then(|t| t.as_str())
                .ok_or("registry returned no token")?
                .to_string();
            resp = head(Some(token)).send().await.map_err(|e| e.to_string())?;
        } else if let Some(auth) = credentials(&r.registry) {
            let decoded = STANDARD.decode(auth).map_err(|e| e.to_string())?;
            let text = String::from_utf8_lossy(&decoded).to_string();
            let (user, pass) = text.split_once(':').unwrap_or(("", ""));
            resp = client
                .head(&url)
                .header(header::ACCEPT, ACCEPT)
                .basic_auth(user, Some(pass))
                .send()
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    if !resp.status().is_success() {
        return Err(format!("registry answered {}", resp.status()));
    }
    resp.headers()
        .get("docker-content-digest")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .ok_or_else(|| "registry sent no digest".into())
}
