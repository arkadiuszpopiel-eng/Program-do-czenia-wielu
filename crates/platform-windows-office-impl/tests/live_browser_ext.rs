//! Przeglądarka Alfy C6.4 na prawdziwym Edge (bez okna, osobny profil) i lokalnym serwerze HTTP:
//! lista wyboru, kliknięcie, czekanie na tekst, konsola i wyjątek skryptu, przewijanie, klawisz,
//! historia karty. Ręcznie: `cargo test -p platform-windows-office-impl --test live_browser_ext
//! -- --ignored`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

#[cfg(windows)]
mod live {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use platform_apps_contract::{
        BrowserKey, BrowserKind, BrowserPort, BrowserSpec, ConsoleLevel, EgressFilter, HistoryNav,
    };
    use platform_windows_office_impl::{BrowserConfig, CdpBrowser};

    const PAGE_A: &str = "<!doctype html><html lang=pl><head><meta charset=utf-8><title>Test Alfy</title></head><body>\
<label>Kraj <select id=k><option value=pl>Polska</option><option value=de>Niemcy</option></select></label>\
<button onclick=\"document.getElementById('out').textContent='Wybrano '+document.getElementById('k').value\">Pokaż</button>\
<p id=out></p><div style='height:3000px'></div><p>Koniec strony</p>\
<script>console.log('start testu');console.error('celowy błąd');\
setTimeout(function(){document.body.insertAdjacentHTML('beforeend','<p>Gotowe po chwili</p>')},600);\
setTimeout(function(){throw new Error('wyjątek testowy')},100);</script></body></html>";
    const PAGE_B: &str = "<!doctype html><html><head><meta charset=utf-8><title>Druga</title></head><body><p>Druga strona</p></body></html>";

    struct Local;
    impl EgressFilter for Local {
        fn allows(&self, host: &str) -> bool {
            host == "127.0.0.1"
        }
    }

    fn serve() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                let _ = reader.read_line(&mut line);
                let mut header = String::new();
                while reader.read_line(&mut header).is_ok_and(|n| n > 2) {
                    header.clear();
                }
                let (status, body) = if line.starts_with("GET /a") {
                    ("200 OK", PAGE_A)
                } else if line.starts_with("GET /b") {
                    ("200 OK", PAGE_B)
                } else {
                    ("404 Not Found", "")
                };
                let mut s = stream;
                let _ = write!(
                    s,
                    "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        port
    }

    fn wait_for(b: &CdpBrowser, id: platform_apps_contract::BrowserSessionId, text: &str) -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if b.has_text(id, text).unwrap() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        false
    }

    #[test]
    #[ignore = "wymaga Windows z Microsoft Edge"]
    fn edge_page_operations_for_testing_pages() {
        let port = serve();
        let root = std::env::temp_dir().join(format!("alfa-browser-ext-{}", std::process::id()));
        let spec = BrowserSpec {
            kind: BrowserKind::Edge,
            executable: None,
            alfa_root: root.clone(),
            profile_dir: root.join("profile"),
            quarantine_dir: root.join("quarantine"),
            headless: true,
        };
        let b = CdpBrowser::system(BrowserConfig::default());
        let id = b.open(&spec, Arc::new(Local)).unwrap();
        let a = format!("http://127.0.0.1:{port}/a");
        assert_eq!(b.navigate(id, &a).unwrap().title, "Test Alfy");
        let snap = b.snapshot(id, 200, 5_000).unwrap();
        let select = snap
            .nodes
            .iter()
            .find(|n| n.role == "combobox")
            .expect("lista wyboru");
        let button = snap
            .nodes
            .iter()
            .find(|n| n.role == "button" && n.name == "Pokaż")
            .expect("przycisk");
        assert!(b.select_option(id, select.node, "zła opcja").is_err());
        b.select_option(id, select.node, "Niemcy").unwrap();
        b.click(id, button.node).unwrap();
        assert!(wait_for(&b, id, "Wybrano de"), "wybór z listy zadziałał");
        assert!(wait_for(&b, id, "gotowe po chwili"), "czekanie na tekst");
        let console = b.console(id, 100).unwrap();
        assert!(
            console
                .iter()
                .any(|e| e.level == ConsoleLevel::Error && e.text.contains("celowy błąd")),
            "{console:?}"
        );
        assert!(
            console
                .iter()
                .any(|e| e.source == "exception" && e.text.contains("wyjątek testowy")),
            "{console:?}"
        );
        assert!(b.console(id, 100).unwrap().is_empty(), "odczyt opróżnia");
        b.scroll(id, None, 2_000).unwrap();
        b.press_key(id, BrowserKey::Tab).unwrap();
        let second = format!("http://127.0.0.1:{port}/b");
        assert_eq!(b.navigate(id, &second).unwrap().title, "Druga");
        assert!(b.history(id, HistoryNav::Back).unwrap().url.ends_with("/a"));
        assert!(
            b.history(id, HistoryNav::Forward)
                .unwrap()
                .url
                .ends_with("/b")
        );
        assert!(
            b.history(id, HistoryNav::Reload)
                .unwrap()
                .url
                .ends_with("/b")
        );
        b.close(id).unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }
}
