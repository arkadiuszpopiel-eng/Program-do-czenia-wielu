use super::*;
use lib_media::MediaKind;

#[test]
fn manifests_are_valid_with_least_privilege_groups() {
    for m in manifests() {
        m.validate().unwrap();
        assert!(m.allowed_for(&["media".into()], false), "{}", m.name);
        assert!(m.untrusted_output.is_none());
    }
    assert!(info_manifest().allowed_for(&["media.read".into()], true));
    assert!(
        !convert_manifest().allowed_for(&["media".into()], true),
        "rola tylko do odczytu"
    );
    assert!(!play_manifest().allowed_for(&["media.read".into()], false));
    assert_eq!(convert_manifest().id, "tools-media.convert");
}

#[test]
fn args_are_validated() {
    for tool in ["media_info", "media_convert", "media_play"] {
        assert_eq!(check_args(tool, &sample_args(tool)), Ok(()), "{tool}");
        assert!(check_args(tool, &serde_json::json!({"path": " "})).is_err());
    }
    for ok in [
        serde_json::json!({"path": "a.mov", "format": "mp4", "output": "b.MP4"}),
        serde_json::json!({"path": "a.png", "format": "jpg", "output": "b.jpeg", "max_side": 800}),
        serde_json::json!({"path": "a.wav", "format": "mp3", "start_s": 0, "duration_s": 1.5}),
    ] {
        assert_eq!(check_args("media_convert", &ok), Ok(()), "{ok}");
    }
    for bad in [
        serde_json::json!({"path": "a.wav", "format": "mp3", "output": "b.wav"}),
        serde_json::json!({"path": "a.wav", "format": "mp3", "output": "bez_rozszerzenia"}),
        serde_json::json!({"path": "a.wav", "format": "exe"}),
        serde_json::json!({"path": "a.wav", "format": "mp3", "duration_s": 0}),
        serde_json::json!({"path": "a.wav", "format": "mp3", "start_s": -1}),
        serde_json::json!({"path": "a.wav", "format": "mp3", "start_s": 1e9}),
        serde_json::json!({"path": "a.wav", "format": "mp3", "max_side": 8}),
        serde_json::json!({"path": "a.wav", "format": "mp3", "args": ["-i", "http://x"]}),
    ] {
        assert!(check_args("media_convert", &bad).is_err(), "{bad}");
    }
    assert!(check_args("media_x", &serde_json::json!({})).is_err());
    let a: ConvertArgs = serde_json::from_value(
        serde_json::json!({"path": "a", "format": "wav", "start_s": 1.25, "duration_s": 2}),
    )
    .unwrap();
    assert_eq!(a.validate(), Ok((Some(1250), Some(2000))));
}

#[test]
fn formats_kinds_demuxers_and_names() {
    assert!(TargetFormat::Mp3.accepts(MediaKind::Video));
    assert!(!TargetFormat::Mp4.accepts(MediaKind::Audio));
    assert!(TargetFormat::Png.accepts(MediaKind::Video));
    assert!(!TargetFormat::Png.accepts(MediaKind::Audio));
    assert_eq!(TargetFormat::Jpg.ext(), "jpg");
    assert_eq!(input_demuxer("m4a"), Some("mov"));
    assert_eq!(input_demuxer("jpeg"), Some("jpeg_pipe"));
    for unsupported in ["heif", "avif", "hls", "concat", ""] {
        assert_eq!(input_demuxer(unsupported), None, "{unsupported}");
    }
    let taken = ["C:/Muzyka/a (Alfa).mp3".to_owned()];
    assert_eq!(
        output_path("C:/Muzyka/a.wav", "mp3", |p| taken.contains(&p.to_owned())).unwrap(),
        "C:/Muzyka/a (Alfa 2).mp3"
    );
    assert_eq!(
        output_path(r"C:\x\.wav", "mp3", |_| false).unwrap(),
        r"C:\x\.wav (Alfa).mp3"
    );
    assert_eq!(
        output_path("film", "gif", |_| false).unwrap(),
        "film (Alfa).gif"
    );
    assert_eq!(output_path("a.wav", "mp3", |_| true), None);
    let clip = AudioClip {
        samples: vec![0.0; 48_000 * 2],
        sample_rate: 48_000,
        channels: 2,
        agent: "alfa".into(),
        label: "x".into(),
    };
    assert_eq!(clip.duration_ms(), 1000);
}
