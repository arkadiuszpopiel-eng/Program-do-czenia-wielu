//! Artefakty HTML/SVG do podglądu w kanwie (C2.8, ADR 0009): dokument dla izolowanej ramki.
//!
//! Ramka kanwy ma `sandbox` bez `allow-scripts` i bez `allow-same-origin`, a dokument dziedziczy
//! CSP okna — to ona jest główną kontrolą pobrań z sieci. Druga warstwa jest tutaj: `ammonia`
//! z szeroką białą listą znaczników układu i rysunku SVG, ale bez skryptów, formularzy, ramek,
//! obiektów i handlerów zdarzeń. Adresy zostają tylko jako kotwice `#…` i obrazy
//! `data:image/(png|jpeg|webp)`. W CSS (atrybut `style`, atrybuty prezentacji SVG, treść
//! `<style>`) pobrania spoza dokumentu (`url(…)`, `@import`, `image-set(`, `src(`) są
//! neutralizowane, a wartości z ucieczkami CSS (np. `\75rl(`) odrzucane; tekst dokumentu zostaje
//! bez zmian. Wynik to pełny dokument dla `srcdoc` — UI tylko go wstawia.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use ammonia::{Builder, UrlRelative};

use crate::url::is_safe_data_image;

/// Największy artefakt przyjmowany do kanwy (bajty UTF-8 źródła).
pub const ARTIFACT_MAX_BYTES: usize = 512 * 1024;

/// Znaczniki HTML dozwolone w artefakcie (układ, tekst, tabele, style; bez formularzy i ramek).
const HTML_TAGS: &str = "a abbr address article aside b bdi bdo blockquote br caption cite \
    code col colgroup data dd del details dfn div dl dt em figcaption figure footer h1 h2 h3 h4 \
    h5 h6 header hgroup hr i img ins kbd li main mark meter nav ol p pre progress q rp rt ruby s \
    samp section small span strong style sub summary sup table tbody td tfoot th thead time tr \
    u ul var wbr";

/// Znaczniki SVG (nazwy po korekcie wielkości liter przez parser HTML).
const SVG_TAGS: &str = "svg g defs symbol use desc circle ellipse line path polygon polyline \
    rect text tspan linearGradient radialGradient stop clipPath mask pattern marker filter \
    feBlend feColorMatrix feComposite feFlood feGaussianBlur feMerge feMergeNode feOffset \
    feDropShadow";

/// Atrybuty dozwolone na każdym znaczniku (prezentacja HTML i SVG; bez `on*`).
const GENERIC_ATTRS: &str = "class id style title lang dir role aria-label aria-hidden \
    aria-describedby aria-labelledby width height align colspan rowspan scope span start \
    reversed datetime open value min max low high optimum alt src href xlink:href viewBox \
    preserveAspectRatio d points x y x1 y1 x2 y2 cx cy r rx ry dx dy fill fill-opacity \
    fill-rule clip-rule stroke stroke-width stroke-linecap stroke-linejoin stroke-dasharray \
    stroke-dashoffset stroke-opacity stroke-miterlimit opacity transform font-size font-family \
    font-weight font-style text-anchor dominant-baseline letter-spacing offset stop-color \
    stop-opacity gradientUnits gradientTransform spreadMethod fx fy clip-path clipPathUnits \
    mask maskUnits filter filterUnits marker-start marker-mid marker-end markerWidth \
    markerHeight markerUnits refX refY orient patternUnits patternTransform stdDeviation in in2 \
    result mode values type operator k1 k2 k3 k4 flood-color flood-opacity vector-effect \
    paint-order pathLength textLength lengthAdjust";

/// Atrybuty będące zwykłym tekstem, nie wartością CSS (ukośnik wsteczny i `url(` dozwolone).
const TEXT_ATTRS: &str = "class id title lang dir role alt aria-label aria-describedby \
    aria-labelledby datetime value";

/// Znaczniki usuwane razem z treścią.
const CLEAN_CONTENT_TAGS: &str = "script iframe object embed noscript template textarea title \
    xmp noembed noframes plaintext math select foreignObject frameset frame";

/// Styl bazowy dokumentu ramki: kolory systemu, czytelna typografia, rysunek mieści się w ramce.
const BASE_STYLE: &str = "\
:root{color-scheme:light dark}\
body{margin:0;padding:16px;font:15px/1.5 system-ui,'Segoe UI',sans-serif;overflow-wrap:anywhere}\
img,svg{max-width:100%;height:auto}\
table{border-collapse:collapse}";

/// Konstrukcje CSS, które pobierają zasób (oprócz `url(…)`), i ich zneutralizowana postać.
const CSS_FETCHES: [(&str, &str); 3] = [
    ("@import", "@blocked"),
    ("image-set(", "blocked-set("),
    ("src(", "blocked-src("),
];

/// Neutralizuje odwołania do zasobów spoza dokumentu w tekście CSS: `url(…)` poza kotwicą `#…`,
/// `@import`, `image-set(`, `src(`. Przyjmuje sam CSS — nie treść dokumentu.
pub fn neutralize_css_fetches(css: &str) -> String {
    let lower = css.to_ascii_lowercase();
    let mut out = String::with_capacity(css.len());
    let mut i = 0;
    'scan: while i < css.len() {
        let rest = &lower[i..];
        if rest.starts_with("url(") && !local_ref(&rest[4..]) {
            out.push_str("blocked(");
            i += 4;
            continue;
        }
        for (needle, replacement) in CSS_FETCHES {
            if rest.starts_with(needle) {
                out.push_str(replacement);
                i += needle.len();
                continue 'scan;
            }
        }
        let ch = css[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8().max(1);
    }
    out
}

/// Czy wartość CSS (małymi literami) pobiera coś spoza dokumentu.
fn fetches(lower: &str) -> bool {
    CSS_FETCHES.iter().any(|(needle, _)| lower.contains(needle))
        || lower
            .match_indices("url(")
            .any(|(at, _)| !local_ref(&lower[at + 4..]))
}

/// Tekst arkusza `<style>`. Z ucieczką CSS (`\75rl(`, `@\69mport`) — odrzucony w całości, bo
/// ucieczki omijają każde porównanie tekstu; w pozostałych pobrania są zneutralizowane.
fn clean_stylesheet(css: &str) -> String {
    if css.contains('\\') {
        return String::new();
    }
    neutralize_css_fetches(css)
}

/// `url(` z kotwicą w tym samym dokumencie (`url(#gradient)`, `url('#a')`).
fn local_ref(after: &str) -> bool {
    after
        .trim_start_matches([' ', '\t', '\n', '"', '\''])
        .starts_with('#')
}

/// Wartość atrybutu po białej liście nazw.
fn filter_attr<'u>(element: &str, attribute: &str, value: &'u str) -> Option<Cow<'u, str>> {
    let keep = match attribute {
        "src" => element == "img" && is_safe_data_image(value.trim()),
        "href" | "xlink:href" => value.trim_start().starts_with('#'),
        _ if TEXT_ATTRS.split_whitespace().any(|a| a == attribute) => true,
        // `style` i atrybuty prezentacji SVG są parsowane jako CSS: bez ucieczek i bez pobrań.
        _ => {
            let lower = value.to_ascii_lowercase();
            !lower.contains('\\')
                && !lower.contains("javascript:")
                && !lower.contains("expression(")
                && !fetches(&lower)
        }
    };
    keep.then_some(Cow::Borrowed(value))
}

/// Względne adresy: wyłącznie kotwice w tym samym dokumencie.
fn anchor_only(url: &str) -> Option<Cow<'_, str>> {
    url.starts_with('#').then_some(Cow::Borrowed(url))
}

/// Długość znacznika zaczynającego się od `<` (do `>` poza cudzysłowami włącznie).
fn tag_len(tag: &str) -> usize {
    let mut quote = None;
    for (i, c) in tag.char_indices().skip(1) {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '>' => return i + 1,
            None => {}
        }
    }
    tag.len()
}

fn is_style_open(tag: &str) -> bool {
    let bytes = tag.as_bytes();
    bytes.len() > 6
        && bytes[..6].eq_ignore_ascii_case(b"<style")
        && matches!(bytes[6], b'>' | b' ' | b'\t' | b'\n' | b'/')
}

/// Czyści treść elementów `<style>` (HTML i SVG) w wyniku `ammonia`. Serializacja zamienia `<`
/// w tekście na encję, więc każdy `<` to znacznik, a arkusz kończy się na pierwszym `</style`.
/// Tekst z encjami (`u&#114;l(` w SVG) parser już zdekodował — sprawdzamy tekst po dekodowaniu.
fn clean_style_elements(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        rest = &rest[lt..];
        let end = tag_len(rest);
        let tag = &rest[..end];
        out.push_str(tag);
        rest = &rest[end..];
        if is_style_open(tag) {
            let close = rest
                .to_ascii_lowercase()
                .find("</style")
                .unwrap_or(rest.len());
            out.push_str(&clean_stylesheet(&rest[..close]));
            rest = &rest[close..];
        }
    }
    out.push_str(rest);
    out
}

fn builder() -> &'static Builder<'static> {
    static BUILDER: OnceLock<Builder<'static>> = OnceLock::new();
    BUILDER.get_or_init(|| {
        let tags: HashSet<&'static str> = HTML_TAGS
            .split_whitespace()
            .chain(SVG_TAGS.split_whitespace())
            .collect();
        let mut builder = Builder::empty();
        builder
            .tags(tags)
            .clean_content_tags(CLEAN_CONTENT_TAGS.split_whitespace().collect())
            .generic_attributes(GENERIC_ATTRS.split_whitespace().collect())
            .tag_attributes(HashMap::new())
            .url_schemes(["data"].into())
            .url_relative(UrlRelative::Custom(Box::new(anchor_only)))
            .link_rel(Some("noopener noreferrer"))
            .strip_comments(true)
            .attribute_filter(filter_attr);
        builder
    })
}

/// Sanitizuje źródło artefaktu HTML albo SVG (fragment albo cały dokument — `<html>`, `<head>`
/// i `<body>` parser pomija, zostaje ich treść).
pub fn sanitize_artifact(source: &str) -> String {
    clean_style_elements(&builder().clean(source).to_string())
}

/// Znacznik języka w bezpiecznej postaci BCP 47 (litery, cyfry, `-`; do 35 znaków).
fn valid_lang(lang: &str) -> bool {
    !lang.is_empty()
        && lang.len() <= 35
        && lang.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Język z `<html lang="…">` artefaktu, jeśli jest i ma poprawną postać.
fn own_lang(source: &str) -> Option<&str> {
    let at = source.to_ascii_lowercase().find("<html")?;
    let tag = &source[at..at + tag_len(&source[at..])];
    let pos = tag.to_ascii_lowercase().find(" lang=")?;
    let value = tag[pos + 6..].trim_start_matches(['"', '\'']);
    let lang = value.split(['"', '\'', ' ', '>', '/']).next()?;
    valid_lang(lang).then_some(lang)
}

/// Pełny dokument dla `srcdoc` izolowanej ramki kanwy. Język: z `<html lang>` artefaktu, bez
/// niego — `ui_lang` (język interfejsu); znacznik o niepoprawnej postaci zastępuje `pl`.
pub fn artifact_document(source: &str, ui_lang: &str) -> String {
    let fallback = if valid_lang(ui_lang) { ui_lang } else { "pl" };
    let lang = own_lang(source).unwrap_or(fallback);
    format!(
        "<!doctype html><html lang=\"{lang}\"><head><meta charset=\"utf-8\">\
         <style>{BASE_STYLE}</style></head><body>{}</body></html>",
        sanitize_artifact(source)
    )
}
