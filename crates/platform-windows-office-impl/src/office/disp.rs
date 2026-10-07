//! Late binding `IDispatch` (Word/Excel bez bibliotek typów): wywołanie po nazwie z argumentami
//! pozycyjnymi, odczyt i zapis właściwości (`DISPID_PROPERTYPUT`), błędy z `EXCEPINFO`,
//! konwersje `VARIANT` (w tym `SAFEARRAY` 2D zakresów Excela) i sesja aplikacji Office z
//! wymuszonym `AutomationSecurity = 3` przed otwarciem czegokolwiek.

#![allow(unsafe_code)]

use std::mem::ManuallyDrop;

use platform_apps_contract::{
    AUTOMATION_SECURITY_FORCE_DISABLE, CellValue, OfficeApp, OfficeError, OfficeSession,
};
use windows::Win32::Foundation::DISP_E_PARAMNOTFOUND;
use windows::Win32::System::Com::{
    CLSCTX_LOCAL_SERVER, CLSIDFromProgID, CoCreateInstance, DISPATCH_FLAGS, DISPATCH_METHOD,
    DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT, DISPPARAMS, EXCEPINFO, IDispatch, SAFEARRAY,
};
use windows::Win32::System::Ole::{
    DISPID_PROPERTYPUT, SafeArrayGetDim, SafeArrayGetElement, SafeArrayGetLBound,
    SafeArrayGetUBound,
};
use windows::Win32::System::Variant::{
    VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_ARRAY, VT_BOOL, VT_BSTR, VT_EMPTY, VT_ERROR,
    VT_NULL, VT_VARIANT,
};
use windows::core::{BSTR, GUID, PCWSTR};

/// `LOCALE_USER_DEFAULT`.
const LCID: u32 = 0x0400;

/// Argument „pominięty” (`VT_ERROR` + `DISP_E_PARAMNOTFOUND`).
pub(crate) fn missing() -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_ERROR,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 {
                    scode: DISP_E_PARAMNOTFOUND.0,
                },
            }),
        },
    }
}

/// Napis jako `VARIANT`.
pub(crate) fn text(s: &str) -> VARIANT {
    VARIANT::from(BSTR::from(s))
}

/// Obiekt automatyzacji.
#[derive(Clone)]
pub(crate) struct Disp(pub(crate) IDispatch);

fn com_err(name: &str, e: &windows::core::Error, mut ex: EXCEPINFO) -> OfficeError {
    let desc = ex.bstrDescription.to_string();
    // SAFETY: napisy `EXCEPINFO` wypełnił serwer i należą do wywołującego; zwalniane raz (pusty
    // BSTR jest bezpieczny).
    unsafe {
        ManuallyDrop::drop(&mut ex.bstrSource);
        ManuallyDrop::drop(&mut ex.bstrDescription);
        ManuallyDrop::drop(&mut ex.bstrHelpFile);
    }
    let msg = if desc.is_empty() { e.message() } else { desc };
    OfficeError::Document(format!("{name}: 0x{:08X} {msg}", e.code().0))
}

impl Disp {
    fn id(&self, name: &str) -> Result<i32, OfficeError> {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let names = [PCWSTR(wide.as_ptr())];
        let mut id = 0i32;
        // SAFETY: tablica jednej nazwy zakończonej zerem i wynik żyją do końca wywołania.
        unsafe {
            self.0
                .GetIDsOfNames(&GUID::zeroed(), names.as_ptr(), 1, LCID, &raw mut id)
        }
        .map_err(|e| OfficeError::Document(format!("{name}: brak członka ({})", e.message())))?;
        Ok(id)
    }

    fn invoke(
        &self,
        name: &str,
        flags: DISPATCH_FLAGS,
        mut args: Vec<VARIANT>,
    ) -> Result<VARIANT, OfficeError> {
        let id = self.id(name)?;
        args.reverse();
        let mut named = DISPID_PROPERTYPUT;
        let put = flags == DISPATCH_PROPERTYPUT;
        let params = DISPPARAMS {
            rgvarg: if args.is_empty() {
                std::ptr::null_mut()
            } else {
                args.as_mut_ptr()
            },
            rgdispidNamedArgs: if put {
                &raw mut named
            } else {
                std::ptr::null_mut()
            },
            cArgs: u32::try_from(args.len()).unwrap_or(0),
            cNamedArgs: u32::from(put),
        };
        let mut result = VARIANT::default();
        let mut excep = EXCEPINFO::default();
        // SAFETY: argumenty, nazwany argument i wynik żyją do końca wywołania; `args` w odwrotnej
        // kolejności zgodnie z konwencją `IDispatch::Invoke`.
        let r = unsafe {
            self.0.Invoke(
                id,
                &GUID::zeroed(),
                LCID,
                flags,
                &raw const params,
                Some(&raw mut result),
                Some(&raw mut excep),
                None,
            )
        };
        r.map_err(|e| com_err(name, &e, excep))?;
        Ok(result)
    }

    /// Właściwość (bez argumentów).
    pub(crate) fn get(&self, name: &str) -> Result<VARIANT, OfficeError> {
        self.invoke(name, DISPATCH_PROPERTYGET, Vec::new())
    }

    /// Właściwość albo metoda z argumentami (`Item(1)`, `Range("A1")`, `Cell(r, c)`).
    pub(crate) fn get_with(&self, name: &str, args: Vec<VARIANT>) -> Result<VARIANT, OfficeError> {
        self.invoke(name, DISPATCH_METHOD | DISPATCH_PROPERTYGET, args)
    }

    /// Metoda.
    pub(crate) fn call(&self, name: &str, args: Vec<VARIANT>) -> Result<VARIANT, OfficeError> {
        self.invoke(name, DISPATCH_METHOD, args)
    }

    /// Zapis właściwości.
    pub(crate) fn put(&self, name: &str, value: VARIANT) -> Result<(), OfficeError> {
        self.invoke(name, DISPATCH_PROPERTYPUT, vec![value])
            .map(|_| ())
    }

    /// Obiekt z właściwości/metody.
    pub(crate) fn obj(&self, name: &str, args: Vec<VARIANT>) -> Result<Disp, OfficeError> {
        let v = self.get_with(name, args)?;
        IDispatch::try_from(&v)
            .map(Disp)
            .map_err(|_| OfficeError::Document(format!("{name}: brak obiektu")))
    }

    /// Liczba całkowita z właściwości.
    pub(crate) fn int(&self, name: &str) -> Result<i32, OfficeError> {
        let v = self.get(name)?;
        i32::try_from(&v).map_err(|_| OfficeError::Document(format!("{name}: nie liczba")))
    }

    /// Napis z właściwości.
    pub(crate) fn string(&self, name: &str) -> Result<String, OfficeError> {
        Ok(variant_text(&self.get(name)?))
    }
}

/// Tekst z `VARIANT` (pusty dla braku/innego typu).
pub(crate) fn variant_text(v: &VARIANT) -> String {
    BSTR::try_from(v).map(|b| b.to_string()).unwrap_or_default()
}

/// Kod błędu komórki Excela (`CVErr`) → tekst.
fn excel_error(scode: i32) -> String {
    match scode & 0xFFFF {
        2000 => "#NULL!",
        2007 => "#DIV/0!",
        2015 => "#VALUE!",
        2023 => "#REF!",
        2029 => "#NAME?",
        2036 => "#NUM!",
        2042 => "#N/A",
        _ => "#ERROR",
    }
    .to_owned()
}

/// Wartość skalarna komórki.
pub(crate) fn cell_value(v: &VARIANT) -> CellValue {
    let vt = v.vt();
    if vt == VT_EMPTY || vt == VT_NULL {
        CellValue::Empty
    } else if vt == VT_BSTR {
        CellValue::Text(variant_text(v))
    } else if vt == VT_BOOL {
        CellValue::Bool(bool::try_from(v).unwrap_or(false))
    } else if vt == VT_ERROR {
        // SAFETY: `vt == VT_ERROR` — aktywne pole unii to `scode`.
        let code = unsafe { v.Anonymous.Anonymous.Anonymous.scode };
        CellValue::Error {
            error: excel_error(code),
        }
    } else {
        f64::try_from(v).map_or(CellValue::Empty, CellValue::Number)
    }
}

/// Zakres Excela (`Value2`/`Formula`): `SAFEARRAY` 2D od 1 albo skalar → wiersze × kolumny.
pub(crate) fn cell_grid(v: &VARIANT) -> Result<Vec<Vec<CellValue>>, OfficeError> {
    if v.vt().0 != VT_ARRAY.0 | VT_VARIANT.0 {
        return Ok(vec![vec![cell_value(v)]]);
    }
    // SAFETY: `vt == VT_ARRAY | VT_VARIANT` — aktywne pole unii to `parray`.
    let psa: *const SAFEARRAY = unsafe { v.Anonymous.Anonymous.Anonymous.parray };
    let bad = || OfficeError::Document("nieoczekiwany kształt zakresu".into());
    // SAFETY: tablica należy do `v` (żyje do końca funkcji); tylko odczyt wymiarów i elementów.
    unsafe {
        if psa.is_null() || SafeArrayGetDim(psa) != 2 {
            return Err(bad());
        }
        let (r1, r2) = (SafeArrayGetLBound(psa, 1), SafeArrayGetUBound(psa, 1));
        let (c1, c2) = (SafeArrayGetLBound(psa, 2), SafeArrayGetUBound(psa, 2));
        let (r1, r2, c1, c2) = (
            r1.map_err(|_| bad())?,
            r2.map_err(|_| bad())?,
            c1.map_err(|_| bad())?,
            c2.map_err(|_| bad())?,
        );
        let mut rows = Vec::new();
        for r in r1..=r2 {
            let mut row = Vec::new();
            for c in c1..=c2 {
                let idx = [r, c];
                let mut el = VARIANT::default();
                SafeArrayGetElement(psa, idx.as_ptr(), (&raw mut el).cast()).map_err(|_| bad())?;
                row.push(cell_value(&el));
            }
            rows.push(row);
        }
        Ok(rows)
    }
}

/// Aplikacja Office z dowodem wyłączenia makr i zapamiętanymi ustawieniami instancji użytkownika.
pub(crate) struct OfficeApplication {
    pub(crate) app: Disp,
    pub(crate) shared: bool,
    previous_security: Option<i32>,
    kind: OfficeApp,
}

impl OfficeApplication {
    /// `CoCreateInstance` (serwer lokalny) → wykrycie instancji użytkownika → `AutomationSecurity
    /// = 3` z odczytem kontrolnym.
    pub(crate) fn start(kind: OfficeApp) -> Result<Self, OfficeError> {
        let id: Vec<u16> = kind.prog_id().encode_utf16().chain(Some(0)).collect();
        let not_installed = |e: windows::core::Error| {
            OfficeError::NotInstalled(format!("{}: {}", kind.prog_id(), e.message()))
        };
        // SAFETY: ProgID zakończony zerem; tworzenie obiektu w zainicjalizowanym STA tego wątku.
        let app: IDispatch = unsafe {
            let clsid = CLSIDFromProgID(PCWSTR(id.as_ptr())).map_err(not_installed)?;
            CoCreateInstance(&clsid, None, CLSCTX_LOCAL_SERVER).map_err(not_installed)?
        };
        let app = Disp(app);
        let docs = match kind {
            OfficeApp::Word => "Documents",
            OfficeApp::Excel => "Workbooks",
        };
        let visible = app
            .get("Visible")
            .ok()
            .and_then(|v| bool::try_from(&v).ok())
            .unwrap_or(true);
        let open_docs = app.obj(docs, Vec::new())?.int("Count")?;
        let shared = visible || open_docs > 0;
        let previous_security = if shared {
            Some(app.int("AutomationSecurity")?)
        } else {
            None
        };
        let me = Self {
            app,
            shared,
            previous_security,
            kind,
        };
        me.app.put(
            "AutomationSecurity",
            VARIANT::from(AUTOMATION_SECURITY_FORCE_DISABLE),
        )?;
        if me.app.int("AutomationSecurity")? != AUTOMATION_SECURITY_FORCE_DISABLE {
            return Err(OfficeError::Policy(
                "nie udało się wyłączyć makr (AutomationSecurity)".into(),
            ));
        }
        Ok(me)
    }

    /// Stan sesji do wyniku (ponowny odczyt `AutomationSecurity`).
    pub(crate) fn session(&self, protected_view: bool, macros_present: bool) -> OfficeSession {
        OfficeSession {
            automation_security: self.app.int("AutomationSecurity").unwrap_or(-1),
            protected_view,
            macros_present,
            shared_instance: self.shared,
        }
    }

    /// Koniec: własna instancja → `Quit`; instancja użytkownika → przywrócenie ustawienia.
    pub(crate) fn finish(self) {
        if self.shared {
            if let Some(prev) = self.previous_security {
                let _ = self.app.put("AutomationSecurity", VARIANT::from(prev));
            }
            return;
        }
        let args = match self.kind {
            // `Quit(SaveChanges = wdDoNotSaveChanges)`.
            OfficeApp::Word => vec![VARIANT::from(0i32)],
            OfficeApp::Excel => Vec::new(),
        };
        let _ = self.app.call("Quit", args);
    }
}
