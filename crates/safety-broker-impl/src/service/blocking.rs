//! Adapter blokującego `Read + Write` (połączenie named pipe z `platform-contract`) do
//! `AsyncRead + AsyncWrite` serwera IPC. Używany wyłącznie na dedykowanym wątku połączenia
//! (`Handle::block_on`), gdzie blokowanie nie zatrzymuje innych zadań; protokół jest
//! żądanie → odpowiedź, więc odczyt i zapis nigdy nie biegną równolegle.

use std::io::{Read, Write};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// Strumień blokujący widziany jako asynchroniczny.
#[derive(Debug)]
pub struct BlockingIo<T>(pub T);

impl<T: Read + Unpin> AsyncRead for BlockingIo<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        Poll::Ready(
            this.0
                .read(buf.initialize_unfilled())
                .map(|n| buf.advance(n)),
        )
    }
}

impl<T: Write + Unpin> AsyncWrite for BlockingIo<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Poll::Ready(self.get_mut().0.write(data))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(self.get_mut().0.flush())
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
