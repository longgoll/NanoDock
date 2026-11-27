pub(crate) fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

pub(crate) fn is_container_create_request(first_line: &str) -> bool {
    let mut parts = first_line.split_whitespace();
    let Some(method) = parts.next() else {
        return false;
    };
    let Some(path) = parts.next() else {
        return false;
    };
    let clean_path = path.split('?').next().unwrap_or(path);
    method.eq_ignore_ascii_case("POST") && clean_path.ends_with("/containers/create")
}

pub(crate) fn is_chunked_message(header_str: &str) -> bool {
    header_str.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, val)| {
            name.trim().eq_ignore_ascii_case("transfer-encoding")
                && val.to_ascii_lowercase().contains("chunked")
        })
    })
}

pub(crate) fn parse_content_length(header_str: &str) -> Option<usize> {
    for line in header_str.lines() {
        if let Some((name, val)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                return val.trim().parse::<usize>().ok();
            }
        }
    }
    None
}

pub(crate) fn is_upgrade_request(header_str: &str) -> bool {
    for line in header_str.lines() {
        if let Some((name, val)) = line.split_once(':') {
            let name = name.trim();
            if name.eq_ignore_ascii_case("upgrade") {
                return true;
            }
            if name.eq_ignore_ascii_case("connection")
                && val.to_ascii_lowercase().contains("upgrade")
            {
                return true;
            }
        }
    }
    false
}

pub(crate) fn has_expect_100(header_str: &str) -> bool {
    header_str.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, val)| {
            name.trim().eq_ignore_ascii_case("expect")
                && val.to_ascii_lowercase().contains("100-continue")
        })
    })
}

pub(crate) fn has_connection_close(header_str: &str) -> bool {
    header_str.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, val)| {
            name.trim().eq_ignore_ascii_case("connection")
                && val
                    .split(',')
                    .any(|token| token.trim().eq_ignore_ascii_case("close"))
        })
    })
}

pub(crate) fn force_connection_close(header_str: &str) -> String {
    rewrite_connection_header(header_str, Some("Connection: close\r\n"))
}

pub(crate) fn strip_connection_close(header_str: &str) -> String {
    if !has_connection_close(header_str) {
        return header_str.to_string();
    }
    rewrite_connection_header(header_str, None)
}

fn rewrite_connection_header(header_str: &str, replacement: Option<&str>) -> String {
    let mut out = String::with_capacity(header_str.len() + 20);
    let mut replaced = false;
    for line in header_str.split_inclusive("\r\n") {
        if line == "\r\n" {
            if !replaced {
                if let Some(rep) = replacement {
                    out.push_str(rep);
                }
                replaced = true;
            }
            out.push_str(line);
            continue;
        }
        if let Some((name, _)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("connection") {
                if !replaced {
                    if let Some(rep) = replacement {
                        out.push_str(rep);
                    }
                    replaced = true;
                }
                continue;
            }
        }
        out.push_str(line);
    }
    out
}

pub(crate) fn replace_chunked_with_content_length(header_str: &str, new_len: usize) -> String {
    let mut out = String::with_capacity(header_str.len() + 32);
    for line in header_str.split_inclusive("\r\n") {
        if let Some((name, _)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("transfer-encoding") {
                out.push_str(&format!("Content-Length: {new_len}\r\n"));
                continue;
            }
        }
        out.push_str(line);
    }
    out
}

pub(crate) fn replace_content_length(header_str: &str, new_len: usize) -> String {
    let mut out = String::with_capacity(header_str.len() + 16);
    for line in header_str.split_inclusive("\r\n") {
        if let Some((name, _)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                out.push_str(&format!("Content-Length: {new_len}\r\n"));
                continue;
            }
        }
        out.push_str(line);
    }
    out
}

pub(crate) fn response_supports_keep_alive(resp_header: &str, client_asked_close: bool) -> bool {
    if client_asked_close {
        return false;
    }
    let status = resp_header
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    if matches!(status, 100..=199 | 204 | 304) {
        return true;
    }
    parse_content_length(resp_header).is_some() || is_chunked_message(resp_header)
}
