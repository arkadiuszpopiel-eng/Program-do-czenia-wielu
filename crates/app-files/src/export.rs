//! Eksport rozmowy (PLAN §14.8 „eksport sesji do MD/HTML”): aktywna gałąź (albo jedna
//! wiadomość) jako Markdown, samodzielny HTML albo JSON. HTML treści renderuje Rust (`lib-markdown`:
//! pulldown-cmark + ammonia), dokument ma CSP bez skryptów i sieci, fonty systemowe; tytuł, autorzy
//! i nazwy załączników są escapowane. Tury systemowe, puste i ukryte z widoku są pomijane.
//!
//! Oznaczenie treści AI (AI Act art. 50 ust. 2, zadanie C2.9): wiadomości agentek mają czytelny
//! znak „AI” (z modelem, jeśli znany) i znacznik maszynowy — w Markdown nagłówek YAML, w HTML
//! znaczniki `meta` i atrybuty `data-ai-generated` / `data-model`. Typ źródła według słownika IPTC
//! (`trainedAlgorithmicMedia` albo `compositeWithTrainedAlgorithmicMedia`, gdy są też wiadomości
//! użytkownika).

use chrono::{DateTime, Local, Utc};
use sessions_contract::{Author, Role, Turn};

use crate::attach::{human_size, turn_attachments};

/// Wiadomość w eksporcie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportMessage {
    /// Autor do wyświetlenia („Ty”, „Alfa”…).
    pub author: String,
    /// Czy autorem jest użytkownik.
    pub user: bool,
    /// Czas zapisu.
    pub at: DateTime<Utc>,
    /// Treść (Markdown).
    pub text: String,
    /// Załączniki: nazwa i opis („image/png, 1,2 KB”).
    pub attachments: Vec<(String, String)>,
    /// Treść wygenerowana przez AI (agentka, przekazanie, Alfa).
    pub ai: bool,
    /// Model, który wytworzył treść (`dostawca/model`), jeśli zapisany w turze.
    pub model: Option<String>,
}

const IPTC: &str = "http://cv.iptc.org/newscodes/digitalsourcetype/";

/// Typ źródła IPTC dla zbioru wiadomości (`None`, gdy żadna nie jest z AI).
pub fn source_type(messages: &[ExportMessage]) -> Option<String> {
    let ai = messages.iter().filter(|m| m.ai).count();
    match ai {
        0 => None,
        n if n == messages.len() => Some(format!("{IPTC}trainedAlgorithmicMedia")),
        _ => Some(format!("{IPTC}compositeWithTrainedAlgorithmicMedia")),
    }
}

fn model_of(usage: Option<&sessions_contract::ModelUsage>) -> Option<String> {
    let u = usage?;
    match (u.provider.trim(), u.model.trim()) {
        (_, "") => None,
        ("", m) => Some(m.to_owned()),
        (p, m) => Some(format!("{p}/{m}")),
    }
}

fn capitalize(id: &str) -> String {
    let mut chars = id.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Wiadomości do eksportu z tur (kolejność zachowana).
pub fn messages(turns: &[Turn]) -> Vec<ExportMessage> {
    turns
        .iter()
        .filter(|t| !t.hidden && matches!(t.role, Role::User | Role::Assistant))
        .filter_map(|t| {
            let attachments: Vec<(String, String)> = turn_attachments(&t.content.blocks)
                .into_iter()
                .map(|a| {
                    let size = a.bytes.map(human_size).unwrap_or_default();
                    let desc = [a.mime, size]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(", ");
                    (a.name, desc)
                })
                .collect();
            let text = t.content.text.trim().to_owned();
            if text.is_empty() && attachments.is_empty() {
                return None;
            }
            let (author, user) = match &t.author {
                Author::User => ("Ty".to_owned(), true),
                Author::Agent { agent } => (capitalize(&agent.to_string()), false),
                Author::Handoff { .. } => ("Przekazanie".to_owned(), false),
                _ => ("Alfa".to_owned(), false),
            };
            Some(ExportMessage {
                author,
                user,
                at: t.created_at,
                text,
                attachments,
                ai: !user,
                model: if user {
                    None
                } else {
                    model_of(t.usage.as_ref())
                },
            })
        })
        .collect()
}

fn stamp(at: DateTime<Utc>) -> String {
    at.with_timezone(&Local)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

/// Liczba wiadomości z AI.
fn ai_count(messages: &[ExportMessage]) -> usize {
    messages.iter().filter(|m| m.ai).count()
}

/// Czytelne oznaczenie wiadomości z AI („AI” albo „AI, dostawca/model”).
fn ai_label(m: &ExportMessage) -> Option<String> {
    m.ai.then(|| match &m.model {
        Some(model) => format!("AI, {model}"),
        None => "AI".to_owned(),
    })
}

const AI_NOTE: &str = "Wiadomości oznaczone „AI” wygenerowała sztuczna inteligencja (modele językowe) — mogą zawierać błędy.";

/// Markdown (CommonMark): nagłówek YAML ze znacznikiem maszynowym, tytuł, wiadomości z autorem,
/// oznaczeniem AI i czasem, załączniki jako lista.
pub fn markdown(title: &str, messages: &[ExportMessage], exported: DateTime<Utc>) -> String {
    let ai = ai_count(messages);
    let mut out = format!(
        "---\ngenerator: Alfa\nexported: {}\nmessages: {}\nai_generated_messages: {ai}\n",
        exported.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        messages.len()
    );
    if let Some(kind) = source_type(messages) {
        out.push_str(&format!("digital_source_type: {kind}\n"));
    }
    out.push_str(&format!(
        "---\n\n# {}\n\n_Eksport z Alfy · {} · wiadomości: {}_\n",
        title.trim(),
        stamp(exported),
        messages.len()
    ));
    if ai > 0 {
        out.push_str(&format!("\n> {AI_NOTE}\n"));
    }
    for m in messages {
        let author = match ai_label(m) {
            Some(label) => format!("{} · {}", m.author, label.replace('`', "'")),
            None => m.author.clone(),
        };
        out.push_str(&format!("\n---\n\n### {author} · {}\n\n", stamp(m.at)));
        if !m.text.is_empty() {
            out.push_str(&m.text);
            out.push('\n');
        }
        if !m.attachments.is_empty() {
            out.push_str("\nZałączniki:\n\n");
            for (name, desc) in &m.attachments {
                out.push_str(&format!("- `{}` ({desc})\n", name.replace('`', "'")));
            }
        }
    }
    out
}

/// Escapowanie tekstu do HTML (treść i atrybuty).
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

const STYLE: &str = "body{margin:0;background:#fafafa;color:#1f1f1f;font:15px/1.6 'Segoe UI Variable','Segoe UI',system-ui,sans-serif}\
main{max-width:760px;margin:0 auto;padding:32px 16px}h1{font-size:24px;margin:0 0 4px}\
.meta{color:#5f5f5f;font-size:13px;margin:0 0 24px}article{padding:16px 0;border-top:1px solid #e0e0e0}\
header{font-size:13px;color:#5f5f5f;margin-bottom:6px}header strong{color:#1f1f1f}\
article.user .body{background:#f0f0f0;border-radius:8px;padding:8px 12px}\
pre,code{font-family:'Cascadia Mono',Consolas,monospace;font-size:13px}\
pre{background:#f3f3f3;padding:12px;border-radius:6px;overflow:auto}table{border-collapse:collapse}\
th,td{border:1px solid #d0d0d0;padding:4px 8px}.att{font-size:13px;color:#5f5f5f}\
.ai{display:inline-block;margin-left:4px;padding:0 5px;border:1px solid currentColor;border-radius:4px;font-size:11px;font-weight:600}\
@media (prefers-color-scheme:dark){body{background:#1c1c1c;color:#e8e8e8}header strong{color:#e8e8e8}\
article{border-color:#333}article.user .body,pre{background:#2a2a2a}.meta,header,.att{color:#a8a8a8}}";

/// Samodzielny HTML (bez skryptów, bez zasobów zewnętrznych — CSP `default-src 'none'`).
pub fn html(title: &str, messages: &[ExportMessage], exported: DateTime<Utc>) -> String {
    let title = escape(title.trim());
    let mut body = String::new();
    for m in messages {
        let class = if m.user { "user" } else { "agent" };
        let (data, badge) = match ai_label(m) {
            Some(label) => (
                format!(
                    " data-ai-generated=\"true\"{}",
                    m.model
                        .as_ref()
                        .map(|model| format!(" data-model=\"{}\"", escape(model)))
                        .unwrap_or_default()
                ),
                format!(
                    " <span class=\"ai\" title=\"Wygenerowane przez sztuczną inteligencję\">{}</span>",
                    escape(&label)
                ),
            ),
            None => (String::new(), String::new()),
        };
        body.push_str(&format!(
            "<article class=\"{class}\"{data}><header><strong>{}</strong>{badge} · <time datetime=\"{}\">{}</time></header>",
            escape(&m.author),
            m.at.to_rfc3339(),
            stamp(m.at)
        ));
        if !m.text.is_empty() {
            body.push_str(&format!(
                "<div class=\"body\">{}</div>",
                lib_markdown::render(&m.text)
            ));
        }
        if !m.attachments.is_empty() {
            body.push_str("<ul class=\"att\" aria-label=\"Załączniki\">");
            for (name, desc) in &m.attachments {
                body.push_str(&format!("<li>{} ({})</li>", escape(name), escape(desc)));
            }
            body.push_str("</ul>");
        }
        body.push_str("</article>\n");
    }
    let ai = ai_count(messages);
    let mut marks = format!(
        "<meta name=\"generator\" content=\"Alfa\">\n<meta name=\"ai-generated-messages\" content=\"{ai}\">\n"
    );
    let mut note = String::new();
    if let Some(kind) = source_type(messages) {
        marks.push_str(&format!(
            "<meta name=\"ai-generated\" content=\"true\">\n<meta name=\"digital-source-type\" content=\"{kind}\">\n"
        ));
        note = format!("<p class=\"meta\">{}</p>\n", escape(AI_NOTE));
    }
    format!(
        "<!doctype html>\n<html lang=\"pl\">\n<head>\n<meta charset=\"utf-8\">\n\
<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; img-src data:\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n{marks}<title>{title}</title>\n\
<style>{STYLE}</style>\n</head>\n<body>\n<main>\n<h1>{title}</h1>\n<p class=\"meta\">Eksport z Alfy · {} · wiadomości: {}</p>\n{note}{body}</main>\n</body>\n</html>\n",
        stamp(exported),
        messages.len()
    )
}

/// JSON (`alfa-conversation`, wersja 1): metadane eksportu ze znacznikiem treści AI i wiadomości
/// z autorem, rolą, `ai`, modelem, czasem (RFC 3339), treścią (Markdown) i załącznikami.
pub fn json(title: &str, messages: &[ExportMessage], exported: DateTime<Utc>) -> String {
    let items: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| {
            serde_json::json!({
                "author": m.author,
                "role": if m.user { "user" } else { "assistant" },
                "ai": m.ai,
                "model": m.model,
                "at": m.at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                "text": m.text,
                "attachments": m
                    .attachments
                    .iter()
                    .map(|(name, desc)| serde_json::json!({ "name": name, "description": desc }))
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    let doc = serde_json::json!({
        "format": "alfa-conversation",
        "version": 1,
        "generator": "Alfa",
        "title": title.trim(),
        "exported": exported.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "ai_generated_messages": ai_count(messages),
        "digital_source_type": source_type(messages),
        "messages": items,
    });
    let mut out = serde_json::to_string_pretty(&doc).unwrap_or_default();
    out.push('\n');
    out
}

/// Bezpieczna nazwa pliku eksportu: tytuł (litery, cyfry, spacje, myślniki; ≤ 60) + data.
pub fn file_name(title: &str, at: DateTime<Utc>, extension: &str) -> String {
    let clean: String = title
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
        .take(60)
        .collect();
    let base = match clean.trim() {
        "" => "rozmowa",
        t => t,
    };
    format!(
        "{base} {}.{extension}",
        at.with_timezone(&Local).format("%Y-%m-%d")
    )
}
