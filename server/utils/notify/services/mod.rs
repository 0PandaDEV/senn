use std::{sync::LazyLock, time::Duration};

use futures::future::BoxFuture;
use serde_json::Value;

use super::{Message, target::Target};

mod chat;
mod email;
mod hooks;
mod incident;
mod push;
mod sms;

pub type Outcome = Result<(), String>;

pub struct Service {
    pub name: &'static str,
    pub schemes: &'static [&'static str],
    pub formats: &'static [&'static str],
    send: fn(Target, Message) -> BoxFuture<'static, Outcome>,
}

macro_rules! service {
    ($name:literal, [$($s:literal),+], [$($f:literal),+], $send:path) => {
        Service {
            name: $name,
            schemes: &[$($s),+],
            formats: &[$($f),+],
            send: |t, m| Box::pin($send(t, m)),
        }
    };
}

pub static SERVICES: &[Service] = &[
    service!("ntfy", ["ntfy", "ntfys"], ["ntfys://topic", "ntfys://host/topic?token=tk_…", "ntfy://user:pass@host/topic"], push::ntfy),
    service!("Gotify", ["gotify", "gotifys"], ["gotifys://host/app_token", "gotifys://host/path/app_token"], push::gotify),
    service!("Pushover", ["pover"], ["pover://user_key@app_token", "pover://user_key@app_token/device?sound=siren"], push::pushover),
    service!("Pushbullet", ["pbul"], ["pbul://access_token", "pbul://access_token/device_id/email/#channel"], push::pushbullet),
    service!("Bark", ["bark", "barks"], ["barks://api.day.app/device_key", "barks://host/key1/key2?sound=alarm&group=senn&level=critical&volume=5&icon=https://…"], push::bark),
    service!("Nextcloud", ["ncloud", "nclouds"], ["nclouds://user:pass@host/target_user"], push::nextcloud),
    service!("Pushsafer", ["psafer", "psafers"], ["psafers://private_key", "psafers://private_key/device_id"], push::pushsafer),
    service!("SimplePush", ["spush"], ["spush://api_key"], push::simplepush),
    service!("Prowl", ["prowl"], ["prowl://api_key", "prowl://api_key/provider_key"], push::prowl),
    service!("Join", ["join"], ["join://api_key", "join://api_key/device_id/group.android"], push::join),
    service!("PushDeer", ["pushdeer", "pushdeers"], ["pushdeer://push_key", "pushdeers://host/push_key"], push::pushdeer),
    service!("PushMe", ["pushme"], ["pushme://push_key"], push::pushme),
    service!("PushPlus", ["pushplus"], ["pushplus://token"], push::pushplus),
    service!("ServerChan", ["schan"], ["schan://send_key"], push::serverchan),
    service!("Discord", ["discord"], ["discord://webhook_id/webhook_token", "discord://botname@webhook_id/webhook_token?thread=id"], chat::discord),
    service!("Telegram", ["tgram"], ["tgram://bot_token/chat_id", "tgram://bot_token/chat_id:topic_id/chat_id2"], chat::telegram),
    service!("Slack", ["slack"], ["slack://TokenA/TokenB/TokenC", "slack://TokenA/TokenB/TokenC/#channel", "slack://xoxb-bot-token/#channel"], chat::slack),
    service!("Mattermost", ["mmost", "mmosts"], ["mmosts://host/hook_id", "mmosts://botname@host/path/hook_id/#channel"], chat::mattermost),
    service!("Rocket.Chat", ["rocket", "rockets"], ["rockets://user:pass@host/#channel/@user", "rockets://webhook_id@host/token/#channel"], chat::rocket),
    service!("Matrix", ["matrix", "matrixs"], ["matrixs://user:pass@host/#room:server", "matrixs://access_token@host/!room_id"], chat::matrix),
    service!("Microsoft Teams", ["msteams"], ["msteams://team/GUID%40tenant/TokenB/TokenC/TokenD"], chat::msteams),
    service!("Power Automate", ["workflow", "workflows"], ["workflows://host:port/workflow_id/signature"], chat::workflows),
    service!("Google Chat", ["gchat"], ["gchat://workspace/key/token", "gchat://workspace/key/token/thread_key"], chat::google_chat),
    service!("Zulip", ["zulip"], ["zulip://botname@organization/token/#stream", "zulip://botname@chat.example.com/token/user@example.com"], chat::zulip),
    service!("Webex", ["wxteams", "webex"], ["wxteams://webhook_token"], chat::webex),
    service!("Guilded", ["guilded"], ["guilded://webhook_id/webhook_token"], chat::guilded),
    service!("Nextcloud Talk", ["nctalk", "nctalks"], ["nctalks://user:pass@host/room_id"], chat::nextcloud_talk),
    service!("Synology Chat", ["synology", "synologys"], ["synologys://host:port/token"], chat::synology),
    service!("Mastodon", ["mastodon", "mastodons", "toot", "toots"], ["mastodons://access_token@host", "mastodons://access_token@host/@user?visibility=direct"], chat::mastodon),
    service!("Misskey", ["misskey", "misskeys"], ["misskeys://access_token@host"], chat::misskey),
    service!("LINE", ["line"], ["line://channel_token/user_id"], chat::line),
    service!("Feishu", ["feishu"], ["feishu://bot_token"], chat::feishu),
    service!("Lark", ["lark"], ["lark://bot_token"], chat::lark),
    service!("DingTalk", ["dingtalk"], ["dingtalk://access_token", "dingtalk://secret@access_token"], chat::dingtalk),
    service!("WeCom Bot", ["wecombot"], ["wecombot://bot_key"], chat::wecom),
    service!("Email", ["mailto", "mailtos"], ["mailtos://user:app_password@gmail.com", "mailtos://user:pass@example.com?smtp=smtp.example.com&from=senn@example.com&to=me@example.com", "mailtos://…?mode=ssl for port 465 · mailto:// for plain SMTP"], email::send),
    service!("Twilio", ["twilio"], ["twilio://account_sid:auth_token@from_phone/to_phone"], sms::twilio),
    service!("Vonage", ["vonage", "nexmo"], ["vonage://api_key:api_secret@from_phone/to_phone"], sms::vonage),
    service!("ClickSend", ["clicksend"], ["clicksend://user:api_key@to_phone/to_phone2"], sms::clicksend),
    service!("Signal API", ["signal", "signals"], ["signal://host:8080/from_phone/to_phone/group.id"], sms::signal),
    service!("Free Mobile", ["freemobile"], ["freemobile://user@password"], sms::freemobile),
    service!("Opsgenie", ["opsgenie"], ["opsgenie://api_key", "opsgenie://api_key/@user/#team/*schedule/^escalation?region=eu"], incident::opsgenie),
    service!("PagerDuty", ["pagerduty"], ["pagerduty://integration_key@api_key", "pagerduty://integration_key@api_key/source/component?region=eu"], incident::pagerduty),
    service!("Splunk On-Call", ["splunk", "victorops"], ["splunk://route_key@api_key", "splunk://route_key@api_key/entity_id"], incident::splunk),
    service!("PagerTree", ["pagertree"], ["pagertree://integration_id"], incident::pagertree),
    service!("Home Assistant", ["hassio", "hassios"], ["hassio://host/long_lived_token", "hassios://host:443/token?to=mobile_app_phone"], hooks::home_assistant),
    service!("Apprise API", ["apprise", "apprises"], ["apprises://host/config_key", "apprise://user:pass@host:8000/config_key?tags=ops"], hooks::apprise),
    service!("IFTTT", ["ifttt"], ["ifttt://webhook_key/event", "ifttt://webhook_key/event1/event2"], hooks::ifttt),
    service!("Webhook (JSON)", ["json", "jsons"], ["jsons://host/path", "jsons://user:pass@host/path?+X-Header=v&-param=v&:message=text&method=PUT"], hooks::json),
    service!("Webhook (form)", ["form", "forms"], ["forms://host/path"], hooks::form),
    service!("Webhook (XML)", ["xml", "xmls"], ["xmls://host/path"], hooks::xml),
];

pub fn find(scheme: &str) -> Option<&'static Service> {
    SERVICES.iter().find(|s| s.schemes.contains(&scheme))
}

pub async fn deliver(t: Target, m: Message) -> Outcome {
    let s = find(&t.scheme)
        .ok_or_else(|| format!("unsupported notification service: {}://", t.scheme))?;
    (s.send)(t, m).await
}

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("senn/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default()
});

fn client() -> &'static reqwest::Client {
    &CLIENT
}

fn snippet(s: &str) -> String {
    s.chars().take(160).collect()
}

fn cut(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn fail(name: &str, e: reqwest::Error) -> String {
    format!("{name}: {}", e.without_url())
}

async fn check(res: Result<reqwest::Response, reqwest::Error>, name: &str) -> Outcome {
    let res = res.map_err(|e| fail(name, e))?;
    if res.status().is_success() {
        return Ok(());
    }
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    Err(format!("{name} answered {status}: {}", snippet(&body)))
}

async fn check_json(
    res: Result<reqwest::Response, reqwest::Error>,
    name: &str,
    ok: impl Fn(&Value) -> bool,
) -> Outcome {
    let res = res.map_err(|e| fail(name, e))?;
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    let parsed = serde_json::from_str::<Value>(&body).unwrap_or(Value::Null);
    if status.is_success() && ok(&parsed) {
        return Ok(());
    }
    Err(format!("{name} answered {status}: {}", snippet(&body)))
}

fn need<T>(v: Option<T>, what: &str) -> Result<T, String> {
    v.ok_or_else(|| format!("notification URL is missing the {what}"))
}

fn stamp() -> String {
    jiff::Timestamp::now().as_nanosecond().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemes_are_unique() {
        let mut all: Vec<_> = SERVICES.iter().flat_map(|s| s.schemes.iter()).collect();
        let n = all.len();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), n);
    }
}
