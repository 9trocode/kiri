//! Takes out of logs and reports what must not leave the Mac: the home
//! folder's path (it names the person), bearer tokens, the values of
//! token, password and secret keys in JSON, query strings and `KEY=value`
//! pairs, and Kiri share tokens (`ks_…`).
//!
//! Patterns, not parsing: it runs over any text (log lines, panic
//! messages, macOS crash reports), so it errs towards taking out too much.

const MARK: &str = "[redacted]";

#[derive(Debug, Default)]
pub struct Redactor {
    home: Option<String>,
    /// Exact strings to take out wherever they appear (the signed-in token).
    secrets: Vec<String>,
}

impl Redactor {
    pub fn new(home: Option<String>, secrets: Vec<String>) -> Redactor {
        let home = home.map(|h| h.trim_end_matches('/').to_owned()).filter(|h| h.len() > 1);
        Redactor { home, secrets: secrets.into_iter().filter(|s| s.len() >= 8).collect() }
    }

    /// `$HOME` → `~`, and the patterns.
    pub fn home_only() -> Redactor {
        Redactor::new(std::env::var("HOME").ok(), Vec::new())
    }

    /// For a report: also the Kiri Cloud token, when signed in.
    pub fn for_user() -> Redactor {
        Redactor::new(std::env::var("HOME").ok(), crate::share::read_config().token.into_iter().collect())
    }

    pub fn apply(&self, text: &str) -> String {
        let mut s = text.to_owned();
        for secret in &self.secrets {
            s = s.replace(secret.as_str(), MARK);
        }
        if let Some(home) = &self.home {
            s = replace_home(&s, home);
        }
        patterns(&s)
    }
}

/// The patterns alone (no home, no known secrets): cheap enough for every
/// log line.
pub fn patterns(s: &str) -> String {
    let s = bearer(s);
    let s = json_values(&s, "\"");
    // The same inside a JSON string: `{\"token\":\"…\"}`.
    let s = json_values(&s, "\\\"");
    let s = assignments(&s);
    share_tokens(&s)
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// A key whose value is secret: `token`, `access_token`, `PASSWORD`,
/// `client_secret`, docker's `auth`, `identitytoken`…
fn sensitive(key: &str) -> bool {
    let k = key.trim_start_matches('-').to_ascii_lowercase();
    k == "auth" || ["token", "password", "passwd", "secret", "apikey", "api_key", "credential", "crashreporterkey"].iter().any(|w| k.contains(w))
}

/// `/Users/ada/x` → `~/x`, but not `/Users/adam`.
fn replace_home(s: &str, home: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(home) {
        let after = &rest[i + home.len()..];
        let whole = after.bytes().next().is_none_or(|b| !(is_word(b) || b == b'-' || b == b'.'));
        out.push_str(&rest[..i]);
        out.push_str(if whole { "~" } else { home });
        rest = after;
    }
    out.push_str(rest);
    out
}

/// `Bearer <token>` (any case) → `Bearer [redacted]`.
fn bearer(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let b = s.as_bytes();
    let token_byte = |c: u8| c.is_ascii_alphanumeric() || b"-._~+/=".contains(&c);
    let (mut out, mut last, mut from) = (String::with_capacity(s.len()), 0, 0);
    while let Some(i) = lower[from..].find("bearer").map(|i| i + from) {
        let mut j = i + 6;
        while j < b.len() && (b[j] == b' ' || b[j] == b'\t') {
            j += 1;
        }
        let start = j;
        while j < b.len() && token_byte(b[j]) {
            j += 1;
        }
        if (i == 0 || !is_word(b[i - 1])) && start > i + 6 && j - start >= 8 {
            out.push_str(&s[last..start]);
            out.push_str(MARK);
            last = j;
        }
        from = j.max(i + 6);
    }
    out.push_str(&s[last..]);
    out
}

/// The end of a string value starting at `s[0]`: the next `q` (for a plain
/// quote, one not escaped).
fn close(s: &str, q: &str) -> Option<usize> {
    if q != "\"" {
        return s.find(q);
    }
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// `"token": "…"` → `"token": "[redacted]"`, with `q` as the quote.
fn json_values(s: &str, q: &str) -> String {
    let (mut out, mut last, mut from) = (String::with_capacity(s.len()), 0, 0);
    while let Some(i) = s[from..].find(q).map(|i| i + from) {
        let key_start = i + q.len();
        let Some(key_len) = s[key_start..].find(q) else { break };
        let key = &s[key_start..key_start + key_len];
        let mut j = key_start + key_len + q.len();
        let skip = |j: usize| j + (s[j..].len() - s[j..].trim_start().len());
        j = skip(j);
        if !key.is_empty() && key.len() <= 64 && !key.contains(['\\', '"']) && s[j..].starts_with(':') {
            j = skip(j + 1);
            if sensitive(key) && s[j..].starts_with(q) {
                let start = j + q.len();
                if let Some(len) = close(&s[start..], q) {
                    out.push_str(&s[last..start]);
                    out.push_str(MARK);
                    last = start + len;
                    from = last + q.len();
                    continue;
                }
            }
            from = j;
            continue;
        }
        // The closing quote may open the next key.
        from = key_start + key_len;
    }
    out.push_str(&s[last..]);
    out
}

/// `?token=…&`, `PASSWORD=…`, `--password "…"` is left, `--password=…` is
/// not: the value after a sensitive key and `=`.
fn assignments(s: &str) -> String {
    let b = s.as_bytes();
    let key_byte = |c: u8| is_word(c) || c == b'-' || c == b'.';
    let (mut out, mut last) = (String::with_capacity(s.len()), 0);
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'=' || b.get(i + 1) == Some(&b'=') || i < last {
            i += 1;
            continue;
        }
        let mut k = i;
        while k > 0 && key_byte(b[k - 1]) {
            k -= 1;
        }
        let start = i + 1;
        let end = match b.get(start) {
            Some(&qc @ (b'"' | b'\'')) => s[start + 1..].find(qc as char).map_or(b.len(), |n| start + 1 + n + 1),
            _ => s[start..].find(|c: char| c.is_whitespace() || "&\"',;)#<>".contains(c)).map_or(b.len(), |n| start + n),
        };
        if k < i && sensitive(&s[k..i]) && end > start && &s[start..end] != MARK {
            let quoted = matches!(b[start], b'"' | b'\'');
            out.push_str(&s[last..start]);
            if quoted {
                out.push(b[start] as char);
                out.push_str(MARK);
                out.push(b[start] as char);
            } else {
                out.push_str(MARK);
            }
            last = end;
        }
        i = end.max(i + 1);
    }
    out.push_str(&s[last..]);
    out
}

/// `ks_…`, a Kiri share token, wherever it appears.
fn share_tokens(s: &str) -> String {
    let b = s.as_bytes();
    let tail = |c: u8| is_word(c) || c == b'-';
    let (mut out, mut last, mut from) = (String::with_capacity(s.len()), 0, 0);
    while let Some(i) = s[from..].find("ks_").map(|i| i + from) {
        let mut j = i + 3;
        while j < b.len() && tail(b[j]) {
            j += 1;
        }
        if (i == 0 || !is_word(b[i - 1])) && j - (i + 3) >= 12 {
            out.push_str(&s[last..i + 3]);
            out.push_str(MARK);
            last = j;
        }
        from = j;
    }
    out.push_str(&s[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r() -> Redactor {
        Redactor::new(Some("/Users/ada/".into()), vec!["s3cret-signed-in-token".into(), "short".into()])
    }

    #[test]
    fn home_becomes_tilde_but_not_a_longer_name() {
        let out = r().apply("open /Users/ada/.kiri/logs and /Users/adam/x and /Users/ada");
        assert_eq!(out, "open ~/.kiri/logs and /Users/adam/x and ~");
    }

    #[test]
    fn known_secrets_go_wherever_they_are() {
        assert_eq!(r().apply("sent s3cret-signed-in-token twice: s3cret-signed-in-token"), "sent [redacted] twice: [redacted]");
        // Too short to be worth matching blindly.
        assert_eq!(r().apply("a short note"), "a short note");
    }

    #[test]
    fn bearer_tokens() {
        assert_eq!(patterns("Authorization: Bearer eyJhbGciOi.J9x-y_z"), "Authorization: Bearer [redacted]");
        assert_eq!(patterns("authorization: bearer abcdefgh1234"), "authorization: bearer [redacted]");
        // Words, not tokens.
        assert_eq!(patterns("the bearer of bad news"), "the bearer of bad news");
        assert_eq!(patterns("Bearer [redacted]"), "Bearer [redacted]");
    }

    #[test]
    fn json_values_of_secret_keys() {
        assert_eq!(
            patterns(r#"{"login":"ada","token":"ks_abc","password" : "p\"w","access_token":"x","auths":{"r":{"auth":"YWRhOnB3"}}}"#),
            r#"{"login":"ada","token":"[redacted]","password" : "[redacted]","access_token":"[redacted]","auths":{"r":{"auth":"[redacted]"}}}"#
        );
        // Escaped, as inside a log line's message.
        assert_eq!(patterns(r#"body {\"secret\":\"hunter2\",\"n\":1}"#), r#"body {\"secret\":\"[redacted]\",\"n\":1}"#);
        // Not a key: a value that happens to say token.
        assert_eq!(patterns(r#"{"msg":"token","n":"1"}"#), r#"{"msg":"token","n":"1"}"#);
    }

    #[test]
    fn query_values_and_assignments() {
        assert_eq!(patterns("GET /cb?code=1&token=abc123&x=2 HTTP/1.1"), "GET /cb?code=1&token=[redacted]&x=2 HTTP/1.1");
        assert_eq!(patterns("https://x/?password=hunter2"), "https://x/?password=[redacted]");
        assert_eq!(patterns("-e DB_PASSWORD=hunter2 -e MODE=dev"), "-e DB_PASSWORD=[redacted] -e MODE=dev");
        assert_eq!(patterns("--password='two words' next"), "--password='[redacted]' next");
        assert_eq!(patterns("KIRI_SHARE_TOKEN=\"abc def\""), "KIRI_SHARE_TOKEN=\"[redacted]\"");
        assert_eq!(patterns("if token == other"), "if token == other");
        assert_eq!(patterns("a=b token="), "a=b token=");
    }

    #[test]
    fn share_tokens_anywhere() {
        assert_eq!(patterns("signed in with ks_0123456789abcdefXYZ, ok"), "signed in with ks_[redacted], ok");
        // Too short, or part of a word.
        assert_eq!(patterns("ks_x and thanks_0123456789abcdef"), "ks_x and thanks_0123456789abcdef");
    }

    #[test]
    fn redacting_twice_changes_nothing() {
        let text = r#"Bearer abcdefghijkl token=abc {"token":"x"} ks_0123456789abcdef /Users/ada/x"#;
        let once = r().apply(text);
        assert_eq!(r().apply(&once), once);
        assert!(!once.contains("abcdefghijkl") && !once.contains("0123456789abcdef") && !once.contains("/Users/ada"), "{once}");
    }

    #[test]
    fn leaves_other_text_and_unicode_alone() {
        let text = "VM booted in 112 ms → 4 CPUs, café=ok, a = b";
        assert_eq!(patterns(text), text);
    }
}
