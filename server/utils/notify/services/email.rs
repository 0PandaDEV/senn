use std::time::Duration;

use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message as Mail, Tokio1Executor,
    message::{Mailbox, header::ContentType},
    transport::smtp::authentication::Credentials,
};

use super::Outcome;
use crate::server::utils::notify::{
    Message,
    target::{Target, split},
};

enum Mode {
    Plain,
    StartTls,
    Tls,
}

fn provider(domain: &str) -> Option<(&'static str, u16, Mode)> {
    let d = domain.to_ascii_lowercase();
    Some(match d.as_str() {
        "gmail.com" | "googlemail.com" => ("smtp.gmail.com", 587, Mode::StartTls),
        "outlook.com" | "hotmail.com" | "live.com" | "msn.com" => {
            ("smtp-mail.outlook.com", 587, Mode::StartTls)
        }
        "yahoo.com" | "ymail.com" => ("smtp.mail.yahoo.com", 465, Mode::Tls),
        "icloud.com" | "me.com" | "mac.com" => ("smtp.mail.me.com", 587, Mode::StartTls),
        "fastmail.com" | "fastmail.fm" => ("smtp.fastmail.com", 465, Mode::Tls),
        "zoho.com" | "zohomail.com" => ("smtp.zoho.com", 465, Mode::Tls),
        "yandex.com" | "yandex.ru" => ("smtp.yandex.com", 465, Mode::Tls),
        "gmx.com" | "gmx.net" | "gmx.de" => ("mail.gmx.net", 465, Mode::Tls),
        "aol.com" => ("smtp.aol.com", 465, Mode::Tls),
        _ => return None,
    })
}

fn mailbox(s: &str) -> Result<Mailbox, String> {
    s.parse()
        .map_err(|e| format!("email: bad address {s}: {e}"))
}

pub async fn send(t: Target, m: Message) -> Outcome {
    let user = t.user.clone().unwrap_or_default();
    let known = provider(&t.host);
    let from_addr = match t.q("from") {
        Some(f) if f.contains('@') => f.to_string(),
        Some(f) => format!("{f}@{}", t.host),
        None if user.contains('@') => user.clone(),
        None => format!("{user}@{}", t.host),
    };
    let login = match (t.q("user"), &known) {
        (Some(u), _) => u.to_string(),
        (None, Some(_)) => from_addr.clone(),
        (None, None) => user.clone(),
    };
    let (smtp, default_port, default_mode) = match (t.q("smtp"), known) {
        (Some(h), _) => (
            h.to_string(),
            if t.secure() { 587 } else { 25 },
            if t.secure() { Mode::StartTls } else { Mode::Plain },
        ),
        (None, Some((h, p, mode))) => (h.to_string(), p, mode),
        (None, None) => (
            t.host.clone(),
            if t.secure() { 587 } else { 25 },
            if t.secure() { Mode::StartTls } else { Mode::Plain },
        ),
    };
    let mode = match t.q("mode").map(str::to_ascii_lowercase).as_deref() {
        Some("ssl" | "tls") => Mode::Tls,
        Some("starttls") => Mode::StartTls,
        Some("insecure" | "plain") => Mode::Plain,
        _ => default_mode,
    };
    let port = t
        .q("port")
        .and_then(|p| p.parse().ok())
        .or(t.port)
        .unwrap_or(match mode {
            Mode::Tls if default_port == 587 => 465,
            _ => default_port,
        });

    let mut to: Vec<String> = t.path.clone();
    to.extend(t.list("to"));
    if to.is_empty() {
        to.push(from_addr.clone());
    }
    let from = match t.q("name") {
        Some(n) => format!("{n} <{from_addr}>"),
        None => format!("senn <{from_addr}>"),
    };
    let mut builder = Mail::builder()
        .from(mailbox(&from)?)
        .subject(m.title.clone())
        .header(ContentType::TEXT_PLAIN);
    for addr in &to {
        builder = builder.to(mailbox(addr)?);
    }
    for addr in t.q("cc").map(split).unwrap_or_default() {
        builder = builder.cc(mailbox(&addr)?);
    }
    for addr in t.q("bcc").map(split).unwrap_or_default() {
        builder = builder.bcc(mailbox(&addr)?);
    }
    let mail = builder
        .body(m.text())
        .map_err(|e| format!("email: {e}"))?;

    let transport = match mode {
        Mode::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp),
        Mode::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp),
        Mode::Plain => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&smtp)),
    }
    .map_err(|e| format!("email: {e}"))?;
    let mut transport = transport.port(port).timeout(Some(Duration::from_secs(20)));
    if let Some(pass) = &t.pass {
        transport = transport.credentials(Credentials::new(login, pass.clone()));
    }
    transport
        .build()
        .send(mail)
        .await
        .map(|_| ())
        .map_err(|e| format!("email via {smtp}:{port}: {e}"))
}
