//! Cykl życia sidecara `llama-server`: start na żądanie (jeden naraz), zdrowie `/health`,
//! restart po awarii z limitem, fallback GPU → CPU, dzierżawa `model-residency`, zwolnienie
//! po bezczynności. Każde uruchomienie: `127.0.0.1`, losowy port, losowy klucz API.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use device_profile_contract::DeviceProfile;
use lib_openai_compat::{AuthScheme, Engine, HttpConfig, ProviderProfile, RetryPolicy};
use model_residency_contract::{Lease, LeaseId, LeaseListener, Placement, Residency, Revocation};
use providers_contract::StaticKey;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::Instant;

use crate::codec::LlamaCodec;
use crate::config::{LaunchPlan, LocalConfig};
use crate::error::{LocalError, LocalEvent};
use crate::manifest::ModelEntry;
use crate::process::{LaunchSpec, SidecarLauncher, SidecarProcess};

/// Właściciel dzierżaw w `model-residency`.
pub const RESIDENCY_OWNER: &str = "providers-local";

struct Running {
    model: String,
    process: Box<dyn SidecarProcess>,
    engine: Arc<Engine<LlamaCodec>>,
    lease: Option<LeaseId>,
    plan: LaunchPlan,
    port: u16,
}

/// Menedżer sidecara.
pub struct Sidecar {
    pub(crate) config: LocalConfig,
    launcher: Arc<dyn SidecarLauncher>,
    pub(crate) device: Option<Arc<dyn DeviceProfile>>,
    pub(crate) residency: Option<Arc<dyn Residency>>,
    state: tokio::sync::Mutex<Option<Running>>,
    /// Dzierżawa działającego sidecara (odczyt synchroniczny przy końcu żądania).
    pub(crate) lease: Mutex<Option<LeaseId>>,
    in_flight: AtomicUsize,
    revoked: AtomicBool,
    suspect: AtomicBool,
    last_used: Mutex<Instant>,
    restarts: Mutex<VecDeque<Instant>>,
    events: Mutex<Option<UnboundedSender<LocalEvent>>>,
    health: reqwest::Client,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

pub(crate) fn ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// Wolny port na `127.0.0.1` (system przydziela; serwer wiąże go chwilę później).
fn free_port() -> Result<u16, LocalError> {
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(l.local_addr()?.port())
}

/// Losowy klucz API (192 bity) — per uruchomienie, tylko w pamięci.
fn random_key() -> Result<String, LocalError> {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).map_err(|e| LocalError::Spawn(format!("losowanie klucza: {e}")))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

struct Listener(Weak<Sidecar>);

impl LeaseListener for Listener {
    fn revoked(&self, revocation: &Revocation) {
        self.release_now(&format!("dzierżawa odebrana: {:?}", revocation.reason));
    }

    fn moved(&self, _lease: &Lease) {
        self.release_now("tryb gry: przeładowanie na CPU przy następnym żądaniu");
    }
}

impl Listener {
    fn release_now(&self, reason: &str) {
        let Some(sidecar) = self.0.upgrade() else {
            return;
        };
        sidecar.revoked.store(true, Ordering::SeqCst);
        if let Ok(rt) = tokio::runtime::Handle::try_current() {
            let reason = reason.to_owned();
            rt.spawn(async move { sidecar.stop_if_idle(&reason).await });
        }
    }
}

impl Sidecar {
    /// Menedżer (sidecar nie jest uruchamiany do pierwszego żądania).
    pub fn new(
        config: LocalConfig,
        launcher: Arc<dyn SidecarLauncher>,
        device: Option<Arc<dyn DeviceProfile>>,
        residency: Option<Arc<dyn Residency>>,
    ) -> Result<Arc<Self>, LocalError> {
        let health = reqwest::Client::builder()
            .connect_timeout(Duration::from_millis(500))
            .build()
            .map_err(|e| LocalError::Spawn(e.to_string()))?;
        let sidecar = Arc::new(Self {
            config,
            launcher,
            device,
            residency,
            state: tokio::sync::Mutex::new(None),
            lease: Mutex::new(None),
            in_flight: AtomicUsize::new(0),
            revoked: AtomicBool::new(false),
            suspect: AtomicBool::new(false),
            last_used: Mutex::new(Instant::now()),
            restarts: Mutex::new(VecDeque::new()),
            events: Mutex::new(None),
            health,
        });
        if let Some(r) = &sidecar.residency {
            r.listen(
                RESIDENCY_OWNER,
                Arc::new(Listener(Arc::downgrade(&sidecar))),
            );
        }
        Ok(sidecar)
    }

    /// Konfiguracja.
    pub fn config(&self) -> &LocalConfig {
        &self.config
    }

    /// Kolejka zdarzeń (moduł przekazuje je na magistralę).
    pub fn set_event_sink(&self, sink: Option<UnboundedSender<LocalEvent>>) {
        *lock(&self.events) = sink;
    }

    pub(crate) fn emit(&self, event: LocalEvent) {
        if let Some(tx) = lock(&self.events).as_ref() {
            let _ = tx.send(event);
        }
    }

    async fn healthy(&self, port: u16) -> bool {
        self.health
            .get(format!("http://127.0.0.1:{port}/health"))
            .timeout(Duration::from_millis(500))
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
    }

    /// Błąd transportu w strumieniu: następne żądanie sprawdzi zdrowie serwera przed użyciem.
    pub(crate) fn mark_suspect(&self) {
        self.suspect.store(true, Ordering::SeqCst);
    }

    /// Początek żądania (licznik w toku, znacznik użycia).
    pub(crate) fn begin(&self) {
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        *lock(&self.last_used) = Instant::now();
    }

    /// Koniec żądania (ostatnie w toku zwalnia oznaczenie użycia dzierżawy).
    pub(crate) fn end(&self) {
        if self.in_flight.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.mark_in_use(false);
        }
        *lock(&self.last_used) = Instant::now();
    }

    /// Plan bieżącego uruchomienia (`None` = zatrzymany).
    pub async fn running_plan(&self) -> Option<(String, LaunchPlan, Option<u32>)> {
        let st = self.state.lock().await;
        st.as_ref()
            .map(|r| (r.model.clone(), r.plan.clone(), r.process.pid()))
    }

    /// Zwraca silnik działającego sidecara dla modelu; uruchamia/restartuje w razie potrzeby.
    pub async fn ensure_running(
        &self,
        entry: &ModelEntry,
        model_path: &std::path::Path,
    ) -> Result<Arc<Engine<LlamaCodec>>, LocalError> {
        let mut st = self.state.lock().await;
        let revoked = self.revoked.swap(false, Ordering::SeqCst);
        let suspect = self.suspect.swap(false, Ordering::SeqCst);
        if let Some(r) = st.as_mut() {
            // Po błędzie transportu proces mógł paść, zanim system go odnotował: sprawdzamy `/health`.
            let mut exited = r.process.exited();
            if exited.is_none() && suspect && !self.healthy(r.port).await {
                r.process.kill();
                exited = Some(r.process.exited().flatten());
            }
            match exited {
                None if r.model == entry.id && !revoked => {
                    self.mark_in_use(true);
                    return Ok(Arc::clone(&r.engine));
                }
                None => {}
                Some(code) => {
                    tracing::warn!(model = %r.model, ?code, "awaria llama-server");
                    self.emit(LocalEvent::SidecarCrashed {
                        model: r.model.clone(),
                        exit_code: code,
                    });
                    lock(&self.restarts).push_back(Instant::now());
                }
            }
        }
        if let Some(old) = st.take() {
            self.shutdown(old, "zmiana modelu lub restart");
        }
        self.check_stability()?;
        let plan = self.plan(entry)?;
        let running = match self.start(entry, model_path, plan.0.clone(), plan.1).await {
            Ok(r) => r,
            Err(e) if plan.0.gpu_layers > 0 => {
                let cpu = self.cpu_plan(&plan.0);
                tracing::warn!(error = %e, "start na GPU nieudany — fallback na CPU");
                self.emit(LocalEvent::BackendFallback {
                    from: plan.0.backend.as_str().into(),
                    to: cpu.backend.as_str().into(),
                    reason: e.to_string(),
                });
                self.release_lease(plan.1);
                let lease = self.acquire(entry, Placement::CpuOnly, cpu.ctx, 0)?;
                self.start(entry, model_path, cpu, lease.map(|l| l.id))
                    .await?
            }
            Err(e) => {
                self.release_lease(plan.1);
                return Err(e);
            }
        };
        let engine = Arc::clone(&running.engine);
        *lock(&self.lease) = running.lease;
        *st = Some(running);
        self.mark_in_use(true);
        Ok(engine)
    }

    /// Awarie w oknie: więcej niż `max_restarts` → bez automatycznego restartu do końca okna.
    fn check_stability(&self) -> Result<(), LocalError> {
        let now = Instant::now();
        let mut r = lock(&self.restarts);
        while r
            .front()
            .is_some_and(|t| now.duration_since(*t) > self.config.restart_window)
        {
            r.pop_front();
        }
        let crashes = u32::try_from(r.len()).unwrap_or(u32::MAX);
        if crashes > self.config.max_restarts {
            return Err(LocalError::Unstable(crashes));
        }
        Ok(())
    }

    async fn start(
        &self,
        entry: &ModelEntry,
        model_path: &std::path::Path,
        plan: LaunchPlan,
        lease: Option<LeaseId>,
    ) -> Result<Running, LocalError> {
        let port = free_port()?;
        let key = random_key()?;
        let spec = LaunchSpec {
            program: plan.program.clone(),
            args: plan.args(entry, model_path, port, &key),
            secret: key.clone(),
        };
        let t0 = Instant::now();
        let mut process = self.launcher.spawn(&spec)?;
        loop {
            if let Some(code) = process.exited() {
                return Err(LocalError::Startup(format!(
                    "proces zakończony (kod {code:?})"
                )));
            }
            if t0.elapsed() > self.config.startup_timeout {
                process.kill();
                return Err(LocalError::Startup(
                    "przekroczony czas ładowania modelu".into(),
                ));
            }
            if self.healthy(port).await {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let engine = Engine::new(
            self.profile(entry, plan.ctx),
            self.http(port),
            Arc::new(StaticKey::new(key)),
            LlamaCodec::new(entry, plan.ctx),
        )
        .map_err(|e| LocalError::Spawn(e.to_string()))?;
        let startup_ms = ms(t0.elapsed());
        tracing::info!(model = %entry.id, backend = plan.backend.as_str(), gpu_layers = plan.gpu_layers, startup_ms, "llama-server gotowy");
        self.emit(LocalEvent::Loaded {
            model: entry.id.clone(),
            gpu_layers: plan.gpu_layers,
            backend: plan.backend.as_str().into(),
            startup_ms,
        });
        Ok(Running {
            model: entry.id.clone(),
            process,
            engine: Arc::new(engine),
            lease,
            plan,
            port,
        })
    }

    /// Profil silnika dla kontekstu uruchomienia (`max_tokens` domyślnie połowa `-c`).
    fn profile(&self, entry: &ModelEntry, ctx: u32) -> ProviderProfile {
        let mut p = ProviderProfile::new(self.config.provider_id.as_str());
        p.privacy = self.config.privacy.clone();
        p.default_model = Some(entry.id.clone());
        p.models.insert(entry.id.clone(), entry.capabilities(ctx));
        p.default_max_tokens = ctx.min(entry.ctx) / 2;
        p
    }

    fn http(&self, port: u16) -> HttpConfig {
        let mut h = HttpConfig::new(format!("http://127.0.0.1:{port}/v1"), AuthScheme::Bearer);
        h.timeouts = self.config.timeouts;
        // Lokalny serwer: bez ponowień (błąd = awaria albo przeciążenie; decyduje Router).
        h.retry = RetryPolicy {
            max_retries: 0,
            ..RetryPolicy::default()
        };
        h
    }

    fn shutdown(&self, mut running: Running, reason: &str) {
        running.process.kill();
        lock(&self.lease).take_if(|id| Some(*id) == running.lease);
        self.release_lease(running.lease);
        self.emit(LocalEvent::Unloaded {
            model: running.model,
            reason: reason.into(),
        });
    }

    /// Zatrzymuje sidecar (np. zatrzymanie modułu).
    pub async fn stop(&self, reason: &str) {
        if let Some(r) = self.state.lock().await.take() {
            self.shutdown(r, reason);
        }
    }

    async fn stop_if_idle(&self, reason: &str) {
        if self.in_flight.load(Ordering::SeqCst) == 0 {
            self.stop(reason).await;
        }
    }

    /// Zwalnia model, jeśli bezczynny od `idle_unload` w chwili `now`; `true` = zatrzymano.
    pub async fn reap_idle_at(&self, now: Instant) -> bool {
        let idle = now.saturating_duration_since(*lock(&self.last_used));
        if self.in_flight.load(Ordering::SeqCst) > 0 || idle < self.config.idle_unload {
            return false;
        }
        let mut st = self.state.lock().await;
        match st.take() {
            Some(r) => {
                self.shutdown(r, "bezczynność");
                true
            }
            None => false,
        }
    }
}
