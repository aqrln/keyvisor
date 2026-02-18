use core::convert::Infallible;

use embassy_embedded_hal::shared_bus::SpiDeviceError;
use esp_hal::dma::DmaBufError;

#[derive(Debug, defmt::Format, derive_more::From)]
pub enum AppError {
    LcdAsyncInitError(#[defmt(Debug2Format)] LcdAsyncInitError),
    SpiError(#[defmt(Debug2Format)] LcdAsyncSpiError),
    DmaBufError(DmaBufError),
    SpiConfigError(esp_hal::spi::master::ConfigError),
    LedcTimerError(esp_hal::ledc::timer::Error),
    LedcChannelError(esp_hal::ledc::channel::Error),
    PubSubError(embassy_sync::pubsub::Error),
}

type LcdAsyncSpiError =
    lcd_async::interface::SpiError<SpiDeviceError<esp_hal::spi::Error, Infallible>, Infallible>;

type LcdAsyncInitError = lcd_async::InitError<LcdAsyncSpiError, Infallible>;
