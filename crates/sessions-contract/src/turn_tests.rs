//! Testy jednostkowe typów tury.

use super::*;

fn turn(text: &str, chars: Option<usize>) -> Turn {
    Turn {
        id: TurnId(1),
        parent: None,
        branch: BranchId(1),
        role: Role::Assistant,
        author: Author::System,
        content: TurnContent::text(text),
        usage: None,
        created_at: DateTime::<Utc>::default(),
        heard_prefix: chars.map(|chars| HeardPrefix {
            chars,
            approximate: true,
        }),
        hidden: false,
    }
}

#[test]
fn searchable_text_skips_duplicates_and_non_text() {
    let content = TurnContent {
        text: "główny".into(),
        blocks: vec![
            Block::Text {
                text: "główny".into(),
            },
            Block::Thinking {
                provider: "p".into(),
                text: "tajne myśli".into(),
                signature: None,
            },
            Block::Text {
                text: "dodatkowy".into(),
            },
        ],
    };
    assert_eq!(content.searchable_text(), "główny\ndodatkowy");
}

#[test]
fn heard_text_counts_chars_not_bytes() {
    assert_eq!(turn("Żółć jest gorzka", Some(4)).heard_text(), Some("Żółć"));
    assert_eq!(turn("abc", Some(3)).heard_text(), Some("abc"));
    assert_eq!(turn("abc", None).heard_text(), None);
}

#[test]
fn fingerprint_ignores_view_flags() {
    let mut t = turn("x", None);
    let before = t.fingerprint();
    t.hidden = true;
    t.heard_prefix = Some(HeardPrefix {
        chars: 1,
        approximate: false,
    });
    assert_eq!(t.fingerprint(), before);
}

#[test]
fn validation() {
    assert_eq!(
        validate_new_turn(&NewTurn::user("  ")),
        Err(SessionError::EmptyTurn)
    );
    let mut t = NewTurn::user("hej");
    t.heard_prefix = Some(HeardPrefix {
        chars: 1,
        approximate: false,
    });
    assert!(matches!(
        validate_new_turn(&t),
        Err(SessionError::InvalidHeardPrefix { .. })
    ));
    let mut a = NewTurn::assistant("alfa", "hej");
    a.heard_prefix = Some(HeardPrefix {
        chars: 4,
        approximate: false,
    });
    assert!(validate_new_turn(&a).is_err());
    a.heard_prefix = Some(HeardPrefix {
        chars: 3,
        approximate: false,
    });
    assert!(validate_new_turn(&a).is_ok());
}
