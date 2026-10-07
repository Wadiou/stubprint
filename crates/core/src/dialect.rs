/// The command language a printer reads. The same bytes mean different things in each, so a
/// listening port is set to one, like the emulation setting of a real printer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialect {
    /// Epson's language, which most printers copy.
    #[default]
    EscPos,
    /// The older language of Star printers.
    StarLine,
    /// The newer language of Star printers.
    StarPrnt,
}

impl serde::Serialize for Dialect {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl Dialect {
    /// The name a configuration or a request uses.
    pub fn name(self) -> &'static str {
        match self {
            Dialect::EscPos => "escpos",
            Dialect::StarLine => "starline",
            Dialect::StarPrnt => "starprnt",
        }
    }

    /// The dialect with this name. The case, and the dashes and underscores, do not matter.
    pub fn from_name(name: &str) -> Option<Self> {
        let name: String = name
            .chars()
            .filter(|c| !matches!(c, '-' | '_' | '/' | ' '))
            .map(|c| c.to_ascii_lowercase())
            .collect();
        match name.as_str() {
            "escpos" => Some(Dialect::EscPos),
            "starline" | "starlinemode" => Some(Dialect::StarLine),
            "starprnt" => Some(Dialect::StarPrnt),
            _ => None,
        }
    }

    /// Both of the Star languages.
    pub fn is_star(self) -> bool {
        matches!(self, Dialect::StarLine | Dialect::StarPrnt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_go_both_ways() {
        for dialect in [Dialect::EscPos, Dialect::StarLine, Dialect::StarPrnt] {
            assert_eq!(Dialect::from_name(dialect.name()), Some(dialect));
        }
        assert_eq!(Dialect::from_name("ESC/POS"), Some(Dialect::EscPos));
        assert_eq!(Dialect::from_name("Star-Line"), Some(Dialect::StarLine));
        assert_eq!(Dialect::from_name("star_prnt"), Some(Dialect::StarPrnt));
        assert_eq!(Dialect::from_name("zpl"), None);
    }

    #[test]
    fn only_star_dialects_are_star() {
        assert!(!Dialect::EscPos.is_star());
        assert!(Dialect::StarLine.is_star());
        assert!(Dialect::StarPrnt.is_star());
    }
}
