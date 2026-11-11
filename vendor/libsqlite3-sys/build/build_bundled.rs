// Wygenerowano z przypiętego źródła; logika upstream bez zmian.
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{is_compiler, win_target};

pub fn main(out_dir: &str, out_path: &Path) {
    let lib_name = super::lib_name();

    // This is just a sanity check, the top level `main` should ensure this.
    assert!(
        !(cfg!(feature = "bundled-windows") && !cfg!(feature = "bundled") && !win_target()),
        "This module should not be used: we're not on Windows and the bundled feature has not been enabled"
    );
    let (include_dir, source) = cfg_select! {
        feature = "bundled-sqlcipher" => {
            let dir = sqlcipher_src::source_dir();
            (dir.to_owned(), dir.join(sqlcipher_src::SOURCE_FILE))
        }
        _ => {
            let dir = format!("{}/{lib_name}", env!("CARGO_MANIFEST_DIR"));
            let source = format!("{lib_name}/sqlite3.c");
            (
                std::path::PathBuf::from(dir),
                std::path::PathBuf::from(source),
            )
        }
    };

    cfg_select! {
        feature = "buildtime_bindgen" => {
            use super::{HeaderLocation, bindings};
            let header = HeaderLocation::FromPath(include_dir.to_string_lossy().into());
            bindings::write_to_out_dir(header, out_path);
        }
        _ => {
            super::copy_bindings(lib_name, "bindgen_bundled_version", out_path);
        }
    }
    println!("cargo:include={}", include_dir.display());
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-changed=sqlite3/wasm32-wasi-vfs.c");
    let mut cfg = cc::Build::new();
    cfg.file(&source)
        .flag("-DSQLITE_CORE")
        .flag("-DSQLITE_DEFAULT_FOREIGN_KEYS=1")
        .flag("-DSQLITE_ENABLE_API_ARMOR")
        .flag("-DSQLITE_ENABLE_COLUMN_METADATA")
        .flag("-DSQLITE_ENABLE_DBSTAT_VTAB")
        .flag("-DSQLITE_ENABLE_FTS3")
        .flag("-DSQLITE_ENABLE_FTS3_PARENTHESIS")
        .flag("-DSQLITE_ENABLE_FTS5")
        .flag("-DSQLITE_ENABLE_JSON1")
        .flag("-DSQLITE_ENABLE_LOAD_EXTENSION=1")
        .flag("-DSQLITE_ENABLE_RTREE")
        .flag("-DSQLITE_ENABLE_STAT4")
        .flag("-DSQLITE_SOUNDEX")
        .flag("-DSQLITE_THREADSAFE=1")
        .flag("-DSQLITE_USE_URI")
        .flag("-DHAVE_USLEEP=1")
        .flag("-DHAVE_ISNAN")
        .flag("-D_POSIX_THREAD_SAFE_FUNCTIONS") // cross compile with MinGW
        .warnings(false);

    if cfg!(feature = "bundled-sqlcipher") {
        cfg.flag("-DSQLITE_HAS_CODEC")
            .flag("-DSQLITE_TEMP_STORE=2")
            .flag("-DSQLITE_EXTRA_INIT=sqlcipher_extra_init")
            .flag("-DSQLITE_EXTRA_SHUTDOWN=sqlcipher_extra_shutdown")
            .flag("-DHAVE_STDINT_H=1");

        let target = env::var("TARGET").unwrap();
        let host = env::var("HOST").unwrap();

        let is_windows = host.contains("windows") && target.contains("windows");
        let is_apple = host.contains("apple") && target.contains("apple");

        let lib_dir = env("OPENSSL_LIB_DIR").map(PathBuf::from);
        let inc_dir = env("OPENSSL_INCLUDE_DIR").map(PathBuf::from);
        let mut use_openssl = false;

        let (lib_dir, inc_dir) = match (lib_dir, inc_dir) {
            (Some(lib_dir), Some(inc_dir)) => {
                use_openssl = true;
                (vec![lib_dir], inc_dir)
            }
            (lib_dir, inc_dir) => match find_openssl_dir(&host, &target) {
                None => {
                    if is_windows && !cfg!(feature = "bundled-sqlcipher-vendored-openssl") {
                        panic!("Missing environment variable OPENSSL_DIR or OPENSSL_DIR is not set")
                    } else {
                        (vec![PathBuf::new()], PathBuf::new())
                    }
                }
                Some(openssl_dir) => {
                    let lib_dir = lib_dir.map(|d| vec![d]).unwrap_or_else(|| {
                        let mut lib_dirs = vec![];
                        // OpenSSL 3.0 now puts its libraries in lib64/ by default,
                        // check for both it and lib/.
                        if openssl_dir.join("lib64").exists() {
                            lib_dirs.push(openssl_dir.join("lib64"));
                        }
                        if openssl_dir.join("lib").exists() {
                            lib_dirs.push(openssl_dir.join("lib"));
                        }
                        lib_dirs
                    });
                    let inc_dir = inc_dir.unwrap_or_else(|| openssl_dir.join("include"));

                    if !lib_dir.iter().all(|p| p.exists()) {
                        panic!("OpenSSL library directory does not exist: {lib_dir:?}");
                    }

                    if !Path::new(&inc_dir).exists() {
                        panic!(
                            "OpenSSL include directory does not exist: {}",
                            inc_dir.to_string_lossy()
                        );
                    }

                    use_openssl = true;
                    (lib_dir, inc_dir)
                }
            },
        };

        if cfg!(feature = "bundled-sqlcipher-vendored-openssl") {
            cfg.include(env::var("DEP_OPENSSL_INCLUDE").unwrap());
            // cargo will resolve downstream to the static lib in
            // openssl-sys
        } else if use_openssl {
            cfg.include(inc_dir.to_string_lossy().as_ref());
            let lib_name = if is_windows { "libcrypto" } else { "crypto" };
            println!("cargo:rustc-link-lib=dylib={lib_name}");
            for lib_dir_item in &lib_dir {
                println!("cargo:rustc-link-search={}", lib_dir_item.to_string_lossy());
            }
        } else if is_apple {
            cfg.flag("-DSQLCIPHER_CRYPTO_CC");
            println!("cargo:rustc-link-lib=framework=Security");
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
        } else {
            // branch not taken on Windows, just `crypto` is fine.
            println!("cargo:rustc-link-lib=dylib=crypto");
        }
    }

    // on android sqlite can't figure out where to put the temp files.
    // the bundled sqlite on android also uses `SQLITE_TEMP_STORE=3`.
    // https://android.googlesource.com/platform/external/sqlite/+/2c8c9ae3b7e6f340a19a0001c2a889a211c9d8b2/dist/Android.mk
    if super::android_target() {
        cfg.flag("-DSQLITE_TEMP_STORE=3");
    }

    if cfg!(feature = "with-asan") {
        cfg.flag("-fsanitize=address");
    }

    // If explicitly requested: enable static linking against the Microsoft Visual
    // C++ Runtime to avoid dependencies on vcruntime140.dll and similar libraries.
    if env::var("CARGO_CFG_TARGET_FEATURE").is_ok_and(|v| v.split(',').any(|tf| tf == "crt-static"))
        && is_compiler("msvc")
    {
        cfg.static_crt(true);
    }

    if !win_target() {
        cfg.flag("-DHAVE_LOCALTIME_R");
    }
    if env::var("TARGET").is_ok_and(|v| v.starts_with("wasm32-wasi")) {
        cfg.flag("-USQLITE_THREADSAFE")
            .flag("-DSQLITE_THREADSAFE=0")
            // https://github.com/rust-lang/rust/issues/74393
            .flag("-DLONGDOUBLE_TYPE=double")
            .flag("-D_WASI_EMULATED_MMAN")
            .flag("-D_WASI_EMULATED_GETPID")
            .flag("-D_WASI_EMULATED_SIGNAL")
            .flag("-D_WASI_EMULATED_PROCESS_CLOCKS");

        if cfg!(feature = "wasm32-wasi-vfs") {
            cfg.file("sqlite3/wasm32-wasi-vfs.c");
        }
    }
    if cfg!(feature = "unlock_notify") {
        cfg.flag("-DSQLITE_ENABLE_UNLOCK_NOTIFY");
    }
    if cfg!(feature = "column_metadata") {
        cfg.flag("-DSQLITE_ENABLE_COLUMN_METADATA");
    }
    if cfg!(feature = "preupdate_hook") {
        cfg.flag("-DSQLITE_ENABLE_PREUPDATE_HOOK");
    }
    if cfg!(feature = "session") {
        cfg.flag("-DSQLITE_ENABLE_SESSION");
    }

    if let Ok(limit) = env::var("SQLITE_MAX_VARIABLE_NUMBER") {
        cfg.flag(format!("-DSQLITE_MAX_VARIABLE_NUMBER={limit}"));
    }
    println!("cargo:rerun-if-env-changed=SQLITE_MAX_VARIABLE_NUMBER");

    if let Ok(limit) = env::var("SQLITE_MAX_EXPR_DEPTH") {
        cfg.flag(format!("-DSQLITE_MAX_EXPR_DEPTH={limit}"));
    }
    println!("cargo:rerun-if-env-changed=SQLITE_MAX_EXPR_DEPTH");

    if let Ok(limit) = env::var("SQLITE_MAX_COLUMN") {
        cfg.flag(format!("-DSQLITE_MAX_COLUMN={limit}"));
    }
    println!("cargo:rerun-if-env-changed=SQLITE_MAX_COLUMN");

    if let Ok(extras) = env::var("LIBSQLITE3_FLAGS") {
        for extra in extras.split_whitespace() {
            if extra.starts_with("-D") || extra.starts_with("-U") {
                cfg.flag(extra);
            } else if extra.starts_with("SQLITE_") {
                cfg.flag(format!("-D{extra}"));
            } else {
                panic!("Don't understand {extra} in LIBSQLITE3_FLAGS");
            }
        }
    }
    println!("cargo:rerun-if-env-changed=LIBSQLITE3_FLAGS");

    cfg.compile(lib_name);

    println!("cargo:lib_dir={out_dir}");
}

fn env(name: &str) -> Option<OsString> {
    let prefix = env::var("TARGET").unwrap().to_uppercase().replace('-', "_");
    let prefixed = format!("{prefix}_{name}");
    println!("cargo:rerun-if-env-changed={prefixed}");
    println!("cargo:rerun-if-env-changed={name}");
    let var = env::var_os(prefixed);

    match var {
        None => env::var_os(name),
        _ => var,
    }
}

fn find_openssl_dir(_host: &str, _target: &str) -> Option<PathBuf> {
    let openssl_dir = env("OPENSSL_DIR");
    openssl_dir.map(PathBuf::from)
}
