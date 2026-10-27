//! Suppress repeated operations already known to fail before execution.
use crate::engine::Engine;
use crate::flow::fingerprint;
use providers_contract::ToolUse;
use tools_common_contract::ToolOutcome;
impl Engine {
    pub(crate) fn note_snapshot_failure(&mut self, tu: &ToolUse, out: &ToolOutcome) {
        if out.data["snapshot_failure"] == true {
            self.cp
                .failed_preflight
                .insert(fingerprint(&tu.name, &tu.input));
        }
    }
}
