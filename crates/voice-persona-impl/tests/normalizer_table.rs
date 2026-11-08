//! Tabelaryczne testy normalizatora PL (≥ 80 przypadków): liczby, daty, godziny, waluty,
//! procenty, jednostki, skróty, URL/e-mail, kod, mieszany PL/EN.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use voice_persona_contract::Lexicon;
use voice_persona_impl::normalize;

const CASES: &[(&str, &str)] = &[
    // Liczebniki główne i rodzaj.
    ("Mam 0 błędów.", "Mam zero błędów."),
    ("Mam 1 kota.", "Mam jeden kota."),
    ("Są 2 minuty.", "Są dwie minuty."),
    ("Zostały 22 osoby.", "Zostały dwadzieścia dwie osoby."),
    ("Mam 1 zadanie.", "Mam jedno zadanie."),
    ("To 1 minuta.", "To jedna minuta."),
    (
        "Dostałam 21 wiadomości.",
        "Dostałam dwadzieścia jeden wiadomości.",
    ),
    ("Liczba 13.", "Liczba trzynaście."),
    ("Liczba 100.", "Liczba sto."),
    (
        "Liczba 215 i 999.",
        "Liczba dwieście piętnaście i dziewięćset dziewięćdziesiąt dziewięć.",
    ),
    ("Rekordów: 1000.", "Rekordów: tysiąc."),
    ("Rekordów: 2000.", "Rekordów: dwa tysiące."),
    ("Rekordów: 12 500.", "Rekordów: dwanaście tysięcy pięćset."),
    ("Rekordów: 1 000 000.", "Rekordów: milion."),
    ("Mamy 3000000 wierszy.", "Mamy trzy miliony wierszy."),
    (
        "Max 999999999999.",
        "Max dziewięćset dziewięćdziesiąt dziewięć miliardów dziewięćset dziewięćdziesiąt dziewięć milionów dziewięćset dziewięćdziesiąt dziewięć tysięcy dziewięćset dziewięćdziesiąt dziewięć.",
    ),
    (
        "Id 1234567890123.",
        "Id jeden dwa trzy cztery pięć sześć siedem osiem dziewięć zero jeden dwa trzy.",
    ),
    ("Kod 007.", "Kod zero zero siedem."),
    ("Temperatura -5.", "Temperatura minus pięć."),
    ("Wynik 3,5 punktu.", "Wynik trzy przecinek pięć punktu."),
    (
        "Wersja 3.12.1 wyszła.",
        "Wersja trzy kropka dwanaście kropka jeden wyszła.",
    ),
    ("Python 3.5 działa.", "Python trzy kropka pięć działa."),
    ("Model GPT-4 odpowiada.", "Model GPT cztery odpowiada."),
    ("Plik mp3 gra.", "Plik mp trzy gra."),
    ("Od 5 do 10 osób.", "Od pięciu do dziesięciu osób."),
    ("Około 21 dni.", "Około dwudziestu jeden dni."),
    ("Czekaj 5-10 minut.", "Czekaj pięć do dziesięciu minut."),
    ("To 2–3 dni.", "To dwa do trzech dni."),
    // Daty i lata.
    (
        "Spotkanie 1 października 2026.",
        "Spotkanie pierwszego października dwa tysiące dwudziestego szóstego roku.",
    ),
    (
        "Dnia 1 października 2026 r. ruszamy.",
        "Dnia pierwszego października dwa tysiące dwudziestego szóstego roku ruszamy.",
    ),
    (
        "Termin: 31 grudnia.",
        "Termin: trzydziestego pierwszego grudnia.",
    ),
    ("Do 5 maja.", "Do piątego maja."),
    ("Urlop 1-5 maja.", "Urlop pierwszego do piątego maja."),
    (
        "Data 01.10.2026.",
        "Data pierwszego października dwa tysiące dwudziestego szóstego roku.",
    ),
    (
        "Data 2026-10-01.",
        "Data pierwszego października dwa tysiące dwudziestego szóstego roku.",
    ),
    (
        "Data 22.02.1999 r. jest stara.",
        "Data dwudziestego drugiego lutego tysiąc dziewięćset dziewięćdziesiątego dziewiątego roku jest stara.",
    ),
    (
        "W 2026 r. zrobimy to.",
        "W dwa tysiące dwudziestym szóstym roku zrobimy to.",
    ),
    ("Od 2020 roku.", "Od dwa tysiące dwudziestego roku."),
    (
        "Rok 2000 rok był dziwny.",
        "Rok dwutysięczny rok był dziwny.",
    ),
    (
        "To było w 1999.",
        "To było w tysiąc dziewięćset dziewięćdziesiątym dziewiątym.",
    ),
    (
        "Stało się to w 2026 r. Potem nic.",
        "Stało się to w dwa tysiące dwudziestym szóstym roku. Potem nic.",
    ),
    // Godziny.
    ("Jest 14:05.", "Jest czternasta zero pięć."),
    ("Jest 8:00.", "Jest ósma."),
    ("Spotkanie o 9:30.", "Spotkanie o dziewiątej trzydzieści."),
    ("Przed 12:15 wyjdę.", "Przed dwunastą piętnaście wyjdę."),
    ("Czynne 14:00-16:00.", "Czynne czternasta do szesnastej."),
    ("Budzik na 7:45.", "Budzik na siódmą czterdzieści pięć."),
    ("Jest 0:30.", "Jest zero trzydzieści."),
    ("Spotkanie o godz. 14.", "Spotkanie o godzinie czternastej."),
    ("Od godz. 8:15.", "Od godziny ósmej piętnaście."),
    (
        "Godz. 23:59 to koniec.",
        "Godzina dwudziesta trzecia pięćdziesiąt dziewięć to koniec.",
    ),
    // Waluty.
    (
        "Kosztuje 12,50 zł.",
        "Kosztuje dwanaście złotych pięćdziesiąt groszy.",
    ),
    ("Kosztuje 1 zł.", "Kosztuje jeden złoty."),
    ("Kosztuje 2 zł.", "Kosztuje dwa złote."),
    ("Kosztuje 5 zł.", "Kosztuje pięć złotych."),
    ("Kosztuje 22 zł.", "Kosztuje dwadzieścia dwa złote."),
    (
        "Kosztuje 0,99 zł.",
        "Kosztuje dziewięćdziesiąt dziewięć groszy.",
    ),
    ("Kosztuje 3,01 PLN.", "Kosztuje trzy złote jeden grosz."),
    (
        "Kosztuje 12,5 zł.",
        "Kosztuje dwanaście złotych pięćdziesiąt groszy.",
    ),
    ("Kosztuje $3.", "Kosztuje trzy dolary."),
    ("Kosztuje $1.", "Kosztuje jeden dolar."),
    (
        "Kosztuje $3.50.",
        "Kosztuje trzy dolary pięćdziesiąt centów.",
    ),
    ("Kosztuje 5 USD.", "Kosztuje pięć dolarów."),
    ("Kosztuje 5 €.", "Kosztuje pięć euro."),
    ("Kosztuje €1.", "Kosztuje jedno euro."),
    ("Kosztuje £2.", "Kosztuje dwa funty."),
    (
        "Budżet 12 tys. zł na rok.",
        "Budżet dwanaście tysięcy złotych na rok.",
    ),
    (
        "Budżet 1,5 mln zł.",
        "Budżet jeden przecinek pięć miliona złotych.",
    ),
    ("Budżet 2 mln.", "Budżet dwa miliony."),
    ("Od 12,50 zł.", "Od dwunastu złotych pięćdziesięciu groszy."),
    ("Wydałam $3 mln.", "Wydałam trzy miliony dolarów."),
    // Procenty i jednostki.
    ("Wzrost o 15%.", "Wzrost o piętnaście procent."),
    ("Wzrost o 2,5%.", "Wzrost o dwa przecinek pięć procent."),
    ("Od 1% w górę.", "Od jednego procenta w górę."),
    ("Jechała 5 km.", "Jechała pięć kilometrów."),
    ("Przeszłam 1 km.", "Przeszłam jeden kilometr."),
    ("Dystans 3 km.", "Dystans trzy kilometry."),
    ("Dystans 2,5 km.", "Dystans dwa przecinek pięć kilometra."),
    ("Waży 5kg.", "Waży pięć kilogramów."),
    ("Dysk 512 GB.", "Dysk pięćset dwanaście gigabajtów."),
    ("Plik 2 MB.", "Plik dwa megabajty."),
    ("Jest 21°C.", "Jest dwadzieścia jeden stopni Celsjusza."),
    ("Jest -2 °C.", "Jest minus dwa stopnie Celsjusza."),
    (
        "Jedzie 50 km/h.",
        "Jedzie pięćdziesiąt kilometrów na godzinę.",
    ),
    ("Opóźnienie 1 ms.", "Opóźnienie jedna milisekunda."),
    (
        "Opóźnienie 22 ms.",
        "Opóźnienie dwadzieścia dwie milisekundy.",
    ),
    ("Czekaj 2 h.", "Czekaj dwie godziny."),
    ("Około 5 km stąd.", "Około pięciu kilometrów stąd."),
    // Skróty.
    ("To np. jabłko.", "To na przykład jabłko."),
    ("Np. tak.", "Na przykład tak."),
    ("Jabłka, gruszki itd.", "Jabłka, gruszki i tak dalej."),
    (
        "Jabłka itd. Potem gruszki.",
        "Jabłka i tak dalej. Potem gruszki.",
    ),
    ("Kupiłam m.in. mleko.", "Kupiłam między innymi mleko."),
    ("Tzn. nie wiem.", "To znaczy nie wiem."),
    ("Rozmawiałam z dr Nowak.", "Rozmawiałam z doktorem Nowak."),
    ("Dr Kowalski przyjmuje.", "Doktor Kowalski przyjmuje."),
    ("Mieszkam na ul. Długiej.", "Mieszkam na ulicy Długiej."),
    ("Ul. Długa jest zamknięta.", "Ulica Długa jest zamknięta."),
    ("Mam ok. 5 minut.", "Mam około pięciu minut."),
    ("Jest ok.", "Jest ok."),
    ("Zob. rozdział 2.", "Zobacz rozdział dwa."),
    ("Pokój nr 12.", "Pokój numer dwanaście."),
    (
        "W 50 r. p.n.e. było inaczej.",
        "W pięćdziesiątym roku przed naszą erą było inaczej.",
    ),
    ("To mój 2 rok studiów.", "To mój drugi rok studiów."),
    ("Kilka tys. osób.", "Kilka tysięcy osób."),
    (
        "Zadzwoń: tel. 600 700 800.",
        "Zadzwoń: telefon sześćset, siedemset, osiemset.",
    ),
    (
        "Numer +48 600 700 800.",
        "Numer plus czterdzieści osiem, sześćset, siedemset, osiemset.",
    ),
    // URL, e-mail, kod.
    (
        "Zobacz https://github.com/anthropics/claude-code teraz.",
        "Zobacz link do github kropka com teraz.",
    ),
    ("Wejdź na www.onet.pl.", "Wejdź na link do onet kropka pl."),
    (
        "Napisz na jan.kowalski@firma.pl, proszę.",
        "Napisz na jan kropka kowalski małpa firma kropka pl, proszę.",
    ),
    (
        "Otwórz main.rs i popraw.",
        "Otwórz main kropka rs i popraw.",
    ),
    (
        "Uruchom `cargo test --all` teraz.",
        "Uruchom (kod na ekranie) teraz.",
    ),
    ("Użyj `cargo` do budowania.", "Użyj cargo do budowania."),
    (
        "Kod:\n```rust\nfn main() { let x = 5; }\n```\nGotowe.",
        "Kod:\n(kod na ekranie)\nGotowe.",
    ),
    // Mieszany PL/EN zostaje.
    (
        "Zrób deploy na staging i code review.",
        "Zrób deploy na staging i code review.",
    ),
    (
        "Pull request jest ready to merge.",
        "Pull request jest ready to merge.",
    ),
];

#[test]
fn table_cases() {
    assert!(CASES.len() >= 80, "za mało przypadków: {}", CASES.len());
    let lexicon = Lexicon::new();
    let mut failures = Vec::new();
    for (input, want) in CASES {
        let got = normalize(input, &lexicon);
        if got != *want {
            failures.push(format!(
                "\n  wejście:   {input}\n  oczekiwano: {want}\n  otrzymano:  {got}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} błędów:{}",
        failures.len(),
        failures.concat()
    );
}
