//! Testy funkcji wspólnych (jednostkowe + property-based).

use proptest::prelude::*;

use super::*;

#[test]
fn sha256_known_vector() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn mime_by_extension() {
    assert_eq!(guess_mime(Path::new("a/Raport.MD")), "text/markdown");
    assert_eq!(guess_mime(Path::new("x.png")), "image/png");
    assert_eq!(
        guess_mime(Path::new("bez_rozszerzenia")),
        "application/octet-stream"
    );
}

#[test]
fn binary_detection() {
    assert!(!looks_binary("zażółć gęślą jaźń".as_bytes()));
    assert!(looks_binary(&[0x89, b'P', b'N', b'G', 0, 1]));
    assert!(looks_binary(&[0xff, 0xfe, b'a']));
    // Ucięty znak wielobajtowy na końcu próbki nie oznacza pliku binarnego.
    let mut text = "a".repeat(BINARY_SNIFF_BYTES - 1).into_bytes();
    text.extend_from_slice("ż".as_bytes());
    assert!(!looks_binary(&text));
}

#[test]
fn preview_cuts_on_char_boundary() {
    let content = "żółw".as_bytes();
    match preview_bytes(content, content.len() as u64, 3, "text/plain") {
        Preview::Text { text, truncated } => {
            assert_eq!(text, "ż");
            assert!(truncated);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        preview_bytes(&[0, 1], 2, 10, "image/png"),
        Preview::Binary {
            mime: "image/png".into(),
            bytes: 2
        }
    );
}

#[test]
fn diff_counts_and_numbers_lines() {
    let d = diff_texts("a\nb\nc\n", "a\nB\nc\nd\n");
    assert_eq!((d.added, d.removed), (2, 1));
    let inserted: Vec<&str> = d
        .lines
        .iter()
        .filter(|l| l.tag == DiffTag::Insert)
        .map(|l| l.text.as_str())
        .collect();
    assert_eq!(inserted, vec!["B", "d"]);
    assert!(d.unified.contains("-b") && d.unified.contains("+B"));
    assert_eq!(d.lines[0].old_line, Some(1));
}

#[test]
fn out_dir_and_actions() {
    let root = Path::new("C:/Users/ja/Alfa");
    assert_eq!(
        default_out_dir(root, "Raport (2)"),
        root.join("Sesje").join("Raport (2)").join("out")
    );
    let s = SessionId::new("s");
    let same = ArtifactAction::SendToSession { target: s.clone() };
    assert!(validate_action(&s, &same).is_err());
    let empty = ArtifactAction::SaveAs {
        target: PathBuf::new(),
    };
    assert!(validate_action(&s, &empty).is_err());
    assert!(validate_action(&s, &ArtifactAction::Reveal).is_ok());
}

proptest! {
    /// Diff odtwarza obie wersje (stara = Equal+Delete, nowa = Equal+Insert).
    #[test]
    fn diff_reconstructs_both_sides(
        a in proptest::collection::vec("[a-ząę ]{0,6}", 0..12),
        b in proptest::collection::vec("[a-ząę ]{0,6}", 0..12),
    ) {
        let old: String = a.iter().map(|l| format!("{l}\n")).collect();
        let new: String = b.iter().map(|l| format!("{l}\n")).collect();
        let d = diff_texts(&old, &new);
        let rebuild = |keep: DiffTag| -> Vec<String> {
            d.lines.iter().filter(|l| l.tag == DiffTag::Equal || l.tag == keep).map(|l| l.text.clone()).collect()
        };
        prop_assert_eq!(rebuild(DiffTag::Delete), a);
        prop_assert_eq!(rebuild(DiffTag::Insert), b);
    }

    /// Podgląd tekstu nigdy nie przekracza limitu i jest prefiksem oryginału.
    #[test]
    fn preview_is_prefix(s in "\\PC{0,64}", max in 0_usize..80) {
        if let Preview::Text { text, .. } = preview_bytes(s.as_bytes(), s.len() as u64, max, "text/plain") {
            prop_assert!(text.len() <= max);
            prop_assert!(s.starts_with(&text));
        }
    }
}
