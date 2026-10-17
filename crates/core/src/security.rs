use std::collections::{HashMap, HashSet};

use regex::Regex;
use serde_json::{json, Value};

const DANGEROUS_CSS: &str = r"(?i)(url\s*\(|@import|expression\s*\(|position\s*:\s*(fixed|absolute)|behavior\s*:|-moz-binding|javascript\s*:)";

fn allowed_tags() -> HashSet<&'static str> {
    [
        "a",
        "b",
        "i",
        "em",
        "strong",
        "u",
        "s",
        "strike",
        "code",
        "pre",
        "blockquote",
        "p",
        "br",
        "div",
        "span",
        "ul",
        "ol",
        "li",
        "table",
        "thead",
        "tbody",
        "tr",
        "td",
        "th",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "hr",
        "img",
        "font",
        "center",
        "small",
        "sub",
        "sup",
        "dl",
        "dt",
        "dd",
    ]
    .into_iter()
    .collect()
}

fn tag_attributes() -> HashMap<&'static str, HashSet<&'static str>> {
    let mut m = HashMap::new();
    m.insert("a", ["href", "name", "title"].into_iter().collect());
    m.insert(
        "img",
        ["src", "alt", "title", "width", "height", "style"]
            .into_iter()
            .collect(),
    );
    m.insert("span", ["style"].into_iter().collect());
    m.insert("div", ["style"].into_iter().collect());
    m.insert("p", ["style"].into_iter().collect());
    m.insert(
        "table",
        ["style", "border", "cellpadding", "cellspacing", "align"]
            .into_iter()
            .collect(),
    );
    m.insert(
        "td",
        ["style", "colspan", "rowspan", "align", "valign"]
            .into_iter()
            .collect(),
    );
    m.insert(
        "th",
        ["style", "colspan", "rowspan", "align", "valign"]
            .into_iter()
            .collect(),
    );
    m.insert("font", ["color", "face", "size"].into_iter().collect());
    m.insert("blockquote", ["style", "cite"].into_iter().collect());
    m
}

pub fn sanitize_thread_html(dirty: &str) -> String {
    let dangerous_css = Regex::new(DANGEROUS_CSS).expect("static regex");
    let pre = strip_data_text_html_images(dirty);
    let mut builder = ammonia::Builder::new();
    builder.tags(allowed_tags());
    builder.tag_attributes(tag_attributes());
    builder.url_schemes(
        ["http", "https", "mailto", "tel", "data", "cid"]
            .into_iter()
            .collect(),
    );
    builder.url_relative(ammonia::UrlRelative::Custom(Box::new(
        deny_protocol_relative,
    )));
    builder.link_rel(Some("noopener noreferrer nofollow"));
    builder.set_tag_attribute_value("a", "target", "_blank");
    builder.attribute_filter(move |element, attribute, value| {
        if attribute == "style" {
            if value.is_empty() || dangerous_css.is_match(value) {
                return None;
            }
            let clipped: String = value.chars().take(2000).collect();
            return Some(std::borrow::Cow::Owned(clipped));
        }
        if element == "img" && attribute == "src" && value.trim().starts_with("data:text/html") {
            return None;
        }
        if attribute == "href" && element != "img" {
            let lower = value.trim().to_ascii_lowercase();
            if lower.starts_with("data:") || lower.starts_with("cid:") {
                return None;
            }
        }
        Some(std::borrow::Cow::Borrowed(value))
    });
    builder.clean(&pre).to_string()
}

const IMG_START_TAG_RE: &str = r#"(?is)<img\b(?:[^>"']|"[^"]*"|'[^']*')*>"#;

const SRC_ATTR_RE: &str = r#"(?is)\bsrc\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#;

fn deny_protocol_relative(url: &str) -> Option<std::borrow::Cow<'_, str>> {
    if url.starts_with("//") {
        None
    } else {
        Some(std::borrow::Cow::Borrowed(url))
    }
}

fn strip_data_text_html_images(dirty: &str) -> std::borrow::Cow<'_, str> {
    if !dirty.to_ascii_lowercase().contains("<img") {
        return std::borrow::Cow::Borrowed(dirty);
    }
    let img_tag = Regex::new(IMG_START_TAG_RE).expect("static img tag regex");
    let src_attr = Regex::new(SRC_ATTR_RE).expect("static src attr regex");
    img_tag.replace_all(dirty, |caps: &regex::Captures<'_>| -> String {
        let tag = caps
            .get(0)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        let is_data_text_html = src_attr.captures(&tag).is_some_and(|m| {
            let value = m
                .get(1)
                .or_else(|| m.get(2))
                .or_else(|| m.get(3))
                .map(|v| v.as_str())
                .unwrap_or_default();
            value
                .trim()
                .to_ascii_lowercase()
                .starts_with("data:text/html")
        });
        if is_data_text_html {
            String::new()
        } else {
            tag
        }
    })
}

pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SsrfCheckResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", rename = "resolvedAddresses")]
    pub resolved: Vec<String>,
}

impl SsrfCheckResult {
    fn refused(reason: impl Into<String>, host: Option<&str>) -> Self {
        Self {
            ok: false,
            reason: Some(reason.into()),
            host: host.map(|h| h.to_string()),
            resolved: Vec::new(),
        }
    }
}

fn is_ipv4_literal(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 3
                && p.chars().all(|c| c.is_ascii_digit())
                && p.parse::<u16>().is_ok_and(|n| n <= 255)
        })
}

fn ipv4_reason(ip: &str) -> Option<String> {
    let parts: Vec<u32> = ip
        .split('.')
        .map(|p| p.parse::<u32>().ok())
        .collect::<Option<Vec<_>>>()?;
    if parts.len() != 4 || parts.iter().any(|p| *p > 255) {
        return Some("malformed IPv4 literal".into());
    }
    let (a, b) = (parts[0], parts[1]);
    Some(match () {
        _ if a == 0 => "0.0.0.0/8 (this-network)".to_string(),
        _ if a == 10 => "10.0.0.0/8 (RFC1918 private)".to_string(),
        _ if a == 127 => "127.0.0.0/8 (loopback)".to_string(),
        _ if a == 169 && b == 254 => "169.254.0.0/16 (link-local / cloud metadata)".to_string(),
        _ if (172..=172).contains(&a) && (16..=31).contains(&b) => {
            "172.16.0.0/12 (RFC1918 private)".to_string()
        }
        _ if a == 192 && b == 168 => "192.168.0.0/16 (RFC1918 private)".to_string(),
        _ if a == 100 && (64..=127).contains(&b) => "100.64.0.0/10 (carrier-grade NAT)".to_string(),
        _ if a == 198 && (b == 18 || b == 19) => "198.18.0.0/15 (benchmarking)".to_string(),
        _ if a >= 224 => format!("{a}.0.0.0/4 (multicast/reserved)"),
        _ => return None,
    })
}

fn expand_ipv6(ip: &str) -> Option<Vec<u32>> {
    let raw: String = ip
        .to_ascii_lowercase()
        .trim_matches(|c| c == '[' || c == ']')
        .to_string();
    if !raw.contains(':') {
        return None;
    }
    let halves: Vec<&str> = raw.split("::").collect();
    if halves.len() > 2 {
        return None;
    }
    #[derive(Debug)]
    enum Group {
        Num(u32),
        V4([u32; 4]),
    }
    let parse_groups = |s: &str| -> Option<Vec<Group>> {
        s.split(':')
            .filter(|g| !g.is_empty())
            .map(|g| {
                if g.contains('.') {
                    let o: Vec<u32> = g
                        .split('.')
                        .map(|p| p.parse::<u32>().ok())
                        .collect::<Option<Vec<_>>>()?;
                    if o.len() != 4 || o.iter().any(|x| *x > 255) {
                        return None;
                    }
                    Some(Group::V4([o[0], o[1], o[2], o[3]]))
                } else {
                    let n = u32::from_str_radix(g, 16).ok()?;
                    if n <= 0xffff {
                        Some(Group::Num(n))
                    } else {
                        None
                    }
                }
            })
            .collect()
    };

    let mut groups: Vec<u32> = Vec::new();
    let emit = |groups: &mut Vec<u32>, g: &Group| match g {
        Group::Num(n) => groups.push(*n),
        Group::V4(o) => {
            groups.push((o[0] << 8) | o[1]);
            groups.push((o[2] << 8) | o[3]);
        }
    };

    if halves.len() == 2 {
        let head = parse_groups(halves[0])?;
        let tail = parse_groups(halves[1])?;
        for g in &head {
            emit(&mut groups, g);
        }
        let tail_slots: usize = tail
            .iter()
            .map(|g| if matches!(g, Group::Num(_)) { 1 } else { 2 })
            .sum();
        if groups.len() + tail_slots > 8 {
            return None;
        }
        while groups.len() + tail_slots < 8 {
            groups.push(0);
        }
        for g in &tail {
            emit(&mut groups, g);
        }
    } else {
        let head = parse_groups(&raw)?;
        for g in &head {
            emit(&mut groups, g);
        }
    }
    if groups.len() == 8 {
        Some(groups)
    } else {
        None
    }
}

fn ipv6_reason(groups: &[u32]) -> Option<String> {
    if groups.len() != 8 {
        return Some("malformed IPv6 literal".into());
    }
    let g0 = groups[0];
    let g5 = groups[5];
    let g6 = groups[6];
    let g7 = groups[7];
    if groups.iter().all(|g| *g == 0) {
        return Some(":: (unspecified address)".into());
    }
    if groups[..7].iter().all(|g| *g == 0) && g7 == 1 {
        return Some("::1 (IPv6 loopback)".into());
    }
    if groups[..5].iter().all(|g| *g == 0) && g5 == 0xffff {
        let v4 = format!("{}.{}.{}.{}", g6 >> 8, g6 & 0xff, g7 >> 8, g7 & 0xff);
        if let Some(reason) = ipv4_reason(&v4) {
            return Some(format!("::ffff:{v4} maps to {reason}"));
        }
    }
    if (g0 & 0xfe00) == 0xfc00 {
        return Some("fc00::/7 (IPv6 unique local)".into());
    }
    if (g0 & 0xffc0) == 0xfe80 {
        return Some("fe80::/10 (IPv6 link-local)".into());
    }
    if (g0 & 0xff00) == 0xff00 {
        return Some("ff00::/8 (IPv6 multicast)".into());
    }
    None
}

pub fn literal_ip_reason(ip: &str) -> Option<String> {
    if is_ipv4_literal(ip) {
        return ipv4_reason(ip);
    }
    if ip.contains(':') {
        return match expand_ipv6(ip) {
            Some(groups) => ipv6_reason(&groups),
            None => Some("unparseable IPv6 literal".into()),
        };
    }
    if !ip.is_empty() && ip.chars().all(|c| c.is_ascii_digit()) {
        return Some("numeric-encoded address form".into());
    }
    let hexish = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_hexdigit() || c == '.' || c == 'x')
    };
    if ip.to_ascii_lowercase().starts_with("0x") && hexish(ip) {
        return Some("numeric-encoded address form".into());
    }
    if !ip.is_empty() && ip.chars().all(|c| c.is_ascii_hexdigit()) && ip.len() >= 7 {
        return Some("numeric-encoded address form".into());
    }
    None
}

fn hostname_reason(host: &str) -> Option<String> {
    let h = host.to_ascii_lowercase();
    let h = h.trim_matches(|c| c == '[' || c == ']');
    if h == "localhost" || h.ends_with(".localhost") {
        return Some(format!("\"{h}\" is a localhost name"));
    }
    if h.ends_with(".local") || h.ends_with(".internal") {
        return Some(format!("\"{h}\" looks like an internal name"));
    }
    None
}

pub fn check_url_literal(raw_url: &str) -> SsrfCheckResult {
    let parsed = match url::Url::parse(raw_url) {
        Ok(u) => u,
        Err(e) => return SsrfCheckResult::refused(format!("invalid URL: {e}"), None),
    };
    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return SsrfCheckResult::refused(
                format!("protocol \"{other}:\" is not allowed (http/https only)"),
                None,
            )
        }
    }
    let host_raw = match parsed.host_str() {
        Some(h) if !h.is_empty() => h.to_string(),
        _ => return SsrfCheckResult::refused("URL has no host", None),
    };
    let host = host_raw.trim_matches(|c| c == '[' || c == ']').to_string();
    if let Some(reason) = literal_ip_reason(&host) {
        return SsrfCheckResult::refused(format!("refused {reason}"), Some(&host));
    }
    if let Some(reason) = hostname_reason(&host) {
        return SsrfCheckResult::refused(reason, Some(&host));
    }
    SsrfCheckResult {
        ok: true,
        reason: None,
        host: Some(host),
        resolved: Vec::new(),
    }
}

pub async fn check_url_resolved(raw_url: &str) -> SsrfCheckResult {
    let literal = check_url_literal(raw_url);
    if !literal.ok {
        return literal;
    }
    let host = literal.host.clone().unwrap_or_default();
    if is_ipv4_literal(&host) || host.contains(':') {
        return literal;
    }
    let host_for_dns = host.clone();
    let addresses = tokio::task::spawn_blocking(move || {
        use std::net::ToSocketAddrs;
        (host_for_dns.as_str(), 0u16)
            .to_socket_addrs()
            .ok()
            .map(|it| it.map(|a| a.ip().to_string()).collect::<Vec<String>>())
    })
    .await
    .unwrap_or(None);
    let Some(addresses) = addresses else {
        return SsrfCheckResult::refused(format!("host \"{host}\" does not resolve"), Some(&host));
    };
    if addresses.is_empty() {
        return SsrfCheckResult::refused(
            format!("host \"{host}\" resolved to no addresses"),
            Some(&host),
        );
    }
    for addr in &addresses {
        if let Some(reason) = literal_ip_reason(addr) {
            return SsrfCheckResult {
                ok: false,
                reason: Some(format!("host \"{host}\" resolves to {addr} ({reason})")),
                host: Some(host),
                resolved: addresses,
            };
        }
    }
    SsrfCheckResult {
        ok: true,
        reason: None,
        host: Some(host),
        resolved: addresses,
    }
}

pub fn validate_ssrf_full(raw_url: &str) -> crate::error::Result<()> {
    let literal = check_url_literal(raw_url);
    if !literal.ok {
        return Err(crate::error::Error::Config(format!(
            "SSRF guard: {}",
            literal.reason.unwrap_or_else(|| "refused".into())
        )));
    }
    let host = literal.host.clone().unwrap_or_default();
    if is_ipv4_literal(&host) || host.contains(':') {
        return Ok(());
    }
    use std::net::ToSocketAddrs;
    match (host.as_str(), 0u16).to_socket_addrs() {
        Err(_) => {
            return Err(crate::error::Error::Config(format!(
                "SSRF guard: host \"{host}\" does not resolve"
            )))
        }
        Ok(addrs) => {
            let list: Vec<std::net::SocketAddr> = addrs.collect();
            if list.is_empty() {
                return Err(crate::error::Error::Config(format!(
                    "SSRF guard: host \"{host}\" resolved to no addresses"
                )));
            }
            for a in &list {
                if let Some(reason) = literal_ip_reason(&a.ip().to_string()) {
                    return Err(crate::error::Error::Config(format!(
                        "SSRF guard: host \"{host}\" resolves to {} ({reason})",
                        a.ip()
                    )));
                }
            }
        }
    }
    Ok(())
}

pub struct RedactionPattern {
    pub name: &'static str,
    pub regex: Regex,
    pub replacement: &'static str,
    pub prefix: Option<Regex>,
}

pub fn redaction_patterns() -> Vec<RedactionPattern> {
    vec![
        RedactionPattern {
            name: "card_number",
            regex: Regex::new(r"\b(?:\d[ -]*?){13,16}\b").unwrap(),
            replacement: "[REDACTED-CARD]",
            prefix: None,
        },
        RedactionPattern {
            name: "cvv",
            regex: Regex::new(r"(?i)\b(?:cvv|cvc|security code)[\s:=-]*(\d{3,4})\b").unwrap(),
            replacement: "[REDACTED-CVV]",
            prefix: Some(Regex::new(r"(?i)^[\s]*(?:cvv|cvc|security code)?[\s:=-]*").unwrap()),
        },
        RedactionPattern {
            name: "api_key",
            regex: Regex::new(r"(?i)\b(?:api[_-]?key|token|secret|password|passwd|pwd)\b[\s:=-]*[A-Za-z0-9_\-.]{8,}").unwrap(),
            replacement: "[REDACTED-SECRET]",
            prefix: Some(Regex::new(r"(?i)^[\s]*(?:api[_-]?key|token|secret|password|passwd|pwd)?[\s:=-]*").unwrap()),
        },
        RedactionPattern {
            name: "bearer",
            regex: Regex::new(r"(?i)\bBearer\s+[A-Za-z0-9\-._~+/]+=*").unwrap(),
            replacement: "[REDACTED-TOKEN]",
            prefix: None,
        },
        RedactionPattern {
            name: "aws_key",
            regex: Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap(),
            replacement: "[REDACTED-AWS-KEY]",
            prefix: None,
        },
        RedactionPattern {
            name: "private_key_block",
            regex: Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----").unwrap(),
            replacement: "[REDACTED-PRIVATE-KEY]",
            prefix: None,
        },
    ]
}

pub fn redact_text(text: &str, enabled: bool) -> (String, Vec<Value>) {
    if !enabled {
        return (text.to_string(), Vec::new());
    }
    let mut out = text.to_string();
    let mut redactions: Vec<Value> = Vec::new();
    for p in redaction_patterns() {
        let matches: Vec<&str> = p.regex.find_iter(&out).map(|m| m.as_str()).collect();
        if !matches.is_empty() {
            let count = matches.len();
            let prefix = p.prefix.clone();
            let replacement = p.replacement;
            out = p
                .regex
                .replace_all(&out, |caps: &regex::Captures<'_>| {
                    let whole = caps.get(0).map(|m| m.as_str()).unwrap_or_default();
                    let keep = prefix
                        .as_ref()
                        .and_then(|pre| pre.find(whole).map(|m| m.as_str().to_string()))
                        .unwrap_or_default();
                    format!("{keep}{replacement}")
                })
                .to_string();
            redactions.push(json!({ "pattern": p.name, "count": count }));
        }
    }
    (out, redactions)
}
