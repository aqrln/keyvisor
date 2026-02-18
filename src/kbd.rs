use bitvec::prelude::*;
use defmt::{Format, debug, info, trace};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    pubsub::{DynSubscriber, PubSubChannel},
};
use embassy_time::{Duration, Ticker, Timer};
use esp_hal::gpio::{AnyPin, DriveMode, Input, InputConfig, Level, Output, OutputConfig, Pull};

use crate::error::AppError;

pub const N_COLS: usize = 3;
pub const N_ROWS: usize = 4;

const SCAN_SPEED_HZ: u64 = 400;
const SCAN_READ_DELAY_MICROS: u64 = 2;
const DEBOUNCE_TICKS: TickCount = 10;

type ColumnState = BitArr!(for N_ROWS, in u8);
type TickCount = u8;

#[derive(Copy, Clone, Debug, Format, PartialEq, Eq)]
pub struct Key {
    pub col: u8,
    pub row: u8,
}

impl Key {
    pub fn char(self) -> char {
        match (self.col, self.row) {
            (0, 0) => '1',
            (1, 0) => '2',
            (2, 0) => '3',
            (0, 1) => '4',
            (1, 1) => '5',
            (2, 1) => '6',
            (0, 2) => '7',
            (1, 2) => '8',
            (2, 2) => '9',
            (0, 3) => '*',
            (1, 3) => '0',
            (2, 3) => '#',
            _ => '?',
        }
    }
}

#[derive(Clone, Copy, Debug, Format)]
pub enum KeyEvent {
    KeyDown(Key),
    KeyUp(Key),
}

static CHANNEL: PubSubChannel<CriticalSectionRawMutex, KeyEvent, 32, 1, 1> = PubSubChannel::new();

pub fn subscriber() -> Result<DynSubscriber<'static, KeyEvent>, AppError> {
    CHANNEL.dyn_subscriber().map_err(<_>::into)
}

pub struct KeyboardInterface<'p> {
    columns: [Output<'p>; N_COLS],
    rows: [Input<'p>; N_ROWS],
}

impl<'p> KeyboardInterface<'p> {
    pub fn new(columns: [AnyPin<'p>; N_COLS], rows: [AnyPin<'p>; N_ROWS]) -> Self {
        Self {
            columns: columns.map(|pin| {
                Output::new(
                    pin,
                    Level::High,
                    OutputConfig::default().with_drive_mode(DriveMode::OpenDrain),
                )
            }),
            rows: rows.map(|pin| Input::new(pin, InputConfig::default().with_pull(Pull::Up))),
        }
    }

    fn clear(&mut self) {
        for column in &mut self.columns {
            column.set_high();
        }
    }

    fn scan(&mut self, col: usize) {
        self.clear();
        self.columns[col].set_low();
    }

    fn read(&mut self) -> ColumnState {
        let mut col = ColumnState::ZERO;
        for (i, row) in self.rows.iter().enumerate() {
            col.set(i, row.is_low());
        }
        col
    }
}

struct ColumnUpdate<'a> {
    stable_state: &'a mut ColumnState,
    staging_state: &'a mut ColumnState,
    tick_counts: &'a mut [TickCount; N_ROWS],
}

#[derive(Default, Format)]
struct ColumnUpdateResult {
    #[defmt(Debug2Format)]
    pressed_keys: ColumnState,
    #[defmt(Debug2Format)]
    released_keys: ColumnState,
}

impl<'a> ColumnUpdate<'a> {
    fn new(
        stable_state: &'a mut ColumnState,
        staging_state: &'a mut ColumnState,
        tick_counts: &'a mut [TickCount; N_ROWS],
    ) -> Self {
        Self {
            stable_state,
            staging_state,
            tick_counts,
        }
    }

    fn apply(&mut self, new_state: ColumnState) -> ColumnUpdateResult {
        let mut result = ColumnUpdateResult::default();

        for r in 0..N_ROWS {
            if new_state[r] != self.staging_state[r] {
                self.tick_counts[r] = 0;
                self.staging_state.set(r, new_state[r]);
                continue;
            }

            if self.tick_counts[r] < DEBOUNCE_TICKS {
                self.tick_counts[r] += 1;
                continue;
            }

            if new_state[r] == self.stable_state[r] {
                continue;
            }

            self.stable_state.set(r, new_state[r]);

            if new_state[r] {
                result.pressed_keys.set(r, true);
            } else {
                result.released_keys.set(r, true);
            }
        }

        result
    }
}

impl ColumnUpdateResult {
    fn any(&self) -> bool {
        (self.pressed_keys | self.released_keys).any()
    }
}

#[embassy_executor::task]
pub async fn task(mut kbd: KeyboardInterface<'static>) {
    info!("starting kbd task");

    let mut ticker = Ticker::every(Duration::from_hz(SCAN_SPEED_HZ));

    let mut stable_states = [ColumnState::ZERO; N_COLS];
    let mut staging_states = [ColumnState::ZERO; N_COLS];
    let mut tick_counts = [[0u8; N_ROWS]; N_COLS];

    loop {
        for col in 0..N_COLS {
            kbd.scan(col);
            Timer::after_micros(SCAN_READ_DELAY_MICROS).await;
            let mask = kbd.read();

            trace!("col {} mask: {}", col, mask.into_inner()[0]);

            let updates = ColumnUpdate::new(
                &mut stable_states[col],
                &mut staging_states[col],
                &mut tick_counts[col],
            )
            .apply(mask);

            if updates.any() {
                debug!("col {} updates: {:?}", col, updates);

                let publisher = CHANNEL.immediate_publisher();

                for row in updates.released_keys.iter_ones() {
                    publisher.publish_immediate(KeyEvent::KeyUp(Key {
                        col: col as u8,
                        row: row as u8,
                    }));
                }

                for row in updates.pressed_keys.iter_ones() {
                    publisher.publish_immediate(KeyEvent::KeyDown(Key {
                        col: col as u8,
                        row: row as u8,
                    }));
                }
            }
        }

        ticker.next().await;
    }
}
