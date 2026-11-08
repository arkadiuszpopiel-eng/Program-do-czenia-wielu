//! Liczebniki główne po polsku (bez polskich znaków, po `fold`): „dwadzieścia trzy”,
//! „sto dwadzieścia pięć”, „dwa tysiące trzysta” → liczba. Klasy maleją w obrębie grupy
//! (setki → dziesiątki → jedności), „tysiąc” mnoży grupę przed nim.

/// Klasa słowa: jedności i -naście (0), dziesiątki (1), setki (2), tysiąc (3).
fn word(w: &str) -> Option<(u64, u8)> {
    let v = match w {
        "zero" => (0, 0),
        "jeden" | "jedna" | "jedno" => (1, 0),
        "dwa" | "dwie" => (2, 0),
        "trzy" => (3, 0),
        "cztery" => (4, 0),
        "piec" => (5, 0),
        "szesc" => (6, 0),
        "siedem" => (7, 0),
        "osiem" => (8, 0),
        "dziewiec" => (9, 0),
        "dziesiec" => (10, 0),
        "jedenascie" => (11, 0),
        "dwanascie" => (12, 0),
        "trzynascie" => (13, 0),
        "czternascie" => (14, 0),
        "pietnascie" => (15, 0),
        "szesnascie" => (16, 0),
        "siedemnascie" => (17, 0),
        "osiemnascie" => (18, 0),
        "dziewietnascie" => (19, 0),
        "dwadziescia" => (20, 1),
        "trzydziesci" => (30, 1),
        "czterdziesci" => (40, 1),
        "piecdziesiat" => (50, 1),
        "szescdziesiat" => (60, 1),
        "siedemdziesiat" => (70, 1),
        "osiemdziesiat" => (80, 1),
        "dziewiecdziesiat" => (90, 1),
        "sto" => (100, 2),
        "dwiescie" => (200, 2),
        "trzysta" => (300, 2),
        "czterysta" => (400, 2),
        "piecset" => (500, 2),
        "szescset" => (600, 2),
        "siedemset" => (700, 2),
        "osiemset" => (800, 2),
        "dziewiecset" => (900, 2),
        "tysiac" | "tysiace" | "tysiecy" => (1000, 3),
        _ => return None,
    };
    Some(v)
}

/// Liczebnik z początku `words`: wartość i liczba zużytych słów (`None`, gdy pierwsze słowo
/// nie jest liczebnikiem). Kończy się na pierwszym słowie, które nie pasuje do kolejności klas.
pub fn parse(words: &[&str]) -> Option<(u64, usize)> {
    let mut total = 0u64;
    let mut group = 0u64;
    let mut last = 4u8;
    let mut used = 0usize;
    let mut thousands = false;
    for w in words {
        let Some((value, class)) = word(w) else { break };
        if class == 3 {
            if thousands {
                break;
            }
            total = group.max(1) * 1000;
            group = 0;
            last = 3;
            thousands = true;
        } else if class < last {
            group += value;
            last = class;
        } else {
            break;
        }
        used += 1;
    }
    (used > 0).then_some((total + group, used))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(text: &str) -> Option<(u64, usize)> {
        parse(&text.split(' ').collect::<Vec<_>>())
    }

    #[test]
    fn composes_numbers() {
        assert_eq!(n("siedem"), Some((7, 1)));
        assert_eq!(n("dwadziescia trzy"), Some((23, 2)));
        assert_eq!(n("sto dwadziescia piec"), Some((125, 3)));
        assert_eq!(n("dwa tysiace trzysta"), Some((2300, 3)));
        assert_eq!(
            n("tysiac dziewiecset osiemdziesiat cztery"),
            Some((1984, 4))
        );
        assert_eq!(n("dwanascie razy"), Some((12, 1)));
        assert_eq!(n("piec piec"), Some((5, 1)), "dwie jedności to dwie liczby");
        assert_eq!(n("razy"), None);
    }
}
