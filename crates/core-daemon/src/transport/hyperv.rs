use std::sync::Arc;

use tracing::{debug, info};

use crate::transport::traits::StreamTransport;
use crate::transport::yamux_client::YamuxClient;

/// Standard Hyper-V / WSL2 `AF_VSOCK` template GUID suffix:
/// `{xxxxxxxx-facb-11e6-bd58-64006a7986d3}` where `xxxxxxxx` is the little-endian `vsock` port.
pub const DEFAULT_VSOCK_PORT: u32 = 47000;

/// Builds the 16-byte Windows GUID for a Linux `AF_VSOCK` port on Hyper-V.
pub fn build_vsock_service_guid(port: u32) -> [u8; 16] {
    let p = port.to_le_bytes();
    // Data1 (LE u32 = port), Data2 (0xfacb LE), Data3 (0x11e6 LE), Data4 (bd 58 64 00 6a 79 86 d3)
    [
        p[0], p[1], p[2], p[3], 0xcb, 0xfa, 0xe6, 0x11, 0xbd, 0x58, 0x64, 0x00, 0x6a, 0x79, 0x86,
        0xd3,
    ]
}

/// Parses a standard hyphenated GUID string (`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`)
/// into its 16-byte mixed-endian Windows `GUID` layout.
pub fn parse_guid(input: &str) -> Option<[u8; 16]> {
    let hex: String = input
        .trim()
        .trim_matches(|c| c == '{' || c == '}')
        .chars()
        .filter(|c| *c != '-')
        .collect();
    if hex.len() != 32 {
        return None;
    }
    let mut raw = [0u8; 16];
    let (chunks, _) = hex.as_bytes().as_chunks::<2>();
    for (i, chunk) in chunks.iter().enumerate() {
        let s = std::str::from_utf8(chunk).ok()?;
        raw[i] = u8::from_str_radix(s, 16).ok()?;
    }
    // Convert Data1 (4B), Data2 (2B), Data3 (2B) from big-endian hex to little-endian Windows GUID.
    raw[0..4].reverse();
    raw[4..6].reverse();
    raw[6..8].reverse();
    Some(raw)
}

#[cfg(windows)]
#[repr(C)]
#[allow(non_snake_case)]
struct SockAddrHv {
    Family: u16,
    Reserved: u16,
    VmId: [u8; 16],
    ServiceId: [u8; 16],
}

/// Attempts to open a direct `AF_HYPERV` (`HV_PROTOCOL_RAW`) socket to the WSL2 utility VM
/// when `NANODOCK_WSL_VM_ID` is configured. Returns `None` if unavailable or denied, allowing
/// `TransportManager` to fall back seamlessly to `YamuxStdioTransport`.
#[cfg(windows)]
pub fn try_connect_hyperv_vsock(port: u32) -> Option<std::net::TcpStream> {
    use std::os::windows::io::FromRawSocket;

    let vm_id_str = std::env::var("NANODOCK_WSL_VM_ID").ok()?;
    let vm_id = parse_guid(&vm_id_str)?;
    let service_id = build_vsock_service_guid(port);

    const AF_HYPERV: i32 = 34;
    const SOCK_STREAM: i32 = 1;
    const HV_PROTOCOL_RAW: i32 = 1;
    const INVALID_SOCKET: usize = usize::MAX;

    #[link(name = "ws2_32")]
    extern "system" {
        fn socket(af: i32, sock_type: i32, protocol: i32) -> usize;
        fn connect(s: usize, name: *const SockAddrHv, namelen: i32) -> i32;
        fn closesocket(s: usize) -> i32;
    }

    let sock = unsafe { socket(AF_HYPERV, SOCK_STREAM, HV_PROTOCOL_RAW) };
    if sock == INVALID_SOCKET {
        return None;
    }

    let addr = SockAddrHv {
        Family: AF_HYPERV as u16,
        Reserved: 0,
        VmId: vm_id,
        ServiceId: service_id,
    };

    let rc = unsafe {
        connect(
            sock,
            &addr as *const SockAddrHv,
            std::mem::size_of::<SockAddrHv>() as i32,
        )
    };
    if rc != 0 {
        unsafe { closesocket(sock) };
        return None;
    }

    // Safety: `sock` is a connected stream socket owned exclusively by us.
    let stream = unsafe { std::net::TcpStream::from_raw_socket(sock as u64) };
    let _ = stream.set_nonblocking(true);
    Some(stream)
}

/// Connects a multiplexed `YamuxClient` over `AF_HYPERV` (`HV_PROTOCOL_RAW`) when available.
/// Returns `None` if `NANODOCK_WSL_VM_ID` is unset or the Hyper-V socket cannot connect.
pub async fn try_connect_hyperv_transport(port: u32) -> Option<Arc<dyn StreamTransport>> {
    #[cfg(windows)]
    {
        let std_stream = tokio::task::spawn_blocking(move || try_connect_hyperv_vsock(port))
            .await
            .ok()
            .flatten()?;
        let tokio_stream = match tokio::net::TcpStream::from_std(std_stream) {
            Ok(s) => s,
            Err(err) => {
                debug!(error = %err, "Failed to adopt AF_HYPERV socket into tokio");
                return None;
            }
        };
        info!(port, "Connected direct AF_HYPERV yamux transport");
        Some(Arc::new(YamuxClient::spawn(tokio_stream, || {})))
    }
    #[cfg(not(windows))]
    {
        let _ = port;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_expected_vsock_guid_for_port() {
        let g = build_vsock_service_guid(DEFAULT_VSOCK_PORT);
        assert_eq!(&g[0..4], &DEFAULT_VSOCK_PORT.to_le_bytes());
        assert_eq!(&g[4..8], &[0xcb, 0xfa, 0xe6, 0x11]);
    }

    #[test]
    fn parses_mixed_endian_windows_guid() {
        let parsed = parse_guid("{12345678-9abc-def0-1122-334455667788}").unwrap();
        assert_eq!(&parsed[0..4], &[0x78, 0x56, 0x34, 0x12]);
        assert_eq!(&parsed[4..6], &[0xbc, 0x9a]);
        assert_eq!(&parsed[6..8], &[0xf0, 0xde]);
        assert_eq!(&parsed[8..16], &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]);
    }
}
