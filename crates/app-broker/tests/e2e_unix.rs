//! Koniec-koniec przez prawdziwy transport systemu (Linux/CI): usługa Brokera, Broker-UI
//! i aplikacja rozmawiają przez gniazda Unix w katalogu 0700 z gniazdem 0600 (odpowiednik
//! named pipe z ACL — jak kanał MCP w `mcp-impl`). PID drugiej strony przekazuje preambuła
//! połączenia (na Windows: `GetNamedPipeClientProcessId` / `GetNamedPipeServerProcessId`), a
//! tożsamość procesów — rejestr testu. Bez TCP.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use app_api::dto::BrokerMode;
use app_broker::KernelStatus;
use app_broker::link::{BrokerLink, LinkConfig, LinkError, ServerCheck};
use broker_ui_fake::Script;
use platform_contract::{
    PeerIdentity, PipeConnection, PipeListener, PipeSecurity, PlatformError, ProcessIdentityPort,
    SecurePipePort, Sid,
};
use safety_broker_contract::contract_tests::{delta, host, request, test_policy};
use safety_broker_contract::{ApprovalStatus, Broker, Capability, CommandOrigin, Decision};
use watchdog_contract::ManualClock;

/// Gniazda Unix jako potoki z ACL (prawa plików zamiast DACL).
#[derive(Clone)]
struct UnixPipes {
    dir: PathBuf,
    pid: u32,
    who: Arc<Mutex<BTreeMap<u32, PeerIdentity>>>,
}

struct Conn {
    stream: UnixStream,
    peer: u32,
}

impl Read for Conn {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stream.read(buf)
    }
}

impl Write for Conn {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.stream.write(data)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

impl PipeConnection for Conn {
    fn peer_pid(&self) -> u32 {
        self.peer
    }
}

fn io(e: std::io::Error) -> PlatformError {
    PlatformError::Io(e.to_string())
}

/// Wymiana PID-ów (preambuła 4 B w każdą stronę).
fn exchange(stream: &mut UnixStream, me: u32) -> Result<u32, PlatformError> {
    stream.write_all(&me.to_le_bytes()).map_err(io)?;
    let mut peer = [0u8; 4];
    stream.read_exact(&mut peer).map_err(io)?;
    Ok(u32::from_le_bytes(peer))
}

struct Listener {
    inner: UnixListener,
    pid: u32,
}

impl PipeListener for Listener {
    fn accept(&mut self) -> Result<Box<dyn PipeConnection>, PlatformError> {
        let (mut stream, _) = self.inner.accept().map_err(io)?;
        let peer = exchange(&mut stream, self.pid)?;
        Ok(Box::new(Conn { stream, peer }))
    }
}

impl UnixPipes {
    fn process(&self, pid: u32) -> Self {
        Self {
            pid,
            ..self.clone()
        }
    }

    fn socket(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.sock"))
    }
}

impl SecurePipePort for UnixPipes {
    fn listen(&self, security: &PipeSecurity) -> Result<Box<dyn PipeListener>, PlatformError> {
        let path = self.socket(security.name());
        if path.exists() {
            return Err(PlatformError::PermissionDenied(
                "pierwsza instancja zajęta".into(),
            ));
        }
        let inner = UnixListener::bind(&path).map_err(io)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(io)?;
        Ok(Box::new(Listener {
            inner,
            pid: self.pid,
        }))
    }

    fn connect(
        &self,
        name: &str,
        _timeout_ms: u32,
    ) -> Result<Box<dyn PipeConnection>, PlatformError> {
        let mut stream = UnixStream::connect(self.socket(name)).map_err(io)?;
        let peer = exchange(&mut stream, self.pid)?;
        Ok(Box::new(Conn { stream, peer }))
    }
}

impl ProcessIdentityPort for UnixPipes {
    fn identify(&self, pid: u32) -> Result<PeerIdentity, PlatformError> {
        self.who
            .lock()
            .unwrap()
            .get(&pid)
            .cloned()
            .ok_or_else(|| PlatformError::UnknownResource(format!("proces {pid}")))
    }

    fn current_user(&self) -> Result<Sid, PlatformError> {
        self.identify(self.pid).map(|i| i.user)
    }
}

fn system() -> (UnixPipes, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let sockets = dir.path().join("pipes");
    std::fs::create_dir(&sockets).unwrap();
    std::fs::set_permissions(&sockets, std::fs::Permissions::from_mode(0o700)).unwrap();
    let who = common::identities()
        .into_iter()
        .map(|i| (i.pid, i))
        .collect::<BTreeMap<_, _>>();
    let pipes = UnixPipes {
        dir: sockets,
        pid: common::SERVER_PID,
        who: Arc::new(Mutex::new(who)),
    };
    (pipes, dir)
}

fn link(pipes: &UnixPipes, pid: u32) -> Arc<BrokerLink> {
    let p = Arc::new(pipes.process(pid));
    let status = Arc::new(KernelStatus::new(BrokerMode::Service, true));
    BrokerLink::new(p.clone(), p, LinkConfig::new(common::PIPE), status)
}

fn service_check() -> ServerCheck {
    ServerCheck::Service {
        user: Some(Sid::parse(common::BROKER).unwrap()),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approval_round_trip_over_unix_sockets_0600() {
    let (pipes, _dir) = system();
    let clock = Arc::new(ManualClock::new(1_000_000));
    let ui = common::UiThreadLauncher::new(
        Arc::new(pipes.process(common::UI_PID)),
        Arc::new(pipes.process(common::UI_PID)),
        clock.clone(),
        Script::Allow,
    );
    let breaker = common::breaker::Breaker::new(Arc::new(pipes.clone()));
    let service = common::service(
        Arc::new(breaker.clone()),
        Arc::new(pipes.clone()),
        test_policy(),
        clock,
        Some(ui.clone()),
    );
    let socket = pipes.socket(common::PIPE);
    common::wait_until("gniazdo usługi", || socket.exists());
    let mode = std::fs::metadata(&socket).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "gniazdo tylko dla właściciela");
    let dir_mode = std::fs::metadata(&pipes.dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(dir_mode, 0o700);
    common::wait_until("Broker-UI połączone", || ui.alive());

    let app = link(&pipes, common::CORE_PID);
    app.connect(&service_check()).unwrap();
    let broker = app_broker::RemoteBroker::new(app.clone());
    let egress = request(
        &delta(),
        Capability::NetEgress(host("x.example.org")),
        CommandOrigin::UserText,
    );
    let Decision::NeedsApproval(ticket) = broker.decide(egress.clone()).await.unwrap() else {
        panic!("oczekiwano prośby");
    };
    common::wait_until("zatwierdzenie w Broker-UI", || {
        matches!(
            broker.approval_status(ticket.id, &delta()),
            Ok(ApprovalStatus::Approved { .. })
        )
    });
    ui.set_script(Script::Deny);
    let Decision::NeedsApproval(second) = broker.decide(egress).await.unwrap() else {
        panic!("oczekiwano prośby");
    };
    common::wait_until("odmowa w Broker-UI", || {
        broker.approval_status(second.id, &delta()) == Ok(ApprovalStatus::Denied)
    });

    // Obcy proces tego samego konta nie przedstawi się jako jądro (obraz spoza wiązania roli).
    let stranger = link(&pipes, common::STRANGER_PID);
    let err = stranger.connect(&service_check()).unwrap_err();
    assert!(matches!(err, LinkError::Rejected(_)), "{err:?}");
    // Aplikacja nie łączy się z serwerem, który nie jest usługą (np. tryb przenośny innego PID-u).
    let wrong = link(&pipes, common::CORE_PID);
    let err = wrong.connect(&ServerCheck::Pid(4242)).unwrap_err();
    assert!(matches!(err, LinkError::Rejected(_)), "{err:?}");

    // Śmierć usługi: łącze aplikacji przechodzi w bezpieczny stan (odmowa, nie czekanie).
    ui.stop();
    breaker.break_all();
    common::wait_until("zerwanie wykryte", || {
        let _ = broker.metrics();
        !app.status().connected()
    });
    let egress = request(
        &delta(),
        Capability::NetEgress(host("x.example.org")),
        CommandOrigin::UserText,
    );
    assert!(matches!(
        broker.decide(egress).await,
        Err(safety_broker_contract::BrokerError::AuditUnavailable(_))
    ));
    drop(service);
}
