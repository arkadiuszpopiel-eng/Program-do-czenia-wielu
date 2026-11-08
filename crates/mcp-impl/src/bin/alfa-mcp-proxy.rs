//! `alfa-mcp-proxy` — serwer MCP stdio dla mostów CLI, który jest tylko pompą bajtów do kanału
//! lokalnego Alfy (named pipe / gniazdo Unix; **nigdy TCP**). Pierwsza linia do Alfy to powitanie
//! z tokenem sesyjnym z `ALFA_MCP_TOKEN`; adres kanału z `ALFA_MCP_ENDPOINT`.
//! Proxy nie interpretuje MCP i nigdy nie wypisuje tokenu.

use std::process::ExitCode;

use mcp_contract::bridge::{ENV_ENDPOINT, ENV_TOKEN};
use mcp_contract::{LocalEndpoint, ProxyHello};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

async fn pump<R, W>(mut from: R, mut to: W) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = from.read(&mut buf).await?;
        if n == 0 {
            let _ = to.shutdown().await;
            return Ok(());
        }
        to.write_all(&buf[..n]).await?;
        to.flush().await?;
    }
}

async fn run() -> Result<(), String> {
    let endpoint = std::env::var(ENV_ENDPOINT)
        .ok()
        .and_then(|v| LocalEndpoint::parse(&v))
        .ok_or_else(|| format!("brak lub niepoprawna zmienna {ENV_ENDPOINT}"))?;
    let token = std::env::var(ENV_TOKEN).map_err(|_| format!("brak zmiennej {ENV_TOKEN}"))?;
    let stream = mcp_impl::listener::connect(&endpoint)
        .await
        .map_err(|e| format!("nie udało się połączyć z Alfą: {e}"))?;
    let (reader, mut writer) = tokio::io::split(stream);
    let hello = format!("{}\n", ProxyHello::new(&token).to_line());
    drop(token);
    writer
        .write_all(hello.as_bytes())
        .await
        .map_err(|e| format!("nie udało się wysłać powitania: {e}"))?;
    let upstream = pump(tokio::io::stdin(), writer);
    let downstream = pump(reader, tokio::io::stdout());
    // Koniec dowolnego kierunku (CLI zamknęło stdin albo Alfa zamknęła kanał) kończy proxy.
    tokio::select! {
        r = upstream => r.map_err(|e| format!("błąd przesyłu do Alfy: {e}")),
        r = downstream => r.map_err(|e| format!("błąd przesyłu do CLI: {e}")),
    }
}

fn main() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("alfa-mcp-proxy: {e}");
            return ExitCode::FAILURE;
        }
    };
    let result = runtime.block_on(run());
    // Wątek czytający stdin może wisieć w blokującym `read` — nie czekamy na niego.
    runtime.shutdown_background();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("alfa-mcp-proxy: {message}");
            ExitCode::FAILURE
        }
    }
}
