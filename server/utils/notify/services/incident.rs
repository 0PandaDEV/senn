use serde_json::{Value, json};

use super::{Outcome, check, client, cut, need, stamp};
use crate::server::utils::notify::{Message, target::Target};

fn eu(t: &Target) -> bool {
    t.q("region").is_some_and(|r| r.eq_ignore_ascii_case("eu"))
}

pub async fn opsgenie(t: Target, m: Message) -> Outcome {
    let responders: Vec<Value> = t
        .targets()
        .iter()
        .map(|x| match x.chars().next() {
            Some('#') => json!({ "type": "team", "name": &x[1..] }),
            Some('*') => json!({ "type": "schedule", "name": &x[1..] }),
            Some('^') => json!({ "type": "escalation", "name": &x[1..] }),
            Some('@') => json!({ "type": "user", "username": &x[1..] }),
            _ => json!({ "type": "user", "username": x }),
        })
        .collect();
    let priority = t
        .q("priority")
        .map(|p| format!("P{}", p.trim_start_matches(['P', 'p'])))
        .unwrap_or_else(|| format!("P{}", 6 - m.priority.clamp(1, 5)));
    let mut body = json!({
        "message": cut(&m.title, 130),
        "description": cut(&m.text(), 15000),
        "priority": priority,
        "source": "senn",
        "responders": responders,
    });
    if let Some(a) = t.q("alias") {
        body["alias"] = a.into();
    }
    let host = if eu(&t) {
        "api.eu.opsgenie.com"
    } else {
        "api.opsgenie.com"
    };
    check(
        client()
            .post(format!("https://{host}/v2/alerts"))
            .header("Authorization", format!("GenieKey {}", t.host))
            .json(&body)
            .send()
            .await,
        "opsgenie",
    )
    .await
}

pub async fn pagerduty(t: Target, m: Message) -> Outcome {
    let key = need(t.user.clone(), "integration key")?;
    let severity = match m.kind() {
        "failure" => "error",
        "warning" => "warning",
        _ => "info",
    };
    let mut payload = json!({
        "summary": cut(&m.title, 1024),
        "severity": t.q("severity").unwrap_or(severity),
        "source": t.path.first().map(String::as_str).unwrap_or("senn"),
        "custom_details": { "details": m.body },
    });
    if let Some(c) = t.path.get(1) {
        payload["component"] = c.as_str().into();
    }
    let mut body = json!({ "routing_key": key, "event_action": "trigger", "payload": payload });
    if let Some(c) = &m.click {
        body["links"] = json!([{ "href": c, "text": "Open in senn" }]);
    }
    let host = if eu(&t) {
        "events.eu.pagerduty.com"
    } else {
        "events.pagerduty.com"
    };
    check(
        client()
            .post(format!("https://{host}/v2/enqueue"))
            .header("Authorization", format!("Token token={}", t.host))
            .json(&body)
            .send()
            .await,
        "pagerduty",
    )
    .await
}

pub async fn splunk(t: Target, m: Message) -> Outcome {
    let route = need(t.user.clone(), "routing key")?;
    let kind = match m.kind() {
        "failure" => "CRITICAL",
        "warning" => "WARNING",
        "success" => "RECOVERY",
        _ => "INFO",
    };
    let entity = t.path.first().cloned().unwrap_or_else(|| m.title.clone());
    let body = json!({
        "message_type": kind,
        "entity_id": entity,
        "entity_display_name": m.title,
        "state_message": m.text(),
        "monitoring_tool": "senn",
    });
    check(
        client()
            .post(format!(
                "https://alert.victorops.com/integrations/generic/20131114/alert/{}/{route}",
                t.host
            ))
            .json(&body)
            .send()
            .await,
        "splunk on-call",
    )
    .await
}

pub async fn pagertree(t: Target, m: Message) -> Outcome {
    let urgency = match m.kind() {
        "failure" => "high",
        "warning" => "medium",
        _ => "low",
    };
    let body = json!({
        "event_type": "create",
        "Id": format!("senn-{}", stamp()),
        "Title": m.title,
        "Description": m.text(),
        "Urgency": t.q("urgency").unwrap_or(urgency),
    });
    check(
        client()
            .post(format!("https://api.pagertree.com/integration/{}", t.host))
            .json(&body)
            .send()
            .await,
        "pagertree",
    )
    .await
}
