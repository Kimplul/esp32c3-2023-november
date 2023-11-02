#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use panic_rtt_target as _;

#[rtic::app(device = esp32c3, dispatchers = [FROM_CPU_INTR0, FROM_CPU_INTR1])]
mod app {
    use rtt_target::{rprintln, rtt_init_print};

    use esp32c3_hal::{
        self as _,
        clock::ClockControl,
        peripherals::{Peripherals, UART0},
        prelude::*,
        uart::{
            config::{Config, DataBits, Parity, StopBits},
            TxRxPins, UartRx, UartTx
        },
        Uart, IO,
    };

    use shared::Command;

    #[shared]
    struct Shared {
        cmd: Option<Command>,
    }

    #[local]
    struct Local {
        uart_rx: UartRx<'static, UART0>,
        uart_tx: UartTx<'static, UART0>,
    }

    #[init]
    fn init(_: init::Context) -> (Shared, Local) {
        rtt_init_print!();
        rprintln!(env!("CARGO_CRATE_NAME"));

        let peripherals = Peripherals::take();
        let mut system = peripherals.SYSTEM.split();
        let clocks = ClockControl::max(system.clock_control).freeze();

        let uart_config = Config {
            baudrate: 115200,
            data_bits: DataBits::DataBits8,
            parity: Parity::ParityNone,
            stop_bits: StopBits::STOP1,
        };

        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
        let uart0_pins = TxRxPins::new_tx_rx(
            io.pins.gpio0.into_push_pull_output(),
            io.pins.gpio1.into_floating_input(),
        );

        let uart0 = Uart::new_with_config(
            peripherals.UART0,
            uart_config,
            Some(uart0_pins),
            &clocks,
            &mut system.peripheral_clock_control,
        );

        let (mut uart_tx, uart_rx) = uart0.split();

        rprintln!("init works");
        let cmd: Option<Command> = None;

        (Shared{cmd}, Local{uart_rx, uart_tx})
    }

    #[idle(local = [uart_tx])]
    fn idle(cx: idle::Context) -> !{
        loop {
            let r = cx.local.uart_tx.write(b'c');
            match r {
                Err(e) => {rprintln!("transmission error: {:?}", e)}
                Ok(_) => {}
            }
        }
    }

    /*
    #[task(binds = UART0, local = [uart_rx], shared = [cmd])]
    fn aggregate(cx: aggregate::Context) {
    }

    #[task(shared = [cmd], priority = 1)]
    async fn broker(cx: broker::Context) {
    }

    #[task(priority = 2)]
    async fn set_blink_data(cx: set_blink_data::Context) {
    }

    #[task(priority = 3)]
    async fn set_rgb_data(cx: set_rgb_data::Context) {
    }

    #[task(priority = 4)]
    async fn set_date_time(cx: set_date_time::Context) {
    }

    #[task(priority = 5)]
    async fn blink(cx: blink::Context) {
    }

    #[task(priority = 6)]
    async fn update_rgb(cx: update_rgb::Context) {
    }
    */
}
