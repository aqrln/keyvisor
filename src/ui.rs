use embedded_graphics::{pixelcolor::Rgb565, prelude::*};

use crate::display::{DisplayState, HEIGHT, WIDTH};

#[embassy_executor::task]
pub async fn task(mut display_state: DisplayState) {
    defmt::info!("starting display task");

    display_state
        .fb
        .clear(Rgb565::BLACK)
        .expect("couldn't clear framebuffer");

    display_state
        .display
        .show_raw_data(0, 0, WIDTH, HEIGHT, display_state.fb.as_bytes())
        .await
        .expect("couldn't update display");
}
