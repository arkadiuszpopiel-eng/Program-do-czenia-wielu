//! Kanał lokalny hosta: na Windows named pipe (tylko klienci lokalni, pierwsza instancja —
//! ochrona przed przejęciem nazwy), poza Windows gniazdo Unix w katalogu 0700 z prawami 0600.
//! **Żadnego nasłuchu sieciowego** (test `no_network_listeners_in_sources`).
//!
//! ACL named pipe: domyślny deskryptor bezpieczeństwa Windows daje zapis tylko właścicielowi
//! (bieżący użytkownik), SYSTEM i Administratorom; pozostali mają co najwyżej odczyt, a protokół
//! wymaga zapisu tokenu jako pierwszej linii. Jawny DACL na SID wymaga windows-rs, który wolno
//! używać tylko w `platform-windows-impl` — do dodania tam jako port (zob. SPEC, otwarte pytania).

use std::path::Path;

use mcp_contract::LocalEndpoint;
use tokio::io::{AsyncRead, AsyncWrite};

/// Strumień dwukierunkowy kanału lokalnego.
pub trait LocalStream: AsyncRead + AsyncWrite + Send + Unpin {}

impl<T: AsyncRead + AsyncWrite + Send + Unpin> LocalStream for T {}

/// Pudełkowany strumień.
pub type BoxedStream = Box<dyn LocalStream>;

/// Nasłuch kanału lokalnego.
pub struct LocalListener {
    endpoint: LocalEndpoint,
    #[cfg(unix)]
    inner: tokio::net::UnixListener,
    #[cfg(windows)]
    next: tokio::net::windows::named_pipe::NamedPipeServer,
}

impl LocalListener {
    /// Tworzy kanał o unikalnej nazwie. `dir` — katalog bazowy gniazda (poza Windows).
    pub fn bind(dir: &Path, unique: &str) -> std::io::Result<Self> {
        bind_impl(dir, unique)
    }

    /// Adres kanału.
    pub fn endpoint(&self) -> &LocalEndpoint {
        &self.endpoint
    }

    /// Czeka na następnego klienta.
    pub async fn accept(&mut self) -> std::io::Result<BoxedStream> {
        accept_impl(self).await
    }
}

#[cfg(unix)]
fn bind_impl(dir: &Path, unique: &str) -> std::io::Result<LocalListener> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let private = dir.join(format!("alfa-mcp-{unique}"));
    std::fs::DirBuilder::new().mode(0o700).create(&private)?;
    std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700))?;
    let socket = private.join("mcp.sock");
    let inner = tokio::net::UnixListener::bind(&socket)?;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
    Ok(LocalListener {
        endpoint: LocalEndpoint::UnixSocket(socket),
        inner,
    })
}

#[cfg(unix)]
async fn accept_impl(listener: &mut LocalListener) -> std::io::Result<BoxedStream> {
    let (stream, _) = listener.inner.accept().await?;
    Ok(Box::new(stream))
}

#[cfg(windows)]
fn bind_impl(_dir: &Path, unique: &str) -> std::io::Result<LocalListener> {
    use tokio::net::windows::named_pipe::ServerOptions;
    let name = format!(r"\\.\pipe\alfa-mcp-{unique}");
    let next = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(&name)?;
    Ok(LocalListener {
        endpoint: LocalEndpoint::NamedPipe(name),
        next,
    })
}

#[cfg(windows)]
async fn accept_impl(listener: &mut LocalListener) -> std::io::Result<BoxedStream> {
    use tokio::net::windows::named_pipe::ServerOptions;
    listener.next.connect().await?;
    let LocalEndpoint::NamedPipe(name) = &listener.endpoint else {
        return Err(std::io::Error::other("zły rodzaj kanału"));
    };
    let fresh = ServerOptions::new()
        .reject_remote_clients(true)
        .create(name)?;
    let connected = std::mem::replace(&mut listener.next, fresh);
    Ok(Box::new(connected))
}

impl Drop for LocalListener {
    fn drop(&mut self) {
        if let LocalEndpoint::UnixSocket(path) = &self.endpoint {
            let _ = std::fs::remove_file(path);
            if let Some(parent) = path.parent() {
                let _ = std::fs::remove_dir(parent);
            }
        }
    }
}

/// Łączy się z kanałem (proxy, testy).
pub async fn connect(endpoint: &LocalEndpoint) -> std::io::Result<BoxedStream> {
    match endpoint {
        #[cfg(unix)]
        LocalEndpoint::UnixSocket(path) => {
            Ok(Box::new(tokio::net::UnixStream::connect(path).await?))
        }
        #[cfg(windows)]
        LocalEndpoint::NamedPipe(name) => connect_pipe(name).await,
        #[allow(unreachable_patterns)]
        other => Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            format!("kanał {other:?} nieobsługiwany na tej platformie"),
        )),
    }
}

#[cfg(windows)]
async fn connect_pipe(name: &str) -> std::io::Result<BoxedStream> {
    use tokio::net::windows::named_pipe::ClientOptions;
    /// `ERROR_PIPE_BUSY` — wszystkie instancje zajęte; ponawiamy.
    const ERROR_PIPE_BUSY: i32 = 231;
    let mut attempts = 0u32;
    loop {
        match ClientOptions::new().open(name) {
            Ok(client) => return Ok(Box::new(client)),
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && attempts < 50 => {
                attempts += 1;
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn unix_socket_is_private_and_works() {
        let base = std::env::temp_dir();
        let unique = format!("t{}", std::process::id());
        let mut listener = LocalListener::bind(&base, &unique).unwrap();
        let LocalEndpoint::UnixSocket(path) = listener.endpoint().clone() else {
            panic!("zły kanał");
        };
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let dir_mode = std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);
        let ep = listener.endpoint().clone();
        let client = tokio::spawn(async move {
            let mut c = connect(&ep).await.unwrap();
            c.write_all(b"hej").await.unwrap();
        });
        let mut server = listener.accept().await.unwrap();
        let mut buf = [0u8; 3];
        server.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hej");
        client.await.unwrap();
        drop(listener);
        assert!(!path.exists());
        assert!(
            connect(&LocalEndpoint::NamedPipe(r"\\.\pipe\x".into()))
                .await
                .is_err()
        );
    }
}
