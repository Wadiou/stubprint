//! Barcodes, and the two dimensional codes of `GS ( k`.

use super::machine::Machine;
use super::reader::{Frame, Input, Step};
use crate::event::{Event, Pdf417, Qr, QrLevel, Symbol, SymbolKind};

/// `GS ( k` picks a symbol with its first byte.
const PDF417_CODE: u8 = 48;
const QR_CODE: u8 = 49;

/// The functions of `GS ( k` that every symbol has: the data is stored, then printed.
const STORE: u8 = 80;
const PRINT: u8 = 81;

impl Machine {
    /// `GS ( k pL pH cn fn ...` stores the settings and the data of a two dimensional code, then
    /// prints it.
    pub(super) fn parse_symbol(&mut self, input: &Input, events: &mut Vec<Event>) -> Step {
        let length = match input.frame(3, 2) {
            Frame::Need | Frame::Waiting => return Step::Incomplete,
            Frame::TooBig => return input.unknown_run(events),
            Frame::Whole(length) => length,
        };
        // A symbol and a function are at least two bytes, so the length must be at least 2.
        if length < 7 {
            return input.framed_unknown(3, 2, events);
        }
        let (symbol, function) = (input.bytes[5], input.bytes[6]);
        let params = &input.bytes[7..length];

        let kind = match symbol {
            PDF417_CODE | QR_CODE => None,
            50 => Some(SymbolKind::MaxiCode),
            51 => Some(SymbolKind::Gs1DataBar),
            52 => Some(SymbolKind::Composite),
            53 => Some(SymbolKind::Aztec),
            54 => Some(SymbolKind::DataMatrix),
            _ => return input.framed_unknown(3, 2, events),
        };

        match function {
            // The data comes after a byte that is always 48.
            STORE => {
                let data = params.get(1..).unwrap_or(&[]).to_vec();
                self.state.symbol_data.insert(symbol, data);
            }
            PRINT => {
                self.flush_line(events);
                let data = self
                    .state
                    .symbol_data
                    .get(&symbol)
                    .cloned()
                    .unwrap_or_default();
                events.push(match kind {
                    None if symbol == QR_CODE => Event::Qr(self.qr(data)),
                    None => Event::Pdf417(self.pdf417(data)),
                    Some(kind) => Event::Symbol(Symbol {
                        kind,
                        data,
                        settings: self
                            .state
                            .symbol_settings
                            .get(&symbol)
                            .map(|settings| {
                                settings
                                    .iter()
                                    .map(|(function, bytes)| (*function, bytes.clone()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    }),
                });
            }
            // 82 asks how big the symbol would be, which only a printer that draws it can say. The
            // others, from 65 up, are the settings of the symbol.
            65..=79 => {
                self.state
                    .symbol_settings
                    .entry(symbol)
                    .or_default()
                    .insert(function, params.to_vec());
            }
            82 => {}
            _ => return input.framed_unknown(3, 2, events),
        }
        Step::Consumed(length)
    }

    fn setting(&self, symbol: u8, function: u8) -> &[u8] {
        self.state
            .symbol_settings
            .get(&symbol)
            .and_then(|settings| settings.get(&function))
            .map_or(&[], Vec::as_slice)
    }

    fn qr(&self, data: Vec<u8>) -> Qr {
        let first = |function: u8| self.setting(QR_CODE, function).first().copied();
        Qr {
            data: String::from_utf8_lossy(&data).into_owned(),
            model: match first(65) {
                Some(49) => 1,
                Some(51) => 3,
                _ => 2,
            },
            size: first(67).unwrap_or(3),
            level: match first(69) {
                Some(49) => QrLevel::M,
                Some(50) => QrLevel::Q,
                Some(51) => QrLevel::H,
                _ => QrLevel::L,
            },
        }
    }

    fn pdf417(&self, data: Vec<u8>) -> Pdf417 {
        let first = |function: u8| self.setting(PDF417_CODE, function).first().copied();
        let level = self.setting(PDF417_CODE, 69);
        Pdf417 {
            data: String::from_utf8_lossy(&data).into_owned(),
            columns: first(65).unwrap_or(0),
            rows: first(66).unwrap_or(0),
            width: first(67).unwrap_or(3),
            height: first(68).unwrap_or(3),
            level_mode: level.first().copied().unwrap_or(48),
            level: level.get(1).copied().unwrap_or(0),
            truncated: matches!(first(70), Some(1 | 49)),
        }
    }
}
