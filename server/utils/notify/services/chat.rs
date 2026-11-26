use base64::{Engine, engine::general_purpose::STANDARD};
use hmac::{Hmac, KeyInit, Mac};
use serde_json::{Value, json};
use sha2::Sha256;

use super::{Outcome, check, check_json, client, cut, fail, need, stamp};
use crate::server::utils::notify::{
    Message,
    target::{Target, encode},
};

fn color(m: &Message) -> u32 {
    match m.kind() {
        "failure" => 0xe5484d,
        "warning" => 0xe3a857,
        "success" => 0x46c27f,
        _ => 0x7aa7ff,
    }
}

fn embed(m: &Message) -> Value {
    json!({
        "title": cut(&m.title, 256),
        "description": cut(&m.body, 4096),
        "color": color(m),
        "url": m.click,
    })
}

pub async fn discord(t: Target, m: Message) -> Outcome {
    let mut url = format!("https://discord.com/api/webhooks/{}/{}", t.host, t.first()?);
    if let Some(th) = t.q("thread") {
        url.push_str(&format!("?thread_id={th}"));
    }
    let mut body = json!({ "embeds": [embed(&m)] });
    if let Some(u) = &t.user {
        body["username"] = u.as_str().into();
    }
    if let Some(a) = t.q("avatar_url").map(String::from).or_else(|| m.icon()) {
        body["avatar_url"] = a.into();
    }
    check(client().post(url).json(&body).send().await, "discord").await
}

pub async fn guilded(t: Target, m: Message) -> Outcome {
    let url = format!(
        "https://media.guilded.gg/webhooks/{}/{}",
        t.host,
        t.first()?
    );
    let mut body = json!({ "embeds": [embed(&m)] });
    if let Some(u) = &t.user {
        body["username"] = u.as_str().into();
    }
    check(client().post(url).json(&body).send().await, "guilded").await
}

pub async fn telegram(t: Target, m: Message) -> Outcome {
    let chats = t.targets();
    if chats.is_empty() {
        return Err("telegram URL is missing a chat id".into());
    }
    let url = format!("https://api.telegram.org/bot{}/sendMessage", t.authority());
    for chat in chats {
        let (chat, topic) = match chat.rsplit_once(':') {
            Some((c, th)) if th.bytes().all(|b| b.is_ascii_digit()) => {
                (c.to_string(), Some(th.to_string()))
            }
            _ => (chat, None),
        };
        let mut body = json!({
            "chat_id": chat,
            "text": cut(&m.full(), 4096),
            "disable_web_page_preview": true,
            "disable_notification": t.q("silent").is_some_and(|v| v == "yes" || v == "true"),
        });
        if let Some(th) = topic {
            body["message_thread_id"] = th.into();
        }
        check_json(
            client().post(&url).json(&body).send().await,
            "telegram",
            |v| v["ok"] == true,
        )
        .await?;
    }
    Ok(())
}

pub async fn slack(t: Target, m: Message) -> Outcome {
    let text = format!("*{}*\n{}", m.title, m.text());
    if t.host.starts_with("xox") {
        let channels = t.targets();
        if channels.is_empty() {
            return Err("slack bot URL is missing a #channel".into());
        }
        for ch in channels {
            let body = json!({ "channel": ch.trim_start_matches('#'), "text": text });
            check_json(
                client()
                    .post("https://slack.com/api/chat.postMessage")
                    .bearer_auth(&t.host)
                    .json(&body)
                    .send()
                    .await,
                "slack",
                |v| v["ok"] == true,
            )
            .await?;
        }
        return Ok(());
    }
    if t.path.len() < 2 {
        return Err("slack URL needs TokenA/TokenB/TokenC".into());
    }
    let url = format!(
        "https://hooks.slack.com/services/{}/{}/{}",
        t.host, t.path[0], t.path[1]
    );
    let mut channels: Vec<Option<String>> = t.path[2..].iter().cloned().map(Some).collect();
    channels.extend(t.list("to").into_iter().map(Some));
    if channels.is_empty() {
        channels.push(None);
    }
    for ch in channels {
        let mut body = json!({ "text": text });
        if let Some(c) = ch {
            body["channel"] = c.into();
        }
        if let Some(u) = &t.user {
            body["username"] = u.as_str().into();
        }
        if let Some(i) = t.q("icon").map(String::from).or_else(|| m.icon()) {
            body["icon_url"] = i.into();
        }
        check(client().post(&url).json(&body).send().await, "slack").await?;
    }
    Ok(())
}

pub async fn mattermost(t: Target, m: Message) -> Outcome {
    let (channels, rest): (Vec<String>, Vec<String>) =
        t.path.iter().cloned().partition(|p| p.starts_with('#'));
    let token = need(rest.last().cloned(), "hook id")?;
    let prefix = rest[..rest.len() - 1]
        .iter()
        .map(|p| format!("/{p}"))
        .collect::<String>();
    let url = format!("{}{prefix}/hooks/{token}", t.base());
    let mut channels: Vec<Option<String>> = channels
        .into_iter()
        .chain(t.list("channel"))
        .map(|c| Some(c.trim_start_matches('#').to_string()))
        .collect();
    if channels.is_empty() {
        channels.push(None);
    }
    for ch in channels {
        let mut body = json!({ "text": format!("**{}**\n{}", m.title, m.text()) });
        if let Some(c) = ch {
            body["channel"] = c.into();
        }
        if let Some(u) = &t.user {
            body["username"] = u.as_str().into();
        }
        if let Some(i) = t.q("icon").map(String::from).or_else(|| m.icon()) {
            body["icon_url"] = i.into();
        }
        check(client().post(&url).json(&body).send().await, "mattermost").await?;
    }
    Ok(())
}

pub async fn rocket(t: Target, m: Message) -> Outcome {
    let text = format!("*{}*\n{}", m.title, m.text());
    let base = t.base();
    let targets = t.targets();
    if let (Some(user), Some(pass)) = (&t.user, &t.pass) {
        if targets.is_empty() {
            return Err("rocket.chat URL is missing a #channel or @user".into());
        }
        let res = client()
            .post(format!("{base}/api/v1/login"))
            .json(&json!({ "user": user, "password": pass }))
            .send()
            .await
            .map_err(|e| fail("rocket.chat", e))?;
        let login: Value = res.json().await.map_err(|e| fail("rocket.chat", e))?;
        let token = need(
            login["data"]["authToken"].as_str(),
            "rocket.chat login token",
        )?;
        let id = need(login["data"]["userId"].as_str(), "rocket.chat user id")?;
        let mut result = Ok(());
        for ch in targets {
            result = check_json(
                client()
                    .post(format!("{base}/api/v1/chat.postMessage"))
                    .header("X-Auth-Token", token)
                    .header("X-User-Id", id)
                    .json(&json!({ "channel": ch, "text": text }))
                    .send()
                    .await,
                "rocket.chat",
                |v| v["success"] == true,
            )
            .await;
            if result.is_err() {
                break;
            }
        }
        let _ = client()
            .post(format!("{base}/api/v1/logout"))
            .header("X-Auth-Token", token)
            .header("X-User-Id", id)
            .send()
            .await;
        return result;
    }
    let id = need(t.user.clone(), "webhook id")?;
    let (hook, targets) = if id.contains('/') {
        (id, targets)
    } else {
        let pos = need(
            targets
                .iter()
                .position(|x| !x.starts_with('#') && !x.starts_with('@')),
            "webhook token",
        )?;
        let mut rest = targets;
        let token = rest.remove(pos);
        (format!("{id}/{token}"), rest)
    };
    let url = format!("{base}/hooks/{hook}");
    let mut channels: Vec<Option<String>> = targets.into_iter().map(Some).collect();
    if channels.is_empty() {
        channels.push(None);
    }
    for ch in channels {
        let mut body = json!({ "text": text });
        if let Some(c) = ch {
            body["channel"] = c.into();
        }
        check(client().post(&url).json(&body).send().await, "rocket.chat").await?;
    }
    Ok(())
}

pub async fn matrix(t: Target, m: Message) -> Outcome {
    let base = t.base();
    let api = format!("{base}/_matrix/client/v3");
    let session = t.user.is_some() && t.pass.is_some();
    let token = match (&t.user, &t.pass) {
        (Some(user), Some(pass)) => {
            let res = client()
                .post(format!("{api}/login"))
                .json(&json!({
                    "type": "m.login.password",
                    "identifier": { "type": "m.id.user", "user": user },
                    "password": pass,
                }))
                .send()
                .await
                .map_err(|e| fail("matrix", e))?;
            if !res.status().is_success() {
                return Err(format!("matrix login answered {}", res.status()));
            }
            let v: Value = res.json().await.map_err(|e| fail("matrix", e))?;
            need(
                v["access_token"].as_str().map(String::from),
                "matrix access token",
            )?
        }
        _ => need(
            t.q("token").map(String::from).or_else(|| t.user.clone()),
            "matrix access token",
        )?,
    };
    let result = matrix_send(&t, &m, &api, &token).await;
    if session {
        let _ = client()
            .post(format!("{api}/logout"))
            .bearer_auth(&token)
            .json(&json!({}))
            .send()
            .await;
    }
    result
}

async fn matrix_send(t: &Target, m: &Message, api: &str, token: &str) -> Outcome {
    let rooms = t.targets();
    if rooms.is_empty() {
        return Err("matrix URL is missing a room".into());
    }
    for room in rooms {
        let room = if (room.starts_with('#') || room.starts_with('!')) && !room.contains(':') {
            format!("{room}:{}", t.host)
        } else if !room.starts_with('#') && !room.starts_with('!') {
            format!("#{room}:{}", t.host)
        } else {
            room
        };
        let res = client()
            .post(format!("{api}/join/{}", encode(&room)))
            .bearer_auth(token)
            .json(&json!({}))
            .send()
            .await
            .map_err(|e| fail("matrix", e))?;
        if !res.status().is_success() {
            return Err(format!("matrix could not join {room}: {}", res.status()));
        }
        let v: Value = res.json().await.map_err(|e| fail("matrix", e))?;
        let id = need(v["room_id"].as_str().map(String::from), "matrix room id")?;
        let body = json!({ "msgtype": t.q("msgtype").unwrap_or("m.text"), "body": m.full() });
        check(
            client()
                .put(format!(
                    "{api}/rooms/{}/send/m.room.message/senn{}",
                    encode(&id),
                    stamp()
                ))
                .bearer_auth(token)
                .json(&body)
                .send()
                .await,
            "matrix",
        )
        .await?;
    }
    Ok(())
}

pub async fn msteams(t: Target, m: Message) -> Outcome {
    let p = &t.path;
    let url = match p.len() {
        3 | 4 => format!(
            "https://{}.webhook.office.com/webhookb2/{}/IncomingWebhook/{}",
            t.host,
            p[0],
            p[1..].join("/")
        ),
        _ => return Err("msteams URL needs team/GUID@tenant/TokenB/TokenC[/TokenD]".into()),
    };
    let body = json!({
        "@type": "MessageCard",
        "@context": "https://schema.org/extensions",
        "summary": m.title,
        "title": m.title,
        "text": m.text().replace('\n', "  \n"),
        "themeColor": format!("{:06x}", color(&m)),
    });
    check(client().post(url).json(&body).send().await, "msteams").await
}

pub async fn workflows(t: Target, m: Message) -> Outcome {
    if t.path.len() < 2 {
        return Err("workflows URL needs host/workflow_id/signature".into());
    }
    let prefix = if t.host.contains("powerplatform.com") {
        "/powerautomate/automations/direct"
    } else {
        ""
    };
    let url = format!(
        "https://{}{prefix}/workflows/{}/triggers/manual/paths/invoke",
        t.authority(),
        t.path[0]
    );
    let mut body_blocks = vec![
        json!({ "type": "TextBlock", "text": m.title, "weight": "Bolder", "size": "Medium", "wrap": true }),
        json!({ "type": "TextBlock", "text": m.body, "wrap": true }),
    ];
    if let Some(c) = &m.click {
        body_blocks.push(
            json!({ "type": "TextBlock", "text": format!("[Open in senn]({c})"), "wrap": true }),
        );
    }
    let body = json!({
        "type": "message",
        "attachments": [{
            "contentType": "application/vnd.microsoft.card.adaptive",
            "content": {
                "$schema": "http://adaptivecards.io/schemas/adaptive-card.json",
                "type": "AdaptiveCard",
                "version": "1.4",
                "body": body_blocks,
            }
        }]
    });
    check(
        client()
            .post(url)
            .query(&[
                ("api-version", t.q("api-version").unwrap_or("2016-06-01")),
                ("sp", "/triggers/manual/run"),
                ("sv", "1.0"),
                ("sig", t.path[1].as_str()),
            ])
            .json(&body)
            .send()
            .await,
        "power automate",
    )
    .await
}

pub async fn google_chat(t: Target, m: Message) -> Outcome {
    if t.path.len() < 2 {
        return Err("gchat URL needs workspace/key/token".into());
    }
    let mut query = vec![("key", t.path[0].clone()), ("token", t.path[1].clone())];
    if let Some(thread) = t.path.get(2) {
        query.push(("threadKey", thread.clone()));
        query.push((
            "messageReplyOption",
            "REPLY_MESSAGE_FALLBACK_TO_NEW_THREAD".into(),
        ));
    }
    let body = json!({ "text": format!("*{}*\n{}", m.title, m.text()) });
    check(
        client()
            .post(format!(
                "https://chat.googleapis.com/v1/spaces/{}/messages",
                t.host
            ))
            .query(&query)
            .json(&body)
            .send()
            .await,
        "google chat",
    )
    .await
}

pub async fn zulip(t: Target, m: Message) -> Outcome {
    let bot = need(t.user.clone(), "bot name")?;
    let host = if t.host.contains('.') {
        t.authority()
    } else {
        format!("{}.zulipchat.com", t.host)
    };
    let domain = host.split(':').next().unwrap_or_default().to_string();
    let email = if bot.contains('@') {
        bot
    } else if bot.ends_with("-bot") {
        format!("{bot}@{domain}")
    } else {
        format!("{bot}-bot@{domain}")
    };
    let token = t.first()?;
    let mut targets: Vec<String> = t.path[1..].to_vec();
    targets.extend(t.list("to"));
    if targets.is_empty() {
        targets.push("general".into());
    }
    for target in targets {
        let form: Vec<(&str, String)> = if target.contains('@') {
            vec![
                ("type", "private".into()),
                ("to", target),
                ("content", m.full()),
            ]
        } else {
            vec![
                ("type", "stream".into()),
                ("to", target.trim_start_matches('#').to_string()),
                ("topic", cut(&m.title, 60)),
                ("content", m.text()),
            ]
        };
        check_json(
            client()
                .post(format!("https://{host}/api/v1/messages"))
                .basic_auth(&email, Some(&token))
                .form(&form)
                .send()
                .await,
            "zulip",
            |v| v["result"] == "success",
        )
        .await?;
    }
    Ok(())
}

pub async fn webex(t: Target, m: Message) -> Outcome {
    let body = json!({ "markdown": format!("**{}**\n{}", m.title, m.text()) });
    check(
        client()
            .post(format!(
                "https://webexapis.com/v1/webhooks/incoming/{}",
                t.host
            ))
            .json(&body)
            .send()
            .await,
        "webex",
    )
    .await
}

pub async fn nextcloud_talk(t: Target, m: Message) -> Outcome {
    let user = need(t.user.clone(), "user")?;
    let rooms = t.targets();
    if rooms.is_empty() {
        return Err("nextcloud talk URL is missing a room id".into());
    }
    for room in rooms {
        check(
            client()
                .post(format!(
                    "{}/ocs/v2.php/apps/spreed/api/v1/chat/{room}",
                    t.base()
                ))
                .basic_auth(&user, t.pass.as_ref())
                .header("OCS-APIRequest", "true")
                .header("Accept", "application/json")
                .json(&json!({ "message": m.full() }))
                .send()
                .await,
            "nextcloud talk",
        )
        .await?;
    }
    Ok(())
}

pub async fn synology(t: Target, m: Message) -> Outcome {
    let (prefix, token) = t.split_token();
    let payload = json!({ "text": m.full() }).to_string();
    check_json(
        client()
            .post(format!("{}{prefix}/webapi/entry.cgi", t.base()))
            .query(&[
                ("api", "SYNO.Chat.External"),
                ("method", "incoming"),
                ("version", "2"),
                ("token", &format!("\"{token}\"")),
            ])
            .form(&[("payload", payload)])
            .send()
            .await,
        "synology chat",
        |v| v["success"] == true,
    )
    .await
}

pub async fn mastodon(t: Target, m: Message) -> Outcome {
    let token = need(t.user.clone(), "access token")?;
    let mentions: Vec<String> = t
        .targets()
        .into_iter()
        .map(|u| {
            if u.starts_with('@') {
                u
            } else {
                format!("@{u}")
            }
        })
        .collect();
    let visibility = t.q("visibility").unwrap_or(if mentions.is_empty() {
        "public"
    } else {
        "direct"
    });
    let prefix = if mentions.is_empty() {
        String::new()
    } else {
        format!("{} ", mentions.join(" "))
    };
    let status = cut(&format!("{prefix}{}", m.full()), 500);
    check(
        client()
            .post(format!("{}/api/v1/statuses", t.base()))
            .bearer_auth(token)
            .json(&json!({ "status": status, "visibility": visibility }))
            .send()
            .await,
        "mastodon",
    )
    .await
}

pub async fn misskey(t: Target, m: Message) -> Outcome {
    let token = need(t.user.clone(), "access token")?;
    let body = json!({
        "i": token,
        "text": cut(&m.full(), 3000),
        "visibility": t.q("visibility").unwrap_or("public"),
    });
    check(
        client()
            .post(format!("{}/api/notes/create", t.base()))
            .json(&body)
            .send()
            .await,
        "misskey",
    )
    .await
}

pub async fn line(t: Target, m: Message) -> Outcome {
    let targets = t.targets();
    if targets.is_empty() {
        return Err("line URL is missing a user, group or room id".into());
    }
    for to in targets {
        let body =
            json!({ "to": to, "messages": [{ "type": "text", "text": cut(&m.full(), 5000) }] });
        check(
            client()
                .post("https://api.line.me/v2/bot/message/push")
                .bearer_auth(&t.host)
                .json(&body)
                .send()
                .await,
            "line",
        )
        .await?;
    }
    Ok(())
}

async fn lark_like(base: &str, t: Target, m: Message, name: &str) -> Outcome {
    let body = json!({ "msg_type": "text", "content": { "text": m.full() } });
    check_json(
        client()
            .post(format!("{base}/open-apis/bot/v2/hook/{}", t.host))
            .json(&body)
            .send()
            .await,
        name,
        |v| v["code"] == 0 || v["StatusCode"] == 0,
    )
    .await
}

pub async fn feishu(t: Target, m: Message) -> Outcome {
    lark_like("https://open.feishu.cn", t, m, "feishu").await
}

pub async fn lark(t: Target, m: Message) -> Outcome {
    lark_like("https://open.larksuite.com", t, m, "lark").await
}

pub async fn dingtalk(t: Target, m: Message) -> Outcome {
    let mut query = vec![("access_token", t.host.clone())];
    if let Some(secret) = &t.user {
        let ts = jiff::Timestamp::now().as_millisecond().to_string();
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
            .map_err(|e| format!("dingtalk: {e}"))?;
        mac.update(format!("{ts}\n{secret}").as_bytes());
        let sign = STANDARD.encode(mac.finalize().into_bytes());
        query.push(("timestamp", ts));
        query.push(("sign", sign));
    }
    let body = json!({
        "msgtype": "markdown",
        "markdown": { "title": m.title, "text": format!("### {}\n{}", m.title, m.text()) },
    });
    check_json(
        client()
            .post("https://oapi.dingtalk.com/robot/send")
            .query(&query)
            .json(&body)
            .send()
            .await,
        "dingtalk",
        |v| v["errcode"] == 0,
    )
    .await
}

pub async fn wecom(t: Target, m: Message) -> Outcome {
    let body = json!({ "msgtype": "text", "text": { "content": m.full() } });
    check_json(
        client()
            .post("https://qyapi.weixin.qq.com/cgi-bin/webhook/send")
            .query(&[("key", t.host.as_str())])
            .json(&body)
            .send()
            .await,
        "wecom",
        |v| v["errcode"] == 0,
    )
    .await
}
