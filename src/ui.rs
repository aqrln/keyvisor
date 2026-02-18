use embedded_graphics::{
    mono_font::{MonoTextStyleBuilder, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle, RoundedRectangle},
    text::{Alignment, Text},
};

use crate::{
    display::{self, DisplayState},
    error::AppError,
    kbd::{Key, KeyEvent, N_COLS, N_ROWS},
};

#[embassy_executor::task]
pub async fn task(display_state: DisplayState) {
    defmt::info!("starting display task");
    ui_main(display_state).await.expect("ui task error");
}

async fn ui_main(mut display_state: DisplayState) -> Result<(), AppError> {
    display_state.fb.clear(Rgb565::BLACK);

    display_state
        .display
        .show_raw_data(
            0,
            0,
            display::WIDTH,
            display::HEIGHT,
            display_state.fb.as_bytes(),
        )
        .await?;

    let mut kbd_events = crate::kbd::subscriber()?;
    let mut kbd_renderer = KbdRenderer::new(display_state);

    for row in 0..N_ROWS {
        for col in 0..N_COLS {
            kbd_renderer
                .draw_unpressed(Key {
                    row: row as u8,
                    col: col as u8,
                })
                .await?;
        }
    }

    loop {
        match kbd_events.next_message_pure().await {
            KeyEvent::KeyDown(key) => kbd_renderer.draw_pressed(key).await?,
            KeyEvent::KeyUp(key) => kbd_renderer.draw_unpressed(key).await?,
        }
    }
}

struct KbdRenderer {
    display_state: DisplayState,
}

impl KbdRenderer {
    fn new(display_state: DisplayState) -> Self {
        Self { display_state }
    }

    async fn draw_pressed(&mut self, key: Key) -> Result<(), AppError> {
        self.draw_button(
            key,
            ButtonStyle {
                bg_color: Rgb565::CSS_SALMON,
                border_color: Rgb565::CSS_DIM_GRAY,
                text_color: Rgb565::CSS_BLACK,
            },
        )
        .await
    }

    async fn draw_unpressed(&mut self, key: Key) -> Result<(), AppError> {
        self.draw_button(
            key,
            ButtonStyle {
                bg_color: Rgb565::CSS_BLACK,
                border_color: Rgb565::CSS_WHITE,
                text_color: Rgb565::CSS_WHITE,
            },
        )
        .await
    }

    async fn draw_button(&mut self, key: Key, btn: ButtonStyle) -> Result<(), AppError> {
        const WIDTH: u32 = display::WIDTH as u32 / N_COLS as u32;
        const HEIGHT: u32 = display::HEIGHT as u32 / N_ROWS as u32;
        const SIZE: Size = Size::new(WIDTH, HEIGHT);

        let style = PrimitiveStyleBuilder::new()
            .stroke_color(btn.border_color)
            .fill_color(btn.bg_color)
            .stroke_width(1)
            .build();

        let top_left = Point::new(
            key.col as i32 * WIDTH as i32,
            key.row as i32 * HEIGHT as i32,
        );
        let rect = Rectangle::new(top_left, SIZE);

        let rect = RoundedRectangle::with_equal_corners(rect, Size::new(10, 10))
            .offset(-3)
            .into_styled(style);

        rect.draw(&mut self.display_state.fb);

        let mut buf = [0u8; 4];
        let label = key.char().encode_utf8(&mut buf);

        _ = Text::with_alignment(
            label,
            top_left + Point::new(WIDTH as i32 / 2, HEIGHT as i32 / 2 + 5),
            MonoTextStyleBuilder::new()
                .text_color(btn.text_color)
                .font(&FONT_10X20)
                .build(),
            Alignment::Center,
        )
        .draw(&mut self.display_state.fb);

        self.display_state
            .display
            .show_raw_data(
                0,
                0,
                display::WIDTH,
                display::HEIGHT,
                self.display_state.fb.as_bytes(),
            )
            .await?;

        Ok(())
    }
}

struct ButtonStyle {
    bg_color: Rgb565,
    border_color: Rgb565,
    text_color: Rgb565,
}
