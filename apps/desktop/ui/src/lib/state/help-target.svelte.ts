// Dział instrukcji do otwarcia (trafienie z wyszukiwania w Ustawieniach). Strona „Instrukcja”
// odczytuje go raz przy ładowaniu i czyści. Osobny moduł — stan aplikacji jest u limitu długości.

export const helpTarget = $state<{ topic: string }>({ topic: '' });

/** Odbiór działu: zwraca id (albo pusty napis) i czyści cel. */
export function takeHelpTopic(): string {
  const topic = helpTarget.topic;
  helpTarget.topic = '';
  return topic;
}
