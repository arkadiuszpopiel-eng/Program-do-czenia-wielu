//! Komendy wbudowanej instrukcji (`help_*`); treść i wyszukiwanie: `app_modules::help`.

use app_modules::help;

use crate::core::AppCore;
use crate::dto::{HelpHit, HelpPage, HelpTopic};
use crate::error::AppError;

impl AppCore {
    /// `help_topics`: spis działów instrukcji.
    pub async fn help_topics(&self) -> Result<Vec<HelpTopic>, AppError> {
        Ok(help::topics())
    }

    /// `help_page`: dział z treścią (HTML zsanitowany w Rust).
    pub async fn help_page(&self, id: String) -> Result<HelpPage, AppError> {
        help::page(&id).ok_or_else(|| AppError::not_found(format!("dział instrukcji „{id}”")))
    }

    /// `help_search`: akapity instrukcji ze wszystkimi słowami zapytania.
    pub async fn help_search(&self, query: String) -> Result<Vec<HelpHit>, AppError> {
        Ok(help::search(&query))
    }
}
