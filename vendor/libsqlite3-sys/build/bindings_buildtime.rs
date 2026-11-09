// Wygenerowano z przypiętego źródła; logika upstream bez zmian.
use super::HeaderLocation;
use bindgen::callbacks::{IntKind, ParseCallbacks};

use std::path::Path;
#[derive(Debug)]
struct SqliteTypeChooser;

impl ParseCallbacks for SqliteTypeChooser {
    fn int_macro(&self, name: &str, _value: i64) -> Option<IntKind> {
        if name == "SQLITE_SERIALIZE_NOCOPY"
            || name.starts_with("SQLITE_DESERIALIZE_")
            || name.starts_with("SQLITE_PREPARE_")
            || name.starts_with("SQLITE_TRACE_")
        {
            Some(IntKind::UInt)
        } else {
            None
        }
    }
}

// Are we generating the bundled bindings? Used to avoid emitting things
// that would be problematic in bundled builds. This env var is set by
// `upgrade.sh`.
fn generating_bundled_bindings() -> bool {
    // Hacky way to know if we're generating the bundled bindings
    println!("cargo:rerun-if-env-changed=LIBSQLITE3_SYS_BUNDLING");
    match std::env::var("LIBSQLITE3_SYS_BUNDLING") {
        Ok(v) => v != "0",
        Err(_) => false,
    }
}

pub fn write_to_out_dir(header: HeaderLocation, out_path: &Path) {
    let header: String = header.into();
    let mut bindings = bindgen::builder()
        .default_macro_constant_type(bindgen::MacroTypeVariation::Signed)
        .disable_nested_struct_naming()
        .generate_comments(false)
        .generate_cstr(true)
        .use_core()
        .trust_clang_mangling(false)
        .header(header.clone())
        .parse_callbacks(Box::new(SqliteTypeChooser));
    if cfg!(feature = "loadable_extension") {
        bindings = bindings.ignore_functions(); // see generate_functions
    } else {
        bindings = bindings
            .blocklist_function("sqlite3_auto_extension")
            .raw_line(
                r#"unsafe extern "C" {
    pub fn sqlite3_auto_extension(
        xEntryPoint: ::core::option::Option<
            unsafe extern "C" fn(
                db: *mut sqlite3,
                pzErrMsg: *mut *mut ::core::ffi::c_char,
                _: *const sqlite3_api_routines,
            ) -> ::core::ffi::c_int,
        >,
    ) -> ::core::ffi::c_int;
}"#,
            )
            .blocklist_function("sqlite3_cancel_auto_extension")
            .raw_line(
                r#"unsafe extern "C" {
    pub fn sqlite3_cancel_auto_extension(
        xEntryPoint: ::core::option::Option<
            unsafe extern "C" fn(
                db: *mut sqlite3,
                pzErrMsg: *mut *mut ::core::ffi::c_char,
                _: *const sqlite3_api_routines,
            ) -> ::core::ffi::c_int,
        >,
    ) -> ::core::ffi::c_int;
}"#,
            )
            .blocklist_function(".*16.*")
            .blocklist_function("sqlite3_close_v2")
            .blocklist_function("sqlite3_create_collation")
            .blocklist_function("sqlite3_create_function")
            .blocklist_function("sqlite3_create_module")
            .blocklist_function("sqlite3_prepare");
    }

    if cfg!(any(feature = "sqlcipher", feature = "bundled-sqlcipher")) {
        bindings = bindings.clang_arg("-DSQLITE_HAS_CODEC");
    }
    if cfg!(feature = "unlock_notify") {
        bindings = bindings.clang_arg("-DSQLITE_ENABLE_UNLOCK_NOTIFY");
    }
    if cfg!(feature = "preupdate_hook") {
        bindings = bindings.clang_arg("-DSQLITE_ENABLE_PREUPDATE_HOOK");
    }
    if cfg!(feature = "session") {
        bindings = bindings.clang_arg("-DSQLITE_ENABLE_SESSION");
    }

    // When cross compiling unless effort is taken to fix the issue, bindgen
    // will find the wrong headers. There's only one header included by the
    // amalgamated `sqlite.h`: `stdarg.h`.
    //
    // Thankfully, there's almost no case where rust code needs to use
    // functions taking `va_list` (It's nearly impossible to get a `va_list`
    // in Rust unless you get passed it by C code for some reason).
    //
    // Arguably, we should never be including these, but we include them for
    // the cases where they aren't totally broken...
    let target_arch = std::env::var("TARGET").unwrap();
    let host_arch = std::env::var("HOST").unwrap();
    let is_cross_compiling = target_arch != host_arch;

    // Note that when generating the bundled file, we're essentially always
    // cross compiling.
    if generating_bundled_bindings() || is_cross_compiling {
        // Get rid of va_list, as it's not
        bindings = bindings
            .blocklist_function("sqlite3_vmprintf")
            .blocklist_function("sqlite3_vsnprintf")
            .blocklist_function("sqlite3_str_vappendf")
            .blocklist_type("va_list")
            .blocklist_item("__.*");
    }

    let bindings = bindings
        .layout_tests(false)
        .generate()
        .unwrap_or_else(|_| panic!("could not run bindgen on header {header}"));

    cfg_select! {
        feature = "loadable_extension" => {
            let mut output = Vec::new();
            bindings
                .write(Box::new(&mut output))
                .expect("could not write output of bindgen");
            let mut output = String::from_utf8(output).expect("bindgen output was not UTF-8?!");
            super::loadable_extension::generate_functions(&mut output);
            std::fs::write(out_path, output.as_bytes())
                .unwrap_or_else(|_| panic!("Could not write to {out_path:?}"));
        }
        _ => {
            bindings
                .write_to_file(out_path)
                .unwrap_or_else(|_| panic!("Could not write to {out_path:?}"));
        }
    }
}
