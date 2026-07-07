use crate::app::{
    ChartMode, KLine, MarketIndex, MinutePoint, QuoteSessionState, Stock, StockSearchResult,
};
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum Event {
    Input(KeyEvent),
    Tick,
    StockUpdate(String, Stock, QuoteSessionState),
    StockError(String, String),
    KLineUpdate(String, ChartMode, Vec<KLine>),
    MinuteUpdate(String, Vec<MinutePoint>),
    SearchResultsUpdate(String, Vec<StockSearchResult>),
    IndicesUpdate(Vec<MarketIndex>),
    MarketStatus(String),
    ApiError(String),
}

pub struct EventHandler {
    rx: mpsc::Receiver<Event>,
}

impl EventHandler {
    /// Creates keyboard and redraw event producers that feed the main app loop.
    pub fn new(tick_rate: Duration) -> (Self, mpsc::Sender<Event>) {
        let (tx, rx) = mpsc::channel(100);
        let tx_input = tx.clone();

        // Background thread to poll Crossterm keyboard events
        tokio::spawn(async move {
            loop {
                // event::poll blocks, so we run it in a blocking thread pool
                let has_event = tokio::task::spawn_blocking(|| {
                    event::poll(Duration::from_millis(50)).unwrap_or(false)
                })
                .await
                .unwrap_or(false);

                if has_event {
                    if let Ok(CrosstermEvent::Key(key)) = event::read() {
                        // Avoid duplicates from key release events (mainly on Windows/Linux, but good practice)
                        if key.kind == KeyEventKind::Press {
                            let _ = tx_input.send(Event::Input(key)).await;
                        }
                    }
                }
            }
        });

        // Background task to send clock Ticks for redrawing
        let tx_tick = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                interval.tick().await;
                let _ = tx_tick.send(Event::Tick).await;
            }
        });

        (Self { rx }, tx)
    }

    /// Waits for the next input, timer, or data event from background tasks.
    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}
