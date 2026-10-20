//! Artefakty HTML/SVG dla kanwy (C2.8, ADR 0009): sanitizacja przed izolowaną ramką.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use lib_markdown::{artifact_document, neutralize_css_fetches, sanitize_artifact};

#[test]
fn keeps_layout_and_inline_styles() {
    let html = "<section class=\"karta\" style=\"color: red\"><h1>Raport</h1>\
                <table><tr><td colspan=\"2\">a</td></tr></table></section>";
    let clean = sanitize_artifact(html);
    assert!(
        clean.contains("<section class=\"karta\" style=\"color: red\">"),
        "{clean}"
    );
    assert!(clean.contains("<h1>Raport</h1>"));
    assert!(clean.contains("<td colspan=\"2\">a</td>"));
}

#[test]
fn whole_document_keeps_style_block_and_body() {
    let doc = "<!doctype html><html><head><title>T</title><meta http-equiv=\"refresh\" \
               content=\"0;url=https://zly.pl\"><style>h1{color:teal}</style></head>\
               <body><h1>Witaj</h1></body></html>";
    let clean = sanitize_artifact(doc);
    assert!(clean.contains("<style>h1{color:teal}</style>"), "{clean}");
    assert!(clean.contains("<h1>Witaj</h1>"));
    assert!(!clean.contains("refresh"));
    assert!(!clean.contains("<title>"));
}

#[test]
fn strips_scripts_handlers_frames_and_forms() {
    let evil = "<script>alert(1)</script><img src=x onerror=alert(1)>\
                <iframe src=\"https://zly.pl\"></iframe><object data=\"x\"></object>\
                <form action=\"https://zly.pl\"><input name=p></form>\
                <a href=\"javascript:alert(1)\" onclick=\"x\">link</a>\
                <div onmouseover=\"alert(1)\">tekst</div>";
    let clean = sanitize_artifact(evil);
    for bad in [
        "script",
        "onerror",
        "iframe",
        "object",
        "form",
        "input",
        "javascript",
        "onclick",
        "onmouseover",
        "zly.pl",
    ] {
        assert!(!clean.contains(bad), "{bad} w {clean}");
    }
    assert!(clean.contains("tekst"));
    assert!(clean.contains(">link</a>"));
}

#[test]
fn svg_drawing_survives_with_camel_case_names() {
    let svg = "<svg viewBox=\"0 0 100 100\" xmlns=\"http://www.w3.org/2000/svg\">\
               <defs><linearGradient id=\"g\"><stop offset=\"0\" stop-color=\"#f00\"/>\
               </linearGradient></defs>\
               <circle cx=\"50\" cy=\"50\" r=\"40\" fill=\"url(#g)\"/>\
               <use href=\"#g\"/><text x=\"10\" y=\"20\">Alfa</text></svg>";
    let clean = sanitize_artifact(svg);
    assert!(clean.contains("<svg viewBox=\"0 0 100 100\">"), "{clean}");
    assert!(clean.contains("<linearGradient id=\"g\">"), "{clean}");
    assert!(clean.contains("fill=\"url(#g)\""), "{clean}");
    assert!(clean.contains("<use href=\"#g\">"), "{clean}");
    assert!(clean.contains(">Alfa</text>"));
}

#[test]
fn svg_vectors_are_removed() {
    let evil = "<svg onload=\"alert(1)\"><script>alert(1)</script>\
                <a xlink:href=\"javascript:alert(1)\"><text>x</text></a>\
                <foreignObject><iframe src=\"https://zly.pl\"></iframe></foreignObject>\
                <image href=\"https://zly.pl/p.png\"/>\
                <use href=\"https://zly.pl/sprite.svg#a\"/>\
                <animate attributeName=\"href\" to=\"javascript:alert(1)\"/></svg>";
    let clean = sanitize_artifact(evil);
    for bad in [
        "onload",
        "script",
        "javascript",
        "foreignObject",
        "iframe",
        "zly.pl",
        "animate",
    ] {
        assert!(!clean.contains(bad), "{bad} w {clean}");
    }
}

#[test]
fn xml_prolog_of_svg_file_is_dropped() {
    let clean = sanitize_artifact("<?xml version=\"1.0\"?>\n<svg width=\"10\"><rect/></svg>");
    assert!(!clean.contains("xml"), "{clean}");
    assert!(
        clean.contains("<svg width=\"10\"><rect></rect></svg>"),
        "{clean}"
    );
}

#[test]
fn remote_fetches_in_css_are_neutralized() {
    let html = "<style>@import 'https://zly.pl/a.css'; body{background:url(https://zly.pl/x?d=1)}\
                .a{fill:url(#lokalny)}</style>\
                <p style=\"background-image: URL('https://zly.pl/p.png')\">a</p>\
                <p style=\"color: blue\">b</p>";
    let clean = sanitize_artifact(html);
    assert!(!clean.contains("url(https"), "{clean}");
    assert!(!clean.contains("@import"), "{clean}");
    assert!(clean.contains("url(#lokalny)"), "{clean}");
    assert!(
        !clean.to_ascii_lowercase().contains("url('https"),
        "{clean}"
    );
    assert!(clean.contains("<p style=\"color: blue\">b</p>"), "{clean}");
    assert_eq!(
        neutralize_css_fetches("a URL( 'x') url(#y) image-set(z)"),
        "a blocked( 'x') url(#y) blocked-set(z)"
    );
}

#[test]
fn images_only_inline_raster_data() {
    let png = "data:image/png;base64,iVBORw0KGgo=";
    let clean = sanitize_artifact(&format!(
        "<img src=\"{png}\" alt=\"a\"><img src=\"https://zly.pl/p.png\" alt=\"b\">\
         <img src=\"data:image/svg+xml;base64,PHN2Zz4=\" alt=\"c\">"
    ));
    assert!(
        clean.contains(&format!("<img src=\"{png}\" alt=\"a\">")),
        "{clean}"
    );
    assert!(clean.contains("<img alt=\"b\">"), "{clean}");
    assert!(clean.contains("<img alt=\"c\">"), "{clean}");
}

#[test]
fn links_keep_only_in_document_anchors() {
    let clean = sanitize_artifact("<a href=\"#sekcja\">do</a><a href=\"https://a.pl\">z</a>");
    assert!(clean.contains("href=\"#sekcja\""), "{clean}");
    assert!(!clean.contains("a.pl"), "{clean}");
}

#[test]
fn document_wraps_clean_body() {
    let doc = artifact_document("<p>x</p><script>y</script>");
    assert!(doc.starts_with("<!doctype html><html lang=\"pl\"><head><meta charset=\"utf-8\">"));
    assert!(doc.ends_with("<body><p>x</p></body></html>"), "{doc}");
    assert!(!doc.contains("script"));
}

#[test]
fn non_ascii_text_is_preserved() {
    assert_eq!(
        sanitize_artifact("<p>Zażółć gęślą jaźń — 😊</p>"),
        "<p>Zażółć gęślą jaźń — 😊</p>"
    );
    assert_eq!(neutralize_css_fetches("ąurl(#a)ę"), "ąurl(#a)ę");
}
