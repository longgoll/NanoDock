#[cfg(unix)]
pub async fn run_vsock_server(port: u32, socket_path: &str) -> anyhow::Result<()> {
    use std::os::unix::io::{FromRawFd, OwnedFd};
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use anyhow::Context as _;
    use tokio::io::unix::AsyncFd;
    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

    const AF_VSOCK: libc::c_int = 40;
    const VMADDR_CID_ANY: u32 = u32::MAX;

    #[repr(C)]
    struct SockAddrVm {
        svm_family: libc::sa_family_t,
        svm_reserved1: u16,
        svm_port: u32,
        svm_cid: u32,
        svm_zero: [u8; 4],
    }

    struct VsockStream(AsyncFd<OwnedFd>);

    impl AsyncRead for VsockStream {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            use std::os::unix::io::AsRawFd;
            loop {
                let mut guard = match self.0.poll_read_ready(cx) {
                    Poll::Ready(Ok(g)) => g,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                };
                let unfilled = buf.initialize_unfilled();
                let rc = unsafe {
                    libc::read(
                        self.0.get_ref().as_raw_fd(),
                        unfilled.as_mut_ptr() as *mut libc::c_void,
                        unfilled.len(),
                    )
                };
                if rc >= 0 {
                    buf.advance(rc as usize);
                    return Poll::Ready(Ok(()));
                }
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::WouldBlock {
                    guard.clear_ready();
                    continue;
                }
                return Poll::Ready(Err(err));
            }
        }
    }

    impl AsyncWrite for VsockStream {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            use std::os::unix::io::AsRawFd;
            loop {
                let mut guard = match self.0.poll_write_ready(cx) {
                    Poll::Ready(Ok(g)) => g,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                };
                let rc = unsafe {
                    libc::write(
                        self.0.get_ref().as_raw_fd(),
                        buf.as_ptr() as *const libc::c_void,
                        buf.len(),
                    )
                };
                if rc >= 0 {
                    return Poll::Ready(Ok(rc as usize));
                }
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::WouldBlock {
                    guard.clear_ready();
                    continue;
                }
                return Poll::Ready(Err(err));
            }
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            use std::os::unix::io::AsRawFd;
            let rc = unsafe { libc::shutdown(self.0.get_ref().as_raw_fd(), libc::SHUT_WR) };
            if rc < 0 {
                Poll::Ready(Err(std::io::Error::last_os_error()))
            } else {
                Poll::Ready(Ok(()))
            }
        }
    }

    let fd = unsafe { libc::socket(AF_VSOCK, libc::SOCK_STREAM | libc::SOCK_NONBLOCK, 0) };
    if fd < 0 {
        anyhow::bail!("AF_VSOCK socket() failed: {}", std::io::Error::last_os_error());
    }
    let listener_fd = unsafe { OwnedFd::from_raw_fd(fd) };

    let addr = SockAddrVm {
        svm_family: AF_VSOCK as libc::sa_family_t,
        svm_reserved1: 0,
        svm_port: port,
        svm_cid: VMADDR_CID_ANY,
        svm_zero: [0; 4],
    };

    use std::os::unix::io::AsRawFd;
    let bind_rc = unsafe {
        libc::bind(
            listener_fd.as_raw_fd(),
            &addr as *const SockAddrVm as *const libc::sockaddr,
            std::mem::size_of::<SockAddrVm>() as libc::socklen_t,
        )
    };
    if bind_rc < 0 {
        anyhow::bail!(
            "AF_VSOCK bind(port={port}) failed: {}",
            std::io::Error::last_os_error()
        );
    }

    if unsafe { libc::listen(listener_fd.as_raw_fd(), 64) } < 0 {
        anyhow::bail!("AF_VSOCK listen() failed: {}", std::io::Error::last_os_error());
    }

    let async_listener = AsyncFd::new(listener_fd).context("register AF_VSOCK AsyncFd")?;

    loop {
        let mut guard = async_listener.readable().await?;
        match guard.try_io(|inner| {
            let conn_fd = unsafe {
                libc::accept4(
                    inner.get_ref().as_raw_fd(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                )
            };
            if conn_fd < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(unsafe { OwnedFd::from_raw_fd(conn_fd) })
            }
        }) {
            Ok(Ok(conn_fd)) => {
                let stream = VsockStream(AsyncFd::new(conn_fd)?);
                let path = socket_path.to_string();
                tokio::spawn(async move {
                    if let Err(err) = crate::server::run_yamux_server(stream, &path).await {
                        eprintln!("nanodock-agent vsock session error: {err:#}");
                    }
                });
            }
            Ok(Err(err)) => return Err(err.into()),
            Err(_would_block) => continue,
        }
    }
}
