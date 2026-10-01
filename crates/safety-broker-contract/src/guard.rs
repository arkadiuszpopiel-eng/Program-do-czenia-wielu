//! Strażnik Jądra: wykrywanie twardych reguł dla zdolności i wyliczanie faktów dla
//! klasyfikatora. Dwa momenty: `check_request` przy wydaniu tokenu (zakres wewnątrz obszaru
//! chronionego = blokada) i `check_use` przy weryfikacji konkretnego użycia (konkretna ścieżka
//! lub host w obszarze chronionym = blokada, nawet jeśli szeroki token ją obejmuje).

use compliance_contract::DenyChecker;
use compliance_contract::deny::{NormPath, PathEnv, host_matches};
use risk_classifier_contract::{
    ActionFacts, CommandOrigin, Destructiveness, KernelRule, ScopeRelation,
};

use crate::action::{ActionRequest, DeclaredFacts, SessionSecurity};
use crate::capability::Capability;
use crate::policy::{KernelPolicy, PROTECTED_SERVICES};
use crate::scope::{AdminOp, HostPattern, PathScope, ServiceAction};
use crate::shell_guard::{ShellContext, check_command, system_areas};

/// Skompilowana polityka Jądra do szybkich sprawdzeń.
pub struct KernelGuard {
    policy: KernelPolicy,
    deny: DenyChecker,
    env: PathEnv,
    windows: NormPath,
    boot: Vec<NormPath>,
}

fn inside(p: &NormPath, root: &NormPath) -> bool {
    p.root == root.root
        && root.comps.len() <= p.comps.len()
        && root.comps.iter().zip(&p.comps).all(|(a, b)| a == b)
}

impl KernelGuard {
    /// Kompiluje politykę; `env` rozwija zmienne w poleceniach powłoki.
    pub fn new(policy: KernelPolicy, env: PathEnv) -> Self {
        let deny = DenyChecker::new(policy.deny_lists.clone(), &env);
        let sd = policy.system_drive;
        let env = env
            .with("SystemRoot", &format!("{sd}:\\Windows"))
            .with("windir", &format!("{sd}:\\Windows"))
            .with("SystemDrive", &format!("{sd}:"));
        let (windows, boot) = system_areas(sd);
        Self {
            policy,
            deny,
            env,
            windows,
            boot,
        }
    }

    /// Polityka źródłowa.
    pub fn policy(&self) -> &KernelPolicy {
        &self.policy
    }

    fn path_rule(&self, scope: &PathScope, writes: bool, destroys: bool) -> Option<KernelRule> {
        let p = scope.norm();
        if self.deny.is_denied_normalized(&p) {
            return Some(KernelRule::CredentialDenylist);
        }
        if !writes {
            return None;
        }
        if self.policy.kernel_paths.iter().any(|k| k.contains(&p)) {
            return Some(KernelRule::KernelPolicyChange);
        }
        if self.boot.iter().any(|b| inside(&p, b)) {
            return Some(KernelRule::BootloaderModification);
        }
        if inside(&p, &self.windows) {
            return Some(KernelRule::SystemRootDeletion);
        }
        // Usunięcie katalogu jest rekurencyjne niezależnie od rodzaju zakresu (`fs_delete` prosi
        // o zakres dokładny): przodek `%SystemRoot%` albo obszaru Jądra = blokada (SR-07).
        if destroys && inside(&self.windows, &p) {
            return Some(KernelRule::SystemRootDeletion);
        }
        if destroys
            && self
                .policy
                .kernel_paths
                .iter()
                .any(|k| inside(&k.norm(), &p))
        {
            return Some(KernelRule::KernelPolicyChange);
        }
        None
    }

    fn egress_rule(&self, host: &HostPattern) -> Option<KernelRule> {
        let lists = self.deny.lists();
        let covers_denied =
            host.subdomains() && lists.domains.iter().any(|d| host_matches(d, host.host()));
        (self.deny.is_denied_domain(host.host()) || covers_denied)
            .then_some(KernelRule::ProviderWebUi)
    }

    fn admin_rule(&self, op: &AdminOp) -> Option<KernelRule> {
        match op {
            AdminOp::DisableAudit => Some(KernelRule::AuditDisable),
            AdminOp::Bootloader => Some(KernelRule::BootloaderModification),
            AdminOp::ChangeKernelPolicy => Some(KernelRule::KernelPolicyChange),
            AdminOp::FormatDisk { drive } => (drive.to_ascii_lowercase()
                == self.policy.system_drive)
                .then_some(KernelRule::SystemDiskFormat),
            AdminOp::ServiceControl { service, action } => {
                let svc = service.to_lowercase();
                let hostile = !matches!(action, ServiceAction::Start);
                (hostile && PROTECTED_SERVICES.contains(&svc.as_str()))
                    .then_some(KernelRule::KillSwitchDisable)
            }
            AdminOp::RegistryMachine { key } => {
                let k = key.to_lowercase().replace('/', "\\");
                let services = PROTECTED_SERVICES
                    .iter()
                    .any(|s| k.contains(&format!("services\\{s}")));
                services.then_some(KernelRule::KillSwitchDisable)
            }
            AdminOp::Other { command } => check_command(command, &self.shell_ctx(None)),
            AdminOp::Install { .. } | AdminOp::Firewall { .. } => None,
        }
    }

    fn shell_ctx<'a>(&'a self, cwd: Option<&'a PathScope>) -> ShellContext<'a> {
        ShellContext {
            env: &self.env,
            deny: &self.deny,
            system_drive: self.policy.system_drive,
            kernel_paths: &self.policy.kernel_paths,
            cwd,
        }
    }

    /// Reguła Jądra dla żądanej zdolności (wydanie tokenu).
    pub fn check_request(&self, cap: &Capability, facts: &DeclaredFacts) -> Option<KernelRule> {
        let destroys = facts.destructive != Destructiveness::None;
        match cap {
            Capability::FsRead(s) => self.path_rule(s, false, false),
            Capability::FsWrite(s) => self.path_rule(s, true, destroys),
            Capability::ShellExec(s) => self.path_rule(s, true, destroys).or_else(|| {
                facts
                    .command
                    .as_deref()
                    .and_then(|c| check_command(c, &self.shell_ctx(Some(s))))
            }),
            Capability::GuiControl(app) => {
                if self.policy.is_protected_process(app) {
                    Some(KernelRule::GuiControlOfKernelProcess)
                } else {
                    // Aplikacje dostawców planów: agentka nie „używa” ich UI (SR-09).
                    self.policy
                        .is_provider_app(app)
                        .then_some(KernelRule::ProviderWebUi)
                }
            }
            Capability::NetEgress(host) => self.egress_rule(host),
            Capability::SecretsRead(_) => None,
            Capability::SystemAdmin(op) => self.admin_rule(op),
        }
    }

    /// Reguła Jądra dla konkretnego użycia (weryfikacja tokenu).
    pub fn check_use(&self, needed: &Capability) -> Option<KernelRule> {
        self.check_request(needed, &DeclaredFacts::new("verify"))
    }

    /// Relacja zdolności do zakresu sesji.
    pub fn scope_relation(&self, cap: &Capability) -> ScopeRelation {
        let in_profile =
            |s: &PathScope| self.policy.profile_roots.iter().any(|r| s.is_subset_of(r));
        match cap {
            Capability::FsRead(s) | Capability::FsWrite(s) | Capability::ShellExec(s) => {
                if in_profile(s) {
                    ScopeRelation::InScope
                } else {
                    ScopeRelation::Outside
                }
            }
            Capability::GuiControl(app) => {
                if self.policy.allowed_apps.contains(app) {
                    ScopeRelation::AllowedApp
                } else {
                    ScopeRelation::Outside
                }
            }
            Capability::NetEgress(h) => {
                if self.egress_allowlisted(h) {
                    ScopeRelation::InScope
                } else {
                    ScopeRelation::Outside
                }
            }
            Capability::SecretsRead(_) => ScopeRelation::InScope,
            Capability::SystemAdmin(_) => ScopeRelation::Outside,
        }
    }

    /// Czy wzorzec hosta mieści się w egress-allowliście.
    pub fn egress_allowlisted(&self, host: &HostPattern) -> bool {
        self.policy
            .egress_allowlist
            .iter()
            .any(|a| host.is_subset_of(a))
    }

    /// Fakty dla klasyfikatora: deklaracje narzędzia + to, co Broker wie sam.
    pub fn derive_facts(&self, req: &ActionRequest, session: &SessionSecurity) -> ActionFacts {
        let cap = &req.capability;
        let mut f = ActionFacts::new(req.facts.tool.clone(), cap.action_class())
            .reversible(req.facts.reversible)
            .scope(self.scope_relation(cap))
            .destructive(req.facts.destructive)
            .bulk(req.facts.bulk.max(1))
            .origin(req.origin);
        if let Capability::NetEgress(h) = cap {
            f = f.egress(h.host(), self.egress_allowlisted(h));
        }
        f.install = req.facts.install;
        f.tainted = session.tainted || req.origin == CommandOrigin::UntrustedContent;
        f.untrusted_input_in_args = req.facts.untrusted_input_in_args;
        f.touches_private_data = session.private_data || req.facts.touches_private_data;
        f.kernel_rule = self.check_request(cap, &req.facts);
        f
    }
}
