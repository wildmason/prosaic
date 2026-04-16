//! Spanish number-to-words.
//!
//! Covers 0-29 by name, 30-99 with tens + units composition, 100 with "cien",
//! 101-999 with "ciento …". Beyond 999, returns the numeric form.
//! Deliberately minimal for v1.5 — most template use is `|plural` and article
//! selection, not spelling numbers.

pub fn number_to_words_es(n: usize) -> String {
    match n {
        0 => "cero".to_string(),
        1 => "uno".to_string(),
        2 => "dos".to_string(),
        3 => "tres".to_string(),
        4 => "cuatro".to_string(),
        5 => "cinco".to_string(),
        6 => "seis".to_string(),
        7 => "siete".to_string(),
        8 => "ocho".to_string(),
        9 => "nueve".to_string(),
        10 => "diez".to_string(),
        11 => "once".to_string(),
        12 => "doce".to_string(),
        13 => "trece".to_string(),
        14 => "catorce".to_string(),
        15 => "quince".to_string(),
        16 => "dieciséis".to_string(),
        17 => "diecisiete".to_string(),
        18 => "dieciocho".to_string(),
        19 => "diecinueve".to_string(),
        20 => "veinte".to_string(),
        21 => "veintiuno".to_string(),
        22 => "veintidós".to_string(),
        23 => "veintitrés".to_string(),
        24 => "veinticuatro".to_string(),
        25 => "veinticinco".to_string(),
        26 => "veintiséis".to_string(),
        27 => "veintisiete".to_string(),
        28 => "veintiocho".to_string(),
        29 => "veintinueve".to_string(),
        30 => "treinta".to_string(),
        40 => "cuarenta".to_string(),
        50 => "cincuenta".to_string(),
        60 => "sesenta".to_string(),
        70 => "setenta".to_string(),
        80 => "ochenta".to_string(),
        90 => "noventa".to_string(),
        100 => "cien".to_string(),
        n if n < 100 => {
            let tens = (n / 10) * 10;
            let units = n % 10;
            let tens_word = number_to_words_es(tens);
            let units_word = number_to_words_es(units);
            format!("{tens_word} y {units_word}")
        }
        n if n < 1000 => {
            let hundreds = n / 100;
            let remainder = n % 100;
            let hundreds_word = hundreds_word(hundreds);
            if remainder == 0 {
                hundreds_word
            } else {
                format!("{hundreds_word} {}", number_to_words_es(remainder))
            }
        }
        n => n.to_string(),
    }
}

fn hundreds_word(h: usize) -> String {
    match h {
        1 => "ciento".to_string(),
        2 => "doscientos".to_string(),
        3 => "trescientos".to_string(),
        4 => "cuatrocientos".to_string(),
        5 => "quinientos".to_string(),
        6 => "seiscientos".to_string(),
        7 => "setecientos".to_string(),
        8 => "ochocientos".to_string(),
        9 => "novecientos".to_string(),
        _ => h.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_to_ten() {
        assert_eq!(number_to_words_es(0), "cero");
        assert_eq!(number_to_words_es(1), "uno");
        assert_eq!(number_to_words_es(10), "diez");
    }

    #[test]
    fn teens() {
        assert_eq!(number_to_words_es(11), "once");
        assert_eq!(number_to_words_es(15), "quince");
        assert_eq!(number_to_words_es(19), "diecinueve");
    }

    #[test]
    fn twenties() {
        assert_eq!(number_to_words_es(20), "veinte");
        assert_eq!(number_to_words_es(21), "veintiuno");
        assert_eq!(number_to_words_es(29), "veintinueve");
    }

    #[test]
    fn compound_tens() {
        assert_eq!(number_to_words_es(31), "treinta y uno");
        assert_eq!(number_to_words_es(45), "cuarenta y cinco");
        assert_eq!(number_to_words_es(99), "noventa y nueve");
    }

    #[test]
    fn hundreds() {
        assert_eq!(number_to_words_es(100), "cien");
        assert_eq!(number_to_words_es(101), "ciento uno");
        assert_eq!(number_to_words_es(200), "doscientos");
        assert_eq!(number_to_words_es(500), "quinientos");
    }

    #[test]
    fn large_number_falls_back_to_numeric() {
        assert_eq!(number_to_words_es(1000), "1000");
        assert_eq!(number_to_words_es(99999), "99999");
    }
}
