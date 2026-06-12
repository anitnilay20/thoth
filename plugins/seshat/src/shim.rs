//! A `Read + Write` stream backed by the host `tcp-client` import, so sync
//! wire-protocol codecs (Postgres) can run over the host-owned socket.

use std::io::{self, Read, Write};

use crate::bindings::thoth::plugin::tcp_client;

/// Owns a host tcp-client stream id and exposes it as a blocking `Read + Write`.
/// TLS is terminated host-side, so this is always a plaintext byte stream.
pub struct TcpShim {
    id: u64,
}

impl TcpShim {
    pub fn connect(host: &str, port: u16, tls: bool) -> io::Result<Self> {
        let id = tcp_client::connect(host, port, tls)
            .map_err(|e| io::Error::other(format!("tcp connect: {}", e.message)))?;
        Ok(Self { id })
    }

    /// Upgrade this (plaintext) stream to TLS in place, after the wire protocol's
    /// own SSL-request negotiation. SNI = `host`.
    pub fn start_tls(&mut self, host: &str) -> io::Result<()> {
        tcp_client::start_tls(self.id, host)
            .map_err(|e| io::Error::other(format!("start_tls: {}", e.message)))
    }
}

impl Read for TcpShim {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let max = buf.len().min(u32::MAX as usize) as u32;
        if max == 0 {
            return Ok(0);
        }
        let chunk = tcp_client::read(self.id, max).map_err(|e| io::Error::other(e.message))?;
        let n = chunk.len();
        buf[..n].copy_from_slice(&chunk);
        Ok(n) // n == 0 signals EOF
    }
}

impl Write for TcpShim {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = tcp_client::write(self.id, buf).map_err(|e| io::Error::other(e.message))?;
        Ok(n as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for TcpShim {
    fn drop(&mut self) {
        tcp_client::close(self.id);
    }
}
