use crate::app::{
    App, ChartMode, GroupNameAction, InputMode, KLine, LayoutMode, MinutePoint, QuoteMeta, Stock,
};
use ansi_parser::AnsiParser;
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, Block, Borders, Chart as RatatuiChart, Clear, Dataset, GraphType, Paragraph, Row,
        Table, Widget,
    },
};
use unicode_width::UnicodeWidthStr;

// A-share color standards: Red for Up, Green for Down
const COLOR_UP: Color = Color::Red;
const COLOR_DOWN: Color = Color::Green;
const COLOR_EVEN: Color = Color::White;
const INTRADAY_SESSION_X_MAX: f64 = 241.0;
const KLINE_CANDLE_WIDTH: usize = 3;

#[derive(Debug, Clone, Copy)]
struct IntradayPlotPoint<'a> {
    point: &'a MinutePoint,
    x: f64,
}

#[derive(Debug, Clone, Copy)]
struct IntradayVolumePoint {
    x: f64,
    volume: f64,
    is_up: bool,
}

struct AnsiChart<'a>(&'a str);

#[derive(Debug, Clone, Copy, PartialEq)]
struct IntradayStats {
    high: f64,
    low: f64,
    pct_vs_prev_close: f64,
}

/// Renders ANSI-colored chart output into a Ratatui buffer.
impl Widget for AnsiChart<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for (line_idx, line) in self.0.lines().enumerate() {
            let y = area.top() + line_idx as u16;
            if y >= area.bottom() {
                break;
            }

            let mut x = area.left();
            let mut style = Style::default();

            for block in line.ansi_parse() {
                match block {
                    ansi_parser::Output::TextBlock(text) => {
                        if x < area.right() {
                            buf.set_string(x, y, text, style);
                            x = x.saturating_add(text.width() as u16);
                        }
                    }
                    ansi_parser::Output::Escape(escape) => match escape {
                        ansi_parser::AnsiSequence::SetGraphicsMode(values) => {
                            style = apply_ansi_style(style, &values);
                        }
                        ansi_parser::AnsiSequence::ResetMode(_) => {
                            style = Style::default();
                        }
                        _ => {}
                    },
                }
            }
        }
    }
}

/// Applies an ANSI SGR sequence to the active Ratatui style.
fn apply_ansi_style(style: Style, values: &[u8]) -> Style {
    fn extended_color(values: &[u8]) -> Option<Color> {
        if values.len() < 2 {
            return None;
        }
        match values[1] {
            2 if values.len() >= 5 => Some(Color::Rgb(values[2], values[3], values[4])),
            5 if values.len() >= 3 => Some(Color::Indexed(values[2])),
            _ => None,
        }
    }

    match values.first() {
        Some(0) => Style::default(),
        Some(1) => style.add_modifier(Modifier::BOLD),
        Some(2) => style.remove_modifier(Modifier::BOLD),
        Some(30) => style.fg(Color::Black),
        Some(31) => style.fg(Color::Red),
        Some(32) => style.fg(Color::Green),
        Some(33) => style.fg(Color::Yellow),
        Some(34) => style.fg(Color::Blue),
        Some(35) => style.fg(Color::Magenta),
        Some(36) => style.fg(Color::Cyan),
        Some(37 | 97) => style.fg(Color::White),
        Some(90) => style.fg(Color::DarkGray),
        Some(91) => style.fg(Color::LightRed),
        Some(92) => style.fg(Color::LightGreen),
        Some(93) => style.fg(Color::LightYellow),
        Some(94) => style.fg(Color::LightBlue),
        Some(95) => style.fg(Color::LightMagenta),
        Some(96) => style.fg(Color::LightCyan),
        Some(38) => extended_color(values).map_or(style, |color| style.fg(color)),
        Some(48) => extended_color(values).map_or(style, |color| style.bg(color)),
        _ => style,
    }
}

pub fn render(f: &mut Frame, app: &mut App) {
    // Overall Layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Top Index Bar
            Constraint::Min(10),   // Main View (Watchlist + Detail)
            Constraint::Length(3), // Bottom Help & Status Bar
        ])
        .split(f.size());

    // 1. Render Top Indices
    render_indices(f, app, chunks[0]);

    // 2. Render Main Body (Left: Watchlist, Right: Detail)
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(match app.layout_mode {
            LayoutMode::Compact => [Constraint::Percentage(32), Constraint::Percentage(68)],
            LayoutMode::LargeChart => [Constraint::Percentage(25), Constraint::Percentage(75)],
            LayoutMode::OrderBook => [Constraint::Percentage(45), Constraint::Percentage(55)],
            LayoutMode::Balanced => [Constraint::Percentage(40), Constraint::Percentage(60)],
        })
        .split(chunks[1]);

    render_watchlist(f, app, body_chunks[0]);
    render_details(f, app, body_chunks[1]);

    // 3. Render Footer (Status & Keybindings)
    render_footer(f, app, chunks[2]);

    // 4. Render Search Dialog (if active)
    if app.input_mode == InputMode::Search {
        render_search_popup(f, app);
    }

    match &app.input_mode {
        InputMode::GroupName(action) => render_group_name_popup(f, app, action),
        InputMode::FilterText => render_filter_text_popup(f, app),
        _ => {}
    }

    if app.show_opening_auction_popup {
        render_opening_auction_popup(f, app);
    }

    if app.show_help_popup {
        render_help_popup(f);
    }
}

fn render_indices(f: &mut Frame, app: &App, rect: Rect) {
    if app.indices.is_empty() {
        let block = Block::default()
            .title(" 市场指数 ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        f.render_widget(
            Paragraph::new("正在加载最近可用指数行情").block(block),
            rect,
        );
        return;
    }

    let index_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(rect);

    for (i, idx) in app.indices.iter().enumerate() {
        let color = if idx.pct_change > 0.0 {
            COLOR_UP
        } else if idx.pct_change < 0.0 {
            COLOR_DOWN
        } else {
            COLOR_EVEN
        };

        let text = vec![Line::from(vec![
            Span::styled(
                format!("{} ", idx.name),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:.2} ", idx.price),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:+.2}%", idx.pct_change),
                Style::default().fg(color),
            ),
        ])];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(paragraph, index_chunks[i]);
    }
}

fn render_watchlist(f: &mut Frame, app: &App, rect: Rect) {
    let group_count = app.config.groups.len();
    if group_count == 0 {
        let block = Block::default().title("自选股").borders(Borders::ALL);
        f.render_widget(Paragraph::new("无自选股板块").block(block), rect);
        return;
    }

    // Build the tab headers
    let mut tabs_spans = Vec::new();
    for (i, group) in app.config.groups.iter().enumerate() {
        if i > 0 {
            tabs_spans.push(Span::raw(" | "));
        }
        if i == app.active_group_idx {
            tabs_spans.push(Span::styled(
                format!(" [{}] ", group.name),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            tabs_spans.push(Span::styled(
                format!("  {}  ", group.name),
                Style::default().fg(Color::Gray),
            ));
        }
    }
    let tabs_line = Line::from(tabs_spans);

    // Watchlist Layout: Tabs top, Stocks Table below
    let watchlist_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Tab headers
            Constraint::Min(2),    // Table list
        ])
        .split(rect);

    f.render_widget(Paragraph::new(tabs_line), watchlist_chunks[0]);

    // Table Header
    let header_cells = ["代码", "名称", "现价", "幅%", "状态"].iter().map(|h| {
        Cell::from(*h).style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        )
    });
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    // Table Rows
    let mut rows = Vec::new();
    if let Some(group) = app.current_group() {
        for i in app.visible_stock_indices() {
            let Some(code) = group.stocks.get(i) else {
                continue;
            };
            let is_selected = i == app.selected_stock_idx;

            // Check if we have dynamic data for this stock code
            let row_cells = if let Some(stock) = app.stock_data.get(code) {
                let color = if stock.pct_change > 0.0 {
                    COLOR_UP
                } else if stock.pct_change < 0.0 {
                    COLOR_DOWN
                } else {
                    COLOR_EVEN
                };

                let clean_code = code.replace("sh", "").replace("sz", "").replace("bj", "");

                let marker = stock
                    .anomaly_tag(app.highlight_thresholds)
                    .map(str::to_string)
                    .unwrap_or_else(|| quote_state_label(app.quote_meta.get(code)));
                vec![
                    Cell::from(clean_code),
                    Cell::from(stock.name.as_str()),
                    Cell::from(format!("{:.2}", stock.price)).style(Style::default().fg(color)),
                    Cell::from(format!("{:+.2}%", stock.pct_change))
                        .style(Style::default().fg(color)),
                    Cell::from(marker),
                ]
            } else {
                let clean_code = code.replace("sh", "").replace("sz", "").replace("bj", "");
                vec![
                    Cell::from(clean_code),
                    Cell::from("加载中..."),
                    Cell::from("--"),
                    Cell::from("--"),
                    Cell::from("缺数据"),
                ]
            };

            let row_style = if is_selected {
                Style::default()
                    .bg(Color::Rgb(40, 44, 52))
                    .add_modifier(Modifier::BOLD)
            } else if app
                .stock_data
                .get(code)
                .is_some_and(|stock| stock.is_anomaly(app.highlight_thresholds))
            {
                Style::default().bg(Color::Rgb(58, 32, 82))
            } else {
                Style::default()
            };

            rows.push(Row::new(row_cells).style(row_style));
        }
    }

    let widths = [
        Constraint::Length(8),
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Length(10),
    ];

    let title = format!(
        " 自选股列表  排序:{}  过滤:{}  布局:{} ",
        app.sort_mode.label(),
        app.filter_mode.label(),
        app.layout_mode.label()
    );
    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(table, watchlist_chunks[1]);
}

fn render_details(f: &mut Frame, app: &App, rect: Rect) {
    let top_height = match app.layout_mode {
        LayoutMode::LargeChart => 10,
        LayoutMode::OrderBook => 18,
        _ => 14,
    };
    let detail_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(top_height), Constraint::Min(5)])
        .split(rect);

    let stock_code = app.selected_stock_code();

    // Check if we have stock data
    let stock = stock_code.and_then(|code| app.stock_data.get(&code));

    match stock {
        Some(s) => {
            render_bid_ask(f, app, s, app.quote_meta.get(&s.code), detail_chunks[0]);
            render_chart(f, app, s, detail_chunks[1]);
        }
        None => {
            let empty_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray));
            f.render_widget(
                Paragraph::new("请在左侧选择股票以查看详情").block(empty_block),
                detail_chunks[0],
            );

            let chart_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray));
            f.render_widget(
                Paragraph::new("图表区域").block(chart_block),
                detail_chunks[1],
            );
        }
    }
}

use ratatui::widgets::Cell;

fn render_bid_ask(f: &mut Frame, app: &App, stock: &Stock, meta: Option<&QuoteMeta>, rect: Rect) {
    // We split this area horizontally: Left for basic stock stats, Right for Level 5 Order Book
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Stats
            Constraint::Percentage(50), // 5-Level Book
        ])
        .split(rect);

    // 1. Stats Panel
    let stat_color = if stock.pct_change > 0.0 {
        COLOR_UP
    } else if stock.pct_change < 0.0 {
        COLOR_DOWN
    } else {
        COLOR_EVEN
    };

    let overview = app.current_group_overview();
    let stats_text = vec![
        Line::from(vec![Span::styled(
            format!("{} ({})", stock.name, stock.code.to_uppercase()),
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::raw("现价: "),
            Span::styled(
                format!("{:.2}  ", stock.price),
                Style::default().fg(stat_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:+.2} ({:+.2}%)", stock.change, stock.pct_change),
                Style::default().fg(stat_color),
            ),
        ]),
        Line::from(vec![
            Span::raw("今开: "),
            Span::styled(
                format!("{:.2}    ", stock.open),
                Style::default().fg(if stock.open > stock.close {
                    COLOR_UP
                } else if stock.open < stock.close {
                    COLOR_DOWN
                } else {
                    COLOR_EVEN
                }),
            ),
            Span::raw("昨收: "),
            Span::raw(format!("{:.2}", stock.close)),
        ]),
        Line::from(vec![
            Span::raw("最高: "),
            Span::styled(
                format!("{:.2}    ", stock.high),
                Style::default().fg(COLOR_UP),
            ),
            Span::raw("最低: "),
            Span::styled(format!("{:.2}", stock.low), Style::default().fg(COLOR_DOWN)),
        ]),
        Line::from(vec![
            Span::raw("成交量: "),
            Span::raw(format!("{:.1}万手    ", stock.volume / 10000.0)),
            Span::raw("成交额: "),
            Span::raw(format!("{:.2}亿", stock.amount / 100_000_000.0)),
        ]),
        Line::from(vec![
            Span::raw("换手: "),
            Span::raw(format_optional_pct(stock.turnover_rate)),
            Span::raw(" 量比: "),
            Span::raw(format_optional_number(stock.volume_ratio)),
            Span::raw(" 振幅: "),
            Span::raw(format_optional_pct(stock.amplitude)),
        ]),
        Line::from(vec![
            Span::raw("涨停: "),
            Span::raw(format_optional_price(stock.limit_up)),
            Span::raw(" 跌停: "),
            Span::raw(format_optional_price(stock.limit_down)),
            Span::raw(" 市值: "),
            Span::raw(format_optional_market_cap(stock.market_cap)),
        ]),
        Line::from(vec![
            Span::raw("分组: "),
            Span::raw(format!(
                "均{:+.2}% 涨{}跌{} 额{:.1}亿",
                overview.avg_pct_change,
                overview.rising,
                overview.falling,
                overview.total_amount / 100_000_000.0
            )),
        ]),
        Line::from(vec![
            Span::raw("状态: "),
            Span::styled(
                quote_detail_label(meta),
                Style::default().fg(
                    if meta.and_then(|meta| meta.last_error.as_ref()).is_some() {
                        Color::LightRed
                    } else {
                        Color::LightCyan
                    },
                ),
            ),
        ]),
    ];

    let stats_block = Block::default()
        .title(" 个股数据 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(Paragraph::new(stats_text).block(stats_block), chunks[0]);

    // 2. 5-Level Book
    let mut rows = Vec::new();
    rows.push(Row::new(vec![
        Cell::from("汇总").style(Style::default().fg(Color::Cyan)),
        Cell::from(format_order_book_summary(stock)),
        Cell::from(format_imbalance(stock)),
    ]));

    // Sell side (Sell 5 down to Sell 1)
    for i in (0..5).rev() {
        if i < stock.ask_prices.len() && i < stock.ask_volumes.len() {
            let price = stock.ask_prices[i];
            let vol = stock.ask_volumes[i];
            let price_color = if price > stock.close {
                COLOR_UP
            } else if price < stock.close {
                COLOR_DOWN
            } else {
                COLOR_EVEN
            };
            rows.push(Row::new(vec![
                Cell::from(format!("卖{}", i + 1)).style(Style::default().fg(Color::Gray)),
                Cell::from(format_order_book_price(price)).style(Style::default().fg(price_color)),
                Cell::from(format_order_book_volume(vol)).style(Style::default().fg(Color::Yellow)),
            ]));
        }
    }

    // Divider
    rows.push(
        Row::new(vec![
            Cell::from("------"),
            Cell::from("------"),
            Cell::from("------"),
        ])
        .style(Style::default().fg(Color::DarkGray)),
    );

    // Buy side (Buy 1 to Buy 5)
    for i in 0..5 {
        if i < stock.bid_prices.len() && i < stock.bid_volumes.len() {
            let price = stock.bid_prices[i];
            let vol = stock.bid_volumes[i];
            let price_color = if price > stock.close {
                COLOR_UP
            } else if price < stock.close {
                COLOR_DOWN
            } else {
                COLOR_EVEN
            };
            rows.push(Row::new(vec![
                Cell::from(format!("买{}", i + 1)).style(Style::default().fg(Color::Gray)),
                Cell::from(format_order_book_price(price)).style(Style::default().fg(price_color)),
                Cell::from(format_order_book_volume(vol)).style(Style::default().fg(Color::Yellow)),
            ]));
        }
    }

    let book_table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(10),
            Constraint::Length(8),
        ],
    )
    .block(
        Block::default()
            .title(" 五档盘口 ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(book_table, chunks[1]);
}

/// Renders historical daily bars for the selected stock when K-line data is available.
fn render_chart(f: &mut Frame, app: &App, stock: &Stock, rect: Rect) {
    match app.chart_mode {
        ChartMode::Intraday => render_intraday_chart(f, app, stock, rect),
        period => render_kline_period(f, app, stock, period, rect),
    }
}

/// Renders the selected stock's intraday minute price chart.
fn render_intraday_chart(f: &mut Frame, app: &App, stock: &Stock, rect: Rect) {
    if let Some(points) = app.intraday_data.get(&stock.code) {
        if !points.is_empty() {
            render_intraday_price_volume(f, points, stock, rect);
            return;
        }
    }

    let chart_block = Block::default()
        .title(" 分时图 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let chart_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  [ 分时图暂未接入 ]  ",
            Style::default()
                .add_modifier(Modifier::ITALIC)
                .fg(Color::Yellow),
        )]),
        Line::from(""),
        Line::from("  切换到分时图时会自动刷新当前股票分时数据。"),
    ];
    f.render_widget(Paragraph::new(chart_text).block(chart_block), rect);
}

/// Renders an intraday price line with a compact volume pane.
fn render_intraday_price_volume(
    f: &mut Frame,
    points: &[crate::app::MinutePoint],
    stock: &Stock,
    rect: Rect,
) {
    let plotted = intraday_plot_points(points);
    let volume_points = intraday_volume_points(points, stock.close);
    if plotted.is_empty() {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(6)])
        .split(rect);

    render_intraday_price_chart(f, &plotted, stock, chunks[0]);
    render_longbridge_intraday_volume(f, &volume_points, chunks[1]);
}

/// Renders intraday price against the previous-close baseline.
fn render_intraday_price_chart(
    f: &mut Frame,
    plotted: &[IntradayPlotPoint<'_>],
    stock: &Stock,
    rect: Rect,
) {
    let price_data: Vec<(f64, f64)> = plotted
        .iter()
        .map(|plot_point| (plot_point.x, plot_point.point.price))
        .collect();
    let average_data = intraday_average_data(plotted);
    if price_data.is_empty() {
        return;
    }

    let prev_close_data = vec![(0.0, stock.close), (INTRADAY_SESSION_X_MAX, stock.close)];
    let min_price = plotted
        .iter()
        .map(|plot_point| plot_point.point.price)
        .filter(|price| price.is_finite())
        .fold(stock.close, f64::min);
    let max_price = plotted
        .iter()
        .map(|plot_point| plot_point.point.price)
        .filter(|price| price.is_finite())
        .fold(stock.close, f64::max);
    let y_radius = ((max_price - stock.close)
        .abs()
        .max((stock.close - min_price).abs())
        * 1.08)
        .max(0.01);
    let y_min = stock.close - y_radius;
    let y_max = stock.close + y_radius;
    let last = plotted.last().expect("plotted intraday points").point;

    let colored_segments = split_price_segments_by_baseline(&price_data, stock.close);
    let mut datasets = Vec::with_capacity(colored_segments.len() + 1);
    for segment in &colored_segments {
        let color = if segment.is_above_baseline {
            COLOR_UP
        } else {
            COLOR_DOWN
        };
        datasets.push(
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(color))
                .data(&segment.points),
        );
    }
    datasets.push(
        Dataset::default()
            .marker(symbols::Marker::Dot)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::DarkGray))
            .data(&prev_close_data),
    );
    datasets.push(
        Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Yellow))
            .data(&average_data),
    );
    let stats = intraday_stats(plotted, stock.close);

    let price_chart = RatatuiChart::new(datasets)
        .block(
            Block::default()
                .title(format!(
                    " 分时图 {} {:.2} 高{:.2} 低{:.2} 距昨{:+.2}% 均线黄 ",
                    last.time, last.price, stats.high, stats.low, stats.pct_vs_prev_close
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .x_axis(
            Axis::default()
                .bounds([0.0, INTRADAY_SESSION_X_MAX])
                .labels(vec![
                    Span::raw("0930"),
                    Span::raw("1130/1300"),
                    Span::raw("1500"),
                ]),
        )
        .y_axis(Axis::default().bounds([y_min, y_max]).labels(vec![
            Span::raw(format!("{:.2}", y_min)),
            Span::raw(format!("{:.2}", stock.close)),
            Span::raw(format!("{:.2}", y_max)),
        ]));
    f.render_widget(price_chart, rect);
}

/// Renders one historical K-line period or a loading/unavailable state.
fn render_kline_period(f: &mut Frame, app: &App, stock: &Stock, period: ChartMode, rect: Rect) {
    if let Some(kline) = app.kline_data.get(&(stock.code.clone(), period)) {
        if !kline.is_empty() {
            render_candlestick_chart(f, kline, period.label(), rect);
            return;
        }
    }

    let chart_block = Block::default()
        .title(format!(" {} ", period.label()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let chart_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            format!("  [ 正在加载最近{}历史数据... ]  ", period.label()),
            Style::default()
                .add_modifier(Modifier::ITALIC)
                .fg(Color::Yellow),
        )]),
        Line::from(""),
        Line::from("  可用周期会自动刷新；行情源不返回时保持此状态。"),
    ];
    f.render_widget(Paragraph::new(chart_text).block(chart_block), rect);
}

/// Renders daily K-lines as Unicode candlesticks with a volume pane.
fn render_candlestick_chart(f: &mut Frame, kline: &[crate::app::KLine], title: &str, rect: Rect) {
    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let area = block.inner(rect);
    f.render_widget(block, rect);

    if area.width < 24 || area.height < 8 {
        return;
    }

    let candle_slots = area.width.saturating_sub(14).max(1) as usize;
    let start = kline.len().saturating_sub(candle_slots);
    let visible = &kline[start..];
    let candles = kline_to_candles(visible);

    if candles.is_empty() {
        f.render_widget(
            Paragraph::new("K线数据格式异常").alignment(ratatui::layout::Alignment::Center),
            area,
        );
        return;
    }

    let mut chart =
        cli_candlestick_chart::Chart::new_with_size(candles, (area.width - 1, area.height));
    let (bull, bear) = a_share_chart_colors();
    chart.set_bull_color(bull);
    chart.set_vol_bull_color(bull);
    chart.set_bear_color(bear);
    chart.set_vol_bear_color(bear);
    chart.set_volume_pane_unicode_fill('▄');
    chart.set_candle_width(KLINE_CANDLE_WIDTH);

    let chart_str = chart.render();
    f.render_widget(AnsiChart(&chart_str), area);
}

/// Returns chart-library colors using A-share red-up/green-down semantics.
fn a_share_chart_colors() -> (cli_candlestick_chart::Color, cli_candlestick_chart::Color) {
    (
        cli_candlestick_chart::Color::BrightRed,
        cli_candlestick_chart::Color::BrightGreen,
    )
}

struct PriceSegment {
    points: Vec<(f64, f64)>,
    is_above_baseline: bool,
}

/// Splits price points whenever the line crosses the previous-close baseline.
fn split_price_segments_by_baseline(points: &[(f64, f64)], baseline: f64) -> Vec<PriceSegment> {
    let mut segments = Vec::new();
    let Some(first) = points.first().copied() else {
        return segments;
    };

    let mut current_is_above = first.1 >= baseline;
    let mut current_points = vec![first];

    for window in points.windows(2) {
        let previous = window[0];
        let next = window[1];
        let next_is_above = next.1 >= baseline;

        if next_is_above != current_is_above {
            if let Some(crossing) = baseline_crossing(previous, next, baseline) {
                current_points.push(crossing);
                segments.push(PriceSegment {
                    points: current_points,
                    is_above_baseline: current_is_above,
                });
                current_points = vec![crossing, next];
            } else {
                segments.push(PriceSegment {
                    points: current_points,
                    is_above_baseline: current_is_above,
                });
                current_points = vec![next];
            }
            current_is_above = next_is_above;
        } else {
            current_points.push(next);
        }
    }

    segments.push(PriceSegment {
        points: current_points,
        is_above_baseline: current_is_above,
    });
    segments
}

/// Computes the chart-space point where a segment crosses the baseline.
fn baseline_crossing(previous: (f64, f64), next: (f64, f64), baseline: f64) -> Option<(f64, f64)> {
    let price_delta = next.1 - previous.1;
    if price_delta.abs() < f64::EPSILON {
        return None;
    }

    let ratio = (baseline - previous.1) / price_delta;
    if !(0.0..=1.0).contains(&ratio) {
        return None;
    }

    Some((previous.0 + (next.0 - previous.0) * ratio, baseline))
}

/// Renders a compact volume pane using Longbridge LineChart candle direction semantics.
fn render_longbridge_intraday_volume(
    f: &mut Frame,
    volume_points: &[IntradayVolumePoint],
    rect: Rect,
) {
    let block = Block::default()
        .title(" 量能 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let area = block.inner(rect);
    f.render_widget(block, rect);

    if area.width == 0 || area.height == 0 || volume_points.is_empty() {
        return;
    }

    let max_volume = volume_points
        .iter()
        .map(|point| point.volume)
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let columns = area.width as usize;
    let rows = area.height as usize;
    let pixel_width = columns * 2;
    let pixel_height = rows * 4;
    let mut volume_bits = vec![vec![0u8; columns]; rows];
    let mut volume_is_up = vec![vec![true; columns]; rows];

    for point in volume_points {
        let volume = point.volume.max(0.0);
        if volume <= 0.0 || !point.x.is_finite() || point.x < 0.0 {
            continue;
        }

        let fill =
            ((volume / max_volume) * (pixel_height.saturating_sub(1)) as f64).round() as usize;
        let pixel_x = intraday_x_to_volume_pixel(point.x, pixel_width);
        let col = pixel_x / 2;
        let dx = pixel_x % 2;

        for pixel_y in 0..=fill {
            let y_from_top = pixel_height - 1 - pixel_y;
            let row = y_from_top / 4;
            let dy = y_from_top % 4;
            if row < rows && col < columns {
                volume_bits[row][col] |= braille_dot_bit(dx, dy);
                volume_is_up[row][col] = point.is_up;
            }
        }
    }

    let mut lines = Vec::with_capacity(rows);
    for row in 0..rows {
        let mut spans = Vec::with_capacity(columns);
        for col in 0..columns {
            let bits = volume_bits[row][col];
            if bits == 0 {
                spans.push(Span::raw(" "));
            } else {
                let color = if volume_is_up[row][col] {
                    COLOR_UP
                } else {
                    COLOR_DOWN
                };
                spans.push(Span::styled(
                    braille_char(bits).to_string(),
                    Style::default().fg(color),
                ));
            }
        }
        lines.push(Line::from(spans));
    }

    f.render_widget(Paragraph::new(lines), area);
}

/// Maps fixed intraday chart-space x into a volume-pane Braille pixel column.
fn intraday_x_to_volume_pixel(x: f64, pixel_width: usize) -> usize {
    if pixel_width <= 1 {
        return 0;
    }

    let clamped_x = x.clamp(0.0, INTRADAY_SESSION_X_MAX);
    ((clamped_x / INTRADAY_SESSION_X_MAX) * (pixel_width - 1) as f64).round() as usize
}

/// Returns the Unicode Braille bit for a 2x4 pixel coordinate inside one cell.
const fn braille_dot_bit(dx: usize, dy: usize) -> u8 {
    match (dx, dy) {
        (0, 0) => 0x01,
        (0, 1) => 0x02,
        (0, 2) => 0x04,
        (0, 3) => 0x40,
        (1, 0) => 0x08,
        (1, 1) => 0x10,
        (1, 2) => 0x20,
        (1, 3) => 0x80,
        _ => 0,
    }
}

/// Converts packed Braille bits into the corresponding Unicode character.
fn braille_char(bits: u8) -> char {
    char::from_u32(0x2800 + u32::from(bits)).unwrap_or(' ')
}

/// Converts daily K-line records into validated chart-library candles.
fn kline_to_candles(kline: &[crate::app::KLine]) -> Vec<cli_candlestick_chart::Candle> {
    kline
        .iter()
        .filter_map(|bar| {
            let prices_valid = [bar.open, bar.high, bar.low, bar.close]
                .iter()
                .all(|value| value.is_finite() && *value > 0.0);
            if !prices_valid
                || bar.high < bar.low
                || bar.high < bar.open
                || bar.high < bar.close
                || bar.low > bar.open
                || bar.low > bar.close
            {
                return None;
            }

            Some(cli_candlestick_chart::Candle {
                open: bar.open,
                high: bar.high,
                low: bar.low,
                close: bar.close,
                volume: Some((bar.volume / 10000.0).max(0.0)),
                timestamp: None,
            })
        })
        .collect()
}

/// Formats compact quote state for one watchlist row.
fn quote_state_label(meta: Option<&QuoteMeta>) -> String {
    let Some(meta) = meta else {
        return "缺数据".to_string();
    };
    if meta.last_error.is_some() {
        return "失败".to_string();
    }
    match meta.session_state {
        crate::app::QuoteSessionState::Live => "实时".to_string(),
        crate::app::QuoteSessionState::ClosedSnapshot => "收盘".to_string(),
        crate::app::QuoteSessionState::ManualSnapshot => "已更新".to_string(),
    }
}

/// Formats detailed quote freshness/source state for the selected stock panel.
fn quote_detail_label(meta: Option<&QuoteMeta>) -> String {
    let Some(meta) = meta else {
        return "未收到行情".to_string();
    };
    let age = chrono::Local::now()
        .signed_duration_since(meta.received_at)
        .num_seconds()
        .max(0);
    let mut parts = Vec::new();
    if matches!(meta.session_state, crate::app::QuoteSessionState::Live) {
        parts.push("实时".to_string());
    } else if matches!(
        meta.session_state,
        crate::app::QuoteSessionState::ClosedSnapshot
    ) {
        parts.push("收盘".to_string());
    }
    parts.push(meta.source.label().to_string());
    parts.push(format!("{}秒前", age));
    let mut label = parts.join(" / ");
    if let Some(error) = &meta.last_error {
        label.push_str(&format!(" / {}", error));
    }
    label
}

/// Formats optional percent fields returned by quote providers.
fn format_optional_pct(value: Option<f64>) -> String {
    value
        .map(|value| format!("{:.2}%", value))
        .unwrap_or_else(|| "--".to_string())
}

/// Formats optional floating-point quote fields.
fn format_optional_number(value: Option<f64>) -> String {
    value
        .map(|value| format!("{:.2}", value))
        .unwrap_or_else(|| "--".to_string())
}

/// Formats optional price fields.
fn format_optional_price(value: Option<f64>) -> String {
    value
        .map(|value| format!("{:.2}", value))
        .unwrap_or_else(|| "--".to_string())
}

/// Formats optional market-cap fields, assuming provider units are hundred-million yuan.
fn format_optional_market_cap(value: Option<f64>) -> String {
    value
        .map(|value| format!("{:.1}亿", value))
        .unwrap_or_else(|| "--".to_string())
}

/// Formats valid order-book prices while preserving unavailable levels.
fn format_order_book_price(price: f64) -> String {
    if price.is_finite() && price > 0.0 {
        format!("{:.2}", price)
    } else {
        "--".to_string()
    }
}

/// Formats valid order-book volumes while preserving unavailable levels.
fn format_order_book_volume(volume: i64) -> String {
    if volume > 0 {
        volume.to_string()
    } else {
        "--".to_string()
    }
}

/// Formats bid/ask totals and spread for the order-book panel.
fn format_order_book_summary(stock: &Stock) -> String {
    let spread = stock
        .bid_ask_spread()
        .map(|spread| format!("{:.2}", spread))
        .unwrap_or_else(|| "--".to_string());
    format!(
        "买{} 卖{} 差{}",
        stock.total_bid_volume(),
        stock.total_ask_volume(),
        spread
    )
}

/// Formats bid/ask imbalance for the order-book panel.
fn format_imbalance(stock: &Stock) -> String {
    stock
        .order_book_imbalance()
        .map(|imbalance| format!("{:+.0}%", imbalance * 100.0))
        .unwrap_or_else(|| "--".to_string())
}

fn render_footer(f: &mut Frame, app: &App, rect: Rect) {
    let help_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Keybindings help
            Constraint::Length(1), // Status message
        ])
        .split(rect);

    // 1. Status Bar
    let status_text = match &app.status_message {
        Some(msg) => format!(" STATUS: {}", msg),
        None => " STATUS: 运行正常".to_string(),
    };
    let status_bar =
        Paragraph::new(status_text).style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(status_bar, help_chunks[0]);

    // 2. Keybindings
    let keys = Line::from(vec![
        Span::styled(
            " q",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(":退出 | "),
        Span::styled(
            "j/k",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":选择股票 | "),
        Span::styled(
            "[/]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":切换板块 | "),
        Span::styled(
            "s/f",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":排序/过滤 | "),
        Span::styled(
            "l",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":布局 | "),
        Span::styled(
            "a",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":添加自选 | "),
        Span::styled(
            "d",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(":删除自选 | "),
        Span::styled(
            "t",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":切换图表 | "),
        Span::styled(
            "o",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":早盘竞价 | "),
        Span::styled(
            "?",
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":帮助 | "),
    ]);
    f.render_widget(Paragraph::new(keys), help_chunks[1]);
}

/// Renders the keyboard help panel with all supported shortcuts.
fn render_help_popup(f: &mut Frame) {
    let area = center_rect(70, 22.min(f.size().height.saturating_sub(2)), f.size());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" 快捷键帮助  ?/Esc关闭 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let text = vec![
        Line::from(vec![Span::styled(
            "全局",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        help_line("q", "退出程序"),
        help_line("?", "打开或关闭本帮助面板"),
        help_line("Esc", "关闭当前弹框"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "行情浏览",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        help_line("j / ↓", "选择下一只股票，并刷新当前股票行情"),
        help_line("k / ↑", "选择上一只股票，并刷新当前股票行情"),
        help_line("[ / ]", "切换自选股板块"),
        help_line("t", "循环切换分时、分钟K、日K、周K、月K"),
        help_line("s", "循环切换自选股排序方式"),
        help_line("f", "循环切换上涨/下跌/平盘/缺数据过滤"),
        help_line("/", "按代码或名称文本过滤自选股"),
        help_line("l", "循环切换平衡、紧凑、大图、盘口布局"),
        help_line("o", "打开或关闭早盘竞价K线弹框"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "自选股",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        help_line("a", "打开添加自选股搜索框"),
        help_line("d", "删除当前选中的自选股"),
        help_line("g", "新建自选股分组"),
        help_line("r", "重命名当前分组"),
        help_line("x", "删除当前分组，至少保留一个分组"),
        help_line("< / >", "前移或后移当前分组"),
        help_line("m / c", "移动或复制当前股票到下一个分组"),
        help_line("e / i", "导出或导入自选股CSV"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "搜索框内",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        help_line("输入文字", "按代码、中文名或拼音搜索A股"),
        help_line("↑ / ↓", "切换搜索结果"),
        help_line("Enter", "添加选中的搜索结果；无结果时尝试按代码添加"),
        help_line("Backspace", "删除输入字符并重新搜索"),
        help_line("Esc", "取消添加并关闭搜索框"),
    ];

    f.render_widget(Paragraph::new(text).block(block), area);
}

/// Formats one keyboard help row.
fn help_line(key: &str, description: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {:<10}", key),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(description.to_string()),
    ])
}

/// Renders a centered popup showing early auction minute points as approximate candles.
fn render_opening_auction_popup(f: &mut Frame, app: &App) {
    let area = center_rect(78, 24.min(f.size().height.saturating_sub(2)), f.size());
    f.render_widget(Clear, area);

    let Some(code) = app.selected_stock_code() else {
        render_opening_auction_message(f, "早盘竞价", "请先选择一只股票", area);
        return;
    };
    let Some(stock) = app.stock_data.get(&code) else {
        render_opening_auction_message(f, "早盘竞价", "正在加载个股行情...", area);
        return;
    };
    let Some(points) = app.intraday_data.get(&code) else {
        render_opening_auction_message(f, "早盘竞价", "正在加载分时数据...", area);
        return;
    };

    let candles = opening_auction_klines(points, stock.close);
    if candles.is_empty() {
        render_opening_auction_message(
            f,
            &format!("{} 早盘竞价", stock.name),
            "暂无 09:15-09:30 竞价分钟数据；请在开盘后刷新，或等待行情源返回竞价段。",
            area,
        );
        return;
    }

    render_candlestick_chart(
        f,
        &candles,
        &format!("{} 早盘竞价  分时近似K  o/Esc关闭", stock.name),
        area,
    );
}

/// Renders a popup message when auction data is unavailable or still loading.
fn render_opening_auction_message(f: &mut Frame, title: &str, message: &str, area: Rect) {
    let block = Block::default()
        .title(format!(" {}  o/Esc关闭 ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            message,
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )),
    ];
    f.render_widget(Paragraph::new(text).block(block), area);
}

/// Builds 09:15-09:30 approximate one-minute candles from intraday price points.
fn opening_auction_klines(points: &[MinutePoint], fallback_open: f64) -> Vec<KLine> {
    let auction_points: Vec<&MinutePoint> = points
        .iter()
        .filter(|point| {
            point.price.is_finite()
                && point.price > 0.0
                && minute_of_day(&point.time)
                    .is_some_and(|minute| (9 * 60 + 15..=9 * 60 + 30).contains(&minute))
        })
        .collect();

    let mut previous_close = if fallback_open.is_finite() && fallback_open > 0.0 {
        fallback_open
    } else {
        auction_points
            .first()
            .map(|point| point.price)
            .unwrap_or_default()
    };

    auction_points
        .into_iter()
        .map(|point| {
            let open = previous_close;
            let close = point.price;
            previous_close = close;
            KLine {
                date: point.time.clone(),
                open,
                close,
                high: open.max(close),
                low: open.min(close),
                volume: point.volume,
            }
        })
        .collect()
}

/// Converts minute ticks into fixed-session volume samples with candle direction colors.
fn intraday_volume_points(points: &[MinutePoint], previous_close: f64) -> Vec<IntradayVolumePoint> {
    let mut previous_price = if previous_close.is_finite() && previous_close > 0.0 {
        previous_close
    } else {
        points
            .iter()
            .find(|point| point.price.is_finite() && point.price > 0.0)
            .map(|point| point.price)
            .unwrap_or_default()
    };

    points
        .iter()
        .filter_map(|point| {
            if !point.price.is_finite() || point.price <= 0.0 {
                return None;
            }

            let x = trade_session_x(&point.time)?;
            let open = previous_price;
            let close = point.price;
            previous_price = close;
            Some(IntradayVolumePoint {
                x,
                volume: point.volume.max(0.0),
                is_up: close >= open,
            })
        })
        .collect()
}

/// Parses compact HHMM or colon HH:MM time strings into minutes since midnight.
fn minute_of_day(raw: &str) -> Option<u16> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 4 {
        return None;
    }
    let time = &digits[digits.len() - 4..];
    let hour = time[..2].parse::<u16>().ok()?;
    let minute = time[2..].parse::<u16>().ok()?;
    if hour >= 24 || minute >= 60 {
        return None;
    }
    Some(hour * 60 + minute)
}

/// Maps A-share continuous trading minutes onto a fixed merged intraday x-axis.
fn trade_session_x(raw: &str) -> Option<f64> {
    let minute = minute_of_day(raw)?;
    let morning_start = 9 * 60 + 30;
    let morning_end = 11 * 60 + 30;
    let afternoon_start = 13 * 60;
    let afternoon_end = 15 * 60;

    if (morning_start..=morning_end).contains(&minute) {
        Some((minute - morning_start) as f64)
    } else if (afternoon_start..=afternoon_end).contains(&minute) {
        Some(121.0 + (minute - afternoon_start) as f64)
    } else {
        None
    }
}

/// Converts raw minute points into validated fixed-session plot coordinates.
fn intraday_plot_points(points: &[MinutePoint]) -> Vec<IntradayPlotPoint<'_>> {
    points
        .iter()
        .filter_map(|point| {
            if !point.price.is_finite() || point.price <= 0.0 {
                return None;
            }
            Some(IntradayPlotPoint {
                point,
                x: trade_session_x(&point.time)?,
            })
        })
        .collect()
}

/// Computes cumulative average-price line points for validated intraday samples.
fn intraday_average_data(plotted: &[IntradayPlotPoint<'_>]) -> Vec<(f64, f64)> {
    let mut sum = 0.0;
    plotted
        .iter()
        .enumerate()
        .map(|(idx, plot_point)| {
            sum += plot_point.point.price;
            (plot_point.x, sum / (idx + 1) as f64)
        })
        .collect()
}

/// Computes high, low, and distance from previous close for an intraday series.
fn intraday_stats(plotted: &[IntradayPlotPoint<'_>], previous_close: f64) -> IntradayStats {
    let mut high = f64::NEG_INFINITY;
    let mut low = f64::INFINITY;
    let mut last = previous_close;
    for plot_point in plotted {
        let price = plot_point.point.price;
        high = high.max(price);
        low = low.min(price);
        last = price;
    }
    let pct_vs_prev_close = if previous_close.abs() > f64::EPSILON {
        (last - previous_close) / previous_close * 100.0
    } else {
        0.0
    };
    IntradayStats {
        high,
        low,
        pct_vs_prev_close,
    }
}

fn render_search_popup(f: &mut Frame, app: &App) {
    let block = Block::default()
        .title(" 添加自选股 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let mut text = vec![
        Line::from("输入代码、中文名或拼音搜索 A 股:"),
        Line::from(""),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(
                app.search_input.as_str(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ]),
        Line::from(""),
    ];

    if app.search_input.trim().is_empty() {
        text.push(Line::from(Span::styled(
            "示例: 600519 / 茅台 / pingan",
            Style::default().fg(Color::Gray),
        )));
    } else if app.search_results.is_empty() {
        text.push(Line::from(Span::styled(
            "暂无匹配，回车仅支持合法股票代码",
            Style::default().fg(Color::Gray),
        )));
    } else {
        for (idx, result) in app.search_results.iter().take(8).enumerate() {
            let selected = idx == app.selected_search_result_idx;
            let marker = if selected { ">" } else { " " };
            let style = if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            text.push(Line::from(vec![Span::styled(
                format!(
                    "{} {}  {}  {}",
                    marker,
                    result.code.to_uppercase(),
                    result.name,
                    result.pinyin
                ),
                style,
            )]));
        }
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled(
        "Enter 添加选中项，↑/↓ 切换，Esc 取消",
        Style::default().fg(Color::Gray),
    )));

    let area = center_rect(60, 14, f.size());
    f.render_widget(Clear, area); // Overwrites anything beneath
    f.render_widget(Paragraph::new(text).block(block), area);
}

/// Renders text input for creating or renaming watchlist groups.
fn render_group_name_popup(f: &mut Frame, app: &App, action: &GroupNameAction) {
    let title = match action {
        GroupNameAction::Create => " 新建分组 ",
        GroupNameAction::Rename => " 重命名分组 ",
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let text = vec![
        Line::from("输入分组名称:"),
        Line::from(""),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(
                app.text_input.as_str(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Enter 确认，Esc 取消",
            Style::default().fg(Color::Gray),
        )),
    ];
    let area = center_rect(50, 9, f.size());
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(text).block(block), area);
}

/// Renders text input for watchlist code/name filtering.
fn render_filter_text_popup(f: &mut Frame, app: &App) {
    let block = Block::default()
        .title(" 文本过滤 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let text = vec![
        Line::from("输入代码或名称片段:"),
        Line::from(""),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(
                app.text_input.as_str(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Enter 应用，空输入清除过滤，Esc 取消",
            Style::default().fg(Color::Gray),
        )),
    ];
    let area = center_rect(50, 9, f.size());
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(text).block(block), area);
}

// Simple helper to construct a centered popup area
fn center_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((r.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_intraday_time_labels() {
        assert_eq!(minute_of_day("0915"), Some(9 * 60 + 15));
        assert_eq!(minute_of_day("09:30"), Some(9 * 60 + 30));
        assert_eq!(minute_of_day("202606300946"), Some(9 * 60 + 46));
        assert_eq!(minute_of_day("2460"), None);
    }

    #[test]
    fn maps_intraday_points_to_fixed_full_day_axis() {
        assert_eq!(trade_session_x("0929"), None);
        assert_eq!(trade_session_x("0930"), Some(0.0));
        assert_eq!(trade_session_x("1130"), Some(120.0));
        assert_eq!(trade_session_x("1200"), None);
        assert_eq!(trade_session_x("1300"), Some(121.0));
        assert_eq!(trade_session_x("1500"), Some(INTRADAY_SESSION_X_MAX));
        assert_eq!(trade_session_x("1501"), None);
    }

    #[test]
    fn filters_intraday_points_before_price_plotting() {
        let points = vec![
            MinutePoint {
                time: "0929".to_string(),
                price: 10.0,
                volume: 100.0,
                amount: 1000.0,
            },
            MinutePoint {
                time: "0930".to_string(),
                price: 10.1,
                volume: 120.0,
                amount: 1212.0,
            },
            MinutePoint {
                time: "1000".to_string(),
                price: 0.0,
                volume: 80.0,
                amount: 0.0,
            },
        ];
        let plotted = intraday_plot_points(&points);

        assert_eq!(plotted.len(), 1);
        assert_eq!(plotted[0].point.time, "0930");
        assert_eq!(plotted[0].x, 0.0);
    }

    #[test]
    fn converts_minute_points_to_per_coordinate_volume_points() {
        let points = vec![
            MinutePoint {
                time: "0930".to_string(),
                price: 10.1,
                volume: 100.0,
                amount: 1010.0,
            },
            MinutePoint {
                time: "0931".to_string(),
                price: 10.0,
                volume: 120.0,
                amount: 1200.0,
            },
            MinutePoint {
                time: "1500".to_string(),
                price: 10.2,
                volume: 140.0,
                amount: 1428.0,
            },
        ];
        let volume_points = intraday_volume_points(&points, 10.0);

        assert_eq!(volume_points.len(), 3);
        assert_eq!(volume_points[0].x, 0.0);
        assert_eq!(volume_points[0].volume, 100.0);
        assert!(volume_points[0].is_up);
        assert_eq!(volume_points[1].x, 1.0);
        assert_eq!(volume_points[1].volume, 120.0);
        assert!(!volume_points[1].is_up);
        assert_eq!(volume_points[2].x, INTRADAY_SESSION_X_MAX);
        assert_eq!(volume_points[2].volume, 140.0);
        assert!(volume_points[2].is_up);
    }

    #[test]
    fn filters_invalid_minute_points_before_volume_rendering() {
        let points = vec![
            MinutePoint {
                time: "0930".to_string(),
                price: 0.0,
                volume: 100.0,
                amount: 0.0,
            },
            MinutePoint {
                time: "0931".to_string(),
                price: 10.1,
                volume: -12.0,
                amount: 1212.0,
            },
        ];
        let volume_points = intraday_volume_points(&points, 10.0);

        assert_eq!(volume_points.len(), 1);
        assert_eq!(volume_points[0].volume, 0.0);
    }

    #[test]
    fn maps_braille_volume_pixels_like_longbridge_line_chart() {
        assert_eq!(braille_dot_bit(0, 0), 0x01);
        assert_eq!(braille_dot_bit(1, 3), 0x80);
        assert_eq!(braille_char(0x81), '⢁');
    }

    #[test]
    fn maps_intraday_volume_to_price_chart_x_axis() {
        assert_eq!(intraday_x_to_volume_pixel(0.0, 482), 0);
        assert_eq!(intraday_x_to_volume_pixel(INTRADAY_SESSION_X_MAX, 482), 481);
        assert_eq!(intraday_x_to_volume_pixel(120.0, 482), 240);
        assert_eq!(intraday_x_to_volume_pixel(121.0, 482), 241);
    }

    #[test]
    fn builds_opening_auction_candles_from_minute_points() {
        let points = vec![
            MinutePoint {
                time: "0914".to_string(),
                price: 10.0,
                volume: 100.0,
                amount: 1000.0,
            },
            MinutePoint {
                time: "0915".to_string(),
                price: 10.1,
                volume: 120.0,
                amount: 1212.0,
            },
            MinutePoint {
                time: "0930".to_string(),
                price: 10.3,
                volume: 180.0,
                amount: 1854.0,
            },
            MinutePoint {
                time: "0931".to_string(),
                price: 10.2,
                volume: 160.0,
                amount: 1632.0,
            },
        ];

        let candles = opening_auction_klines(&points, 10.0);

        assert_eq!(candles.len(), 2);
        assert_eq!(candles[0].date, "0915");
        assert_eq!(candles[0].open, 10.0);
        assert_eq!(candles[0].close, 10.1);
        assert_eq!(candles[1].date, "0930");
        assert_eq!(candles[1].open, 10.1);
        assert_eq!(candles[1].close, 10.3);
    }

    #[test]
    fn computes_intraday_average_line_and_stats() {
        let points = vec![
            MinutePoint {
                time: "0930".to_string(),
                price: 10.0,
                volume: 100.0,
                amount: 1000.0,
            },
            MinutePoint {
                time: "0931".to_string(),
                price: 12.0,
                volume: 120.0,
                amount: 1440.0,
            },
        ];
        let plotted = intraday_plot_points(&points);
        let average = intraday_average_data(&plotted);
        let stats = intraday_stats(&plotted, 10.0);

        assert_eq!(average, vec![(0.0, 10.0), (1.0, 11.0)]);
        assert_eq!(
            stats,
            IntradayStats {
                high: 12.0,
                low: 10.0,
                pct_vs_prev_close: 20.0,
            }
        );
    }

    #[test]
    fn formats_missing_order_book_levels_as_unavailable() {
        assert_eq!(format_order_book_price(0.0), "--");
        assert_eq!(format_order_book_price(10.25), "10.25");
        assert_eq!(format_order_book_volume(0), "--");
        assert_eq!(format_order_book_volume(12), "12");
    }
}
