//! Załączniki PDF (C2.11): tekst wyciągnięty dla modelu jako treść niezaufana (chip „w całości”),
//! PDF z hasłem — notka zamiast tekstu (chip „tylko opis”), sesja skażona jak przy tekście.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_api::dto::{AttachmentDelivery, AttachmentKind};
use common::Env;
use lopdf::{Document, EncryptionState, EncryptionVersion, Permissions};
use providers_contract::ContentBlock;
use sessions_contract::SessionCatalog;

/// Jednostronicowy PDF z napisem `text` (Helvetica, WinAnsi; `\363` = ó).
fn pdf(text: &str) -> Vec<u8> {
    let content = format!("BT /F1 12 Tf 72 720 Td ({text}) Tj ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 4 0 R >> >> \
         /Contents 5 0 R >>"
            .to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_owned(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    let id = "<00112233445566778899AABBCCDDEEFF>";
    out.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R /ID [{id} {id}] >>\nstartxref\n{xref}\n%%EOF\n")
            .as_bytes(),
    );
    out
}

/// Ten sam PDF zaszyfrowany hasłem użytkownika.
fn locked(bytes: &[u8]) -> Vec<u8> {
    let mut doc = Document::load_mem(bytes).unwrap();
    let state = EncryptionState::try_from(EncryptionVersion::V2 {
        document: &doc,
        owner_password: "wlasc",
        user_password: "tajne",
        key_length: 128,
        permissions: Permissions::all(),
    })
    .unwrap();
    doc.encrypt(&state).unwrap();
    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap();
    out
}

#[tokio::test]
async fn pdf_text_reaches_the_model_and_a_locked_pdf_gets_a_note() {
    let env = Env::new();
    let s = env.session("Umowy");
    let open = env.file("umowa.pdf", &pdf("Kr\\363tki termin: 30 dni"));
    let secret = env.file("tajne.pdf", &locked(&pdf("Sekretna kwota")));
    let added = env
        .app
        .attachments()
        .add_paths(&s, &[open, secret])
        .unwrap();
    let (a, b) = (&added.added[0], &added.added[1]);
    assert_eq!(
        (a.kind, a.delivery),
        (AttachmentKind::Document, AttachmentDelivery::Full)
    );
    assert_eq!(
        (b.kind, b.delivery),
        (AttachmentKind::Document, AttachmentDelivery::MetadataOnly)
    );
    let blocks = env.app.prepare(&s, &[a.id.clone(), b.id.clone()]).unwrap();
    assert!(
        env.sessions.session(&s).unwrap().tainted,
        "PDF to treść z zewnątrz"
    );
    let model = env.app.provider_blocks(&s, &blocks);
    let texts: Vec<&str> = model
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts.len(), 2);
    assert!(
        texts[0].contains("tekst wyciągnięty z dokumentu")
            && texts[0].contains("--- Strona 1 ---\nKrótki termin: 30 dni")
            && texts[0].contains("<<<NIEZAUFANE"),
        "{}",
        texts[0]
    );
    assert!(
        texts[1].contains("„tajne.pdf”")
            && texts[1].contains("zabezpieczony hasłem")
            && !texts[1].contains("Sekretna"),
        "{}",
        texts[1]
    );
}
