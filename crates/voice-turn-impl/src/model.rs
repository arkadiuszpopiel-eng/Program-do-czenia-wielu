//! Heurystyczny model końca tury oparty na tekście (zastępczy do czasu Smart Turn v3.2 ONNX).

use voice_turn_contract::{TurnError, TurnModel, TurnModelInput};

use crate::hesitation::{Hesitation, ends_clearly, hesitation};

/// Prawdopodobieństwo po pytaniu.
pub const P_QUESTION: f32 = 0.95;
/// Prawdopodobieństwo po kropce / wykrzykniku.
pub const P_CLEAR: f32 = 0.9;
/// Prawdopodobieństwo po hezytacji.
pub const P_HESITATION: f32 = 0.15;
/// Prawdopodobieństwo bez sygnałów.
pub const P_NEUTRAL: f32 = 0.55;

/// Model tekstowy: interpunkcja końcowa → wysoko, hezytacja → nisko, brak tekstu → neutralnie.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeuristicTurnModel;

impl TurnModel for HeuristicTurnModel {
    fn name(&self) -> &str {
        "heuristic-text"
    }

    fn end_probability(&self, input: &TurnModelInput<'_>) -> Result<f32, TurnError> {
        let text = input.partial_text.unwrap_or("").trim();
        if text.is_empty() {
            return Ok(P_NEUTRAL);
        }
        Ok(match hesitation(text) {
            Some(Hesitation::Filler | Hesitation::Unfinished) => P_HESITATION,
            None if text.ends_with('?') => P_QUESTION,
            None if ends_clearly(text) => P_CLEAR,
            None => P_NEUTRAL,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(text: Option<&str>) -> f32 {
        HeuristicTurnModel
            .end_probability(&TurnModelInput {
                audio: None,
                partial_text: text,
                silence_ms: 0,
            })
            .unwrap()
    }

    #[test]
    fn probabilities_follow_text() {
        assert_eq!(p(Some("Która godzina?")), P_QUESTION);
        assert_eq!(p(Some("Dziękuję.")), P_CLEAR);
        assert_eq!(p(Some("no i yyy")), P_HESITATION);
        assert_eq!(p(Some("otwórz pocztę")), P_NEUTRAL);
        assert_eq!(p(None), P_NEUTRAL);
        assert_eq!(HeuristicTurnModel.name(), "heuristic-text");
    }
}
