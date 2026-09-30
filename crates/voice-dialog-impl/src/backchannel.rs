//! Klasyfikacja wypowiedzi w trakcie mowy agentki: backchannel („mhm”, „nie no, dobrze”) vs treść.

use voice_cmd_contract::fold;

/// Klasa transkryptu częściowego.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackchannelClass {
    /// Brak tekstu (STT jeszcze nie odpowiedział).
    Empty,
    /// Pełna fraza backchannelu.
    Full,
    /// Początek frazy backchannelu („nie” → „nie no, dobrze”) — czekamy.
    Prefix,
    /// Treść — przerwanie.
    Content,
}

/// Słowa po `fold` (bez interpunkcji i polskich znaków).
pub fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(fold)
        .filter(|w| !w.is_empty())
        .collect()
}

/// Klasyfikuje transkrypt względem fraz backchannelu (frazy po `fold`).
pub fn classify(text: &str, phrases: &[String]) -> BackchannelClass {
    let w = words(text);
    if w.is_empty() {
        return BackchannelClass::Empty;
    }
    let mut prefix = false;
    for phrase in phrases {
        let p = words(phrase);
        if p == w {
            return BackchannelClass::Full;
        }
        if p.len() > w.len() && p.starts_with(&w) {
            prefix = true;
        }
    }
    if prefix {
        BackchannelClass::Prefix
    } else {
        BackchannelClass::Content
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voice_dialog_contract::DialogConfig;

    #[test]
    fn classes() {
        let p = DialogConfig::default().backchannel_phrases;
        assert_eq!(classify("", &p), BackchannelClass::Empty);
        assert_eq!(classify("Mhm.", &p), BackchannelClass::Full);
        assert_eq!(classify("nie no, dobrze", &p), BackchannelClass::Full);
        assert_eq!(classify("nie", &p), BackchannelClass::Prefix);
        assert_eq!(classify("tak, ale", &p), BackchannelClass::Content);
        assert_eq!(classify("nie, chodziło mi", &p), BackchannelClass::Content);
        assert_eq!(classify("W porządku", &p), BackchannelClass::Full);
    }
}
