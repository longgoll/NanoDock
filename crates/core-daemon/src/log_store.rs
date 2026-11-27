use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

pub fn default_log_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("NanoDock")
            .join("logs")
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nanodock")
            .join("logs")
    }
}

pub fn default_log_path() -> PathBuf {
    default_log_dir().join("core-daemon.log")
}

#[derive(Clone)]
pub struct TeeLogWriter {
    file: Option<Arc<Mutex<File>>>,
}

impl TeeLogWriter {
    pub fn open(path: &Path) -> Self {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(meta) = fs::metadata(path) {
            if meta.len() > MAX_LOG_BYTES {
                let backup = path.with_extension("log.old");
                let _ = fs::rename(path, backup);
            }
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()
            .map(|f| Arc::new(Mutex::new(f)));
        Self { file }
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for TeeLogWriter {
    type Writer = TeeWriterInstance;

    fn make_writer(&'a self) -> Self::Writer {
        TeeWriterInstance {
            file: self.file.clone(),
        }
    }
}

pub struct TeeWriterInstance {
    file: Option<Arc<Mutex<File>>>,
}

impl Write for TeeWriterInstance {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let _ = io::stderr().write_all(buf);
        if let Some(ref f) = self.file {
            if let Ok(mut guard) = f.lock() {
                let _ = guard.write_all(buf);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let _ = io::stderr().flush();
        if let Some(ref f) = self.file {
            if let Ok(mut guard) = f.lock() {
                let _ = guard.flush();
            }
        }
        Ok(())
    }
}

pub fn read_tail_lines(path: &Path, max_lines: usize) -> String {
    let Ok(content) = fs::read_to_string(path) else {
        return String::new();
    };
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}
