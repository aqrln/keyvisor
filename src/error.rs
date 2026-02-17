use core::convert::Infallible;

use embassy_embedded_hal::shared_bus::SpiDeviceError;
use esp_hal::dma::DmaBufError;

#[derive(Debug, derive_more::From)]
pub enum AppError {
    LcdAsyncInitError(LcdAsyncInitError),
    DmaBufError(DmaBufError),
    SpiConfigError(esp_hal::spi::master::ConfigError),
    LedcTimerError(esp_hal::ledc::timer::Error),
    LedcChannelError(esp_hal::ledc::channel::Error),
}

type LcdAsyncInitError = lcd_async::InitError<
    lcd_async::interface::SpiError<SpiDeviceError<esp_hal::spi::Error, Infallible>, Infallible>,
    Infallible,
>;
