//! Wyszukanie używa tej samej ścieżki OpenArgs/Broker/BrowserPort co browser_open.

use tools_browser_contract::{SearchArgs, SearchEngine, SearchStage};
use tools_common_contract::{ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, parse_args};

use crate::ops::{Core, Step};
use crate::out::fail;

impl Core {
    pub(crate) async fn search(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        manifest: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let request: SearchArgs = parse_args(args)?;
        let navigation = request
            .navigation()
            .map_err(|e| fail(ToolErrorKind::InvalidArgs, e))?;
        let args = serde_json::to_value(navigation).map_err(|_| {
            fail(
                ToolErrorKind::Internal,
                "Nie można utworzyć nawigacji.".into(),
            )
        })?;
        let mut outcome = self.open(args, ctx, manifest).await?;
        if outcome.is_ok() {
            annotate(&mut outcome, request.engine)?;
            outcome.text.push_str("\nTo wywołanie wykonało tylko nawigację do silnika; nie odczytuje wyników. Następny krok: browser_read, potem weryfikacja źródeł.");
        }
        Ok(outcome)
    }
}

fn annotate(outcome: &mut ToolOutcome, engine: SearchEngine) -> Step<()> {
    if !outcome
        .data
        .get("url")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|url| engine.matches_page_url(url))
    {
        let mut failure = ToolOutcome::failed(
            ToolErrorKind::Io,
            "Nie potwierdzono nawigacji do wybranego silnika. Sprawdź bieżącą stronę i blokowane hosty przez browser_read; nie deklaruj otrzymania wyników.",
        );
        failure.data = outcome.data.clone();
        failure.untrusted = outcome.untrusted.clone();
        failure.approval = outcome.approval;
        return Err(Box::new(failure));
    }
    let value = serde_json::to_value(engine).map_err(|_| {
        fail(
            ToolErrorKind::Internal,
            "Nie można opisać wyszukania.".into(),
        )
    })?;
    let stage = serde_json::to_value(SearchStage::NavigationOnly)
        .map_err(|_| fail(ToolErrorKind::Internal, "Nie można opisać etapu.".into()))?;
    let data = outcome.data.as_object_mut().ok_or_else(|| {
        fail(
            ToolErrorKind::Internal,
            "Nieprawidłowy wynik nawigacji.".into(),
        )
    })?;
    data.insert("search_engine".into(), value);
    data.insert("search_stage".into(), stage);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unexpected_page_cannot_be_labelled_as_open_search_page() {
        for url in [
            "https://previous.example/",
            "about:blank",
            "http://duckduckgo.com/",
            "https://duckduckgo.com.evil.example/",
        ] {
            let mut outcome = ToolOutcome::ok("Stara strona", serde_json::json!({"url":url}));
            assert!(
                annotate(&mut outcome, SearchEngine::Duckduckgo).is_err(),
                "{url}"
            );
            assert!(outcome.data.get("search_stage").is_none());
        }
    }
}
