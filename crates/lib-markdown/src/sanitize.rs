//! Druga warstwa obrony: `ammonia` z białą listą znaczników i atrybutów (ADR 0009).
//!
//! Writer generuje wyłącznie znaczniki z tej listy, a surowy HTML escapuje, więc w poprawnym
//! przebiegu sanitizer niczego nie usuwa. Jest po to, żeby błąd writera nie stał się XSS.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use ammonia::{Builder, UrlRelative};

use crate::RenderOptions;
use crate::url::{safe_image_src, safe_link_href};
use crate::writer::code_lang;

/// Dozwolone znaczniki (dokładnie te, które produkuje writer).
pub const ALLOWED_TAGS: &[&str] = &[
    "a",
    "blockquote",
    "br",
    "code",
    "del",
    "em",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "hr",
    "img",
    "input",
    "li",
    "ol",
    "p",
    "pre",
    "strong",
    "sub",
    "sup",
    "table",
    "tbody",
    "td",
    "th",
    "thead",
    "tr",
    "ul",
];

/// Znaczniki usuwane razem z treścią.
const CLEAN_CONTENT_TAGS: [&str; 16] = [
    "script",
    "style",
    "iframe",
    "object",
    "embed",
    "noscript",
    "template",
    "textarea",
    "title",
    "xmp",
    "noembed",
    "noframes",
    "plaintext",
    "svg",
    "math",
    "select",
];

/// Wartość atrybutu `rel` na każdym linku.
pub const LINK_REL: &str = "noopener noreferrer nofollow";

/// Sanitizer skonfigurowany dla danego zestawu opcji.
pub(crate) struct Sanitizer {
    builder: Builder<'static>,
}

fn is_digits(value: &str, max_len: usize) -> bool {
    !value.is_empty() && value.len() <= max_len && value.bytes().all(|b| b.is_ascii_digit())
}

/// Filtr wartości atrybutów (po białej liście nazw).
fn filter_attr<'u>(
    opts: RenderOptions,
    element: &str,
    attribute: &str,
    value: &'u str,
) -> Option<Cow<'u, str>> {
    let keep = match (element, attribute) {
        ("a", "href") => return safe_link_href(value).map(Cow::Owned),
        ("img", "src") => safe_image_src(value, &opts).as_deref() == Some(value),
        ("input", "type") => value.eq_ignore_ascii_case("checkbox"),
        ("th" | "td", "data-align") => matches!(value, "left" | "center" | "right"),
        ("code", "data-lang") => code_lang(value).as_deref() == Some(value),
        ("pre", "data-lines" | "data-code") => is_digits(value, 9),
        ("ol", "start") => is_digits(value, 9),
        _ => true,
    };
    keep.then_some(Cow::Borrowed(value))
}

impl Sanitizer {
    fn new(opts: RenderOptions) -> Self {
        let images = opts.allow_data_images || opts.allow_remote_images;
        let tags: HashSet<&'static str> = ALLOWED_TAGS
            .iter()
            .copied()
            .filter(|tag| images || *tag != "img")
            .collect();
        let tag_attributes: HashMap<&'static str, HashSet<&'static str>> = [
            ("a", vec!["href", "title"]),
            ("img", vec!["src", "alt", "title"]),
            ("ol", vec!["start"]),
            ("input", vec!["type", "checked", "disabled"]),
            ("code", vec!["data-lang"]),
            ("pre", vec!["data-lines", "data-code"]),
            ("th", vec!["data-align"]),
            ("td", vec!["data-align"]),
        ]
        .into_iter()
        .map(|(tag, attrs)| (tag, attrs.into_iter().collect()))
        .collect();
        let mut schemes: HashSet<&'static str> = ["http", "https", "mailto"].into();
        if opts.allow_data_images {
            schemes.insert("data");
        }
        let mut builder = Builder::empty();
        builder
            .tags(tags)
            .clean_content_tags(CLEAN_CONTENT_TAGS.into())
            .generic_attributes(HashSet::new())
            .tag_attributes(tag_attributes)
            // Po jednym wymuszanym atrybucie na znacznik: ammonia dokłada je w kolejności
            // iteracji `HashMap`, a przy jednym wynik jest deterministyczny.
            .set_tag_attribute_value("input", "disabled", "")
            .set_tag_attribute_value("a", "data-external", "")
            .link_rel(Some(LINK_REL))
            .url_schemes(schemes)
            .url_relative(UrlRelative::Deny)
            .strip_comments(true)
            .attribute_filter(move |element, attribute, value| {
                filter_attr(opts, element, attribute, value)
            });
        Self { builder }
    }

    /// Sanitizuje fragment HTML.
    pub(crate) fn clean(&self, html: &str) -> String {
        self.builder.clean(html).to_string()
    }

    /// Współdzielony sanitizer dla opcji (budowany raz na proces).
    pub(crate) fn shared(opts: RenderOptions) -> &'static Sanitizer {
        static CACHE: [OnceLock<Sanitizer>; 4] = [const { OnceLock::new() }; 4];
        let index =
            usize::from(opts.allow_data_images) | (usize::from(opts.allow_remote_images) << 1);
        CACHE[index].get_or_init(|| Sanitizer::new(opts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(html: &str) -> String {
        Sanitizer::shared(RenderOptions::default()).clean(html)
    }

    #[test]
    fn strips_dangerous_markup_even_if_writer_leaked_it() {
        assert_eq!(clean("<script>alert(1)</script>x"), "x");
        assert_eq!(clean("<img src=x onerror=alert(1)>"), "");
        assert_eq!(
            clean("<a href=\"javascript:alert(1)\" onclick=\"x\">t</a>"),
            "<a data-external=\"\" rel=\"noopener noreferrer nofollow\">t</a>"
        );
        assert_eq!(
            clean("<input type=\"text\" value=\"x\">"),
            "<input disabled=\"\">"
        );
        assert_eq!(
            clean("<input type=\"checkbox\" checked>"),
            "<input type=\"checkbox\" checked=\"\" disabled=\"\">"
        );
        assert_eq!(clean("<td data-align=\"x;evil\">a</td>"), "a");
    }

    #[test]
    fn keeps_writer_output() {
        let html = "<pre data-code=\"0\" data-lines=\"1\"><code data-lang=\"rust\">fn</code></pre>";
        assert_eq!(clean(html), html);
        let link = clean("<a href=\"https://a.pl\">x</a>");
        assert_eq!(
            link,
            "<a href=\"https://a.pl\" data-external=\"\" rel=\"noopener noreferrer nofollow\">x</a>"
        );
    }
}
