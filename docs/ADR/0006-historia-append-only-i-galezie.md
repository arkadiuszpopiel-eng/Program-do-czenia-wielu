# ADR 0006 — Historia rozmowy append-only + gałęzie

| Pole | Wartość |
|---|---|
| Status | **Tymczasowy** — do potwierdzenia spike'iem (g), odroczonym do uzyskania klucza Anthropic |
| Data | 2026-09-30 |
| Powiązane sekcje planu | §2 (zasada 4), §5.3, §6.5, §13, §14.8, §16.2 (F0 spike g), §17 |

## Kontekst

Barge-in przerywa odpowiedź agentki w połowie zdania; użytkownik usłyszał tylko prefiks. Klasyczne podejście edytuje ostatnią turę asystenta do usłyszanego fragmentu. Według aktualnej dokumentacji Anthropic (§5.3) bloki myślenia Opus 5.5 są związane z modelem i rozmową, a edycja wcześniejszych tur może je unieważnić. Jednocześnie UI ma funkcje „edytuj i wyślij ponownie", „ponów innym modelem", „kontynuuj" — wszystkie zmieniają przeszłość. Plan (zasada 4) i tak wymaga event sourcingu.

## Decyzja

1. **Dziennik zdarzeń jest append-only.** Widoczna rozmowa to projekcja drzewa gałęzi.
2. **Nie edytujemy wcześniejszych tur.** „Edytuj", „ponów", „przekaż agentce" tworzą **nowe gałęzie**; każda gałąź to nieedytowana historia, więc bloki myślenia dostawcy pozostają ważne. Warianty odpowiedzi w UI: `‹ 1/3 ›`.
3. **IR trzyma osobno** `assistant_full` i `assistant_heard_prefix`.
4. **Adapter dostawcy decyduje o renderowaniu przerwania:**
   - Anthropic: pełna tura + dopisana notka „użytkownik usłyszał tylko: „…" i przerwał";
   - dostawcy z natywnym truncate (OpenAI Realtime `conversation.item.truncate`): obcięcie po stronie dostawcy.
5. „Usłyszany prefiks" wyznacza hierarchia: znaczniki słów z TTS → forced alignment → zliczanie odtworzonych próbek skorygowane o `GetStreamLatency` (flaga `przybliżone`). Metryka: ±1 słowo.
6. Klasyfikacja intencji przerwania działa na wejściu `{usłyszany_prefiks, nie_powiedziane, wypowiedź}`.

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Edycja ostatniej tury do usłyszanego prefiksu | może unieważnić bloki myślenia Anthropic; łamie append-only; niszczy replay i audyt |
| Wycinanie nieusłyszanej części u wszystkich dostawców jednolicie | dostawcy różnią się (natywny truncate vs brak); adapter musi decydować |
| Historia liniowa z „cofnij" | „ponów"/„edytuj" wymagają zachowania obu wersji; drzewo jest naturalną strukturą |
| Zapisywanie tylko prefiksu (bez `assistant_full`) | model nie wie, co „chciał" powiedzieć; klasyfikacja intencji („kontynuuj od punktu cięcia") wymaga pełnej treści |

## Konsekwencje

- Format IR i schemat zdarzeń wersjonowane od F1; upcastery i testy migracji (§13).
- UI: pozycja przewijania zachowana przy zmianie wariantu/gałęzi; rozgałęzienie do nowej sesji; „ukryj z widoku" nie usuwa z audytu.
- Koszt: dłuższy kontekst (pełne tury zamiast obciętych) — mierzony w `cost-meter`; cache promptów Anthropic (stabilny prefiks) łagodzi.
- **Do potwierdzenia w spike (g):** czy notka „usłyszał tylko…" jest przez model respektowana i czy bloki myślenia rzeczywiście unieważniają się przy edycji — bez klucza Anthropic nie da się tego sprawdzić.

## Jak cofnąć

- Jeśli spike (g) wykaże, że edycja tur jest bezpieczna u wszystkich używanych dostawców: adapter Anthropic może przejść na obcięcie, ale **append-only dziennika zostaje** (wymóg zasady 4, audytu i replay). Zmiana dotyczy tylko projekcji wysyłanej do modelu.
- Status ADR zmienia się na „Zaakceptowany" lub ADR jest zastępowany nowym po spike'u (g).
