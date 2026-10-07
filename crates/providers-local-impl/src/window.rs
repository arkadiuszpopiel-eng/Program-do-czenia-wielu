//! Okno rozmowy w kontekście `llama-server`. Czat wysyła całą gałąź historii (append-only), a
//! serwer odrzuca żądanie, którego prompt nie mieści się w `-c` — więc do serwera idzie prompt
//! systemowy, narzędzia i najnowsze tury mieszczące się w kontekście uruchomienia pomniejszonym
//! o rezerwę na odpowiedź. Historia w sesji zostaje nietknięta; przycięte jest tylko to żądanie.

use std::borrow::Cow;

use providers_contract::{ChatRequest, ContentBlock, Message, Role};

/// Zapas na szablon czatu, tokeny specjalne i błąd szacunku.
pub const WINDOW_MARGIN: u32 = 256;
/// Narzut na wiadomość (znaczniki roli w szablonie czatu).
const PER_MESSAGE: u32 = 8;

/// Zawyżony szacunek tokenów: bajty UTF-8 / 3 (słownik APT4 Bielika ≈ 4 znaki na token po
/// polsku, ogólne tokenizery ≈ 3; polskie litery mają 2 bajty — szacunek tylko w górę).
pub fn estimate_tokens(text: &str) -> u32 {
    u32::try_from(text.len().div_ceil(3)).unwrap_or(u32::MAX)
}

fn message_tokens(message: &Message) -> u32 {
    let body = serde_json::to_string(&message.content).map_or(0, |s| estimate_tokens(&s));
    body.saturating_add(PER_MESSAGE)
}

/// Rezerwa na odpowiedź: `max_tokens`, ale najwyżej ćwierć kontekstu — serwer i tak kończy
/// generowanie na granicy kontekstu, a pełne `max_tokens` (domyślnie połowa) zjadałoby historię.
pub fn reply_reserve(ctx: u32, max_tokens: u32) -> u32 {
    max_tokens.min(ctx / 4)
}

/// Wynik dopasowania: żądanie do wysłania i liczba pominiętych najstarszych wiadomości.
pub struct Fitted<'a> {
    /// Żądanie (pożyczone, gdy nic nie przycięto).
    pub request: Cow<'a, ChatRequest>,
    /// Ile najstarszych wiadomości pominięto.
    pub dropped: usize,
}

/// Dopasowuje historię do kontekstu `ctx` przy rezerwie na odpowiedź `reserve`. Ostatnia
/// wiadomość zostaje zawsze (gdyby sama się nie mieściła, odpowie błędem serwer). Okno zaczyna
/// się od wiadomości użytkownika bez wyników narzędzi — para „wywołanie → wynik” nie jest rozrywana,
/// a szablony czatu wymagające naprzemienności ról dostają poprawny początek.
pub fn fit(request: &ChatRequest, ctx: u32, reserve: u32) -> Fitted<'_> {
    let fixed = request
        .system
        .as_deref()
        .map_or(0, estimate_tokens)
        .saturating_add(serde_json::to_string(&request.tools).map_or(0, |s| estimate_tokens(&s)));
    let budget = ctx
        .saturating_sub(reserve)
        .saturating_sub(WINDOW_MARGIN)
        .saturating_sub(fixed);
    let messages = &request.messages;
    let mut used = 0u32;
    let mut start = messages.len();
    for (i, message) in messages.iter().enumerate().rev() {
        let cost = message_tokens(message);
        if start < messages.len() && used.saturating_add(cost) > budget {
            break;
        }
        used = used.saturating_add(cost);
        start = i;
    }
    // Pierwsza wiadomość otwierająca okno w budżecie; gdy takiej nie ma (np. długa seria
    // wywołań narzędzi), najbliższa przed nim — ponad budżet, ale bez osieroconych wyników.
    start = (start..messages.len())
        .find(|&i| opens_window(&messages[i]))
        .or_else(|| (0..start).rev().find(|&i| opens_window(&messages[i])))
        .unwrap_or(0);
    if start == 0 {
        return Fitted {
            request: Cow::Borrowed(request),
            dropped: 0,
        };
    }
    let mut trimmed = request.clone();
    trimmed.messages = messages[start..].to_vec();
    Fitted {
        request: Cow::Owned(trimmed),
        dropped: start,
    }
}

/// Czy wiadomość może otwierać okno: tura użytkownika bez wyników narzędzi.
fn opens_window(message: &Message) -> bool {
    message.role == Role::User
        && !message
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolResult(_)))
}
