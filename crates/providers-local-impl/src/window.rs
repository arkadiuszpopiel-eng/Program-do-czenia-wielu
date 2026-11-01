//! Okno rozmowy w kontekście `llama-server`. Czat wysyła całą gałąź historii (append-only), a
//! serwer odrzuca żądanie, którego prompt nie mieści się w `-c` — więc do serwera idzie prompt
//! systemowy, narzędzia i najnowsze tury mieszczące się w kontekście uruchomienia pomniejszonym
//! o rezerwę na odpowiedź. Historia w sesji zostaje nietknięta; przycięte jest tylko to żądanie.

use std::borrow::Cow;

use providers_contract::{ChatRequest, ContentBlock, Message, Role, ToolResultPart};

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
///
/// Pierwsza wiadomość użytkownika (polecenie zadania, temat rozmowy) zostaje także po przycięciu,
/// jeśli zajmuje najwyżej ćwierć budżetu: bez niej mały model po długiej pętli narzędzi albo przy
/// weryfikacji widział samo „sprawdź swoją odpowiedź” — bez celu (przegląd 1.11). Trafia na początek
/// okna razem z informacją, ile wiadomości pominięto ([`omitted_note`]).
///
/// Gdy okno i tak przekracza budżet (długa pętla narzędzi bez tury użytkownika w środku), starsze
/// wyniki narzędzi są skracane do początku i końca ([`shrink_results`]) — zamiast żądania, które
/// serwer odrzuci, a zadanie bez modelu zapasowego zakończy błędem.
pub fn fit(request: &ChatRequest, ctx: u32, reserve: u32) -> Fitted<'_> {
    let budget = budget_of(request, ctx, reserve);
    let fitted = fit_window(request, budget);
    let used: u32 = fitted.request.messages.iter().map(message_tokens).sum();
    if used <= budget {
        return fitted;
    }
    let mut request = fitted.request.into_owned();
    // Najpierw początek i koniec każdego starszego wyniku; gdy to za mało — sama adnotacja.
    for (head, tail) in [(KEEP_HEAD, KEEP_TAIL), (0, 0)] {
        let used: u32 = request.messages.iter().map(message_tokens).sum();
        if used <= budget {
            break;
        }
        shrink_results(&mut request.messages, used - budget, head, tail);
    }
    Fitted {
        request: Cow::Owned(request),
        dropped: fitted.dropped,
    }
}

/// Wynik narzędzia krótszy niż to (znaki) nie jest skracany.
const SHRINK_MIN: usize = 200;
/// Ile znaków początku i końca zostaje w skróconym wyniku (pierwsze przejście).
const KEEP_HEAD: usize = 300;
const KEEP_TAIL: usize = 150;

/// Skraca teksty wyników narzędzi od najstarszych (bez dwóch ostatnich wiadomości — bieżący krok
/// pętli zostaje w całości) do `head` + `tail` znaków, aż oszczędność pokryje `over` tokenów.
fn shrink_results(messages: &mut [Message], over: u32, head: usize, tail: usize) {
    let keep = messages.len().saturating_sub(2);
    let mut saved = 0u32;
    for message in &mut messages[..keep] {
        for block in &mut message.content {
            let ContentBlock::ToolResult(result) = block else {
                continue;
            };
            for part in &mut result.content {
                let ToolResultPart::Text { text } = part else {
                    continue;
                };
                let count = text.chars().count();
                let cut = count.saturating_sub(head + tail);
                if saved >= over || count < SHRINK_MIN || cut < SHRINK_MIN || text.starts_with("[…")
                {
                    continue;
                }
                let first: String = text.chars().take(head).collect();
                let last: String = text.chars().skip(count - tail).collect();
                let short = format!(
                    "{first}[… pominięto {cut} znaków starszego wyniku — kontekst modelu lokalnego;                      wywołaj narzędzie ponownie, jeśli potrzebujesz całości …]{last}"
                );
                saved = saved.saturating_add(
                    message_part_tokens(text).saturating_sub(message_part_tokens(&short)),
                );
                *text = short;
            }
        }
    }
}

/// Szacunek tokenów tekstu tak, jak liczy go okno (zapis JSON — ze znakami ucieczki).
fn message_part_tokens(text: &str) -> u32 {
    serde_json::to_string(text).map_or(0, |s| estimate_tokens(&s))
}

fn budget_of(request: &ChatRequest, ctx: u32, reserve: u32) -> u32 {
    let fixed = request
        .system
        .as_deref()
        .map_or(0, estimate_tokens)
        .saturating_add(serde_json::to_string(&request.tools).map_or(0, |s| estimate_tokens(&s)));
    ctx.saturating_sub(reserve)
        .saturating_sub(WINDOW_MARGIN)
        .saturating_sub(fixed)
}

fn fit_window(request: &ChatRequest, budget: u32) -> Fitted<'_> {
    let messages = &request.messages;
    let start = window_start(messages, budget);
    if start == 0 {
        return Fitted {
            request: Cow::Borrowed(request),
            dropped: 0,
        };
    }
    let anchor = messages
        .first()
        .filter(|m| opens_window(m))
        .map(|m| (m, message_tokens(m).saturating_add(NOTE_TOKENS)))
        .filter(|(_, cost)| *cost <= budget / 4);
    let Some((anchor, cost)) = anchor else {
        let mut trimmed = request.clone();
        trimmed.messages = messages[start..].to_vec();
        return Fitted {
            request: Cow::Owned(trimmed),
            dropped: start,
        };
    };
    let start = window_start(messages, budget.saturating_sub(cost)).max(1);
    if start == 1 {
        return Fitted {
            request: Cow::Borrowed(request),
            dropped: 0,
        };
    }
    let dropped = start - 1;
    let mut window = messages[start..].to_vec();
    // Polecenie i informacja o pominięciu w pierwszej turze okna — jedna wiadomość użytkownika,
    // więc naprzemienność ról zostaje zachowana.
    let mut head = anchor.content.clone();
    head.push(ContentBlock::text(omitted_note(dropped)));
    head.append(&mut window[0].content);
    window[0].content = head;
    let mut trimmed = request.clone();
    trimmed.messages = window;
    Fitted {
        request: Cow::Owned(trimmed),
        dropped,
    }
}

/// Szacunek tokenów informacji o pominięciu.
const NOTE_TOKENS: u32 = 96;

/// Informacja dla modelu o wiadomościach pominiętych przez okno kontekstu.
pub fn omitted_note(dropped: usize) -> String {
    format!(
        "[Pominięto {dropped} wcześniejszych wiadomości — nie mieszczą się w kontekście modelu \
         lokalnego. Jeśli czegoś z nich potrzebujesz, powiedz o tym zamiast zgadywać.]"
    )
}

/// Początek okna w budżecie `budget` (indeks pierwszej wysyłanej wiadomości).
fn window_start(messages: &[Message], budget: u32) -> usize {
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
    (start..messages.len())
        .find(|&i| opens_window(&messages[i]))
        .or_else(|| (0..start).rev().find(|&i| opens_window(&messages[i])))
        .unwrap_or(0)
}

/// Czy wiadomość może otwierać okno: tura użytkownika bez wyników narzędzi.
fn opens_window(message: &Message) -> bool {
    message.role == Role::User
        && !message
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolResult(_)))
}
