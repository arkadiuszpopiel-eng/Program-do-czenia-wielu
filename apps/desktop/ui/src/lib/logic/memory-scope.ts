// Nazwa zakresu pamięci do wyświetlenia: „Rodzaj: nazwa”, ale bez powtórzeń typu „Globalna: Globalna”
// czy „Agentka: Agentka beta”, gdy nazwa z rdzenia sama zaczyna się od rodzaju.

export function scopeTitle(kind: string, label: string): string {
  const name = label.trim();
  if (!name) return kind;
  return name.toLocaleLowerCase('pl').startsWith(kind.trim().toLocaleLowerCase('pl'))
    ? name
    : `${kind}: ${name}`;
}
