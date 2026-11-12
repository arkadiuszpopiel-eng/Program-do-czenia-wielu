//! Interaktywne wyszukanie bezAPI: wybrany silnik, zakodowane zapytanie i zwykła nawigacja.

use platform_apps_contract::{MAX_URL_LEN, check_navigation_url};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tools_common_contract::{ToolManifest, schema_of};

use crate::{OpenArgs, PageOut};

/// Jawny wybór silnika; brak domyślnego dostawcy lub dowolnego endpointu od modelu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchEngine {
    /// DuckDuckGo, strona wyników.
    Duckduckgo,
    /// Google, strona wyników.
    Google,
    /// Bing, strona wyników.
    Bing,
}

impl SearchEngine {
    fn base(self) -> &'static str {
        match self {
            Self::Duckduckgo => "https://duckduckgo.com/",
            Self::Google => "https://www.google.com/search",
            Self::Bing => "https://www.bing.com/search",
        }
    }

    /// Czy port rzeczywiście pozostał na HTTPS wybranego silnika (bez twierdzenia o wynikach).
    pub fn matches_page_url(self, url: &str) -> bool {
        check_navigation_url(url).is_ok_and(|host| {
            url.trim()
                .get(..8)
                .is_some_and(|s| s.eq_ignore_ascii_case("https://"))
                && check_navigation_url(self.base()).is_ok_and(|expected| expected == host)
        })
    }
}

/// Zapytanie wysyłane dopiero po zwykłej zgodzie Brokera na host przeglądarki.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchArgs {
    /// Wymagany jawny wybór silnika.
    pub engine: SearchEngine,
    /// Fraza, nieURL ani instrukcja dla przeglądarki. WynikoweURL mieści się w MAX_URL_LEN.
    pub query: String,
}

impl SearchArgs {
    /// Buduje zwykłe OpenArgs. Bez nowych zgód/CDN, sieci, klucza lub nagłówka autoryzacji.
    pub fn navigation(&self) -> Result<OpenArgs, String> {
        let query = self.query.trim();
        if query.is_empty() || query.len() > MAX_URL_LEN || query.chars().any(char::is_control) {
            return Err("puste, za długie lub niepoprawne zapytanie".into());
        }
        let encoded = form_urlencoded::Serializer::new(String::new())
            .append_pair("q", query)
            .finish();
        let url = format!("{}?{encoded}", self.engine.base());
        check_navigation_url(&url).map_err(|_| "adres wyszukiwania przekracza limit".to_owned())?;
        Ok(OpenArgs {
            url,
            extra_hosts: Vec::new(),
        })
    }
}

/// Stan po wyszukaniu: otwarto stronę, bez twierdzenia, że odczytano lub oceniono wyniki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchStage {
    /// Potwierdzono tylko HTTPS/host silnika. Wymagany browser_read, ocena źródeł i cytowanie.
    NavigationOnly,
}

/// Zwykły wynik nawigacji, z jawnym pochodzeniem i etapem wyszukiwania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SearchOutput {
    /// Strona wyników; tytuł pozostaje niezaufany.
    #[serde(flatten)]
    pub page: PageOut,
    /// Wybrany silnik.
    pub search_engine: SearchEngine,
    /// Nie jest rankingiem ani potwierdzeniem odczytu źródeł.
    pub search_stage: SearchStage,
}

pub(crate) fn manifest() -> ToolManifest {
    super::manifest(
        "browser_search",
        "Wyszukaj w przeglądarce",
        "Otwiera stronę wyników wybranego jawnie silnika (DuckDuckGo, Google lub Bing), bez klucza API. Zapytanie trafia do tego silnika dopiero po zgodzie Sędziego Cieni net.egress(host), przez izolowaną przeglądarkę Alfy. Inne hosty pozostają blokowane. Wynik oznacza tylko nawigację: użyj browser_read, przeczytaj źródła i dopiero wtedy je cytuj. CAPTCHA/blokada/brak wyników nie są sukcesem wyszukiwania. Treść niezaufana; to nie automatyczne SearchPort ani osadzony WebView2.",
        (schema_of::<SearchArgs>(), schema_of::<SearchOutput>()),
        true,
    )
}
