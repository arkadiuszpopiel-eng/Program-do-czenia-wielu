//! Deterministyczny rdzeń rozpoznawania komend z gramatyki (jak `VadMachine` czy `DialogMachine`
//! w innych kontraktach): czyste funkcje bez I/O, współdzielone przez `voice-cmd-impl` (który je
//! udostępnia jako implementację modułu) i runnery ewaluacji (`voice-pipeline` / zestaw F2), które
//! nie mogą zależeć od cudzego `-impl`.

mod fuzzy;
mod pattern;
mod recognizer;

pub use fuzzy::{EXACT, ONE_EDIT, TWO_EDITS, levenshtein, word_score};
pub use recognizer::GrammarRecognizer;
