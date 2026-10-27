//! Échanges avec l'IA (ADR-0014) : formats de réponse imposés et leur contrôle. Le domaine ne parle pas au
//! réseau : il décrit ce qu'on attend, et vérifie ce qu'on reçoit. Un élément invalide est écarté et
//! signalé, jamais corrigé en silence.

pub mod plan;
pub mod recipe;

/// Pseudonyme d'un membre du foyer dans ce qu'on envoie à l'IA : une lettre, jamais son nom (ADR-0007).
/// A, B… Z, puis AA, AB…
pub fn pseudonym(index: usize) -> String {
    let letter = |i: usize| char::from(b'A' + (i % 26) as u8);
    if index < 26 { letter(index).to_string() } else { format!("{}{}", letter(index / 26 - 1), letter(index)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pseudonyms_are_letters() {
        assert_eq!((pseudonym(0), pseudonym(2), pseudonym(25)), ("A".into(), "C".into(), "Z".into()));
        assert_eq!((pseudonym(26), pseudonym(27)), ("AA".into(), "AB".into()));
    }
}
