//! Wspólne narzędzia testów `search-impl`.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

pub mod reindex;

use std::ops::Deref;
use std::sync::Arc;

use search_fake::HashEmbedder;
use search_impl::SqliteSearch;
use sessions_fake::TempDbProvider;

/// Usługa na bazach tymczasowych z embedderem atrapy.
pub struct Harness {
    pub provider: Arc<TempDbProvider>,
    pub search: SqliteSearch,
}

impl Deref for Harness {
    type Target = SqliteSearch;

    fn deref(&self) -> &SqliteSearch {
        &self.search
    }
}

pub fn harness() -> Harness {
    let provider = Arc::new(TempDbProvider::new().unwrap());
    let search = SqliteSearch::new(provider.clone(), Arc::new(HashEmbedder::new())).unwrap();
    Harness { provider, search }
}
