//! „System” procesów dla testów `KernelProcesses`: uruchomienie `alfa-broker` = usługa Brokera
//! w wątku na koncie użytkownika (tryb przenośny), `alfa-watchdog` = proces z komunikatami na
//! stdout pisanymi przez test; awaria Brokera (zerwane połączenia, zwolniony potok).

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use app_broker::children::{ChildProc, ChildSpec, Spawner};
use app_broker::mode::{PORTABLE_PIPE, image_name};
use platform_contract::{IntegrityLevel, SecurePipePort, Sid};
use platform_fake::FakePipes;
use safety_broker_contract::contract_tests::test_policy;
use watchdog_contract::ManualClock;

use super::breaker::Breaker;

pub struct FakeChild {
    pub pid: u32,
    pub alive: Arc<AtomicBool>,
    pub stdout: Option<std::io::PipeReader>,
    pub killed: Arc<Mutex<Vec<u32>>>,
}

impl ChildProc for FakeChild {
    fn pid(&self) -> u32 {
        self.pid
    }
    fn running(&mut self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }
    fn take_stdout(&mut self) -> Option<Box<dyn std::io::Read + Send>> {
        self.stdout
            .take()
            .map(|r| Box::new(r) as Box<dyn std::io::Read + Send>)
    }
    fn take_stderr(&mut self) -> Option<Box<dyn std::io::Read + Send>> {
        None
    }
    fn kill(&mut self) {
        self.alive.store(false, Ordering::SeqCst);
        self.killed.lock().unwrap().push(self.pid);
    }
}

pub struct BrokerProc {
    pub pid: u32,
    pub alive: Arc<AtomicBool>,
    pub breaker: Breaker,
    pub service: super::Service,
}

/// „System”: uruchomienie `alfa-broker` = usługa w wątku na koncie użytkownika (tryb
/// przenośny), `alfa-watchdog` = proces z komunikatami na stdout pisanymi przez test.
pub struct FakeSystem {
    pub sys: FakePipes,
    pub clock: Arc<ManualClock>,
    pub next: AtomicU32,
    pub spawned: Mutex<Vec<ChildSpec>>,
    pub brokers: Mutex<Vec<BrokerProc>>,
    pub watchdog: Mutex<Option<std::io::PipeWriter>>,
    pub killed: Arc<Mutex<Vec<u32>>>,
}

impl FakeSystem {
    pub fn new(sys: FakePipes) -> Arc<Self> {
        Arc::new(Self {
            sys,
            clock: Arc::new(ManualClock::new(1_000_000)),
            next: AtomicU32::new(100),
            spawned: Mutex::new(Vec::new()),
            brokers: Mutex::new(Vec::new()),
            watchdog: Mutex::new(None),
            killed: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn specs(&self) -> Vec<ChildSpec> {
        self.spawned.lock().unwrap().clone()
    }

    /// Awaria procesu Brokera: połączenia zerwane, potok zwolniony, proces zakończony.
    pub fn crash_broker(&self) {
        let mut brokers = self.brokers.lock().unwrap();
        let Some(b) = brokers.last_mut() else { return };
        b.breaker.break_all();
        b.alive.store(false, Ordering::SeqCst);
        b.service.stop.stop();
        // Odblokowuje oczekujący `accept`, żeby nasłuch zwolnił nazwę potoku.
        let _ = self
            .sys
            .process(super::STRANGER_PID)
            .connect(PORTABLE_PIPE, 10);
        let _ = b.pid;
    }

    pub fn watchdog_says(&self, line: &str) {
        let mut w = self.watchdog.lock().unwrap();
        let w = w.as_mut().unwrap();
        writeln!(w, "{line}").unwrap();
        w.flush().unwrap();
    }

    pub fn watchdog_exits(&self) {
        self.watchdog.lock().unwrap().take();
    }
}

impl Spawner for FakeSystem {
    fn spawn(&self, spec: &ChildSpec) -> Result<Box<dyn ChildProc>, String> {
        self.spawned.lock().unwrap().push(spec.clone());
        let pid = self.next.fetch_add(1, Ordering::SeqCst);
        let alive = Arc::new(AtomicBool::new(true));
        let name = spec
            .image
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        if name == image_name("alfa-broker") {
            self.sys.register(super::ident(
                pid,
                super::USER,
                "alfa-broker.exe",
                IntegrityLevel::Medium,
            ));
            let breaker = Breaker::new(Arc::new(self.sys.process(pid)));
            let service = super::service_with(
                Arc::new(breaker.clone()),
                Arc::new(self.sys.process(pid)),
                test_policy(),
                self.clock.clone(),
                None,
                |c| {
                    c.pipe_name = PORTABLE_PIPE.into();
                    c.broker_user = Sid::parse(super::USER).unwrap();
                    c.dev_mode = true;
                },
            );
            self.brokers.lock().unwrap().push(BrokerProc {
                pid,
                alive: alive.clone(),
                breaker,
                service,
            });
            return Ok(Box::new(FakeChild {
                pid,
                alive,
                stdout: None,
                killed: self.killed.clone(),
            }));
        }
        let (reader, mut writer) = std::io::pipe().unwrap();
        writeln!(writer, "[alfa-watchdog] kill-switch Ctrl+Shift+F12 aktywny").unwrap();
        writeln!(writer, r#"{{"event":"ready","hotkey":"Ctrl+Shift+F12"}}"#).unwrap();
        *self.watchdog.lock().unwrap() = Some(writer);
        Ok(Box::new(FakeChild {
            pid,
            alive,
            stdout: Some(reader),
            killed: self.killed.clone(),
        }))
    }
}
