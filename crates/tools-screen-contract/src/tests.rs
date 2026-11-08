use platform_contract::MaskedArea;

use super::*;

#[test]
fn manifest_and_args() {
    let m = capture_manifest();
    m.validate().unwrap();
    assert!(!m.mutating && m.untrusted_output == Some(TaintSource::Screen));
    assert!(m.allowed_for(&["gui.control".into()], true));
    assert_eq!(
        check_args("screen_capture", &sample_args("screen_capture")),
        Ok(())
    );
    for bad in [
        serde_json::json!({"target": "window"}),
        serde_json::json!({"target": "region", "x": 1, "y": 1}),
        serde_json::json!({"target": "monitor", "window": 3}),
        serde_json::json!({"target": "monitor", "max_side": 10}),
        serde_json::json!({"target": "monitor", "extra": 1}),
        serde_json::json!({"target": "region", "x": 0, "y": 0, "width": 0, "height": 10}),
    ] {
        assert!(check_args("screen_capture", &bad).is_err(), "{bad}");
    }
    assert!(check_args("x", &serde_json::json!({})).is_err());
    let r = to_request(
        &serde_json::from_value(
            serde_json::json!({"target": "window", "window": 7, "max_side": 800}),
        )
        .unwrap(),
        &ScreenToolsConfig {
            masked_apps: vec!["bank.exe".into()],
            ..ScreenToolsConfig::default()
        },
    )
    .unwrap();
    assert_eq!(r.max_width, 800);
    assert!(
        r.is_masked_app("claude.exe")
            && r.is_masked_app("BANK.EXE")
            && r.is_masked_app("keepass.exe")
    );
}

#[test]
fn output_scale_and_reasons() {
    let shot = Screenshot {
        png: vec![1, 2, 3],
        width: 960,
        height: 540,
        source: ScreenRect::from_xywh(0, 0, 1920, 1080),
        masked: vec![MaskedArea {
            rect: ScreenRect::from_xywh(1, 2, 3, 4),
            reason: MaskReason::PasswordField,
        }],
        black_frame: false,
    };
    let o = to_output(&shot);
    assert!((o.scale - 2.0).abs() < 1e-9);
    assert_eq!(o.masked[0].reason, "password_field");
    assert_eq!(o.png_bytes, 3);
}
