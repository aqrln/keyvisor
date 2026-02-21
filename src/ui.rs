use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle, RoundedRectangle},
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};
use u8g2_fonts::{U8g2TextStyle, fonts::u8g2_font_helvB18_te};

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

    for row in 0..N_ROWS {
        for col in 0..N_COLS {
            let key = Key {
                row: row as u8,
                col: col as u8,
            };
            Button::new(key, button_pos(row, col), ButtonStyle::unpressed())
                .draw(&mut display_state.fb);
        }
    }

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

    loop {
        let bounds = match kbd_events.next_message_pure().await {
            KeyEvent::KeyDown(key) => update(key, Direction::Down, &mut display_state.fb),
            KeyEvent::KeyUp(key) => update(key, Direction::Up, &mut display_state.fb),
        }?;

        let y = bounds.top_left.y as usize;
        let height = bounds.size.height as usize;

        let stripe_start = y * display_state.fb.width() * display::PIXEL_SIZE;
        let stripe_end = (y + height) * display_state.fb.width() * display::PIXEL_SIZE;

        let pixel_data = &display_state.fb.as_bytes()[stripe_start..stripe_end];

        display_state
            .display
            .show_raw_data(0, y as u16, display::WIDTH, height as u16, pixel_data)
            .await?;
    }
}

fn button_pos(row: usize, col: usize) -> Point {
    Point::new(
        col as i32 * Button::WIDTH as i32,
        row as i32 * Button::HEIGHT as i32,
    )
}

enum Direction {
    Up,
    Down,
}

fn update<D: DrawTarget<Color = Rgb565>>(
    key: Key,
    direction: Direction,
    target: &mut D,
) -> Result<Rectangle, D::Error> {
    let point = button_pos(key.row as usize, key.col as usize);
    let style = match direction {
        Direction::Down => ButtonStyle::pressed(),
        Direction::Up => ButtonStyle::unpressed(),
    };

    let btn = Button::new(key, point, style);
    btn.draw(target)?;

    Ok(btn.bounds())
}

struct ButtonStyle {
    bg_color: Rgb565,
    border_color: Rgb565,
    text_color: Rgb565,
}

impl ButtonStyle {
    fn pressed() -> Self {
        Self {
            bg_color: Rgb565::CSS_SALMON,
            border_color: Rgb565::CSS_DIM_GRAY,
            text_color: Rgb565::CSS_BLACK,
        }
    }

    fn unpressed() -> Self {
        Self {
            bg_color: Rgb565::CSS_BLACK,
            border_color: Rgb565::CSS_WHITE,
            text_color: Rgb565::CSS_WHITE,
        }
    }
}

struct Button {
    top_left: Point,
    style: ButtonStyle,
    key: Key,
}

impl Button {
    const WIDTH: u32 = display::WIDTH as u32 / N_COLS as u32;
    const HEIGHT: u32 = display::HEIGHT as u32 / N_ROWS as u32;
    const SIZE: Size = Size::new(Self::WIDTH, Self::HEIGHT);

    fn new(key: Key, top_left: Point, style: ButtonStyle) -> Self {
        Self {
            key,
            top_left,
            style,
        }
    }

    fn bounds(&self) -> Rectangle {
        Rectangle::new(self.top_left, Self::SIZE)
    }
}

impl Drawable for Button {
    type Color = Rgb565;

    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        let style = PrimitiveStyleBuilder::new()
            .stroke_color(self.style.border_color)
            .fill_color(self.style.bg_color)
            .stroke_width(1)
            .build();

        let rect = RoundedRectangle::with_equal_corners(self.bounds(), Size::new(10, 10))
            .offset(-3)
            .into_styled(style);

        rect.draw(target)?;

        let mut buf = [0u8; 4];
        let label = self.key.char().encode_utf8(&mut buf);

        Text::with_text_style(
            label,
            self.bounds().center(),
            U8g2TextStyle::new(u8g2_font_helvB18_te, self.style.text_color),
            TextStyleBuilder::new()
                .alignment(Alignment::Center)
                .baseline(Baseline::Middle)
                .build(),
        )
        .draw(target)?;

        Ok(())
    }
}
