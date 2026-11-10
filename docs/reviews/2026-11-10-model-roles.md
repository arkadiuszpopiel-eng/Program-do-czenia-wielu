# Role modeli: główny8–12B, lekka pomoc lokalna i chmurowa — 10.11.2026

Aktualne [rozliczenieCI177/Windows99, wynikQwenCPU i korekta](2026-11-10-model-roles-ci.md).
CI177/Windows99 są niezaliczone; nowe poprawki wymagają osobnegoCI.

Nowe polecenie właściciela po zakończeniu nocnego okna. Prace wznowiono wyłącznie w tym
zakresie; oba nocne harmonogramy pozostają wyłączone. Poprzedni raport i błędy zachowane.

## Wynikowe zachowanie

- Qwen3-8B Q4_K_M jest głównym lokalnym modelem: domyślny dostawca, pierwszy LLM onboardingu
  i pakiety1–6 na osobnej karcie12GB. Tryb bez thinking ogranicza ukryte zużycie wyjścia;
  nie zwiększono limitów. Domyślnie8k kontekstu; większy kontekst podlega budżetowi pamięci.
- Bielik11B v2.3 Q4_K_M jest alternatywą tekstową. Brak potwierdzonych narzędzi: tools=false.
  Ma mniejszy zapas VRAM, więc zalecanym punktem wyjścia pozostaje8B.
- Bielik1.5B jest pomocnikiem do lekkich streszczeń/porządków. Gdy nie ma głównego modelu,
  nie zostaje automatycznie jego zastępcą. W ścieżce local-light pozostaje pomocniczy.
- W zwykłej pracy z profilem8–12B Router najpierw próbuje model lokalny. Dostępna, znana
  kontu Luna/Haiku lub odpowiednik jest pomocą po problemie lokalnym, także bez pobranego
  modelu. Nie tworzy konta, klucza ani udawanej odpowiedzi API.
- Trudne zadania i eskalacja po odrzuceniu odpowiedzi zachowują hierarchię. Ręczne kolejki,
  przypięcia, ustawienia właściciela, prywatność, zgodność, możliwości i budżety są respektowane.
  Ta zmiana nie nadaje modelowi dodatkowych uprawnień do narzędzi.

Lekka chmura może też sprawdzić wynik lokalnego8B. Regresja z samąLuną wykryła
powtórny wybór tego samego lokalnego modelu; poprawka ma ponowne64/0/0 i Clippy0.
Sprawdzanie nadal respektuje ręczne kolejki i mocny szczebel ustawiony przez właściciela.

## Pamięć docelowej RTX2060 12GB

Wszystko wMiB; szacunki, nie pomiarGPU. Wagi/narzut zawierają550MiB runtime; dołożono
osobno1500MiB STT,600MiB TTS i768MiB ekranu. Rzeczywista wolna pamięć może być mniejsza.

| Model | Wagi/narzut | KV przy8k | Razem z mową i ekranem | Zapas z12288MiB |
| --- | ---: | ---: | ---: | ---: |
| Qwen3-8B Q4_K_M | 5345 | 1152 | 9365 | 2923 |
| Bielik11B Q4_K_M | 6963 | 1600 | 11431 | 857 |

Qwen16k:10517MiB łącznie,zapas1771MiB. Bielik16k nie mieści tego samego budżetu i schodzi
w planie do8k. Dla własnego obciążenia większy wariant może wymagać przeniesienia mowy naCPU.
Oficjalne rewizje, wielkości iSHA256 przypięto. Lokalny nagłówek Qwen potwierdza36 warstw
oraz144MiB KV/1k; wcześniejszy próbny wpis32/128 był zaniżony i nie trafił do produkcji.
[Metadane i architektury](artifacts/main-model-metadata.json). Licencje/karty pozostają
niepotwierdzone przez człowieka; nie zmieniono polityki licencji zależności.

## Testy i ograniczenia

| Moduł | RED | Końcowy wynik lokalny |
| --- | --- | --- |
| providers-local #65 | 1/3; osobny fallback0/1 | 80/0/0,12 wyników celów; Clippy0; pamięć5/5 |
| app-models #66 | 0/3 | 85/0/1,17 wyników celów; Clippy0 |
| Router #67 | 3/3 po korekcie fixture; brak głównego0/1 | 64/0/0,16 wyników celów; Clippy0 |

Zapis wyników: zaliczone/błędy/ignorowane. Pierwszy build app-models zakończył linker101
po wyczerpaniu miejsca. Po zachowaniu binariów dawnych prób w zweryfikowanym gzip i usunięciu
nieużywanego cache pełne testy przeszły. PierwszyClippy Routera wykrył nieużywany import;
naprawiono bez allow. Wszystkie pierwotne logi i kody zachowane w scratch/model-roles.
Ignorowany live_catalog nie został tutaj wykonany. Nie zmieniono progów ewaluacji.

Testy z atrapami sprawdzają brak konta/API, niedostępny model, blokady prywatności/budżetu,
trudne zadanie, ręczny wybór oraz przypadki Alfy,Bety,Gamy,Delty i własnejLuny. PełnyRouter
obejmuje wcześniejsze regresje Stop i zakazu odtwarzania rozpoczętego narzędzia. Nie jest to
odbiór płatnegoAPI lub semantyki odpowiedzi person. Nowy head wymaga własnego pełnegoCI;
nie przypisujemy mu zielonych CI176/Windows98 poprzednika4c683568.

Końcowe lint,format,102 strony HTML i odnośniki: kod0; schematy bez zmian.
Pierwszy lint wykrył401 linii zamiast400: usunięto pustą linię dokumentacji, bez zmiany limitu.
[Końcowe kontrole](artifacts/model-roles-final-checks.json). Logika połączonych modułów
odpowiada sprawdzonym osobnym przyrostom.

## Dalszy odbiór

#41/#48 zachowują historię: Bielik1.5B miał50/50 fikcyjnych potwierdzeń w obu wariantach#58.
Po zmianie przeznaczenia główne kryteria należy sprawdzić na8–12B, bez uznawania dawnych
porażek za sukces. Qwen miał30/30 w pojedynczej sondzie, lecz pełnyCoreCPU przekroczyłIdle
Bety po2/40 próbach (38 niewykonanych). GPU12GB, prawdziwy mikrofon, rzeczywisteAPI/hybryda
i jakość nowych modeli pozostają do odbioru. Produkcyjny dostawcaWWW nadal niewybrany.

Następnie: niezależny przegląd i pełneCI, próby8k/16k i narzędzi naRTX2060 12GB, powtarzalna
jakość wszystkich person, rzeczywista pomoc kontAPI zgodnie z ich uprawnieniami. Bez merge,
wydania lub deklaracji gotowościEnterprise.

Szczegóły: [#65](2026-11-10-local-main-models.md), [#66](2026-11-10-main-model-profile.md),
[#67](2026-11-10-main-local-routing.md). Nocne raporty nie są usuwane ani przepisywane.
