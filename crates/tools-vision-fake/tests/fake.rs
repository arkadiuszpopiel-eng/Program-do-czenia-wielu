//! Atrapy przechodzą testy kontraktowe, OCR jest deterministyczny, opis respektuje prywatność.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use providers_contract::CancellationToken;
use tools_common_contract::{ToolCtx, ToolOutcome, Toolset};
use tools_vision_contract::{
    DescribeError, DescribePort, DescribeRequest, OcrError, OcrPort, OcrRequest, PrivacyLookup,
    VisionPrivacy,
};
use tools_vision_fake::{FakeDescriber, FakeOcr, FakePrivacy, FakeTools, line};

#[tokio::test]
async fn contract_and_scripting() {
    let fake = FakeTools::default();
    tools_vision_contract::contract_tests::run_all(&fake.tools()).await;
    let first = fake.tools().remove(0);
    let name = first.manifest().name.clone();
    fake.push(&name, ToolOutcome::ok("skrypt", serde_json::json!({})));
    let ctx = ToolCtx::new(safety_broker_contract::Holder::agent("s1", "delta"));
    let out = first
        .call(tools_vision_contract::sample_args(&name), &ctx)
        .await;
    assert_eq!(out.text, "skrypt");
    assert_eq!(out.untrusted, first.manifest().untrusted_output.clone());
}

#[test]
fn ocr_is_deterministic_and_records_images() {
    let ocr = FakeOcr::new(vec![line("Plik Edycja", 0.0, 0.0, 100.0, 10.0)]);
    let req = OcrRequest {
        image: vec![1, 2, 3],
        language: None,
    };
    let a = ocr.recognize(&req).unwrap();
    assert_eq!(a, ocr.recognize(&req).unwrap());
    assert_eq!(a.lines[0].words.len(), 2);
    assert_eq!(ocr.requests().len(), 2);
    let de = OcrRequest {
        image: vec![1],
        language: Some("de".into()),
    };
    assert_eq!(ocr.recognize(&de), Err(OcrError::Language("de".into())));
    assert!(
        ocr.recognize(&OcrRequest {
            image: vec![],
            language: None
        })
        .is_err()
    );
    ocr.push(Err(OcrError::Unsupported("x".into())));
    assert!(ocr.recognize(&req).is_err());
    assert_eq!(ocr.languages().unwrap(), vec!["pl", "en-US"]);
}

fn request(privacy: VisionPrivacy) -> DescribeRequest {
    DescribeRequest {
        image: vec![0; 10],
        media_type: "image/png".into(),
        question: None,
        privacy,
        session: "s1".into(),
        max_tokens: 100,
    }
}

#[tokio::test]
async fn describe_respects_privacy() {
    let cloud_only = FakeDescriber::new(false, true);
    let cancel = CancellationToken::new();
    assert_eq!(
        cloud_only
            .describe(request(VisionPrivacy::LocalOnly), cancel.clone())
            .await,
        Err(DescribeError::PrivateNoLocal)
    );
    let d = cloud_only
        .describe(request(VisionPrivacy::Normal), cancel.clone())
        .await
        .unwrap();
    assert!(!d.local);
    let both = FakeDescriber::new(true, true);
    assert!(
        both.describe(request(VisionPrivacy::LocalOnly), cancel.clone())
            .await
            .unwrap()
            .local
    );
    both.push(Ok("Okno z przyciskiem.".into()));
    assert_eq!(
        both.describe(request(VisionPrivacy::Normal), cancel.clone())
            .await
            .unwrap()
            .text,
        "Okno z przyciskiem."
    );
    assert_eq!(both.requests().len(), 2);
    assert!(
        FakeDescriber::new(false, false)
            .describe(request(VisionPrivacy::Normal), cancel.clone())
            .await
            .is_err()
    );
    cancel.cancel();
    assert_eq!(
        both.describe(request(VisionPrivacy::Normal), cancel).await,
        Err(DescribeError::Cancelled)
    );
    let privacy = FakePrivacy::default();
    privacy.set("s1", VisionPrivacy::Normal);
    assert_eq!(privacy.vision_privacy("s1"), VisionPrivacy::Normal);
    assert_eq!(privacy.vision_privacy("nieznana"), VisionPrivacy::LocalOnly);
}
