//! What the commands print: tables, times and the one line that says how a printer is.

use serde_json::Value;

/// A text of a JSON object, or nothing when it has none.
pub fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}

/// A number of a JSON object, or 0.
pub fn number(value: &Value, key: &str) -> u64 {
    value[key].as_u64().unwrap_or(0)
}

/// Rows under their headings, each column as wide as its widest cell.
pub fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let widths: Vec<usize> = (0..headers.len())
        .map(|column| {
            rows.iter()
                .map(|row| row[column].chars().count())
                .chain([headers[column].chars().count()])
                .max()
                .unwrap_or(0)
        })
        .collect();
    let line = |cells: Vec<&str>| {
        cells
            .iter()
            .zip(&widths)
            .map(|(cell, width)| format!("{cell:<width$}"))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    let mut lines = vec![line(headers.to_vec())];
    lines.extend(
        rows.iter()
            .map(|row| line(row.iter().map(String::as_str).collect())),
    );
    lines.join("\n")
}

/// A text cut to this many characters, with `…` where it was cut.
pub fn shorten(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_string();
    }
    let kept: String = text.chars().take(most.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// Milliseconds since 1970 as a date and a time in UTC.
pub fn clock(ms: u64) -> String {
    let seconds = ms / 1000;
    let (days, rest) = (
        i64::try_from(seconds / 86_400).unwrap_or(0),
        seconds % 86_400,
    );
    // The date of a day count, by the algorithm of Howard Hinnant.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// How a printer is, in a few words: `online`, or what is wrong with it.
pub fn state(status: &Value) -> String {
    let mut problems = Vec::new();
    if status["online"] == false {
        problems.push("offline".to_string());
    }
    if status["cover_open"] == true {
        problems.push("cover open".to_string());
    }
    match status["paper"].as_str() {
        Some("near_end") => problems.push("paper near end".to_string()),
        Some("end") => problems.push("paper end".to_string()),
        _ => {}
    }
    if status["drawer_open"] == true {
        problems.push("drawer open".to_string());
    }
    if status["feed_button"] == true {
        problems.push("feed button".to_string());
    }
    if let Some(fault) = status["fault"].as_str() {
        problems.push(format!("fault: {fault}"));
    }
    if problems.is_empty() {
        "online".to_string()
    } else {
        problems.join(", ")
    }
}

/// A command of a receipt or of decoded bytes as one line: its kind, and what it carries.
pub fn event_line(event: &Value) -> String {
    let kind = text(event, "type");
    match event.get("value") {
        None | Some(Value::Null) => kind.to_string(),
        // A text carries its style too, which the trace of the API has for whoever wants it.
        Some(value) => match value["text"].as_str() {
            Some(printed) => shorten(&format!("{kind} {printed:?}"), 100),
            None => shorten(&format!("{kind} {value}"), 100),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_table_lines_its_columns_up_and_leaves_no_space_at_the_end() {
        let rows = vec![
            vec!["front".to_string(), "online".to_string()],
            vec!["kitchen".to_string(), "".to_string()],
        ];
        assert_eq!(
            table(&["NAME", "STATE"], &rows),
            "NAME     STATE\nfront    online\nkitchen"
        );
    }

    #[test]
    fn a_time_is_a_date_and_a_time_in_utc() {
        assert_eq!(clock(0), "1970-01-01 00:00:00");
        assert_eq!(clock(1_000_000_000_000), "2001-09-09 01:46:40");
        // A leap day.
        assert_eq!(clock(1_709_210_096_000), "2024-02-29 12:34:56");
    }

    #[test]
    fn a_printer_that_has_nothing_wrong_is_online_and_one_that_has_says_what() {
        let fine = json!({"online": true, "cover_open": false, "paper": "adequate", "fault": null});
        assert_eq!(state(&fine), "online");
        let bad =
            json!({"online": false, "cover_open": true, "paper": "near_end", "fault": "cutter"});
        assert_eq!(
            state(&bad),
            "offline, cover open, paper near end, fault: cutter"
        );
    }

    #[test]
    fn a_long_text_is_cut_where_the_room_ends() {
        assert_eq!(shorten("short", 10), "short");
        assert_eq!(shorten("0123456789", 5), "0123…");
    }

    #[test]
    fn an_event_is_its_kind_and_its_value() {
        assert_eq!(event_line(&json!({"type": "initialize"})), "initialize");
        assert_eq!(
            event_line(&json!({"type": "align", "value": "center"})),
            "align \"center\""
        );
        // A text shows only what it prints.
        assert_eq!(
            event_line(
                &json!({"type": "text", "value": {"text": "Hello", "style": {"bold": true}}})
            ),
            "text \"Hello\""
        );
    }
}
