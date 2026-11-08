//! Atrapy MCP: testy kontraktowe hosta mostu na `FakeBridgeMcpHost` i TTL na zegarze ręcznym.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mcp_contract::contract_tests::{ProxySide, RecordingRouter, run_all};
use mcp_contract::{BridgeMcpHost, BridgeScope, LocalEndpoint};
use mcp_fake::FakeBridgeMcpHost;
use tokio::io::{AsyncRead, AsyncWrite};

trait Stream: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> Stream for T {}

async fn connect(endpoint: LocalEndpoint) -> std::io::Result<Box<dyn Stream>> {
    match endpoint {
        #[cfg(unix)]
        LocalEndpoint::UnixSocket(p) => Ok(Box::new(tokio::net::UnixStream::connect(p).await?)),
        #[cfg(windows)]
        LocalEndpoint::NamedPipe(n) => Ok(Box::new(
            tokio::net::windows::named_pipe::ClientOptions::new().open(n)?,
        )),
        #[allow(unreachable_patterns)]
        other => Err(std::io::Error::other(format!("{other:?}"))),
    }
}

#[tokio::test]
async fn fake_host_passes_contract() {
    let host = FakeBridgeMcpHost::start(60_000).unwrap();
    run_all(&host, connect).await;
    assert!(host.rejected() >= 2);
    assert_eq!(host.approvals().len(), 1);
}

#[tokio::test]
async fn expired_token_is_rejected() {
    let host = FakeBridgeMcpHost::start(1_000).unwrap();
    let reg = host
        .register(
            BridgeScope::windows_v0("t"),
            Some(RecordingRouter::allowing()),
        )
        .await
        .unwrap();
    host.advance_ms(1_000);
    let stream = connect(host.endpoint().clone()).await.unwrap();
    let mut side = ProxySide::hello(stream, reg.launch.token().unwrap()).await;
    assert!(side.initialize().await.is_none());
    assert_eq!(host.rejected(), 1);
}
