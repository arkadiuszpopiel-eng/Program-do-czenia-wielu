// Atrapa drzewa §15: ustawienia silników na stronie „Modele i silniki” — te same klucze, teksty
// i zakresy co `crates/app-core/data/settings-pages.json` (silnik `llama-server`, kontekst, wątki,
// zwalnianie po bezczynności, równoległe pobierania).
import type { SettingDef } from '../types-system';
import { L, number, select } from './settings-helpers';

export const ENGINE_SETTINGS: SettingDef[] = [
  select(
    'engines.llm.backend',
    L('Silnik modelu rozmowy', 'Chat model engine'),
    L(
      'Na czym liczy lokalny model rozmowy. Automatycznie: karta NVIDIA — CUDA, karta AMD lub Intel — Vulkan, bez karty albo na baterii — procesor. Zalecenie: Automatycznie; „Procesor” wybierz, gdy sterownik karty sprawia problemy. Brak zainstalowanej wersji silnika → Alfa użyje zastępczej i zapisze ostrzeżenie w dzienniku. Zmiana działa po ponownym uruchomieniu Alfy.',
      'What the local chat model runs on. Automatic: NVIDIA GPU — CUDA, AMD or Intel GPU — Vulkan, no GPU or on battery — CPU. Recommended: Automatic; choose “CPU” when the GPU driver causes problems. If that engine build is not installed, Alfa uses a substitute and logs a warning. Takes effect after restarting Alfa.',
    ),
    [
      ['auto', L('Automatycznie (zalecane)', 'Automatic (recommended)')],
      ['cuda', L('Karta NVIDIA (CUDA)', 'NVIDIA GPU (CUDA)')],
      ['vulkan', L('Karta graficzna (Vulkan)', 'GPU (Vulkan)')],
      ['cpu', L('Procesor', 'CPU')],
    ],
    'auto',
    'machine',
  ),
  select(
    'engines.llm.context',
    L('Długość kontekstu', 'Context length'),
    L(
      'Ile ostatnich tokenów rozmowy model ma przed oczami. Dłuższy kontekst pamięta więcej, ale zajmuje więcej pamięci (Bielik 4.5B: ok. 60 MB na 1000 tokenów). Automatycznie: do 8192, a na małej karcie mniej, żeby model zmieścił się w całości. Zalecenie: Automatycznie; 2048 tylko przy braku pamięci — to oszczędność zasobów kosztem pamięci rozmowy (ISO/IEC 25010: wykorzystanie zasobów). Zmiana po ponownym uruchomieniu.',
      'How many recent conversation tokens the model can see. A longer context remembers more but uses more memory (Bielik 4.5B: about 60 MB per 1000 tokens). Automatic: up to 8192, less on a small GPU so the model fits entirely. Recommended: Automatic; 2048 only when memory is short — saving resources at the cost of conversation memory (ISO/IEC 25010: resource utilisation). Takes effect after a restart.',
    ),
    [
      ['auto', L('Automatycznie (zalecane)', 'Automatic (recommended)')],
      ['4096', L('4096 tokenów', '4096 tokens')],
      ['2048', L('2048 tokenów (oszczędnie)', '2048 tokens (saving)')],
    ],
    'auto',
    'machine',
  ),
  number(
    'engines.llm.threads',
    L('Wątki procesora', 'CPU threads'),
    L(
      'Ile wątków procesora liczy model, gdy pracuje na procesorze (w całości albo częściowo). 0 — automatycznie: tyle, ile rdzeni fizycznych. Zalecenie: 0; zmniejsz, gdy równocześnie grasz albo pracujesz w ciężkich programach — Alfa odpowie wolniej, a system będzie płynniejszy. Więcej wątków niż rdzeni fizycznych zwykle spowalnia. Zmiana po ponownym uruchomieniu.',
      'How many CPU threads the model uses when it runs on the CPU (fully or partly). 0 — automatic: as many as physical cores. Recommended: 0; lower it when you game or run heavy programs at the same time — Alfa replies slower and the system stays smoother. More threads than physical cores usually slows things down. Takes effect after a restart.',
    ),
    0,
    0,
    64,
    1,
    null,
    'machine',
  ),
  number(
    'engines.llm.idle_unload_min',
    L('Zwolnij model po bezczynności', 'Unload the model when idle'),
    L(
      'Po tylu minutach bez rozmowy model zwalnia pamięć RAM i karty graficznej. Krócej — więcej wolnej pamięci dla innych programów i gier, ale pierwsza odpowiedź po przerwie czeka na ponowne wczytanie modelu (od kilku do kilkunastu sekund). Zalecenie: 5 min na laptopie, 10–15 min na komputerze stacjonarnym. Zmiana po ponownym uruchomieniu.',
      'After this many minutes without conversation the model frees RAM and GPU memory. Shorter — more free memory for other programs and games, but the first reply after a break waits for the model to load again (a few to over ten seconds). Recommended: 5 min on a laptop, 10–15 min on a desktop. Takes effect after a restart.',
    ),
    10,
    1,
    120,
    1,
    'min',
    'machine',
  ),
  number(
    'models.parallel_downloads',
    L('Pobierania naraz', 'Downloads at once'),
    L(
      'Ile plików modeli i silników pobiera się równocześnie; reszta czeka w kolejce. Zalecenie: 2 dla łącza domowego; 1 przy łączu komórkowym albo z limitem danych; 3–4 tylko przy szybkim łączu światłowodowym. Każdy plik i tak jest sprawdzany sumą SHA-256. Zmiana po ponownym uruchomieniu.',
      'How many model and engine files download at the same time; the rest wait in a queue. Recommended: 2 on a home connection; 1 on mobile or metered connections; 3–4 only on fast fibre. Every file is checked with SHA-256 anyway. Takes effect after a restart.',
    ),
    2,
    1,
    4,
    1,
    null,
    'machine',
  ),
];
