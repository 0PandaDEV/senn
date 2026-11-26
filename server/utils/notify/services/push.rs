use serde_json::{Value, json};

use super::{Outcome, check, check_json, client, need};
use crate::server::utils::notify::{Message, target::Target};

pub async fn ntfy(t: Target, m: Message) -> Outcome {
    let (base, topics) = if t.path.is_empty() {
        ("https://ntfy.sh".to_string(), vec![t.host.clone()])
    } else {
        (t.base(), t.path.clone())
    };
    let priority = t
        .q("priority")
        .and_then(|p| p.parse::<u8>().ok())
        .unwrap_or(m.priority);
    let tags: Vec<&str> = t.q("tags").unwrap_or(m.tags).split(',').collect();
    for topic in topics {
        let mut body = json!({
            "topic": topic,
            "title": m.title,
            "message": m.body,
            "priority": priority,
            "tags": tags,
        });
        if let Some(c) = &m.click {
            body["click"] = c.clone().into();
        }
        if let Some(i) = t.q("icon").map(String::from).or_else(|| m.icon()) {
            body["icon"] = i.into();
        }
        let mut req = client().post(format!("{base}/")).json(&body);
        if let Some(token) = t.q("token") {
            req = req.bearer_auth(token);
        } else if let Some(u) = &t.user {
            req = req.basic_auth(u, t.pass.as_ref());
        }
        check(req.send().await, "ntfy").await?;
    }
    Ok(())
}

pub async fn gotify(t: Target, m: Message) -> Outcome {
    let (prefix, token) = t.split_token();
    let mut body = json!({
        "title": m.title,
        "message": m.body,
        "priority": t.q("priority").and_then(|p| p.parse::<u32>().ok()).unwrap_or(m.priority as u32 * 2),
    });
    if let Some(c) = &m.click {
        body["extras"] = json!({ "client::notification": { "click": { "url": c } } });
    }
    let url = format!("{}{prefix}/message", t.base());
    check(
        client()
            .post(url)
            .header("X-Gotify-Key", token)
            .json(&body)
            .send()
            .await,
        "gotify",
    )
    .await
}

pub async fn pushover(t: Target, m: Message) -> Outcome {
    let user = need(t.user.clone(), "user key")?;
    let mut body = json!({
        "token": t.host,
        "user": user,
        "title": m.title,
        "message": m.body,
        "priority": t.q("priority").and_then(|p| p.parse::<i32>().ok()).unwrap_or((m.priority as i32 - 3).clamp(-1, 1)),
    });
    if !t.path.is_empty() {
        body["device"] = t.path.join(",").into();
    }
    if let Some(s) = t.q("sound") {
        body["sound"] = s.into();
    }
    if let Some(c) = &m.click {
        body["url"] = c.clone().into();
    }
    check(
        client()
            .post("https://api.pushover.net/1/messages.json")
            .json(&body)
            .send()
            .await,
        "pushover",
    )
    .await
}

pub async fn pushbullet(t: Target, m: Message) -> Outcome {
    let base = json!({ "type": "note", "title": m.title, "body": m.text() });
    let targets = t.targets();
    let bodies: Vec<Value> = if targets.is_empty() {
        vec![base]
    } else {
        targets
            .iter()
            .map(|x| {
                let mut b = base.clone();
                if let Some(ch) = x.strip_prefix('#') {
                    b["channel_tag"] = ch.into();
                } else if x.contains('@') {
                    b["email"] = x.as_str().into();
                } else {
                    b["device_iden"] = x.as_str().into();
                }
                b
            })
            .collect()
    };
    for body in bodies {
        check(
            client()
                .post("https://api.pushbullet.com/v2/pushes")
                .header("Access-Token", t.host.clone())
                .json(&body)
                .send()
                .await,
            "pushbullet",
        )
        .await?;
    }
    Ok(())
}

pub async fn bark(t: Target, m: Message) -> Outcome {
    let keys = t.targets();
    if keys.is_empty() {
        return Err("bark URL is missing a device key".into());
    }
    let level = t.q("level").unwrap_or(if m.priority >= 4 {
        "timeSensitive"
    } else {
        "active"
    });
    for key in keys {
        let mut body = json!({
            "device_key": key,
            "title": m.title,
            "body": m.body,
            "group": t.q("group").unwrap_or("senn"),
            "level": level,
        });
        if let Some(i) = t.q("icon").map(String::from).or_else(|| m.icon()) {
            body["icon"] = i.into();
        }
        for k in [
            "sound",
            "badge",
            "call",
            "image",
            "volume",
            "isArchive",
            "copy",
            "autoCopy",
            "subtitle",
        ] {
            if let Some(v) = t.q(k) {
                body[k] = v.into();
            }
        }
        if let Some(c) = &m.click {
            body["url"] = c.clone().into();
        }
        let mut req = client().post(format!("{}/push", t.base())).json(&body);
        if let Some(u) = &t.user {
            req = req.basic_auth(u, t.pass.as_ref());
        }
        check(req.send().await, "bark").await?;
    }
    Ok(())
}

pub async fn nextcloud(t: Target, m: Message) -> Outcome {
    let user = need(t.user.clone(), "user")?;
    let targets = t.targets();
    let targets = if targets.is_empty() {
        vec![user.clone()]
    } else {
        targets
    };
    for who in targets {
        let url = format!(
            "{}/ocs/v2.php/apps/admin_notifications/api/v1/notifications/{who}",
            t.base()
        );
        check(
            client()
                .post(url)
                .basic_auth(&user, t.pass.as_ref())
                .header("OCS-APIREQUEST", "true")
                .form(&[
                    ("shortMessage", m.title.as_str()),
                    ("longMessage", m.text().as_str()),
                ])
                .send()
                .await,
            "nextcloud",
        )
        .await?;
    }
    Ok(())
}

pub async fn pushsafer(t: Target, m: Message) -> Outcome {
    let device = t.path.first().cloned().unwrap_or_else(|| "a".into());
    let mut form = vec![
        ("k", t.host.clone()),
        ("t", m.title.clone()),
        ("m", m.body.clone()),
        ("d", device),
    ];
    for (key, q) in [
        ("s", "sound"),
        ("v", "vibration"),
        ("i", "icon"),
        ("pr", "priority"),
    ] {
        if let Some(v) = t.q(q) {
            form.push((key, v.to_string()));
        }
    }
    if let Some(c) = &m.click {
        form.push(("u", c.clone()));
    }
    check_json(
        client()
            .post("https://www.pushsafer.com/api")
            .form(&form)
            .send()
            .await,
        "pushsafer",
        |v| v["status"] == 1,
    )
    .await
}

pub async fn simplepush(t: Target, m: Message) -> Outcome {
    let mut form = vec![
        ("key", t.host.clone()),
        ("title", m.title.clone()),
        ("msg", m.text()),
    ];
    if let Some(e) = t.q("event") {
        form.push(("event", e.to_string()));
    }
    check_json(
        client()
            .post("https://api.simplepush.io/send")
            .form(&form)
            .send()
            .await,
        "simplepush",
        |v| v["status"] == "OK",
    )
    .await
}

pub async fn prowl(t: Target, m: Message) -> Outcome {
    let priority = t
        .q("priority")
        .map(String::from)
        .unwrap_or_else(|| (m.priority as i32 - 3).clamp(-2, 2).to_string());
    let mut form = vec![
        ("apikey", t.host.clone()),
        ("application", "senn".to_string()),
        ("event", m.title.clone()),
        ("description", m.body.clone()),
        ("priority", priority),
    ];
    if let Some(p) = t.path.first() {
        form.push(("providerkey", p.clone()));
    }
    if let Some(c) = &m.click {
        form.push(("url", c.clone()));
    }
    check(
        client()
            .post("https://api.prowlapp.com/publicapi/add")
            .form(&form)
            .send()
            .await,
        "prowl",
    )
    .await
}

pub async fn join(t: Target, m: Message) -> Outcome {
    let devices = t.targets();
    let devices = if devices.is_empty() {
        "group.all".to_string()
    } else {
        devices.join(",")
    };
    let mut query = vec![
        ("apikey", t.host.clone()),
        ("deviceIds", devices),
        ("title", m.title.clone()),
        ("text", m.body.clone()),
    ];
    if let Some(c) = &m.click {
        query.push(("url", c.clone()));
    }
    check_json(
        client()
            .get("https://joinjoaomgcd.appspot.com/_ah/api/messaging/v1/sendPush")
            .query(&query)
            .send()
            .await,
        "join",
        |v| v["success"] == true,
    )
    .await
}

pub async fn pushdeer(t: Target, m: Message) -> Outcome {
    let (base, key) = match t.path.first() {
        Some(k) => (t.base(), k.clone()),
        None => ("https://api2.pushdeer.com".to_string(), t.authority()),
    };
    check_json(
        client()
            .post(format!("{base}/message/push"))
            .form(&[
                ("pushkey", key.as_str()),
                ("text", m.title.as_str()),
                ("desp", m.text().as_str()),
                ("type", "markdown"),
            ])
            .send()
            .await,
        "pushdeer",
        |v| v["code"] == 0,
    )
    .await
}

pub async fn pushme(t: Target, m: Message) -> Outcome {
    check(
        client()
            .post("https://push.i-i.me/")
            .form(&[
                ("push_key", t.host.as_str()),
                ("title", m.title.as_str()),
                ("content", m.text().as_str()),
            ])
            .send()
            .await,
        "pushme",
    )
    .await
}

pub async fn pushplus(t: Target, m: Message) -> Outcome {
    let body = json!({ "token": t.host, "title": m.title, "content": m.text(), "template": "txt" });
    check_json(
        client()
            .post("https://www.pushplus.plus/send")
            .json(&body)
            .send()
            .await,
        "pushplus",
        |v| v["code"] == 200,
    )
    .await
}

pub async fn serverchan(t: Target, m: Message) -> Outcome {
    let key = t.host.clone();
    let url = match key
        .strip_prefix("sctp")
        .and_then(|r| r.split_once('t'))
        .filter(|(uid, _)| !uid.is_empty() && uid.bytes().all(|b| b.is_ascii_digit()))
    {
        Some((uid, _)) => format!("https://{uid}.push.ft07.com/send/{key}.send"),
        None => format!("https://sctapi.ftqq.com/{key}.send"),
    };
    check_json(
        client()
            .post(url)
            .form(&[("title", m.title.as_str()), ("desp", m.text().as_str())])
            .send()
            .await,
        "serverchan",
        |v| v["code"] == 0,
    )
    .await
}
