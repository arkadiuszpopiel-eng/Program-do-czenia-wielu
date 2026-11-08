//! Zestaw XSS (OWASP XSS Filter Evasion Cheat Sheet + wektory specyficzne dla Markdownu).
//! Kryterium: 0 przejść — każdy wynik spełnia wyrocznię `assert_safe` (biała lista znaczników,
//! atrybutów i schematów URL; brak surowego `<` w tekście).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use lib_markdown::{ALLOWED_TAGS, LINK_REL, RenderOptions, render, render_with};
use proptest::prelude::*;

/// Dozwolone atrybuty per znacznik (lustrzane odbicie polityki sanitizera).
fn allowed_attr(tag: &str, attr: &str) -> bool {
    matches!(
        (tag, attr),
        ("a", "href" | "title" | "rel" | "data-external")
            | ("img", "src" | "alt" | "title")
            | ("ol", "start")
            | ("input", "type" | "checked" | "disabled")
            | ("code", "data-lang")
            | ("pre", "data-lines" | "data-code")
            | ("th" | "td", "data-align")
    )
}

fn parse_attrs(inner: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let mut rest = inner.trim();
    while !rest.is_empty() {
        let name_end = rest.find(['=', ' ']).unwrap_or(rest.len());
        let name = rest[..name_end].to_owned();
        rest = rest[name_end..].trim_start();
        let mut value = String::new();
        if let Some(after) = rest.strip_prefix('=') {
            let after = after.strip_prefix('"').expect("atrybut bez cudzysłowu");
            let close = after.find('"').expect("niedomknięty atrybut");
            value = after[..close].to_owned();
            rest = after[close + 1..].trim_start();
        }
        attrs.push((name, value));
    }
    attrs
}

/// Wyrocznia bezpieczeństwa wyniku (wynik `ammonia` jest znormalizowany: atrybuty w `"…"`).
fn assert_safe(input: &str, html: &str, opts: RenderOptions) {
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        let after = &rest[lt + 1..];
        let gt = after.find('>').expect("niedomknięty znacznik");
        let tag_src = after[..gt].trim_end_matches('/');
        let (closing, body) = tag_src
            .strip_prefix('/')
            .map_or((false, tag_src), |b| (true, b));
        let name_end = body.find(' ').unwrap_or(body.len());
        let name = &body[..name_end];
        assert!(
            ALLOWED_TAGS.contains(&name),
            "niedozwolony znacznik <{tag_src}>\nwejście: {input:?}\nwynik: {html}"
        );
        if !closing {
            let attrs = parse_attrs(&body[name_end..]);
            for (attr, value) in &attrs {
                assert!(
                    allowed_attr(name, attr),
                    "atrybut {attr} na <{name}>\nwejście: {input:?}\nwynik: {html}"
                );
                check_value(name, attr, value, opts, input);
            }
            if name == "a" {
                assert!(attrs.contains(&("rel".into(), LINK_REL.into())), "{html}");
                assert!(attrs.iter().any(|(a, _)| a == "data-external"), "{html}");
            }
            if name == "input" {
                assert!(
                    attrs.contains(&("type".into(), "checkbox".into())),
                    "{html}"
                );
                assert!(attrs.iter().any(|(a, _)| a == "disabled"), "{html}");
            }
        }
        rest = &after[gt + 1..];
    }
}

fn check_value(tag: &str, attr: &str, value: &str, opts: RenderOptions, input: &str) {
    let v = value.to_ascii_lowercase();
    match (tag, attr) {
        ("a", "href") => assert!(
            v.starts_with("https://") || v.starts_with("http://") || v.starts_with("mailto:"),
            "href {value}\nwejście: {input:?}"
        ),
        ("img", "src") => assert!(
            (opts.allow_remote_images && v.starts_with("https://"))
                || (opts.allow_data_images
                    && ["data:image/png;", "data:image/jpeg;", "data:image/webp;"]
                        .iter()
                        .any(|p| v.starts_with(p))),
            "src {value}\nwejście: {input:?}"
        ),
        ("th" | "td", "data-align") => assert!(matches!(value, "left" | "center" | "right")),
        _ => {}
    }
}

/// ≥ 40 wektorów: OWASP cheat sheet + Markdown (linki, obrazy, tabele, zagnieżdżenia).
const VECTORS: &[&str] = &[
    "<script>alert(1)</script>",
    "<SCRIPT SRC=https://xss.example/xss.js></SCRIPT>",
    "<img src=x onerror=alert(1)>",
    "<IMG SRC=JaVaScRiPt:alert('XSS')>",
    "<IMG SRC=\"jav&#x0A;ascript:alert('XSS');\">",
    "<IMG SRC=\"jav&#x09;ascript:alert('XSS');\">",
    "<IMG SRC=&#106;&#97;&#118;&#97;&#115;&#99;&#114;&#105;&#112;&#116;&#58;alert(1)>",
    "<svg><script>alert(1)</script></svg>",
    "<svg onload=alert(1)>",
    "<svg><a xlink:href=\"javascript:alert(1)\"><text>x</text></a></svg>",
    "<math href=\"javascript:alert(1)\">x</math>",
    "<a href=\"javascript:alert(1)\">x</a>",
    "<a href=\"jav&#x09;ascript:alert(1)\">x</a>",
    "<a href=\"&#14;javascript:alert(1)\">x</a>",
    "<a href=javascript&#58;alert(1)>x</a>",
    "<a href=\"https://ok.pl\" onclick=\"alert(1)\">x</a>",
    "[x](javascript:alert(1))",
    "[x](JaVaScRiPt:alert(1))",
    "[x](jav&#x09;ascript:alert(1))",
    "[x](jav&#x0A;ascript:alert(1))",
    "[x](&#106;avascript:alert(1))",
    "[x](&#x6A;avascript:alert(1))",
    "[x](javascript&colon;alert(1))",
    "[x](%6Aavascript:alert(1))",
    "[x](<javascript:alert(1)>)",
    "[x](  javascript:alert(1)  \"t\")",
    "[x](java\u{0}script:alert(1))",
    "[x](vbscript:msgbox(1))",
    "[x](data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==)",
    "[x](javascript:alert(1)//https://ok.pl)",
    "[x](https://ok.pl \"t\\\" onmouseover=\\\"alert(1)\")",
    "[x](https://ok.pl' onclick='alert(1))",
    "<javascript:alert(1)>",
    "![x](javascript:alert(1))",
    "![x](x\" onerror=\"alert(1))",
    "![x](data:image/svg+xml;base64,PHN2ZyBvbmxvYWQ9YWxlcnQoMSk+)",
    "[![i](javascript:alert(1))](javascript:alert(2))",
    "[a]: javascript:alert(1)\n\n[a]",
    "> [a]: javascript:alert(1)\n>\n> [a]",
    "<iframe src=\"javascript:alert(1)\"></iframe>",
    "<object data=\"javascript:alert(1)\"></object>",
    "<embed src=\"javascript:alert(1)\">",
    "<form action=\"javascript:alert(1)\"><input type=submit></form>",
    "<button formaction=javascript:alert(1)>x</button>",
    "<input onfocus=alert(1) autofocus>",
    "<body onload=alert(1)>",
    "<details open ontoggle=alert(1)>",
    "<video><source onerror=alert(1)></video>",
    "<div style=\"background:url(javascript:alert(1))\">x</div>",
    "<style>@import 'javascript:alert(1)';</style>",
    "<base href=\"javascript:alert(1)//\">",
    "<meta http-equiv=\"refresh\" content=\"0;url=javascript:alert(1)\">",
    "<link rel=stylesheet href=\"javascript:alert(1)\">",
    "<math><mtext><table><mglyph><style><img src=x onerror=alert(1)>",
    "<noscript><p title=\"</noscript><img src=x onerror=alert(1)>\">",
    "<!--<img src=\"--><img src=x onerror=alert(1)//\">",
    "<scr<script>ipt>alert(1)</script>",
    "<<script>script>alert(1)<</script>/script>",
    "| a | <img src=x onerror=alert(1)> |\n|---|---|\n| <script>alert(1)</script> | [x](javascript:alert(1)) |",
    "| <a href=\"javascript:alert(1)\">x</a> |\n|:-:|\n| ![i](javascript:alert(1)) |",
    "- [ ] <img src=x onerror=alert(1)>\n- [x] [x](javascript:alert(1))",
    "> <script>alert(1)</script>\n> > [x](javascript:alert(1))",
    "- a\n  - <svg onload=alert(1)>\n    - [x](vbscript:x)",
    "**<a href=\"javascript:alert(1)\">x</a>**",
    "```html\n<script>alert(1)</script>\n```",
    "```\"><script>alert(1)</script>\ncode\n```",
    "`<script>alert(1)</script>`",
    "www.example.com\"onmouseover=\"alert(1)",
    "https://example.com/<script>alert(1)</script>",
    "<a href=\"data:text/html,<script>alert(1)</script>\">x</a>",
    "[x](https://ok.pl)<img src=x onerror=alert(1)>",
    "<img src=\"x\nonerror=alert(1)>",
];

#[test]
fn owasp_and_markdown_vectors_never_pass() {
    assert!(VECTORS.len() >= 40, "za mało wektorów: {}", VECTORS.len());
    let all = RenderOptions {
        allow_data_images: true,
        allow_remote_images: true,
    };
    for vector in VECTORS {
        assert_safe(vector, &render(vector), RenderOptions::default());
        assert_safe(vector, &render_with(vector, all), all);
    }
}

#[test]
fn raw_html_stays_visible_as_text() {
    assert_eq!(
        render("<script>alert(1)</script>"),
        "<p>&lt;script&gt;alert(1)&lt;/script&gt;</p>\n"
    );
    assert_eq!(render("[x](javascript:alert(1))"), "<p>x</p>\n");
    // Autolink o niedozwolonym schemacie: widoczny tekst, bez `<a>`.
    assert_eq!(
        render("<javascript:alert(1)>"),
        "<p>javascript:alert(1)</p>\n"
    );
}

fn hostile() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec![
            "<",
            ">",
            "\"",
            "'",
            "=",
            "&",
            "#",
            ";",
            ":",
            "/",
            "(",
            ")",
            "[",
            "]",
            "!",
            "`",
            "*",
            "\n",
            "\t",
            " ",
            "|",
            "-",
            "script",
            "img",
            "svg",
            "a",
            "href",
            "src",
            "onerror",
            "onload",
            "javascript",
            "java\tscript",
            "&#x09;",
            "&#106;",
            "data",
            "alert(1)",
            "x",
            "https",
            "www.",
            "\u{0}",
            "\u{200b}",
            "style",
            "iframe",
            "math",
            "- [ ] ",
        ]),
        0..60,
    )
    .prop_map(|parts| parts.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]
    /// Losowe wrogie wejścia: wyrocznia zawsze spełniona.
    #[test]
    fn random_hostile_input_is_safe(input in hostile()) {
        assert_safe(&input, &render(&input), RenderOptions::default());
    }
}

/// Wyrocznia sama wykrywa niebezpieczny wynik (kontrola testu).
#[test]
#[should_panic(expected = "niedozwolony znacznik")]
fn oracle_rejects_script_tag() {
    assert_safe("x", "<script>alert(1)</script>", RenderOptions::default());
}

#[test]
#[should_panic(expected = "atrybut onerror")]
fn oracle_rejects_event_handler() {
    assert_safe(
        "x",
        "<p onerror=\"alert(1)\">x</p>",
        RenderOptions::default(),
    );
}

#[test]
#[should_panic(expected = "href javascript")]
fn oracle_rejects_javascript_href() {
    let html =
        format!("<a href=\"javascript:alert(1)\" rel=\"{LINK_REL}\" data-external=\"\">x</a>");
    assert_safe("x", &html, RenderOptions::default());
}
