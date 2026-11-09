// Wygenerowano z przypiętego źródła; logika upstream bez zmian.
/// try to generate similar rust code for all `#define sqlite3_xyz
/// sqlite3_api->abc` macros` in sqlite3ext.h
pub fn generate_functions(output: &mut String) {
    // (1) parse sqlite3_api_routines fields from bindgen output
    let ast: syn::File = syn::parse_str(output).expect("could not parse bindgen output");
    let sqlite3_api_routines: syn::ItemStruct = ast
        .items
        .into_iter()
        .find_map(|i| {
            if let syn::Item::Struct(s) = i {
                if s.ident == "sqlite3_api_routines" {
                    Some(s)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .expect("could not find sqlite3_api_routines");
    let sqlite3_api_routines_ident = sqlite3_api_routines.ident;
    let p_api = quote::format_ident!("p_api");
    let mut stores = Vec::new();
    let mut malloc = Vec::new();
    // (2) `#define sqlite3_xyz sqlite3_api->abc` => `pub unsafe fn
    // sqlite3_xyz(args) -> ty {...}` for each `abc` field:
    for field in sqlite3_api_routines.fields {
        let ident = field.ident.expect("unnamed field");
        let span = ident.span();
        let name = ident.to_string();
        if name.contains("16") {
            continue; // skip UTF-16 api as rust uses UTF-8
        } else if name == "vmprintf" || name == "xvsnprintf" || name == "str_vappendf" {
            continue; // skip va_list
        } else if name == "aggregate_count"
            || name == "expired"
            || name == "global_recover"
            || name == "thread_cleanup"
            || name == "transfer_bindings"
            || name == "create_collation"
            || name == "create_function"
            || name == "create_module"
            || name == "prepare"
            || name == "close_v2"
        {
            continue; // omit deprecated
        }
        let sqlite3_name = match name.as_ref() {
            "xthreadsafe" => "sqlite3_threadsafe".to_owned(),
            "interruptx" => "sqlite3_interrupt".to_owned(),
            _ => {
                format!("sqlite3_{name}")
            }
        };
        let ptr_name = syn::Ident::new(format!("__{}", sqlite3_name.to_uppercase()).as_ref(), span);
        let sqlite3_fn_name = syn::Ident::new(&sqlite3_name, span);
        let method =
            extract_method(&field.ty).unwrap_or_else(|| panic!("unexpected type for {name}"));
        let arg_names: syn::punctuated::Punctuated<&syn::Ident, syn::token::Comma> = method
            .inputs
            .iter()
            .map(|i| &i.name.as_ref().unwrap().0)
            .collect();
        let args = &method.inputs;
        // vtab_config/sqlite3_vtab_config: ok
        let varargs = &method.variadic;
        if varargs.is_some() && "db_config" != name && "log" != name && "vtab_config" != name {
            continue; // skip ...
        }
        let ty = &method.output;
        let tokens = if "db_config" == name {
            quote::quote! {
                static #ptr_name: ::core::sync::atomic::AtomicPtr<()> = ::core::sync::atomic::AtomicPtr::new(::core::ptr::null_mut());
                pub unsafe fn #sqlite3_fn_name(#args arg3: ::core::ffi::c_int, arg4: *mut ::core::ffi::c_int) #ty {
                    unsafe {
                        let ptr = #ptr_name.load(::core::sync::atomic::Ordering::Acquire);
                        assert!(!ptr.is_null(), "SQLite API not initialized");
                        let fun: unsafe extern "C" fn(#args #varargs) #ty = ::core::mem::transmute(ptr);
                        (fun)(#arg_names, arg3, arg4)
                    }
                }
            }
        } else if "log" == name {
            quote::quote! {
                static #ptr_name: ::core::sync::atomic::AtomicPtr<()> = ::core::sync::atomic::AtomicPtr::new(::core::ptr::null_mut());
                pub unsafe fn #sqlite3_fn_name(#args arg3: *const ::core::ffi::c_char) #ty {
                    unsafe {
                        let ptr = #ptr_name.load(::core::sync::atomic::Ordering::Acquire);
                        assert!(!ptr.is_null(), "SQLite API not initialized");
                        let fun: unsafe extern "C" fn(#args #varargs) #ty = ::core::mem::transmute(ptr);
                        (fun)(#arg_names, arg3)
                    }
                }
            }
        } else {
            quote::quote! {
                static #ptr_name: ::core::sync::atomic::AtomicPtr<()> = ::core::sync::atomic::AtomicPtr::new(::core::ptr::null_mut());
                pub unsafe fn #sqlite3_fn_name(#args) #ty {
                    unsafe {
                        let ptr = #ptr_name.load(::core::sync::atomic::Ordering::Acquire);
                        assert!(!ptr.is_null(), "SQLite API not initialized or SQLite feature omitted");
                        let fun: unsafe extern "C" fn(#args #varargs) #ty = ::core::mem::transmute(ptr);
                        (fun)(#arg_names)
                    }
                }
            }
        };
        output.push_str(&prettyplease::unparse(
            &syn::parse2(tokens).expect("could not parse quote output"),
        ));
        output.push('\n');
        if name == "malloc" {
            &mut malloc
        } else {
            &mut stores
        }
        .push(quote::quote! {
            if let Some(fun) = (*#p_api).#ident {
                #ptr_name.store(
                    fun as usize as *mut (),
                    ::core::sync::atomic::Ordering::Release,
                );
            }
        });
    }
    // (3) generate rust code similar to SQLITE_EXTENSION_INIT2 macro
    let tokens = quote::quote! {
        /// Like SQLITE_EXTENSION_INIT2 macro
        pub unsafe fn rusqlite_extension_init2(#p_api: *mut #sqlite3_api_routines_ident) -> ::core::result::Result<(), crate::InitError> {
            unsafe {
                #(#malloc)* // sqlite3_malloc needed by to_sqlite_error
                if let Some(fun) = (*#p_api).libversion_number {
                    let version = fun();
                    if SQLITE_VERSION_NUMBER > version {
                        return Err(crate::InitError::VersionMismatch{compile_time: SQLITE_VERSION_NUMBER, runtime: version});
                    }
                } else {
                    return Err(crate::InitError::NullFunctionPointer);
                }
                #(#stores)*
                Ok(())
            }
        }
    };
    output.push_str(&prettyplease::unparse(
        &syn::parse2(tokens).expect("could not parse quote output"),
    ));
    output.push('\n');
}

fn extract_method(ty: &syn::Type) -> Option<&syn::TypeFnPtr> {
    match ty {
        syn::Type::Path(tp) => tp.path.segments.last(),
        _ => None,
    }
    .map(|seg| match &seg.arguments {
        syn::PathArguments::AngleBracketed(args) => args.args.first(),
        _ => None,
    })?
    .map(|arg| match arg {
        syn::GenericArgument::Type(t) => Some(t),
        _ => None,
    })?
    .map(|ty| match ty {
        syn::Type::FnPtr(r) => Some(r),
        _ => None,
    })?
}
