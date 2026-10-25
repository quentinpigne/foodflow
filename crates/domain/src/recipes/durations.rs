//! Durées ISO 8601 des recettes importées (`PT1H30M`, `P0DT0H45M`), en minutes (R-REC-62).

/// Minutes d'une durée ISO 8601 ; `None` si elle est illisible. Les secondes sont arrondies à la minute
/// supérieure ; jours, heures et minutes sont admis, pas les mois ni les années.
pub fn parse_minutes(value: &str) -> Option<u32> {
    let text = value.trim().to_ascii_uppercase();
    let rest = text.strip_prefix('P')?;
    let (days_part, time_part) = match rest.split_once('T') {
        Some((d, t)) => (d, t),
        None => (rest, ""),
    };
    let mut minutes: u64 = 0;
    let mut seconds: u64 = 0;
    let mut read = |part: &str, units: &[(char, u64)], seconds_unit: bool| -> Option<()> {
        let mut number = String::new();
        for c in part.chars() {
            if c.is_ascii_digit() || c == '.' || c == ',' {
                number.push(c);
                continue;
            }
            let factor = units.iter().find(|(u, _)| *u == c)?.1;
            // Les fractions de minute ou de seconde sont tronquées : une recette n'en a pas besoin.
            let whole: u64 = number.split(['.', ',']).next()?.parse().ok()?;
            if seconds_unit && c == 'S' {
                seconds += whole;
            } else {
                minutes += whole * factor;
            }
            number.clear();
        }
        number.is_empty().then_some(())
    };
    read(days_part, &[('D', 24 * 60), ('W', 7 * 24 * 60)], false)?;
    read(time_part, &[('H', 60), ('M', 1), ('S', 0)], true)?;
    u32::try_from(minutes + seconds.div_ceil(60)).ok()
}

/// Minutes → durée ISO 8601 (`PT1H30M`), pour l'export (R-REC-70).
pub fn format_minutes(minutes: u32) -> String {
    let (hours, rest) = (minutes / 60, minutes % 60);
    match (hours, rest) {
        (0, m) => format!("PT{m}M"),
        (h, 0) => format!("PT{h}H"),
        (h, m) => format!("PT{h}H{m}M"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_rec_62_iso_durations() {
        assert_eq!(parse_minutes("PT1H30M"), Some(90));
        assert_eq!(parse_minutes("PT45M"), Some(45));
        assert_eq!(parse_minutes("P0DT0H20M"), Some(20));
        assert_eq!(parse_minutes("PT90S"), Some(2));
        assert_eq!(parse_minutes("P1D"), Some(1_440));
        assert_eq!(parse_minutes("pt2h"), Some(120));
        assert_eq!(parse_minutes("1h30"), None);
        assert_eq!(parse_minutes("PT1Y"), None);
        assert_eq!(format_minutes(90), "PT1H30M");
        assert_eq!(format_minutes(120), "PT2H");
        assert_eq!(format_minutes(5), "PT5M");
    }
}
