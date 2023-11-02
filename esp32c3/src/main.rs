#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use panic_rtt_target as _;

#[rtic::app(device = esp32c3, dispatchers = [FROM_CPU_INTR0, FROM_CPU_INTR1])]
mod app {
    use rtt_target::{rprint, rprintln, rtt_init_print};

    use esp32c3_hal::{
        self as _,
        clock::ClockControl,
        peripherals::{Peripherals, UART0},
        prelude::*,
        uart::{
            config::{Config, DataBits, Parity, StopBits},
            TxRxPins, UartRx, UartTx,
        },
        Uart, IO,
    };

    use shared::{
        deserialize_crc_cobs, serialize_crc_cobs, Ack, BlinkerOptions, Command, DateTime, IN_SIZE,
        OUT_SIZE,
    };

    #[derive(Debug)]
    pub enum RgbState {
        On,
        Off,
    }

    #[shared]
    struct Shared {
        cmd: [u8; OUT_SIZE],
        rgb_state: RgbState,
        blink_data: BlinkerOptions,
        reference_time: DateTime,
    }

    #[local]
    struct Local {
        uart_rx: UartRx<'static, UART0>,
        uart_tx: UartTx<'static, UART0>,
        cmd_idx: usize,
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

        let mut uart0 = Uart::new_with_config(
            peripherals.UART0,
            uart_config,
            Some(uart0_pins),
            &clocks,
            &mut system.peripheral_clock_control,
        );

        /* this is apparently dumb */
        uart0.set_rx_fifo_full_threshold(1).unwrap();
        uart0.listen_rx_fifo_full();

        let (uart_tx, uart_rx) = uart0.split();

        rprintln!("init works");

        (
            Shared {
                blink_data: BlinkerOptions::Off,
                cmd: [0; OUT_SIZE],
                rgb_state: RgbState::Off,
                reference_time: DateTime::Now,
            },
            Local {
                uart_rx,
                uart_tx,
                cmd_idx: 0,
            },
        )
    }

    #[task(binds = UART0, local = [cmd_idx, uart_rx], shared = [cmd])]
    fn aggregate(mut cx: aggregate::Context) {
        rprint!("received UART0 rx interrupt: ");

        if let nb::Result::Ok(c) = cx.local.uart_rx.read() {
            rprint!("{}", c);
            cx.shared.cmd.lock(|cmd| {
                cmd[*cx.local.cmd_idx] = c;
                *cx.local.cmd_idx += 1;
            });

            if c == 0 {
                rprint!(" full packet at {}", *cx.local.cmd_idx);
                broker::spawn().unwrap();
                *cx.local.cmd_idx = 0;
            }
        }

        rprintln!("");
        cx.local.uart_rx.reset_rx_fifo_full_interrupt();
    }

    #[task(shared = [cmd], local = [uart_tx])]
    async fn broker(mut cx: broker::Context) {
        let cmd = cx
            .shared
            .cmd
            .lock(|cmd| deserialize_crc_cobs::<Command>(cmd));

        let ack = if let Ok(cmd) = cmd {
            match cmd {
                Command::SetBlinker(options) => {
                    set_blink_data::spawn(options).unwrap();
                }
                Command::SetDateTime(t) => set_date_time::spawn(t).unwrap(),
                Command::RgbOn => update_rgb_data::spawn(RgbState::On).unwrap(),
                Command::RgbOff => update_rgb_data::spawn(RgbState::Off).unwrap(),
            }
            Ack::Ok
        } else {
            cmd.unwrap_err();
            Ack::NotOk
        };

        let mut buf: [u8; IN_SIZE] = [0; IN_SIZE];
        serialize_crc_cobs(&ack, &mut buf);
        /* todo */
        let _ = cx.local.uart_tx.write_bytes(&buf);
    }

    #[task(shared = [blink_data])]
    async fn set_blink_data(mut cx: set_blink_data::Context, options: BlinkerOptions) {
        rprintln!("Inside set_blink_data task");
        cx.shared
            .blink_data
            .lock(|blink_data| *blink_data = options)
    }

    #[task(shared = [rgb_state])]
    async fn update_rgb_data(mut cx: update_rgb_data::Context, state: RgbState) {
        rprintln!("Inside update rgb task");
        cx.shared.rgb_state.lock(|rgb_state| {
            *rgb_state = state;
        });
    }

    #[task(shared = [reference_time])]
    async fn set_date_time(mut cx: set_date_time::Context, new_time: DateTime) {
        cx.shared
            .reference_time
            .lock(|reference_time| *reference_time = new_time);
    }

    #[task()]
    async fn blink(cx: blink::Context) {}

    #[task()]
    async fn update_rgb(cx: update_rgb::Context) {}
}
