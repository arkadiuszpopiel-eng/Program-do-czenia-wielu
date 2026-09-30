//! Renderowanie przyrostowe: wynik końcowy = render całości; zamknięte bloki niezmienne.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use lib_markdown::{Block, IncrementalRenderer, RenderOptions, render_blocks, render_with};
use proptest::prelude::*;

fn fragment() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "# Nagłówek\n",
        "## Pod\n",
        "Zrobiłam to. ",
        "zażółć gęślą jaźń\n",
        "\n",
        "\n\n",
        "- punkt\n",
        "  - zagnieżdżony\n",
        "1. raz\n",
        "2. dwa\n",
        "> cytat\n",
        "```\n",
        "```rust\n",
        "fn main() {}\n",
        "    wcięty kod\n",
        "| a | b |\n",
        "|---|:-:|\n",
        "| 1 | 2 |\n",
        "**gruby** ",
        "_kursywa_ ",
        "~~skreślone~~ ",
        "`kod` ",
        "[link](https://a.pl) ",
        "https://b.pl/x ",
        "www.c.pl ",
        "<div>\n",
        "</div>\n",
        "<script>alert(1)</script>\n",
        "---\n",
        "===\n",
        "- [ ] zadanie\n",
        "- [x] gotowe\n",
        "\t",
        "\r\n",
        "<!-- k\n",
        "-->\n",
        "![o](https://a.pl/o.png) ",
        "\\",
        "*",
        "_",
        "|",
        "`",
        "[",
        "]",
        "(",
        ")",
        " ",
    ])
    .prop_map(str::to_owned)
}

/// Dzieli tekst na delty w granicach znaków (UTF-8) wg losowych długości.
fn chunk(text: &str, sizes: &[usize]) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut pos = 0;
    let mut i = 0;
    while pos < chars.len() {
        let size = sizes
            .get(i % sizes.len().max(1))
            .copied()
            .unwrap_or(1)
            .max(1);
        let end = (pos + size).min(chars.len());
        out.push(chars[pos..end].iter().collect());
        pos = end;
        i += 1;
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn stream_equals_whole_and_closed_blocks_are_stable(
        parts in prop::collection::vec(fragment(), 0..40),
        sizes in prop::collection::vec(1usize..24, 1..8),
    ) {
        let md: String = parts.concat();
        let opts = RenderOptions::default();
        let mut renderer = IncrementalRenderer::new(opts);
        let mut closed: Vec<Block> = Vec::new();
        let mut buffer = String::new();
        for delta in chunk(&md, &sizes) {
            buffer.push_str(&delta);
            let update = renderer.push(&delta);
            closed.extend(update.closed);
            // Zamknięte dotąd bloki są prefiksem renderu bieżącego bufora (nie zmieniły się),
            // a otwarty blok to złożenie pozostałych (co najwyżej 2: niedokończona linia).
            let now = render_blocks(&buffer, opts);
            prop_assert!(now.len() >= closed.len());
            prop_assert_eq!(&now[..closed.len()], &closed[..]);
            let rest = &now[closed.len()..];
            prop_assert!(rest.len() <= 2, "otwartych bloków: {}", rest.len());
            let expected_open: String = rest.iter().map(|b| b.html.as_str()).collect();
            prop_assert_eq!(update.open.map(|b| b.html), (!rest.is_empty()).then_some(expected_open));
            prop_assert_eq!(renderer.closed_blocks(), &closed[..]);
        }
        prop_assert_eq!(renderer.source(), md.as_str());
        let last = renderer.finish();
        prop_assert_eq!(last.open, None);
        closed.extend(last.closed);
        let ids: Vec<u64> = closed.iter().map(|b| b.id).collect();
        prop_assert_eq!(ids, (0..closed.len() as u64).collect::<Vec<_>>());
        let streamed: String = closed.iter().map(|b| b.html.as_str()).collect();
        prop_assert_eq!(streamed, render_with(&md, opts));
    }
}

#[test]
fn typical_llm_stream() {
    let md = "Sprawdziłam pliki.\n\n```python\nprint('x')\n```\n\n| k | v |\n|---|---|\n| a | 1 |\n\nGotowe.";
    let mut renderer = IncrementalRenderer::new(RenderOptions::default());
    let mut closed = Vec::new();
    let mut opens = 0;
    for ch in md.chars() {
        let update = renderer.push(&ch.to_string());
        closed.extend(update.closed);
        opens += usize::from(update.open.is_some());
    }
    // Akapit i kod zamknięte w trakcie; tabela czeka, aż linia „Gotowe.” będzie pełna.
    assert_eq!(closed.len(), 2);
    assert_eq!(closed[1].code[0].lang.as_deref(), Some("python"));
    assert!(opens > 0);
    let tail = renderer.finish();
    assert_eq!(tail.closed.len(), 2);
    assert!(tail.closed[0].html.starts_with("<table>"));
    assert_eq!(tail.closed[1].html, "<p>Gotowe.</p>\n");
    assert_eq!(tail.closed[1].id, 3);
}

#[test]
fn empty_and_whitespace_stream() {
    let mut renderer = IncrementalRenderer::new(RenderOptions::default());
    assert_eq!(renderer.push(""), Default::default());
    assert_eq!(renderer.push("\n\n  \n"), Default::default());
    assert!(renderer.finish().closed.is_empty());
}
