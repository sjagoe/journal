// A simple progress spinner based on ratatui::widgets::LineGauge

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Styled};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Widget};

pub struct ProgressSpinner {
    complete: bool,
    value: f64,
    filled_size: f64,
    style: Style,
}

impl Default for ProgressSpinner {
    fn default() -> Self {
        Self {
            complete: false,
            value: 0.0,
            filled_size: 0.2,
            style: Style::default(),
        }
    }
}

impl ProgressSpinner {
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn complete(mut self) -> Self {
        self.value = 0.0;
        self.filled_size = 1.0;
        self.complete = true;
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn value(mut self, value: f64) -> Self {
        if value > 1.0 {
            self.value = value - value.floor();
        } else {
            self.value = value;
        }
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }
}

impl Widget for ProgressSpinner {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Widget::render(&self, area, buf);
    }
}

impl Widget for &ProgressSpinner {
    fn render(self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, self.style);

        let block = Block::bordered().title(match self.complete {
            true => " Exporting ... ",
            false => " Export Complete ",
        });

        let block = block.as_ref();
        block.render(area, buf);

        let gauge_area = block.inner(area);
        if gauge_area.is_empty() {
            return;
        }

        let unfilled_value = self.value;
        let mut filled_value = self.value + self.filled_size;
        if filled_value > 1.0 {
            filled_value = 1.0;
        }

        let start = gauge_area.left() + 1;
        let end = gauge_area.right() - 1;
        let row = gauge_area.top();
        if start >= gauge_area.right() {
            return;
        }

        let filled_start =
            start + (f64::from(end.saturating_sub(start)) * unfilled_value).floor() as u16;
        let filled_end =
            start + (f64::from(end.saturating_sub(start)) * filled_value).floor() as u16;

        let filled_style = Style::new().white().on_black().bold();
        let unfilled_style = Style::default();

        let filled_symbol = symbols::block::FULL;
        let unfilled_symbol = symbols::line::HORIZONTAL;

        for col in start..filled_start {
            buf[(col, row)]
                .set_symbol(unfilled_symbol)
                .set_style(unfilled_style);
        }

        if filled_start < filled_end {
            for col in filled_start..filled_end {
                buf[(col, row)]
                    .set_symbol(filled_symbol)
                    .set_style(filled_style);
            }
        }

        if filled_end < end {
            for col in filled_end..end {
                buf[(col, row)]
                    .set_symbol(unfilled_symbol)
                    .set_style(unfilled_style);
            }
        }

        let style = if self.complete {
            Style::default().bg(Color::Cyan).fg(Color::Black)
        } else {
            Style::default().bg(Color::DarkGray)
        };
        let button = Line::from(vec![Span::styled("Close", style)]).right_aligned();
        let left = gauge_area.x + 1;
        let top = gauge_area.y + gauge_area.height - 1;
        let button_area = Rect::new(left, top, gauge_area.width - 2, 1);

        button.render(button_area, buf);
    }
}

impl Styled for ProgressSpinner {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style)
    }
}
