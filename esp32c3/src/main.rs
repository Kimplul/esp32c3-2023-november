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
        gpio::{Gpio7, Output, PushPull},
        peripherals::{Peripherals, TIMG0, TIMG1, UART0},
        prelude::*,
        rmt::{Channel0, Rmt},
        timer::{Timer, Timer0, TimerGroup},
        uart::{
            config::{Config, DataBits, Parity, StopBits},
            TxRxPins, UartRx, UartTx,
        },
        Rtc, Uart, IO,
    };

    use esp_hal_smartled::{smartLedAdapter, SmartLedsAdapter};

    use smart_leds::{brightness, SmartLedsWrite, RGB};

    use shared::{
        deserialize_crc_cobs, serialize_crc_cobs, Ack, BlinkerOptions, Command, DateTime, IN_SIZE,
        OUT_SIZE,
    };

    #[derive(Debug)]
    pub enum RgbState {
        On,
        Off,
    }

    pub struct ReferenceTimes {
        utc_reference: u64,
        rtc_reference: u64,
    }

    impl ReferenceTimes {
        fn update(&mut self, utc_ref: u64, rtc_ref: u64) {
            self.rtc_reference = rtc_ref;
            self.utc_reference = utc_ref;
        }

        pub fn get_time(&mut self, rtc_now: u64) -> u64 {
            self.utc_reference + (rtc_now - self.rtc_reference) / 1000
        }

        pub fn new() -> Self {
            ReferenceTimes {
                utc_reference: 0,
                rtc_reference: 0,
            }
        }
    }

    impl Default for ReferenceTimes {
        fn default() -> Self {
            Self::new()
        }
    }

    #[shared]
    struct Shared {
        cmd: [u8; OUT_SIZE],
        rgb_state: RgbState,
        blink_data: BlinkerOptions,
        reference_times: ReferenceTimes,
        rtc: Rtc<'static>,
        timer0: Timer<Timer0<TIMG0>>,
        timer1: Timer<Timer0<TIMG1>>,
    }

    #[local]
    struct Local {
        uart_rx: UartRx<'static, UART0>,
        uart_tx: UartTx<'static, UART0>,
        cmd_idx: usize,
        led: Gpio7<Output<PushPull>>,
        rgb_led: SmartLedsAdapter<Channel0<0>, 0, 25>,
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

        // Configure RMT peripheral globally
        let rmt = Rmt::new(
            peripherals.RMT,
            80u32.MHz(),
            &mut system.peripheral_clock_control,
            &clocks,
        )
        .unwrap();

        let rgb_led = <smartLedAdapter!(0, 1)>::new(rmt.channel0, io.pins.gpio2);

        /* this is apparently dumb */
        uart0.set_rx_fifo_full_threshold(1).unwrap();
        uart0.listen_rx_fifo_full();

        let (uart_tx, uart_rx) = uart0.split();

        let timer_group0 = TimerGroup::new(
            peripherals.TIMG0,
            &clocks,
            &mut system.peripheral_clock_control,
        );

        let mut timer0 = timer_group0.timer0;
        timer0.listen();

        let timer_group1 = TimerGroup::new(
            peripherals.TIMG1,
            &clocks,
            &mut system.peripheral_clock_control,
        );

        let mut timer1 = timer_group1.timer0;
        timer1.listen();

        let led = io.pins.gpio7.into_push_pull_output();

        rprintln!("init works");

        (
            Shared {
                blink_data: BlinkerOptions::Off,
                cmd: [0; OUT_SIZE],
                rgb_state: RgbState::Off,
                reference_times: ReferenceTimes::new(),
                rtc: Rtc::new(peripherals.RTC_CNTL),
                timer0,
                timer1,
            },
            Local {
                uart_rx,
                uart_tx,
                cmd_idx: 0,
                led,
                rgb_led,
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

            if c == 0 || *cx.local.cmd_idx >= OUT_SIZE {
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
                Command::SetDateTime(t) => handle_new_datetime(t),
                Command::SetBlinker(options) => handle_new_blink_data(options),
                Command::RgbOn => {
                    update_rgb_data::spawn(RgbState::On).unwrap();
                    Ack::Ok
                }
                Command::RgbOff => {
                    update_rgb_data::spawn(RgbState::Off).unwrap();
                    Ack::Ok
                }
            }
        } else {
            rprintln!("illegal cmd: {:?}", cmd.unwrap_err());
            Ack::NotOk
        };

        let mut buf: [u8; IN_SIZE] = [0; IN_SIZE];
        let b = serialize_crc_cobs(&ack, &mut buf);
        /* todo */
        cx.local
            .uart_tx
            .write_bytes(b)
            .expect("Failed to write response back to the host");
    }

    fn handle_new_datetime(time: DateTime) -> Ack {
        if let DateTime::Utc(t) = time {
            set_date_time::spawn(t).unwrap();
            Ack::Ok
        } else {
            Ack::NotOk
        }
    }
    fn handle_new_blink_data(options: BlinkerOptions) -> Ack {
        if let BlinkerOptions::On {
            date_time: _,
            freq,
            duration: _,
        } = options
        {
            if freq == 0 {
                return Ack::NotOk;
            }
        }

        set_blink_data::spawn(options).unwrap();
        Ack::Ok
    }
    #[task(shared = [blink_data, timer0, rtc, reference_times])]
    async fn set_blink_data(mut cx: set_blink_data::Context, options: BlinkerOptions) {
        rprintln!("Inside set_blink_data task");

        cx.shared.blink_data.lock(|blink_data| match options {
            BlinkerOptions::Off => (),
            BlinkerOptions::On {
                date_time,
                freq,
                duration,
            } => match date_time {
                DateTime::Now => {
                    let rtc_now = cx.shared.rtc.lock(|rtc| rtc.get_time_ms());
                    let time_now = cx.shared.reference_times.lock(|r| r.get_time(rtc_now));
                    *blink_data = BlinkerOptions::On {
                        date_time: DateTime::Utc(time_now),
                        freq,
                        duration,
                    };
                }
                DateTime::Utc(_) => *blink_data = options,
            },
        });

        cx.shared.timer0.lock(|t| t.start(0u64.secs()));
    }

    #[task(shared = [rgb_state, timer1])]
    async fn update_rgb_data(mut cx: update_rgb_data::Context, state: RgbState) {
        rprintln!("Inside update rgb task");
        cx.shared.rgb_state.lock(|rgb_state| {
            *rgb_state = state;
        });

        cx.shared.timer1.lock(|t| t.start(0u64.secs()));
    }

    #[task(shared = [reference_times, rtc, timer1])]
    async fn set_date_time(mut cx: set_date_time::Context, new_time: u64) {
        rprintln!("set_date_time {:?}", new_time);
        rprintln!("hours {}", new_time / 3600 % 24);

        let rtc_ref = cx.shared.rtc.lock(|r| r.get_time_ms());
        cx.shared
            .reference_times
            .lock(|reference_times| reference_times.update(new_time, rtc_ref));

        cx.shared.timer1.lock(|t| t.start(0u64.secs()));
    }

    #[task(binds = TG0_T0_LEVEL,local=[led], shared=[timer0, blink_data, rtc, reference_times], priority=1)]
    fn blink(mut cx: blink::Context) {
        rprintln!("Inside blink task");
        cx.shared.timer0.lock(|t| t.clear_interrupt());

        let opts = cx.shared.blink_data.lock(|d| *d);
        match opts {
            BlinkerOptions::Off => {
                cx.local.led.set_low().expect("Failed to turn off the led");
            }
            BlinkerOptions::On {
                date_time,
                freq,
                duration,
            } => {
                let rtc_now = cx.shared.rtc.lock(|rtc| rtc.get_time_ms());
                let time_now = cx.shared.reference_times.lock(|r| r.get_time(rtc_now));

                match date_time {
                    DateTime::Now => panic!("Should never end here"), // Should never be this variant
                    DateTime::Utc(s_time) => {
                        if time_now >= s_time + duration {
                            cx.local.led.set_low().expect("Failed to turn off the led");
                            return;
                        }
                        if time_now >= s_time {
                            cx.local.led.toggle().expect("Led toggle failed");
                            // TODO not checking for dividing by 0 but should not happen
                            let period = ((1f32 / freq as f32) * 1000f32) as u32;
                            cx.shared.timer0.lock(|t| t.start(period.millis()));
                            return;
                        }
                        let time_left = s_time - time_now;
                        cx.shared.timer0.lock(|t| t.start(time_left.secs()));
                        // rprintln!("Curr time : {}\nStart_time{}", time_now, s_time);
                        // cx.shared.timer0.lock(|t| t.start(1u64.secs()));
                    }
                }
            }
        }
    }

    #[task(binds=TG1_T0_LEVEL, local=[rgb_led], shared=[rtc, reference_times, timer1,rgb_state])]
    fn update_rgb(mut cx: update_rgb::Context) {
        cx.shared.timer1.lock(|t| t.clear_interrupt());

        let state = cx.shared.rgb_state.lock(|s| match s {
            RgbState::On => true,
            RgbState::Off => false,
        });

        if state {
            let rtc_now = cx.shared.rtc.lock(|rtc| rtc.get_time_ms());
            let time_now = cx.shared.reference_times.lock(|r| r.get_time(rtc_now));
            let hours = time_now / 3600 % 24;

            let color = match hours {
                x if (3..9).contains(&x) => RGB {
                    r: 0xF8,
                    g: 0xF3,
                    b: 0x2B,
                },
                x if (9..15).contains(&x) => RGB {
                    r: 0x9C,
                    g: 0xFF,
                    b: 0xFA,
                },
                x if (15..21).contains(&x) => RGB {
                    r: 0x05,
                    g: 0x3C,
                    b: 0x5E,
                },
                x if !(3..21).contains(&x) => RGB {
                    r: 0x31,
                    g: 0x08,
                    b: 0x1F,
                },
                _ => RGB { r: 0, g: 0, b: 0 },
            };

            cx.local
                .rgb_led
                .write(brightness([color].into_iter(), 20))
                .unwrap();
            cx.shared.timer1.lock(|t| t.start(1u64.secs()));
        } else {
            cx.local
                .rgb_led
                .write(brightness([RGB { r: 0, g: 0, b: 0 }].into_iter(), 0))
                .unwrap();
        }
    }
}
