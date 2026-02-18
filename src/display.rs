use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embedded_graphics::pixelcolor::Rgb565;
use esp_hal::{
    dma::{DmaRxBuf, DmaTxBuf},
    gpio::{AnyPin, Level, Output},
    ledc::{
        self, LSGlobalClkSource, Ledc,
        channel::ChannelIFace as _,
        timer::{LSClockSource, TimerIFace as _, config::Duty},
    },
    peripherals::{DMA_CH0, LEDC, SPI2},
    spi::{self, master::Spi},
    time::Rate,
};
use lcd_async::{
    Builder, Display,
    interface::SpiInterface,
    models::ST7789,
    options::{ColorInversion, Orientation, Rotation},
    raw_framebuf::RawFrameBuf,
};
use static_cell::{ConstStaticCell, StaticCell};

use crate::error::AppError;

pub const WIDTH: u16 = 240;
pub const HEIGHT: u16 = 240;
pub const PIXEL_SIZE: usize = 2; // RGB565 = 2 bytes per pixel

pub struct DisplayState {
    pub display: Display<DisplayInterface, ST7789, Output<'static>>,
    pub fb: RawFrameBuf<Rgb565, &'static mut [u8]>,
    pub backlight: Backlight,
}

type DisplayInterface =
    SpiInterface<SpiDevice<'static, NoopRawMutex, SpiBus, Output<'static>>, Output<'static>>;

pub struct DisplayPeripherals {
    pub scl: AnyPin<'static>,
    pub sda: AnyPin<'static>,
    pub rst: AnyPin<'static>,
    pub dc: AnyPin<'static>,
    pub cs: AnyPin<'static>,
    pub bl: AnyPin<'static>,
    pub ledc: LEDC<'static>,
    pub spi: SPI2<'static>,
    pub dma_ch: DMA_CH0<'static>,
}

impl DisplayState {
    pub async fn init(peripherals: DisplayPeripherals) -> Result<Self, AppError> {
        let backlight = Backlight::init(peripherals.ledc, peripherals.bl)?;

        let rst = Output::new(peripherals.rst, Level::Low, Default::default());
        let dc = Output::new(peripherals.dc, Level::Low, Default::default());
        let cs = Output::new(peripherals.cs, Level::High, Default::default());

        let spi_bus = init_spi_bus(SpiBusPerhipherals {
            scl: peripherals.scl,
            sda: peripherals.sda,
            spi: peripherals.spi,
            dma_ch: peripherals.dma_ch,
        })?;
        let spi_device = SpiDevice::new(spi_bus, cs);
        let di = SpiInterface::new(spi_device, dc);

        let display = Builder::new(ST7789, di)
            .reset_pin(rst)
            .display_size(WIDTH, HEIGHT)
            .orientation(Orientation {
                rotation: Rotation::Deg0,
                mirrored: false,
            })
            .display_offset(0, 0)
            .invert_colors(ColorInversion::Inverted)
            .init(&mut embassy_time::Delay)
            .await?;

        const FRAME_SIZE: usize = (WIDTH as usize) * (HEIGHT as usize) * PIXEL_SIZE;
        static FRAME_BUFFER: ConstStaticCell<[u8; FRAME_SIZE]> =
            ConstStaticCell::new([0; FRAME_SIZE]);

        let fb_bytes = FRAME_BUFFER.take();
        let fb = RawFrameBuf::new(fb_bytes.as_mut_slice(), WIDTH.into(), HEIGHT.into());

        Ok(Self {
            display,
            fb,
            backlight,
        })
    }
}

struct SpiBusPerhipherals {
    scl: AnyPin<'static>,
    sda: AnyPin<'static>,
    spi: SPI2<'static>,
    dma_ch: DMA_CH0<'static>,
}

type SpiBus = spi::master::SpiDmaBus<'static, esp_hal::Async>;
type SpiBusMutex = Mutex<NoopRawMutex, SpiBus>;

fn init_spi_bus(peripherals: SpiBusPerhipherals) -> Result<&'static SpiBusMutex, AppError> {
    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = esp_hal::dma_buffers!(4, 32_000);
    let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer)?;
    let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer)?;

    static SPI_BUS: StaticCell<SpiBusMutex> = StaticCell::new();

    let spi_bus = Spi::new(
        peripherals.spi,
        spi::master::Config::default()
            .with_frequency(Rate::from_mhz(20))
            .with_mode(spi::Mode::_0),
    )?
    .with_sck(peripherals.scl)
    .with_mosi(peripherals.sda)
    .with_dma(peripherals.dma_ch)
    .with_buffers(dma_rx_buf, dma_tx_buf)
    .into_async();

    Ok(SPI_BUS.init(Mutex::new(spi_bus)))
}

pub struct Backlight {
    pwm_timer: &'static ledc::timer::Timer<'static, ledc::LowSpeed>,
    pwm_channel: ledc::channel::Channel<'static, ledc::LowSpeed>,
}

impl Backlight {
    pub fn init(ledc: LEDC<'static>, bl: AnyPin<'static>) -> Result<Self, AppError> {
        let mut ledc = Ledc::new(ledc);
        ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

        static PWM_TIMER: StaticCell<ledc::timer::Timer<'static, ledc::LowSpeed>> =
            StaticCell::new();

        let pwm_timer = PWM_TIMER.init(ledc.timer(ledc::timer::Number::Timer0));
        pwm_timer.configure(ledc::timer::config::Config {
            duty: Duty::Duty5Bit,
            clock_source: LSClockSource::APBClk,
            frequency: Rate::from_khz(24),
        })?;

        let pwm_channel = ledc.channel(esp_hal::ledc::channel::Number::Channel0, bl);

        let mut backlight = Self {
            pwm_timer,
            pwm_channel,
        };

        backlight.set_brightness_pct(10)?;

        Ok(backlight)
    }

    pub fn set_brightness_pct(&mut self, brightness: u8) -> Result<(), AppError> {
        self.pwm_channel
            .configure(ledc::channel::config::Config {
                timer: self.pwm_timer,
                duty_pct: brightness,
                drive_mode: esp_hal::gpio::DriveMode::PushPull,
            })
            .map_err(From::from)
    }
}
