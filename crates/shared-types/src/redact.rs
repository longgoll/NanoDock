/// Redacts usernames and common secret/token patterns from diagnostic text before sharing.
pub fn redact_sensitive_text(input: &str) -> String {
    let mut out = input.to_string();

    for var in ["USERNAME", "USER"] {
        if let Ok(user) = std::env::var(var) {
            let trimmed = user.trim();
            if trimmed.len() >= 2 {
                out = out.replace(&format!("C:\\Users\\{trimmed}\\"), "C:\\Users\\<USER>\\");
                out = out.replace(&format!("C:/Users/{trimmed}/"), "C:/Users/<USER>/");
                out = out.replace(&format!("/home/{trimmed}/"), "/home/<USER>/");
                out = out.replace(&format!("/Users/{trimmed}/"), "/Users/<USER>/");
            }
        }
    }

    let mut sanitized_lines = Vec::new();
    for line in out.lines() {
        sanitized_lines.push(redact_line_secrets(line));
    }
    let mut joined = sanitized_lines.join("\n");
    if input.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

fn redact_line_secrets(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    for marker in ["bearer ", "token=", "password=", "passwd=", "secret=", "api_key="] {
        if let Some(pos) = lower.find(marker) {
            let prefix_end = pos + marker.len();
            let rest = &line[prefix_end..];
            let secret_len = rest
                .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '&' | ';' | ','))
                .unwrap_or(rest.len());
            if secret_len > 0 {
                return format!(
                    "{}***REDACTED***{}",
                    &line[..prefix_end],
                    &rest[secret_len..]
                );
            }
        }
    }
    line.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_bearer_tokens_and_key_value_secrets() {
        let raw = "Authorization: Bearer abc123xyz\nurl=https://x?token=mysecret&ok=1\npassword=hunter2";
        let clean = redact_sensitive_text(raw);
        assert!(!clean.contains("abc123xyz"));
        assert!(!clean.contains("mysecret"));
        assert!(!clean.contains("hunter2"));
        assert!(clean.contains("***REDACTED***"));
        assert!(clean.contains("&ok=1"));
    }
}
