// Wygenerowano z przypiętego źródła; logika upstream bez zmian.
use super::HeaderLocation;

use std::path::Path;

static PREBUILT_BINDGENS: &[&str] = &["bindgen_3.45.3"];

pub fn write_to_out_dir(_header: HeaderLocation, out_path: &Path) {
    let name = PREBUILT_BINDGENS[PREBUILT_BINDGENS.len() - 1];
    super::copy_bindings("bindgen-bindings", name, out_path);
}
