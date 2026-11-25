use serde_json::json;

use super::{Outcome, check, check_json, client, cut, need};
use crate::server::utils::notify::{Message, target::Target};

fn phone(p: &str) -> String {
    let digits: String = p.chars().filter(|c| c.is_ascii_digit()).collect();
    if p.starts_with("group.") || digits.is_empty() {
        p.to_string()
    } else {
        format!("+{digits}")
    }
}

fn recipients(t: &Target) -> Result<Vec<String>, String> {
    let to = t.targets();
    if to.is_empty() {
        return Err(format!("{}:// URL is missing a phone number", t.scheme));
    }
    Ok(to.iter().map(|p| phone(p)).collect())
}

pub async fn twilio(t: Target, m: Message) -> Outcome {
    let sid = need(t.user.clone(), "account sid")?;
    let token = need(t.pass.clone(), "auth token")?;
    let from = phone(&t.host);
    let url = format!("https://api.twilio.com/2010-04-01/Accounts/{sid}/Messages.json");
    for to in recipients(&t)? {
        check(
            client()
                .post(&url)
                .basic_auth(&sid, Some(&token))
                .form(&[
                    ("From", from.as_str()),
                    ("To", to.as_str()),
                    ("Body", cut(&m.full(), 1600).as_str()),
                ])
                .send()
                .await,
            "twilio",
        )
        .await?;
    }
    Ok(())
}

pub async fn vonage(t: Target, m: Message) -> Outcome {
    let key = need(t.user.clone(), "api key")?;
    let secret = need(t.pass.clone(), "api secret")?;
    let from = t.host.trim_start_matches('+').to_string();
    for to in recipients(&t)? {
        check_json(
            client()
                .post("https://rest.nexmo.com/sms/json")
                .form(&[
                    ("api_key", key.as_str()),
                    ("api_secret", secret.as_str()),
                    ("from", from.as_str()),
                    ("to", to.trim_start_matches('+')),
                    ("text", m.full().as_str()),
                ])
                .send()
                .await,
            "vonage",
            |v| v["messages"][0]["status"] == "0",
        )
        .await?;
    }
    Ok(())
}

pub async fn clicksend(t: Target, m: Message) -> Outcome {
    let user = need(t.user.clone(), "user")?;
    let mut to = vec![phone(&t.host)];
    to.extend(t.targets().iter().map(|p| phone(p)));
    let messages: Vec<_> = to
        .iter()
        .map(|p| json!({ "source": "senn", "body": m.full(), "to": p }))
        .collect();
    check(
        client()
            .post("https://rest.clicksend.com/v3/sms/send")
            .basic_auth(user, t.pass.as_ref())
            .json(&json!({ "messages": messages }))
            .send()
            .await,
        "clicksend",
    )
    .await
}

pub async fn signal(t: Target, m: Message) -> Outcome {
    let from = phone(&t.first()?);
    let mut to: Vec<String> = t.path[1..].iter().map(|p| phone(p)).collect();
    to.extend(t.list("to").iter().map(|p| phone(p)));
    if to.is_empty() {
        to.push(from.clone());
    }
    let mut req = client()
        .post(format!("{}/v2/send", t.base()))
        .json(&json!({ "message": m.full(), "number": from, "recipients": to }));
    if let Some(u) = &t.user {
        req = req.basic_auth(u, t.pass.as_ref());
    }
    check(req.send().await, "signal").await
}

pub async fn freemobile(t: Target, m: Message) -> Outcome {
    let user = need(t.user.clone(), "user")?;
    check(
        client()
            .get("https://smsapi.free-mobile.fr/sendmsg")
            .query(&[
                ("user", user.as_str()),
                ("pass", t.host.as_str()),
                ("msg", m.full().as_str()),
            ])
            .send()
            .await,
        "free mobile",
    )
    .await
}
