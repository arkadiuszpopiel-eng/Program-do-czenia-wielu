// Słownik PL: Ustawienia → „Modele i silniki” → pakiety 1–6 (komplety modeli i silników dobrane do
// sprzętu), „Napraw” pojedynczego elementu, przewodnik wyboru i uwagi o jakości według norm.
import type { Message } from './core';

export const plBundles = {
  'bundles.title': 'Pakiety — od 6 (wzorcowy) do 1 (minimalny)',
  'bundles.intro':
    'Pakiet to komplet modeli i silników na ten komputer: 6 to pełna Alfa lokalnie (rozmowa głosowa, agentki z narzędziami, weryfikacja głosu), 1 — sama rozmowa tekstowa. Wersje silników (karta NVIDIA — CUDA, karta AMD lub Intel — Vulkan, bez karty — procesor) Alfa dobiera sama. Każdy element możesz też pobrać, sprawdzić albo naprawić osobno.',
  'bundles.loading': 'Ładowanie pakietów…',
  'bundles.machine': 'Ten komputer: {gpu} · RAM {ram} · procesor: {cores}',
  'bundles.machine.gpu': '{name} ({vram})',
  'bundles.machine.noGpu': 'bez karty graficznej',
  'bundles.cores': {
    one: '{n} rdzeń',
    few: '{n} rdzenie',
    many: '{n} rdzeni',
    other: '{n} rdzenia',
  },
  'bundles.guide.title': 'Jak wybrać pakiet — zalecenia',
  'bundles.guide.recommended':
    'Zacznij od pakietu z oznaczeniem „Zalecany” — to najwyższa ocena, która na tym komputerze działa bez kompromisów.',
  'bundles.guide.tight':
    '„Na styk” znaczy: zadziała, ale wolniej (np. część modelu liczy się na procesorze). Do płynnej rozmowy głosowej wybierz pakiet, który pasuje w pełni.',
  'bundles.guide.agents':
    'Agentki z narzędziami (pliki, polecenia, okna) potrzebują Bielika 4.5B — pakietów 5–6 — albo modelu w chmurze.',
  'bundles.guide.shared':
    'Pakiety się nie wykluczają: wspólne elementy pobierają się raz. Możesz zacząć od mniejszego i doinstalować większy później.',
  'bundles.guide.repair':
    'Każdy plik jest sprawdzany sumą SHA-256. Gdy element się uszkodzi albo źle zainstaluje, użyj „Napraw” przy tym elemencie — reszta pakietu zostaje bez zmian.',
  'bundles.guide.standards':
    'Uwagi o jakości odwołują się do norm międzynarodowych (ISO/IEC 25010 i 25059, ITU-T P.800/P.808 i G.114, ISO/IEC 19795-1, WER) jako zalecenia albo metody pomiaru — to nie są certyfikaty.',
  'bundles.rating': 'Ocena {n} z 6',
  'bundles.recommended': 'Zalecany dla tego komputera',
  'bundles.fit.fits': 'Pasuje do tego komputera',
  'bundles.fit.tight': 'Na styk — działa z kompromisem',
  'bundles.fit.too_weak': 'Za słaby sprzęt',
  'bundles.requirements': 'Wymagania: {text}',
  'bundles.size': 'Rozmiar: {size}',
  'bundles.missing': 'do pobrania: {size}',
  'bundles.count': {
    one: 'Zainstalowano {done} z {n} elementu',
    few: 'Zainstalowano {done} z {n} elementów',
    many: 'Zainstalowano {done} z {n} elementów',
    other: 'Zainstalowano {done} z {n} elementu',
  },
  'bundles.progress': 'Postęp pakietu {name}',
  'bundles.state.not_installed': 'Nie pobrano',
  'bundles.state.partial': 'Zainstalowany częściowo',
  'bundles.state.installed': 'Zainstalowany',
  'bundles.state.corrupt': 'Element uszkodzony albo z błędem — napraw',
  'bundles.state.downloading': 'Pobieranie w tle…',
  'bundles.state.needs_trust':
    'Czeka na Twoją zgodę: pliki bez przypiętej sumy SHA-256 zatwierdzisz w katalogu poniżej',
  'bundles.download': 'Pobierz pakiet',
  'bundles.resume': 'Dokończ pobieranie',
  'bundles.repair': 'Napraw pakiet',
  'bundles.verify': 'Sprawdź pliki (SHA-256)',
  'bundles.started': 'Pobieram pakiet „{name}” w tle — postęp widać przy elementach.',
  'bundles.verified': 'Sprawdziłam pakiet „{name}”: {state}.',
  'bundles.weak.title': 'Pakiet „{name}” jest za mocny dla tego komputera',
  'bundles.weak.body':
    '{reason} Możesz go pobrać, ale część silników nie ruszy albo będzie działać bardzo wolno. Zalecany tutaj: {recommended}.',
  'bundles.weak.none': 'brak — rozważ model w chmurze',
  'bundles.weak.confirm': 'Pobierz mimo to',
  'bundles.items': 'Elementy pakietu ({n})',
  'bundles.kind.llm': 'model rozmowy',
  'bundles.kind.stt': 'rozpoznawanie mowy',
  'bundles.kind.tts': 'głos (synteza mowy)',
  'bundles.kind.vad': 'wykrywanie mowy',
  'bundles.kind.wake': 'słowo wywoławcze',
  'bundles.kind.speaker': 'weryfikacja głosu',
  'bundles.kind.embed': 'wyszukiwanie znaczeniowe',
  'bundles.kind.sidecar': 'silnik',
  'bundles.item.fallback': 'zapas: procesor',
  'bundles.item.manual': 'instalacja ręczna — opis w katalogu poniżej',
  'bundles.item.trust': 'zatwierdź w katalogu poniżej',
  'bundles.item.download': 'Pobierz',
  'bundles.item.resume': 'Wznów',
  'bundles.item.repair': 'Napraw',
  'bundles.item.verify': 'Sprawdź',
  'bundles.item.actions': 'Działania: {name}',
  'bundles.quality': 'Jakość i normy ({n})',
  'bundles.quality.disclaimer':
    'Normy podajemy jako zalecenie albo metodę pomiaru, bez deklaracji certyfikacji. Wartości „do zmierzenia” poznasz po pomiarze na tym komputerze.',
  'engines.repair': 'Napraw',
  'engines.repairing': 'Naprawiam „{name}”: usunęłam pliki i pobieram od nowa.',
  'engines.repairConfirm.title': 'Naprawić „{name}”?',
  'engines.repairConfirm.body':
    'Alfa usunie pliki tego elementu (razem z częściowymi pobraniami) i pobierze go od nowa — {size}. Pozostałe elementy zostają bez zmian.',
} as const satisfies Record<string, Message>;
