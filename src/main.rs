mod api;
mod app;
mod config;
mod event;
mod ui;

use app::{AddStockStatus, App, InputMode};
use crossterm::{
    cursor,
    event::KeyCode,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use event::{Event, EventHandler};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Installs a panic hook that restores terminal state before printing failures.
fn setup_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, cursor::Show);
        original_hook(panic_info);
    }));
}

/// Collects unique stock codes from every configured watchlist group.
fn get_all_stock_codes(app: &App) -> Vec<String> {
    let mut codes = Vec::new();
    for group in &app.config.groups {
        for stock in &group.stocks {
            if !codes.contains(stock) {
                codes.push(stock.clone());
            }
        }
    }
    codes
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_panic_hook();

    // 1. Setup Terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 2. Initialize App and Shared State
    let mut app = App::new();
    let initial_codes = get_all_stock_codes(&app);
    let codes_to_poll = Arc::new(Mutex::new(initial_codes));

    // 3. Start Event & API Loops
    let tick_rate = Duration::from_millis(250);
    let (mut event_handler, event_tx) = EventHandler::new(tick_rate);
    let search_tx = event_tx.clone();
    let refresh_tx = event_tx.clone();

    // Poll API every 3 seconds
    api::start_api_loop(event_tx, Arc::clone(&codes_to_poll), Duration::from_secs(3));
    if let Some(code) = app.selected_stock_code() {
        api::start_stock_snapshot_refresh(refresh_tx.clone(), code, true);
    }

    // 4. Main Event Loop
    while !app.should_quit {
        terminal.draw(|f| ui::render(f, &mut app))?;

        if let Some(event) = event_handler.next().await {
            match event {
                Event::Input(key) => {
                    match app.input_mode {
                        InputMode::Normal => match key.code {
                            KeyCode::Esc if app.show_help_popup => {
                                app.close_help_popup();
                            }
                            KeyCode::Esc if app.show_opening_auction_popup => {
                                app.close_opening_auction_popup();
                            }
                            KeyCode::Char('?') => {
                                app.toggle_help_popup();
                            }
                            _ if app.show_help_popup => {}
                            KeyCode::Char('q') => {
                                app.should_quit = true;
                            }
                            KeyCode::Char('o') => {
                                app.toggle_opening_auction_popup();
                                if app.show_opening_auction_popup {
                                    if let Some(code) = app.selected_stock_code() {
                                        app.status_message =
                                            Some(format!("查看早盘竞价: {}", code));
                                        api::start_stock_snapshot_refresh(
                                            refresh_tx.clone(),
                                            code,
                                            false,
                                        );
                                    }
                                }
                            }
                            _ if app.show_opening_auction_popup => {}
                            KeyCode::Char('j') | KeyCode::Down => {
                                app.next_stock();
                                if let Some(code) = app.selected_stock_code() {
                                    api::start_stock_snapshot_refresh(
                                        refresh_tx.clone(),
                                        code,
                                        true,
                                    );
                                }
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                app.prev_stock();
                                if let Some(code) = app.selected_stock_code() {
                                    api::start_stock_snapshot_refresh(
                                        refresh_tx.clone(),
                                        code,
                                        true,
                                    );
                                }
                            }
                            KeyCode::Char('[') => {
                                app.prev_group();
                                if let Some(code) = app.selected_stock_code() {
                                    api::start_stock_snapshot_refresh(
                                        refresh_tx.clone(),
                                        code,
                                        true,
                                    );
                                }
                            }
                            KeyCode::Char(']') => {
                                app.next_group();
                                if let Some(code) = app.selected_stock_code() {
                                    api::start_stock_snapshot_refresh(
                                        refresh_tx.clone(),
                                        code,
                                        true,
                                    );
                                }
                            }
                            KeyCode::Char('t') => {
                                app.chart_mode = match app.chart_mode {
                                    app::ChartMode::Intraday => app::ChartMode::DailyK,
                                    app::ChartMode::DailyK => app::ChartMode::Intraday,
                                };
                                if let Some(code) = app.selected_stock_code() {
                                    match app.chart_mode {
                                        app::ChartMode::Intraday => {
                                            app.status_message =
                                                Some(format!("正在刷新分时数据: {}", code));
                                        }
                                        app::ChartMode::DailyK => {
                                            app.status_message =
                                                Some(format!("正在刷新日K数据: {}", code));
                                        }
                                    }
                                    api::start_stock_snapshot_refresh(
                                        refresh_tx.clone(),
                                        code,
                                        true,
                                    );
                                }
                            }
                            KeyCode::Char('a') => {
                                app.input_mode = InputMode::Search;
                                app.clear_search_state();
                            }
                            KeyCode::Char('d') => {
                                app.delete_selected_stock();
                                // Update shared codes for background polling
                                let updated_codes = get_all_stock_codes(&app);
                                if let Ok(mut lock) = codes_to_poll.lock() {
                                    *lock = updated_codes;
                                }
                                api::start_snapshot_refresh(
                                    refresh_tx.clone(),
                                    get_all_stock_codes(&app),
                                    true,
                                );
                            }
                            _ => {}
                        },
                        InputMode::Search => match key.code {
                            KeyCode::Enter => {
                                if !app.search_input.is_empty() {
                                    let code_to_add = app
                                        .selected_search_result()
                                        .map(|result| result.code.clone())
                                        .unwrap_or_else(|| app.search_input.clone());
                                    let add_result = app.add_stock(code_to_add);

                                    if matches!(
                                        add_result.status,
                                        AddStockStatus::Added | AddStockStatus::AlreadyExists
                                    ) {
                                        // Update shared codes so the background loop observes add changes.
                                        let updated_codes = get_all_stock_codes(&app);
                                        if let Ok(mut lock) = codes_to_poll.lock() {
                                            *lock = updated_codes;
                                        }
                                    }

                                    if let Some(code) = add_result.code {
                                        api::start_stock_snapshot_refresh(
                                            refresh_tx.clone(),
                                            code,
                                            true,
                                        );
                                    }
                                }
                                app.input_mode = InputMode::Normal;
                                app.clear_search_state();
                            }
                            KeyCode::Esc => {
                                app.input_mode = InputMode::Normal;
                                app.clear_search_state();
                            }
                            KeyCode::Down => {
                                app.next_search_result();
                            }
                            KeyCode::Up => {
                                app.prev_search_result();
                            }
                            KeyCode::Backspace => {
                                app.search_input.pop();
                                if app.search_input.trim().is_empty() {
                                    app.update_search_results(Vec::new());
                                } else {
                                    api::start_stock_search(
                                        search_tx.clone(),
                                        app.search_input.clone(),
                                    );
                                }
                            }
                            KeyCode::Char(c) => {
                                // Accept Chinese, pinyin, exchange-prefixed codes, and plain 6-digit codes.
                                if !c.is_control() && app.search_input.chars().count() < 24 {
                                    app.search_input.push(c);
                                    api::start_stock_search(
                                        search_tx.clone(),
                                        app.search_input.clone(),
                                    );
                                }
                            }
                            _ => {}
                        },
                    }
                }
                Event::Tick => {
                    // Tick event triggers redrawing (loop cycles)
                }
                Event::StockUpdate(code, stock) => {
                    app.stock_data.insert(code, stock);
                }
                Event::KLineUpdate(code, kline) => {
                    app.update_kline_data(code, kline);
                }
                Event::MinuteUpdate(code, points) => {
                    app.intraday_data.insert(code, points);
                }
                Event::SearchResultsUpdate(query, results) => {
                    if app.input_mode == InputMode::Search && query == app.search_input {
                        app.update_search_results(results);
                    }
                }
                Event::IndicesUpdate(indices) => {
                    app.indices = indices;
                }
                Event::MarketStatus(status) => {
                    app.status_message = Some(status);
                }
                Event::ApiError(err) => {
                    app.status_message = Some(err);
                }
            }
        }
    }

    // 5. Restore Terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show)?;
    Ok(())
}
