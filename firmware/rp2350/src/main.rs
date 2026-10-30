#![no_std]
#![no_main]

use core::pin::pin;

use axis_control::{
    encoder::icmd,
    motor::{
        Direction, MotorDriver,
        tmc2209::{self, PioConfig},
    },
};
use defmt::info;
use embassy_embedded_hal::shared_bus::asynch::spi::SpiDeviceWithConfig;
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts, gpio,
    peripherals::{DMA_CH0, DMA_CH1, PIO0, PIO1, UART0, USB},
    spi, uart, usb,
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::{Duration, Ticker};
use embassy_usb::{Config, UsbDevice, driver::Driver};
use ergot::{
    endpoint,
    exports::bbqueue::{prod_cons::framed::FramedConsumer, traits::coordination::cs::CsCoord},
    toolkits::embassy_usb_v0_6 as kit,
};
use icd::commands2d::{HomeEndpoint, MoveAbsEndpoint, MoveRelEndpoint, Position};
use mutex::raw_impls::cs::CriticalSectionRawMutex;
use static_cell::{ConstStaticCell, StaticCell};

use {defmt_rtt as _, panic_probe as _};

mod comms;

const OUT_QUEUE_SIZE: usize = 4096;
const MAX_PACKET_SIZE: usize = 1024;

// Our rp2040-specific USB driver
pub type AppDriver = usb::Driver<'static, USB>;
// The type of our RX Worker
type RxWorker = ergot::interface_manager::transports::eusb_0_6::RxWorker<
    &'static Stack,
    AppDriver,
    ergot::interface_manager::profiles::direct_edge::EdgeFrameProcessor,
>;
// The type of our netstack
type Stack = kit::Stack<&'static Queue, CriticalSectionRawMutex>;
// The type of our outgoing queue
type Queue = kit::Queue<OUT_QUEUE_SIZE, CsCoord>;

/// Statically store our netstack
static STACK: Stack = kit::new_target_stack(OUTQ.framed_producer(), MAX_PACKET_SIZE as u16);
/// Statically store our USB app buffers
static STORAGE: kit::WireStorage<256, 256, 64, 256> = kit::WireStorage::new();
/// Statically store our outgoing packet buffer
static OUTQ: Queue = kit::Queue::new();

// Define some endpoints
endpoint!(LedEndpoint, bool, (), "led/set");

bind_interrupts!(pub struct Irqs {
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
    UART0_IRQ => uart::BufferedInterruptHandler<UART0>;
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<DMA_CH0>, embassy_rp::dma::InterruptHandler<DMA_CH1>;
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO0>;
    PIO1_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO1>;
});

fn usb_config(serial: &'static str) -> Config<'static> {
    let mut config = Config::new(0x16c0, 0x27DD);
    config.manufacturer = Some("Reto Trappitsch, EPFL");
    config.product = Some("Stage controller v0.1.0");
    config.serial_number = Some(serial);

    // Required for windows compatibility.
    // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config.composite_with_iads = true;

    config
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // SYSTEM INIT
    info!("Start");
    let p = embassy_rp::init(Default::default());
    // Obtain the flash ID
    let unique_id: u64 = embassy_rp::otp::get_chipid().unwrap();
    static SERIAL_STRING: StaticCell<[u8; 16]> = StaticCell::new();
    let mut ser_buf = [b' '; 16];
    // This is a simple number-to-hex formatting
    unique_id
        .to_be_bytes()
        .iter()
        .zip(ser_buf.as_chunks_mut::<2>().0)
        .for_each(|(b, chs)| {
            let mut b = *b;
            for c in chs {
                *c = match b >> 4 {
                    v @ 0..10 => b'0' + v,
                    v @ 10..16 => b'A' + (v - 10),
                    _ => b'X',
                };
                b <<= 4;
            }
        });
    let ser_buf = SERIAL_STRING.init(ser_buf);
    let ser_buf = core::str::from_utf8(ser_buf.as_slice()).unwrap();

    // SET UP THE MOTORS
    // Set up the buffered UART interface for async comms.
    let (tx_pin, rx_pin, uart) = (p.PIN_16, p.PIN_17, p.UART0);
    // set up 8 byte buffers for TX and RX (equal to command length).
    static TX_BUF_UART: StaticCell<[u8; 64]> = StaticCell::new();
    let tx_buf = &mut TX_BUF_UART.init([0; 64])[..];
    static RX_BUF_UART: StaticCell<[u8; 64]> = StaticCell::new();
    let rx_buf = &mut RX_BUF_UART.init([0; 64])[..];
    // set up the UART
    let uart = uart::BufferedUart::new(
        uart,
        tx_pin,
        rx_pin,
        Irqs,
        tx_buf,
        rx_buf,
        uart::Config::default(),
    );

    // Load a motor driver that will be shared. Address is set in the `Tmc2209Driver` (does not
    // matter here!).
    static MOTOR_DRIVER: StaticCell<tmc2209::Tmc2209Mutex<uart::BufferedUart>> = StaticCell::new();
    let shared_motor = MOTOR_DRIVER.init(Mutex::new(tmc2209::Tmc2209::new(uart, 0x00)));

    // Motor configs
    let mot_current_config = tmc2209::CurrentConfig {
        irun: 11,
        ihold: 0,
        hold_delay: 0,
    };
    let motor_config = tmc2209::Tmc2209Config {
        invert: false,
        current: mot_current_config,
        move_config: tmc2209::MoveConfig {
            movement_mode: tmc2209::MoveMode::StealthChop,
            microsteps: tmc2209::MicrostepResolution::M16,
            rough_velocity: 1000,
            fine_velocity: 200,
        },
    };

    // x motor setup and PIO init
    let xmot_address = 0x00;
    let xmot_pout_en = gpio::Output::new(p.PIN_20, gpio::Level::High);
    let embassy_rp::pio::Pio {
        common: xmot_pio_common,
        irq0: xmot_pio_irq,
        sm0: xmot_pio_sm,
        ..
    } = embassy_rp::pio::Pio::new(p.PIO0, Irqs);
    let xmot_pio_config = PioConfig {
        p_step: p.PIN_19,
        p_dir: p.PIN_18,
        common: xmot_pio_common,
        irq: xmot_pio_irq,
        sm: xmot_pio_sm,
    };

    // Create the x motor. this must for some reason be done before we init the PIO for y motor.
    let mut x_motor = tmc2209::Tmc2209Driver::try_new(
        xmot_pout_en,
        xmot_pio_config,
        shared_motor,
        xmot_address,
        motor_config,
    )
    .await
    .unwrap();

    // y motor setup and PIO init
    let ymot_address = 0x01;
    let ymot_pout_en = gpio::Output::new(p.PIN_21, gpio::Level::High);
    let embassy_rp::pio::Pio {
        common: ymot_pio_common,
        irq0: ymot_pio_irq,
        sm0: ymot_pio_sm,
        ..
    } = embassy_rp::pio::Pio::new(p.PIO1, Irqs);
    let ymot_pio_config = PioConfig {
        p_step: p.PIN_26,
        p_dir: p.PIN_22,
        common: ymot_pio_common,
        irq: ymot_pio_irq,
        sm: ymot_pio_sm,
    };

    let mut y_motor = tmc2209::Tmc2209Driver::try_new(
        ymot_pout_en,
        ymot_pio_config,
        shared_motor,
        ymot_address,
        motor_config,
    )
    .await
    .unwrap();

    // ENCODER/COUNTER CONFIG
    let cipo = p.PIN_0;
    let copi = p.PIN_3;
    let clk = p.PIN_2;

    let mut spi_config = spi::Config::default();
    spi_config.frequency = 10_000_000;
    spi_config.phase = spi::Phase::CaptureOnFirstTransition;
    spi_config.polarity = spi::Polarity::IdleLow;

    let spi = spi::Spi::new(
        p.SPI0,
        clk,
        copi,
        cipo,
        p.DMA_CH0,
        p.DMA_CH1,
        Irqs,
        spi_config.clone(),
    );

    let spi_bus: Mutex<NoopRawMutex, _> = Mutex::new(spi);

    let xenc_cs = gpio::Output::new(p.PIN_1, gpio::Level::High);
    let yenc_cs = gpio::Output::new(p.PIN_6, gpio::Level::High);

    let xenc_spi = SpiDeviceWithConfig::new(&spi_bus, xenc_cs, spi_config.clone());
    let yenc_spi = SpiDeviceWithConfig::new(&spi_bus, yenc_cs, spi_config);

    // now we get the iC-MD async devices.
    let xenc_icmd = icmd::IcMdAsync::new(xenc_spi);
    let yenc_icmd = icmd::IcMdAsync::new(yenc_spi);

    let xenc_cnt_setup = icmd::CntSetup::default();
    let xenc_cnt_cfg = icmd::CntCfg::Cnt1Bit48(xenc_cnt_setup);
    let yenc_cnt_setup = icmd::CntSetup::new(icmd::CntDirection::CCW, icmd::CntZSignal::Normal);
    let yenc_cnt_cfg = icmd::CntCfg::Cnt1Bit48(yenc_cnt_setup);

    // encoders
    let _x_encoder = icmd::IcMdDriver::try_new(xenc_icmd, xenc_cnt_cfg)
        .await
        .unwrap();
    let _y_encoder = icmd::IcMdDriver::try_new(yenc_icmd, yenc_cnt_cfg)
        .await
        .unwrap();

    // USB/RPC INIT
    let driver = usb::Driver::new(p.USB, Irqs);
    let config = usb_config(ser_buf);
    let (device, tx_impl, ep_out) = STORAGE.init_ergot(driver, config);

    static RX_BUF: ConstStaticCell<[u8; MAX_PACKET_SIZE]> =
        ConstStaticCell::new([0u8; MAX_PACKET_SIZE]);
    let rxvr: RxWorker = RxWorker::new(
        &STACK,
        ep_out,
        ergot::interface_manager::profiles::direct_edge::EdgeFrameProcessor::new(),
        (),
    );

    info!("Spawning");

    // Ergot required tasks.
    spawner.spawn(usb_task(device).unwrap());
    spawner.spawn(run_tx(tx_impl, OUTQ.framed_consumer()).unwrap());
    spawner.spawn(run_rx(rxvr, RX_BUF.take()).unwrap());
    spawner.spawn(pingserver().unwrap());

    // Stage comms tasks.
    // spawner.spawn(comms::current_position_broadcast().unwrap());
    spawner.spawn(stage_controller_move_abs_demo().unwrap());
    spawner.spawn(stage_controller_move_rel_demo().unwrap());
    spawner.spawn(stage_controller_home_demo().unwrap());

    let mut ticker = Ticker::every(Duration::from_secs(3));
    loop {
        ticker.next().await;
        // TODO: Feed a watchdog.

        // FIXME: TODEL
        info!("Move positive");
        x_motor
            .move_relative(10000, Direction::Positive)
            .await
            .unwrap();
        y_motor
            .move_relative(20000, Direction::Positive)
            .await
            .unwrap();

        ticker.next().await;

        info!("Move negative");
        x_motor
            .move_relative(10000, Direction::Negative)
            .await
            .unwrap();
        y_motor
            .move_relative(20000, Direction::Negative)
            .await
            .unwrap();
    }
}

#[embassy_executor::task]
async fn pingserver() {
    STACK.services().ping_handler::<4>().await;
}

/// This handles the low level USB management
#[embassy_executor::task]
pub async fn usb_task(mut usb: UsbDevice<'static, AppDriver>) {
    usb.run().await;
}

#[embassy_executor::task]
async fn run_rx(rcvr: RxWorker, recv_buf: &'static mut [u8]) {
    rcvr.run(recv_buf, kit::USB_FS_MAX_PACKET_SIZE).await;
}

#[embassy_executor::task]
async fn run_tx(
    mut ep_in: <AppDriver as Driver<'static>>::EndpointIn,
    rx: FramedConsumer<&'static Queue>,
) {
    kit::tx_worker::<AppDriver, OUT_QUEUE_SIZE, CsCoord>(
        &mut ep_in,
        rx,
        kit::DEFAULT_TIMEOUT_MS_PER_FRAME,
        kit::USB_FS_MAX_PACKET_SIZE,
    )
    .await;
}

#[embassy_executor::task]
async fn stage_controller_move_abs_demo() {
    let socket = STACK
        .endpoints()
        .bounded_server::<MoveAbsEndpoint, 2>(Some("command2d/move_absolute"));
    let socket = pin!(socket);
    let mut hdl = socket.attach();

    loop {
        let _ = hdl
            .serve(async |pos| {
                info!("Move abs pkg: {:?}", pos);
                pos.clone()
            })
            .await;
    }
}

#[embassy_executor::task]
async fn stage_controller_move_rel_demo() {
    let socket = STACK
        .endpoints()
        .bounded_server::<MoveRelEndpoint, 2>(Some("command2d/move_relative"));
    let socket = pin!(socket);
    let mut hdl = socket.attach();

    loop {
        let _ = hdl
            .serve(async |pos| {
                info!("Move relpkg: {:?}", pos);
                pos.clone()
            })
            .await;
    }
}

#[embassy_executor::task]
async fn stage_controller_home_demo() {
    let socket = STACK
        .endpoints()
        .bounded_server::<HomeEndpoint, 2>(Some("command2d/home"));
    let socket = pin!(socket);
    let mut hdl = socket.attach();

    loop {
        let _ = hdl
            .serve(async |_| {
                info!("Homing...");
                Position::default()
            })
            .await;
    }
}
