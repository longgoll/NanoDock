/// Incrementally decodes UTF-8 byte chunks without corrupting multi-byte characters
/// split across adjacent `read()` calls.
#[derive(Default)]
pub struct Utf8CarryDecoder {
    pending: Vec<u8>,
}

impl Utf8CarryDecoder {
    pub fn push(&mut self, bytes: &[u8]) -> String {
        if bytes.is_empty() && self.pending.is_empty() {
            return String::new();
        }
        self.pending.extend_from_slice(bytes);
        let mut out = String::new();
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(valid) => {
                    out.push_str(valid);
                    self.pending.clear();
                    break;
                }
                Err(err) => {
                    let valid_end = err.valid_up_to();
                    if valid_end > 0 {
                        if let Ok(prefix) = std::str::from_utf8(&self.pending[..valid_end]) {
                            out.push_str(prefix);
                        }
                    }
                    if let Some(invalid_len) = err.error_len() {
                        out.push('\u{FFFD}');
                        self.pending.drain(..valid_end + invalid_len);
                    } else {
                        self.pending.drain(..valid_end);
                        break;
                    }
                }
            }
        }
        out
    }

    pub fn flush(&mut self) -> String {
        if self.pending.is_empty() {
            return String::new();
        }
        let s = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending.clear();
        s
    }
}

/// Incrementally decodes Docker `/containers/{id}/logs` streams, handling both
/// `Transfer-Encoding: chunked` framing and 8-byte multiplexed log headers.
pub struct DockerLogStreamDecoder {
    chunked: bool,
    raw_buf: Vec<u8>,
    chunk_remaining: usize,
    expect_chunk_crlf: bool,
    chunk_eof: bool,
    payload_buf: Vec<u8>,
    utf8: Utf8CarryDecoder,
}

impl DockerLogStreamDecoder {
    pub fn new(chunked: bool) -> Self {
        Self {
            chunked,
            raw_buf: Vec::new(),
            chunk_remaining: 0,
            expect_chunk_crlf: false,
            chunk_eof: false,
            payload_buf: Vec::new(),
            utf8: Utf8CarryDecoder::default(),
        }
    }

    pub fn push(&mut self, bytes: &[u8]) -> String {
        if self.chunked {
            self.raw_buf.extend_from_slice(bytes);
            self.decode_chunks();
        } else {
            self.payload_buf.extend_from_slice(bytes);
        }
        self.decode_payload()
    }

    pub fn flush(&mut self) -> String {
        let mut out = self.decode_payload();
        if !self.payload_buf.is_empty() {
            out.push_str(&self.utf8.push(&self.payload_buf));
            self.payload_buf.clear();
        }
        out.push_str(&self.utf8.flush());
        out
    }

    fn decode_chunks(&mut self) {
        while !self.chunk_eof {
            if self.expect_chunk_crlf {
                if self.raw_buf.len() < 2 {
                    break;
                }
                if self.raw_buf.starts_with(b"\r\n") {
                    self.raw_buf.drain(..2);
                }
                self.expect_chunk_crlf = false;
            }

            if self.chunk_remaining == 0 {
                let Some(pos) = self.raw_buf.windows(2).position(|w| w == b"\r\n") else {
                    break;
                };
                let line = String::from_utf8_lossy(&self.raw_buf[..pos]);
                let hex = line.trim().split(';').next().unwrap_or("");
                let Ok(size) = usize::from_str_radix(hex, 16) else {
                    // Fallback: treat remainder as raw payload if malformed.
                    self.payload_buf.append(&mut self.raw_buf);
                    break;
                };
                self.raw_buf.drain(..pos + 2);
                if size == 0 {
                    self.chunk_eof = true;
                    break;
                }
                self.chunk_remaining = size;
            }

            if self.raw_buf.is_empty() {
                break;
            }
            let take = self.raw_buf.len().min(self.chunk_remaining);
            self.payload_buf.extend_from_slice(&self.raw_buf[..take]);
            self.raw_buf.drain(..take);
            self.chunk_remaining -= take;
            if self.chunk_remaining == 0 {
                self.expect_chunk_crlf = true;
            }
        }
    }

    fn decode_payload(&mut self) -> String {
        let mut out = String::new();
        while !self.payload_buf.is_empty() {
            if self.payload_buf.len() < 8 {
                let maybe_header = matches!(self.payload_buf[0], 0..=2)
                    && self.payload_buf[1..].iter().all(|&b| b == 0);
                if maybe_header {
                    break;
                }
                out.push_str(&self.utf8.push(&self.payload_buf));
                self.payload_buf.clear();
                break;
            }

            let is_multiplexed = matches!(self.payload_buf[0], 0..=2)
                && self.payload_buf[1] == 0
                && self.payload_buf[2] == 0
                && self.payload_buf[3] == 0;
            if is_multiplexed {
                let size = u32::from_be_bytes([
                    self.payload_buf[4],
                    self.payload_buf[5],
                    self.payload_buf[6],
                    self.payload_buf[7],
                ]) as usize;
                if self.payload_buf.len() < 8 + size {
                    break;
                }
                out.push_str(&self.utf8.push(&self.payload_buf[8..8 + size]));
                self.payload_buf.drain(..8 + size);
            } else {
                out.push_str(&self.utf8.push(&self.payload_buf));
                self.payload_buf.clear();
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_split_multibyte_utf8_across_reads() {
        let mut dec = Utf8CarryDecoder::default();
        let text = "Xin chào Việt Nam 🚀";
        let bytes = text.as_bytes();
        let mut combined = String::new();
        for chunk in bytes.chunks(3) {
            combined.push_str(&dec.push(chunk));
        }
        combined.push_str(&dec.flush());
        assert_eq!(combined, text);
    }

    #[test]
    fn decodes_chunked_and_split_multiplexed_docker_logs() {
        let mut frame1 = vec![1, 0, 0, 0, 0, 0, 0, 6];
        frame1.extend_from_slice(b"hello\n");
        let mut frame2 = vec![2, 0, 0, 0, 0, 0, 0, 4];
        frame2.extend_from_slice(b"err\n");

        let chunked = format!(
            "{:x}\r\n{}\r\n{:x}\r\n{}\r\n0\r\n\r\n",
            frame1.len(),
            String::from_utf8_lossy(&frame1),
            frame2.len(),
            String::from_utf8_lossy(&frame2)
        );

        let mut dec = DockerLogStreamDecoder::new(true);
        let mut out = String::new();
        for piece in chunked.as_bytes().chunks(5) {
            out.push_str(&dec.push(piece));
        }
        out.push_str(&dec.flush());
        assert_eq!(out, "hello\nerr\n");
    }
}
