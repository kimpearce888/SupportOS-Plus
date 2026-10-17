use leptos_router::*;

#[must_use]
pub fn encode_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub type Param = (&'static str, Option<String>);

#[must_use]
pub fn query_string(pairs: &[Param]) -> String {
    let mut out = String::new();
    for (name, value) in pairs {
        if let Some(v) = value {
            if v.is_empty() {
                continue;
            }
            if out.is_empty() {
                out.push('?');
            } else {
                out.push('&');
            }
            out.push_str(name);
            out.push('=');
            out.push_str(&encode_value(v));
        }
    }
    out
}

#[must_use]
pub fn query_str(map: &ParamsMap, key: &str) -> Option<String> {
    map.get(key)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

#[must_use]
pub fn query_pos_int(map: &ParamsMap, key: &str) -> Option<i64> {
    query_str(map, key).and_then(|v| v.parse::<i64>().ok().filter(|n| *n > 0))
}

pub fn replace_query<N>(navigate: &N, pathname: &str, pairs: &[Param])
where
    N: Fn(&str, NavigateOptions) + 'static,
{
    navigate(
        &format!("{}{}", pathname, query_string(pairs)),
        NavigateOptions {
            replace: true,
            ..Default::default()
        },
    );
}
