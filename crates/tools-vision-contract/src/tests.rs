use super::*;
use tools_screen_contract::TargetArg;

#[test]
fn manifests_are_valid_and_read_only() {
    for m in manifests() {
        m.validate().unwrap();
        assert!(!m.mutating, "{}", m.name);
        assert!(m.allowed_for(&["vision".into()], true));
        assert!(!m.allowed_for(&["gui.control".into(), "fs".into()], false));
        assert_eq!(
            m.untrusted_output,
            Some(safety_broker_contract::TaintSource::Screen)
        );
    }
    assert!(ocr_manifest().allowed_for(&["vision.ocr".into()], true));
    assert!(!describe_manifest().allowed_for(&["vision.ocr".into()], true));
}

#[test]
fn sources_are_exclusive_and_validated() {
    for tool in ["vision_ocr", "vision_describe"] {
        assert_eq!(check_args(tool, &sample_args(tool)), Ok(()));
        for ok in [
            serde_json::json!({"source": "file", "path": "C:/a.png"}),
            serde_json::json!({"source": "screen"}),
            serde_json::json!({"source": "screen", "target": "window", "window": 5}),
            serde_json::json!({"source": "screen", "target": "region", "x": 0, "y": 0, "width": 10, "height": 10}),
        ] {
            assert_eq!(check_args(tool, &ok), Ok(()), "{tool}: {ok}");
        }
        for bad in [
            serde_json::json!({"source": "file"}),
            serde_json::json!({"source": "file", "path": "  "}),
            serde_json::json!({"source": "file", "path": "a.png", "window": 1}),
            serde_json::json!({"source": "screen", "path": "a.png"}),
            serde_json::json!({"source": "screen", "target": "window"}),
            serde_json::json!({"source": "screen", "target": "region", "x": 1}),
            serde_json::json!({"source": "kamera"}),
            serde_json::json!({"source": "screen", "extra": 1}),
        ] {
            assert!(check_args(tool, &bad).is_err(), "{tool}: {bad}");
        }
    }
    assert!(check_args("x", &serde_json::json!({})).is_err());
    let long = "?".repeat(args::MAX_QUESTION_CHARS + 1);
    assert!(
        check_args(
            "vision_describe",
            &serde_json::json!({"source": "screen", "question": long})
        )
        .is_err()
    );
}

#[test]
fn language_tags_and_capture_side() {
    for ok in ["pl", "en-US", "zh-Hans-CN", "deu"] {
        assert!(valid_language(ok), "{ok}");
    }
    for bad in ["", "p", "polski", "pl_PL", "en-US-x-y-z", "pl-", "../x"] {
        assert!(!valid_language(bad), "{bad}");
    }
    let config = VisionToolsConfig::default();
    let a: OcrArgs =
        serde_json::from_value(serde_json::json!({"source": "screen", "language": "pl"})).unwrap();
    match a.image(&config).unwrap() {
        ImageSpec::Screen(c) => {
            assert_eq!(c.target, TargetArg::Monitor);
            assert_eq!(c.max_side, Some(config.ocr_max_side));
        }
        ImageSpec::File(_) => panic!("oczekiwano zrzutu"),
    }
    let bad: OcrArgs =
        serde_json::from_value(serde_json::json!({"source": "screen", "language": "x y"})).unwrap();
    assert!(bad.image(&config).is_err());
}

#[test]
fn lines_map_to_screen_coordinates() {
    let text = OcrText {
        language: "pl".into(),
        lines: vec![
            OcrLine {
                text: "Zapisz plik".into(),
                words: vec![
                    OcrWord {
                        text: "Zapisz".into(),
                        rect: OcrRect {
                            x: 10.0,
                            y: 20.0,
                            width: 40.0,
                            height: 10.0,
                        },
                    },
                    OcrWord {
                        text: "plik".into(),
                        rect: OcrRect {
                            x: 55.0,
                            y: 19.0,
                            width: 20.0,
                            height: 12.0,
                        },
                    },
                ],
            },
            OcrLine {
                text: "pusta".into(),
                words: vec![],
            },
        ],
        angle: None,
    };
    let lines = screen_lines(&text, 2.0, (100, 50));
    assert_eq!(
        lines[0],
        OcrLineOut {
            text: "Zapisz plik".into(),
            x: 120,
            y: 88,
            width: 130,
            height: 24
        }
    );
    assert_eq!((lines[1].width, lines[1].height), (0, 0));
    assert_eq!(VisionPrivacy::default(), VisionPrivacy::LocalOnly);
}
