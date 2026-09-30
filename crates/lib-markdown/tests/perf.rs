//! Budżet: 100 KB Markdownu < 20 ms w release (ADR 0009, PLAN §14.7).
//! Uruchomienie: `cargo test -p lib-markdown --release --test perf -- --ignored --nocapture`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use lib_markdown::{IncrementalRenderer, RenderOptions, render};

/// Budżet renderu całości.
const BUDGET: Duration = Duration::from_millis(20);

/// Realistyczna odpowiedź LLM (~100 KB): nagłówki, akapity, listy, kod, tabele, linki.
fn sample(target_bytes: usize) -> String {
    let section = "## Sekcja\n\nSprawdziłam konfigurację i **zmieniłam** trzy rzeczy w `config.toml`; \
szczegóły w [dokumentacji](https://docs.example.com/a) oraz https://example.org/x.\n\n\
- pierwsza zmiana z _uzasadnieniem_\n- druga zmiana ~~stara~~ nowa\n  - zagnieżdżony punkt\n\
- [x] przetestowane\n\n```rust\nfn main() {\n    let x = vec![1, 2, 3];\n    println!(\"{x:?}\");\n}\n```\n\n\
| Plik | Zmiana | Rozmiar |\n|:--|:-:|--:|\n| a.rs | dodano | 12 KB |\n| b.rs | usunięto | 3 KB |\n\n\
> Uwaga: zażółć gęślą jaźń <b>surowy html</b> zostaje tekstem.\n\n";
    let mut md = String::with_capacity(target_bytes + section.len());
    while md.len() < target_bytes {
        md.push_str(section);
    }
    md
}

fn best_of(runs: usize, mut f: impl FnMut()) -> Duration {
    (0..runs)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed()
        })
        .min()
        .unwrap_or_default()
}

#[test]
#[ignore = "pomiar wydajności; uruchamiać w release"]
fn render_100_kb_under_budget() {
    let md = sample(100 * 1024);
    let html_len = render(&md).len();
    let whole = best_of(10, || {
        std::hint::black_box(render(std::hint::black_box(&md)));
    });
    let stream = best_of(3, || {
        let mut renderer = IncrementalRenderer::new(RenderOptions::default());
        let chars: Vec<char> = md.chars().collect();
        for delta in chars.chunks(64) {
            let delta: String = delta.iter().collect();
            std::hint::black_box(renderer.push(&delta));
        }
        std::hint::black_box(renderer.finish());
    });
    eprintln!(
        "lib-markdown: {} B Markdown → {} B HTML: render {:?} (budżet {:?}); strumień po 64 znaki: {:?}",
        md.len(),
        html_len,
        whole,
        BUDGET,
        stream
    );
    if !cfg!(debug_assertions) {
        assert!(whole < BUDGET, "render 100 KB: {whole:?} ≥ {BUDGET:?}");
    }
}
