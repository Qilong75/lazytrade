use crate::{chart::CandleType, chart_data::ChartData, Candle};
use colored::{Color, Colorize};
use std::{cell::RefCell, rc::Rc};

pub struct VolumePane {
    pub chart_data: Rc<RefCell<ChartData>>,
    pub height: i64,
    pub enabled: bool,
    pub bearish_color: Color,
    pub bullish_color: Color,
    pub unicode_fill: char,
}

impl VolumePane {
    pub fn new(chart_data: Rc<RefCell<ChartData>>, height: i64) -> Self {
        let candle_set_has_volume = chart_data
            .borrow()
            .visible_candle_set
            .candles
            .iter()
            .any(|candle| candle.volume.unwrap_or_default() > 0.0);

        Self {
            chart_data,
            height,
            enabled: candle_set_has_volume,
            bullish_color: Color::TrueColor {
                r: 52,
                g: 208,
                b: 88,
            },
            bearish_color: Color::TrueColor {
                r: 234,
                g: 74,
                b: 90,
            },
            unicode_fill: '┃',
        }
    }

    fn colorize(&self, candle_type: CandleType, string: &str) -> String {
        let color = match candle_type {
            CandleType::Bearish => self.bearish_color,
            CandleType::Bullish => self.bullish_color,
        };
        string.color(color).to_string()
    }

    /// Renders one volume bar cell at the configured candle width.
    pub fn render(&self, candle: &Candle, y: i64, candle_width: usize) -> String {
        let max_volume = self.chart_data.borrow().visible_candle_set.max_volume;
        let volume = candle.volume.unwrap_or_default();
        let candle_width = candle_width.max(1);

        let volume_percent_of_max = volume / max_volume;
        let ratio = volume_percent_of_max * self.height as f64;

        if y < ratio.ceil() as i64 {
            return self.colorize(candle.get_type(), &self.unicode_fill.to_string().repeat(candle_width));
        }

        if y == 1 && self.unicode_fill == '┃' {
            return self.colorize(candle.get_type(), &"╻".repeat(candle_width));
        }

        " ".repeat(candle_width)
    }
}
