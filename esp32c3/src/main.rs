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
            TxRxPins, UartRx, UartTx
        },
        Uart, IO,
    };

    use shared::{Command, Ack,
    OUT_SIZE, IN_SIZE,
    deserialize_crc_cobs, serialize_crc_cobs};

    #[shared]
    struct Shared {
        cmd: [u8; OUT_SIZE],
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
        let cmd: [u8; OUT_SIZE] = [0; OUT_SIZE];
        let cmd_idx: usize = 0;

        (Shared{cmd}, Local{uart_rx, uart_tx, cmd_idx})
    }

    #[task(binds = UART0, local = [cmd_idx, uart_rx], shared = [cmd])]
    fn aggregate(mut cx: aggregate::Context) {
        rprint!("received UART0 rx interrupt: ");

       if let nb::Result::Ok(c) = cx.local.uart_rx.read() {
           rprint!("{}", c);
           cx.shared.cmd.lock(|cmd|{
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
        let cmd = cx.shared.cmd.lock(|cmd|{
            deserialize_crc_cobs::<Command>(cmd)
        });

        let ack = match cmd {
            Err(_) => {rprintln!("invalid command"); Ack::NotOk}
            Ok(c) =>  {rprintln!("valid command: {:?}", c); Ack::Ok}
        };

        let mut buf: [u8; IN_SIZE] = [0; IN_SIZE];
        serialize_crc_cobs(&ack, &mut buf);
        /* todo */
        let _ = cx.local.uart_tx.write_bytes(&buf);
    }

    #[task()]
    async fn set_blink_data(cx: set_blink_data::Context) {
    }

    #[task()]
    async fn set_rgb_data(cx: set_rgb_data::Context) {
    }

    #[task()]
    async fn set_date_time(cx: set_date_time::Context) {
    }

    #[task()]
    async fn blink(cx: blink::Context) {
    }

    #[task()]
    async fn update_rgb(cx: update_rgb::Context) {
    }
}
