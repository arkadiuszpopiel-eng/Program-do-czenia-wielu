//! Szablony promptów na prawdziwym rdzeniu: zapis z walidacją, identyfikatory, odczyt po
//! ponownym zapisie, odmowa złej zmiennej bez utraty listy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use app_core::dto::PromptTemplate;
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use common::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn templates_are_validated_and_stored() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let opts = options(Some(provider), Arc::new(HeadlessShell::default()));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    assert!(core.prompt_templates_get().await.unwrap().is_empty());
    let mail = PromptTemplate {
        id: String::new(),
        name: "E-mail".into(),
        text: "Napisz e-mail do {{adresat}} w sprawie {{temat}}.".into(),
    };
    let saved = core.prompt_templates_set(vec![mail.clone()]).await.unwrap();
    assert_eq!(saved[0].id, "e-mail");
    let mut broken = mail;
    broken.text = "Napisz {{adresat".into();
    assert!(core.prompt_templates_set(vec![broken]).await.is_err());
    let now = core.prompt_templates_get().await.unwrap();
    assert_eq!(now, saved, "odmowa nie zmienia zapisanej listy");
}
