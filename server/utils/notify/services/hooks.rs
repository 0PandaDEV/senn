use serde_json::{Map, Value, json};

use super::{Outcome, check, client, need};
use crate::server::utils::notify::{Message, target::Target};

enum Format {
    Json,
    Form,
    Xml,
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn fields(t: &Target, m: &Message) -> Vec<(String, Value)> {
    let mut fields: Vec<(String, Value)> = vec![
        ("version".into(), "1.0".into()),
        ("title".into(), m.title.clone().into()),
        ("message".into(), m.body.clone().into()),
        ("type".into(), m.kind().into()),
        ("url".into(), m.click.clone().into()),
    ];
    for (k, v) in t.prefixed(':') {
        match fields.iter().position(|(n, _)| n == k) {
            Some(i) if v.is_empty() => {
                fields.remove(i);
            }
            Some(i) => fields[i].0 = v.to_string(),
            None => fields.push((k.to_string(), v.into())),
        }
    }
    fields
}

fn plain(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

async fn custom(t: Target, m: Message, format: Format) -> Outcome {
    let url = format!("{}/{}", t.base(), t.path.join("/"));
    let method = t.q("method").unwrap_or("POST").to_ascii_uppercase();
    let method = reqwest::Method::from_bytes(method.as_bytes())
        .map_err(|_| format!("unknown HTTP method {method}"))?;
    let fields = fields(&t, &m);
    let mut req = client().request(method.clone(), url);
    let params: Vec<(&str, &str)> = t.prefixed('-').collect();
    if !params.is_empty() {
        req = req.query(&params);
    }
    for (k, v) in t.prefixed('+') {
        req = req.header(k, v);
    }
    if let Some(u) = &t.user {
        req = req.basic_auth(u, t.pass.as_ref());
    }
    req = if method == reqwest::Method::GET {
        let q: Vec<(String, String)> = fields.iter().map(|(k, v)| (k.clone(), plain(v))).collect();
        req.query(&q)
    } else {
        match format {
            Format::Json => req.json(&fields.into_iter().collect::<Map<_, _>>()),
            Format::Form => {
                let f: Vec<(String, String)> =
                    fields.iter().map(|(k, v)| (k.clone(), plain(v))).collect();
                req.form(&f)
            }
            Format::Xml => {
                let inner: String = fields
                    .iter()
                    .map(|(k, v)| format!("<{k}>{}</{k}>", escape(&plain(v))))
                    .collect();
                req.header("Content-Type", "application/xml; charset=utf-8").body(format!(
                    "<?xml version=\"1.0\" encoding=\"utf-8\"?><soapenv:Envelope xmlns:soapenv=\"http://schemas.xmlsoap.org/soap/envelope/\"><soapenv:Body><Notification>{inner}</Notification></soapenv:Body></soapenv:Envelope>"
                ))
            }
        }
    };
    check(req.send().await, "webhook").await
}

pub async fn json(t: Target, m: Message) -> Outcome {
    custom(t, m, Format::Json).await
}

pub async fn form(t: Target, m: Message) -> Outcome {
    custom(t, m, Format::Form).await
}

pub async fn xml(t: Target, m: Message) -> Outcome {
    custom(t, m, Format::Xml).await
}

pub async fn apprise(t: Target, m: Message) -> Outcome {
    let (prefix, token) = t.split_token();
    let token = need(Some(token).filter(|k| !k.is_empty()), "config key")?;
    let mut body = json!({ "title": m.title, "body": m.text(), "type": m.kind() });
    if let Some(tags) = t.q("tags").or(t.q("tag")) {
        body["tag"] = tags.into();
    }
    let mut req = client()
        .post(format!("{}{prefix}/notify/{token}", t.base()))
        .json(&body);
    if let Some(u) = &t.user {
        req = req.basic_auth(u, t.pass.as_ref());
    }
    check(req.send().await, "apprise").await
}

pub async fn ifttt(t: Target, m: Message) -> Outcome {
    let events = t.targets();
    if events.is_empty() {
        return Err("ifttt URL is missing an event name".into());
    }
    let body = json!({ "value1": m.title, "value2": m.text(), "value3": m.kind() });
    for event in events {
        check(
            client()
                .post(format!(
                    "https://maker.ifttt.com/trigger/{event}/with/key/{}",
                    t.host
                ))
                .json(&body)
                .send()
                .await,
            "ifttt",
        )
        .await?;
    }
    Ok(())
}

pub async fn home_assistant(t: Target, m: Message) -> Outcome {
    let (prefix, token) = t.split_token();
    let token = need(Some(token).filter(|k| !k.is_empty()), "access token")?;
    let base = format!(
        "{}://{}:{}{prefix}",
        if t.secure() { "https" } else { "http" },
        t.host,
        t.port.unwrap_or(8123)
    );
    let services = t.list("to");
    if services.is_empty() {
        let mut body = json!({ "title": m.title, "message": m.text() });
        if let Some(id) = t.q("nid") {
            body["notification_id"] = id.into();
        }
        return check(
            client()
                .post(format!(
                    "{base}/api/services/persistent_notification/create"
                ))
                .bearer_auth(&token)
                .json(&body)
                .send()
                .await,
            "home assistant",
        )
        .await;
    }
    for svc in services {
        let svc = svc.trim_start_matches("notify.");
        let mut body = json!({ "title": m.title, "message": m.body });
        if let Some(c) = &m.click {
            body["data"] = json!({ "url": c, "clickAction": c });
        }
        check(
            client()
                .post(format!("{base}/api/services/notify/{svc}"))
                .bearer_auth(&token)
                .json(&body)
                .send()
                .await,
            "home assistant",
        )
        .await?;
    }
    Ok(())
}
