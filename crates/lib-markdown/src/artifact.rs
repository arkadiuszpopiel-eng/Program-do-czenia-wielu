//! Artefakty HTML/SVG do podglądu w kanwie (C2.8, ADR 0009): dokument dla izolowanej ramki.
//!
//! Ramka kanwy ma `sandbox` bez `allow-scripts` i bez `allow-same-origin`, a dokument dziedziczy
//! CSP okna. To pierwsza warstwa. Druga jest tutaj: `ammonia` z szeroką białą listą znaczników
//! układu i rysunku SVG, ale bez skryptów, formularzy, ramek, obiektów i handlerów zdarzeń.
//! Adresy zostają tylko jako kotwice `#…` i obrazy `data:image/(png|jpeg|webp)`; odwołania
//! `url(…)` do zasobów spoza dokumentu i `@import` w stylach są neutralizowane (wyciek danych
//! przez pobranie obrazka). Wynik to pełny dokument dla `srcdoc` — UI tylko go wstawia.

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

/// Znaczniki usuwane razem z treścią.
const CLEAN_CONTENT_TAGS: &str = "script iframe object embed noscript template textarea title \
    xmp noembed noframes plaintext math select foreignObject frameset frame";

/// Styl bazowy dokumentu ramki: kolory systemu, czytelna typografia, rysunek mieści się w ramce.
const BASE_STYLE: &str = "\
:root{color-scheme:light dark}\
body{margin:0;padding:16px;font:15px/1.5 system-ui,'Segoe UI',sans-serif;overflow-wrap:anywhere}\
img,svg{max-width:100%;height:auto}\
table{border-collapse:collapse}";

/// Neutralizuje odwołania do zasobów spoza dokumentu w CSS (`url(…)` poza kotwicą `#…`,
/// `@import`, `image-set(`). Działa na całym źródle — także na treści elementów `<style>`, której
/// `ammonia` nie filtruje.
pub fn neutralize_css_fetches(source: &str) -> String {
    let lower = source.to_ascii_lowercase();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < source.len() {
        let rest = &lower[i..];
        if rest.starts_with("url(") && !local_ref(&rest[4..]) {
            out.push_str("blocked(");
            i += 4;
        } else if rest.starts_with("@import") {
            out.push_str("@blocked");
            i += 7;
        } else if rest.starts_with("image-set(") {
            out.push_str("blocked-set(");
            i += 10;
        } else {
            let ch = source[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8().max(1);
        }
    }
    out
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
        _ => {
            let lower = value.to_ascii_lowercase();
            !lower.contains("javascript:")
                && !lower.contains("expression(")
                && lower
                    .match_indices("url(")
                    .all(|(at, _)| local_ref(&lower[at + 4..]))
        }
    };
    keep.then_some(Cow::Borrowed(value))
}

/// Względne adresy: wyłącznie kotwice w tym samym dokumencie.
fn anchor_only(url: &str) -> Option<Cow<'_, str>> {
    url.starts_with('#').then_some(Cow::Borrowed(url))
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
    builder().clean(&neutralize_css_fetches(source)).to_string()
}

/// Pełny dokument dla `srcdoc` izolowanej ramki kanwy.
pub fn artifact_document(source: &str) -> String {
    format!(
        "<!doctype html><html lang=\"pl\"><head><meta charset=\"utf-8\">\
         <style>{BASE_STYLE}</style></head><body>{}</body></html>",
        sanitize_artifact(source)
    )
}
