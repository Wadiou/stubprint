use serde::Serialize;

use crate::event::{Event, LineSpacing};

/// How big a motion unit is: a horizontal one is 1/x inch and a vertical one is 1/y inch. 0 means
/// the printer's own default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct MotionUnits {
    /// A horizontal motion unit is 1/x inch, or the printer's own default when this is 0.
    pub x: u8,
    /// A vertical motion unit is 1/y inch, or the printer's own default when this is 0.
    pub y: u8,
}

/// The settings that decide where the next text lands on the paper. A printer keeps them until it
/// is reset, and they are known from the events alone, so this follows the events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Layout {
    /// The left margin, in horizontal motion units.
    pub margin: u16,
    /// The width of the print area, in horizontal motion units, or the rest of the paper.
    pub print_width: Option<u16>,
    pub motion_units: MotionUnits,
    pub line_spacing: LineSpacing,
    /// The columns the tab character jumps to. A printer that was not told has one every 8
    /// characters.
    pub tab_stops: Vec<u8>,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            margin: 0,
            print_width: None,
            motion_units: MotionUnits::default(),
            line_spacing: LineSpacing::Default,
            tab_stops: (1..=31).map(|n| n * 8).collect(),
        }
    }
}

impl Layout {
    /// Takes in what an event changes.
    pub fn apply(&mut self, event: &Event) {
        match event {
            Event::Initialize => *self = Self::default(),
            Event::Margin(units) => self.margin = *units,
            Event::PrintWidth(units) => self.print_width = Some(*units),
            Event::MotionUnits { x, y } => self.motion_units = MotionUnits { x: *x, y: *y },
            Event::LineSpacing(spacing) => self.line_spacing = *spacing,
            Event::TabStops(stops) => self.tab_stops = stops.clone(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_events_that_set_the_layout_are_followed_and_a_reset_takes_them_back() {
        let mut layout = Layout::default();
        assert_eq!(layout.tab_stops.len(), 31);
        for event in [
            Event::Margin(100),
            Event::PrintWidth(200),
            Event::MotionUnits { x: 180, y: 200 },
            Event::LineSpacing(LineSpacing::Units(40)),
            Event::TabStops(vec![4, 8]),
            Event::Cut(crate::event::CutMode::Full),
        ] {
            layout.apply(&event);
        }
        assert_eq!(layout.margin, 100);
        assert_eq!(layout.print_width, Some(200));
        assert_eq!(layout.motion_units, MotionUnits { x: 180, y: 200 });
        assert_eq!(layout.line_spacing, LineSpacing::Units(40));
        assert_eq!(layout.tab_stops, [4, 8]);
        layout.apply(&Event::Initialize);
        assert_eq!(layout, Layout::default());
    }
}
