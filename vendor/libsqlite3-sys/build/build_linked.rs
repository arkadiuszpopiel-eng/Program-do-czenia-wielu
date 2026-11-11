// Wygenerowano z przypiętego źródła; logika upstream bez zmian.
#[cfg(feature = "vcpkg")]
extern crate vcpkg;

use super::{HeaderLocation, bindings, env_prefix, is_compiler, lib_name, win_target};
use std::env;
use std::path::Path;

pub fn main(_out_dir: &str, out_path: &Path) {
    let header = find_sqlite();
    if (cfg!(any(
        feature = "bundled_bindings",
        feature = "bundled",
        feature = "bundled-sqlcipher"
    )) || (win_target() && cfg!(feature = "bundled-windows")))
        && !cfg!(feature = "buildtime_bindgen")
    {
        // Generally means the `bundled_bindings` feature is enabled.
        // Most users are better off with turning
        // on buildtime_bindgen instead, but this is still supported as we
        // have runtime version checks and there are good reasons to not
        // want to run bindgen.
        super::copy_bindings(lib_name(), "bindgen_bundled_version", out_path);
    } else {
        bindings::write_to_out_dir(header, out_path);
    }
}

#[cfg(not(feature = "loadable_extension"))]
fn find_link_mode() -> &'static str {
    // If the user specifies SQLITE3_STATIC (or SQLCIPHER_STATIC), do static
    // linking, unless it's explicitly set to 0.
    match &env::var(format!("{}_STATIC", env_prefix())) {
        Ok(v) if v != "0" => "static",
        _ => "dylib",
    }
}
// Prints the necessary cargo link commands and returns the path to the header.
fn find_sqlite() -> HeaderLocation {
    let link_lib = lib_name();

    println!("cargo:rerun-if-env-changed={}_INCLUDE_DIR", env_prefix());
    println!("cargo:rerun-if-env-changed={}_LIB_DIR", env_prefix());
    println!("cargo:rerun-if-env-changed={}_STATIC", env_prefix());
    if cfg!(feature = "vcpkg") && is_compiler("msvc") {
        println!("cargo:rerun-if-env-changed=VCPKGRS_DYNAMIC");
    }

    // dependents can access `DEP_SQLITE3_LINK_TARGET` (`sqlite3` being the
    // `links=` value in our Cargo.toml) to get this value. This might be
    // useful if you need to ensure whatever crypto library sqlcipher relies
    // on is available, for example.
    #[cfg(not(feature = "loadable_extension"))]
    println!("cargo:link-target={link_lib}");

    // Allow users to specify where to find SQLite.
    if let Ok(dir) = env::var(format!("{}_LIB_DIR", env_prefix())) {
        // Try to use pkg-config to determine link commands
        let pkgconfig_path = Path::new(&dir).join("pkgconfig");
        unsafe { env::set_var("PKG_CONFIG_PATH", pkgconfig_path) };
        #[cfg(not(feature = "loadable_extension"))]
        if pkg_config::Config::new()
            .atleast_version("3.45.3")
            .probe(link_lib)
            .is_err()
        {
            // Otherwise just emit the bare minimum link commands.
            println!("cargo:rustc-link-lib={}={link_lib}", find_link_mode());
            println!("cargo:rustc-link-search={dir}");
        }
        return HeaderLocation::FromEnvironment;
    }

    if let Some(header) = try_vcpkg() {
        return header;
    }

    // See if pkg-config can do everything for us.
    if let Ok(mut lib) = pkg_config::Config::new()
        .atleast_version("3.45.3")
        .print_system_libs(false)
        .probe(link_lib)
    {
        if let Some(header) = lib.include_paths.pop() {
            HeaderLocation::FromPath(header.to_string_lossy().into())
        } else {
            HeaderLocation::Wrapper
        }
    } else {
        // No env var set and pkg-config couldn't help; just output the link-lib
        // request and hope that the library exists on the system paths. We used to
        // output /usr/lib explicitly, but that can introduce other linking problems;
        // see https://github.com/rusqlite/rusqlite/issues/207.
        #[cfg(not(feature = "loadable_extension"))]
        println!("cargo:rustc-link-lib={}={link_lib}", find_link_mode());
        HeaderLocation::Wrapper
    }
}

fn try_vcpkg() -> Option<HeaderLocation> {
    if cfg!(feature = "vcpkg") && is_compiler("msvc") {
        // See if vcpkg can find it.
        if let Ok(mut lib) = vcpkg::Config::new().probe(lib_name())
            && let Some(header) = lib.include_paths.pop()
        {
            return Some(HeaderLocation::FromPath(header.to_string_lossy().into()));
        }
        None
    } else {
        None
    }
}
