//! What a printer can be set to. Each setting is listed once, in the `settings!` call below: from
//! that list come the settings a printer has, the changes a request or `--printer` can make to
//! them, and what the API says about them. A new setting is one entry in it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::DEFAULT_PRINTER_BUFFER;

/// Reads a whole number, written either as a number or as text such as `"4096"`.
fn number<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<usize>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Size {
        Number(usize),
        Text(String),
    }
    match Option::<Size>::deserialize(deserializer)? {
        None => Ok(None),
        Some(Size::Number(bytes)) => Ok(Some(bytes)),
        Some(Size::Text(text)) => text
            .parse()
            .map(Some)
            .map_err(|_| serde::de::Error::custom(format!("`{text}` is not a whole number"))),
    }
}

/// What is left of a request or of `--printer` when its settings were read. It takes nothing, and
/// fails for a key that no setting has, so that a misspelt one is an error.
#[derive(Debug, Default)]
pub(crate) struct Unknown;

impl<'de> Deserialize<'de> for Unknown {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let rest = BTreeMap::<String, serde::de::IgnoredAny>::deserialize(deserializer)?;
        match rest.keys().next() {
            None => Ok(Self),
            Some(key) => Err(serde::de::Error::custom(format!("unknown field `{key}`"))),
        }
    }
}

macro_rules! settings {
    ($( $(#[$meta:meta])* $name:ident = $default:expr; )*) => {
        /// What a printer is set to.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
        pub struct Settings {
            $( $(#[$meta])* pub $name: usize, )*
        }

        impl Default for Settings {
            fn default() -> Self {
                Self { $( $name: $default, )* }
            }
        }

        /// A change to the settings: the ones it names are set, and the others are left as they are.
        #[derive(Debug, Clone, Copy, Default, Deserialize, ToSchema)]
        pub struct SettingsChange {
            $( $(#[$meta])* #[serde(default, deserialize_with = "number")] pub $name: Option<usize>, )*
        }

        impl SettingsChange {
            /// The settings once this change is made to them.
            pub fn applied_to(self, mut settings: Settings) -> Settings {
                $( if let Some(value) = self.$name { settings.$name = value; } )*
                settings
            }
        }

        impl From<Settings> for SettingsChange {
            /// The change that sets everything to these settings.
            fn from(settings: Settings) -> Self {
                Self { $( $name: Some(settings.$name), )* }
            }
        }
    };
}

settings! {
    /// How many bytes it holds while it cannot print, as a real printer keeps them in its receive
    /// buffer: it is off line, its cover is open, its paper has run out or it has a fault. A program
    /// that sends more has to wait. What it held prints when the printer can print again, even if
    /// the program has gone. 0 holds nothing: it prints whatever it receives. The default is 4096.
    /// Connections that are open follow a change.
    #[schema(example = 4096)]
    #[serde(alias = "hold_offline")]
    printer_buffer = DEFAULT_PRINTER_BUFFER;
    /// How many bytes the network keeps for what a program sends before the printer reads it, as
    /// the network chip of a real printer does. A program that sends more than that, and more than
    /// its own system keeps, has to wait. 0, the default, leaves it to the system, which keeps a
    /// lot, so a program may not wait at all. The system rounds the number. A change applies to
    /// connections that open from then on.
    #[schema(example = 8192)]
    #[serde(alias = "receive_buffer")]
    network_buffer = 0;
    /// How long its paper roll is when it is full, in millimetres, such as 80000 for 80 m. It runs
    /// down as receipts print: it reports near end for the last tenth and end when it is used up,
    /// and a printer with no paper stops printing until paper is put in. The default is 0, a roll
    /// that never runs out, and then the paper is set by hand. Setting it puts in a new full roll,
    /// and a printer that stopped at the end of its paper prints what it held.
    #[schema(example = 80000)]
    paper_roll = 0;
    /// How fast it prints, in millimetres of paper a second: a real receipt printer does 100 to
    /// 300. A receipt keeps the printer busy for as long as its paper takes to come out, and a
    /// program that sends more has to wait until it is done. The default is 0, which prints without
    /// taking any time.
    #[schema(example = 150)]
    print_speed = 0;
    /// How many programs can be connected to it at once, as a real printer takes one or a few. The
    /// next one that connects is closed at once, and one that was refused can try again when a
    /// connection has closed. The default is 0, no limit of its own: up to 64. Programs that are
    /// connected stay, even if there are more than a new limit.
    #[schema(example = 1)]
    max_connections = 0;
    /// How many seconds a connection can send nothing before the printer closes it, as one does
    /// that is tired of waiting. What it was sent still prints. Time spent waiting for the printer
    /// to print does not count. The default is 0, which never closes one. A change applies to the
    /// connections that are open too.
    #[schema(example = 30)]
    idle_timeout = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_sets_what_it_names_and_leaves_the_rest() {
        let change: SettingsChange =
            serde_json::from_str(r#"{"print_speed": 150, "idle_timeout": "30"}"#).unwrap();
        let settings = change.applied_to(Settings::default());
        assert_eq!((settings.print_speed, settings.idle_timeout), (150, 30));
        assert_eq!(settings.printer_buffer, DEFAULT_PRINTER_BUFFER);
        assert_eq!(settings.max_connections, 0);
    }

    #[test]
    fn the_change_that_sets_everything_gives_the_same_settings() {
        let settings = Settings {
            print_speed: 7,
            idle_timeout: 9,
            ..Settings::default()
        };
        let again = SettingsChange::from(settings).applied_to(Settings::default());
        assert_eq!(again, settings);
    }

    #[test]
    fn settings_are_written_with_their_names_and_in_full() {
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(json["printer_buffer"], DEFAULT_PRINTER_BUFFER);
        assert_eq!(json.as_object().unwrap().len(), 6);
    }
}
