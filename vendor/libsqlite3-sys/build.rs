use std::env;
use std::path::Path;

// Just to keep MSRV low
#[cfg(any(
    feature = "bundled",
    feature = "bundled-windows",
    feature = "bundled-sqlcipher",
    feature = "buildtime_bindgen",
))]
macro_rules! cfg_select {
    ({ $($tt:tt)* }) => {{
        $crate::cfg_select! { $($tt)* }
    }};
    (_ => { $($output:tt)* }) => {
        $($output)*
    };
    (
        $cfg:meta => $output:tt
        $($( $rest:tt )+)?
    ) => {{
        #[cfg($cfg)]
        cfg_select! { _ => $output }
        $(
            #[cfg(not($cfg))]
            cfg_select! { $($rest)+ }
        )?
    }}
}

#[cfg(all(feature = "loadable_extension", feature = "preupdate_hook"))]
compile_error!(
    "feature \"loadable_extension\" and feature \"preupdate_hook\" cannot be enabled at the same time"
);

/// Tells whether we're building for Windows. This is more suitable than a plain
/// `cfg!(windows)`, since the latter does not properly handle cross-compilation
///
/// Note that there is no way to know at compile-time which system we'll be
/// targeting, and this test must be made at run-time (of the build script) See
/// <https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts>
fn win_target() -> bool {
    env::var("CARGO_CFG_WINDOWS").is_ok()
}

/// Tells whether we're building for Android.
/// See [`win_target`]
#[cfg(any(feature = "bundled", feature = "bundled-windows"))]
fn android_target() -> bool {
    env::var("CARGO_CFG_TARGET_OS").is_ok_and(|v| v == "android")
}

/// Tells whether a given compiler will be used `compiler_name` is compared to
/// the content of `CARGO_CFG_TARGET_ENV` (and is always lowercase)
///
/// See [`win_target`]
fn is_compiler(compiler_name: &str) -> bool {
    env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|v| v == compiler_name)
}

/// Copy bindgen file from `dir` to `out_path`.
fn copy_bindings<T: AsRef<Path>>(dir: &str, bindgen_name: &str, out_path: T) {
    let from = if cfg!(feature = "loadable_extension") {
        format!("{dir}/{bindgen_name}_ext.rs")
    } else {
        format!("{dir}/{bindgen_name}.rs")
    };
    std::fs::copy(from, out_path).expect("Could not copy bindings to output directory");
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("bindgen.rs");
    if cfg!(feature = "in_gecko") {
        // When inside mozilla-central, we are included into the build with
        // sqlite3.o directly, so we don't want to provide any linker arguments.
        copy_bindings("sqlite3", "bindgen_bundled_version", out_path);
        return;
    }

    println!("cargo:rerun-if-env-changed=LIBSQLITE3_SYS_USE_PKG_CONFIG");
    if env::var_os("LIBSQLITE3_SYS_USE_PKG_CONFIG").is_some_and(|s| s != "0")
        || cfg!(feature = "loadable_extension")
    {
        build_linked::main(&out_dir, &out_path);
    } else if cfg!(all(
        feature = "sqlcipher",
        not(feature = "bundled-sqlcipher")
    )) {
        if cfg!(feature = "bundled") || (win_target() && cfg!(feature = "bundled-windows")) {
            println!(
                "cargo:warning=For backwards compatibility, feature 'sqlcipher' overrides
                features 'bundled' and 'bundled-windows'. If you want a bundled build of
                SQLCipher (available for the moment only on Unix), use feature 'bundled-sqlcipher'
                or 'bundled-sqlcipher-vendored-openssl' to also bundle OpenSSL crypto."
            );
        }
        build_linked::main(&out_dir, &out_path);
    } else if cfg!(feature = "bundled")
        || (win_target() && cfg!(feature = "bundled-windows"))
        || cfg!(feature = "bundled-sqlcipher")
    {
        #[cfg(any(
            feature = "bundled",
            feature = "bundled-windows",
            feature = "bundled-sqlcipher"
        ))]
        build_bundled::main(&out_dir, &out_path);
        #[cfg(not(any(
            feature = "bundled",
            feature = "bundled-windows",
            feature = "bundled-sqlcipher"
        )))]
        panic!("The runtime test should not run this branch, which has not compiled any logic.")
    } else {
        build_linked::main(&out_dir, &out_path);
    }
}

#[cfg(any(
    feature = "bundled",
    feature = "bundled-windows",
    feature = "bundled-sqlcipher"
))]
#[path = "build/build_bundled.rs"]
mod build_bundled;

fn env_prefix() -> &'static str {
    if cfg!(any(feature = "sqlcipher", feature = "bundled-sqlcipher")) {
        "SQLCIPHER"
    } else {
        "SQLITE3"
    }
}

fn lib_name() -> &'static str {
    if cfg!(any(feature = "sqlcipher", feature = "bundled-sqlcipher")) {
        "sqlcipher"
    } else {
        "sqlite3"
    }
}

pub enum HeaderLocation {
    FromEnvironment,
    Wrapper,
    FromPath(String),
}

impl From<HeaderLocation> for String {
    fn from(header: HeaderLocation) -> Self {
        match header {
            HeaderLocation::FromEnvironment => {
                let prefix = env_prefix();
                let mut header = env::var(format!("{prefix}_INCLUDE_DIR")).unwrap_or_else(|_| {
                    panic!("{prefix}_INCLUDE_DIR must be set if {prefix}_LIB_DIR is set")
                });
                header.push_str(if cfg!(feature = "loadable_extension") {
                    "/sqlite3ext.h"
                } else {
                    "/sqlite3.h"
                });
                header
            }
            HeaderLocation::Wrapper => if cfg!(feature = "loadable_extension") {
                "wrapper_ext.h"
            } else {
                "wrapper.h"
            }
            .into(),
            HeaderLocation::FromPath(path) => format!(
                "{}/{}",
                path,
                if cfg!(feature = "loadable_extension") {
                    "sqlite3ext.h"
                } else {
                    "sqlite3.h"
                }
            ),
        }
    }
}

#[path = "build/build_linked.rs"]
mod build_linked;

#[cfg(not(feature = "buildtime_bindgen"))]
#[allow(dead_code)]
#[path = "build/bindings_prebuilt.rs"]
mod bindings;

#[cfg(feature = "buildtime_bindgen")]
#[path = "build/bindings_buildtime.rs"]
mod bindings;

#[cfg(all(feature = "buildtime_bindgen", feature = "loadable_extension"))]
#[path = "build/loadable_extension.rs"]
mod loadable_extension;
