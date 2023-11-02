#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use panic_rtt_target as _;

#[rtic::app(device = esp32c3)]
mod app {
    use rtt_target::{rprintln, rtt_init_print};

    use esp32c3_hal::{
        self as _,
        clock::ClockControl,
        peripherals::Peripherals,
        prelude::*
    };

    use shared::Command;

    #[shared]
    struct Shared {
        cmd: Option<Command>
    }

    #[local]
    struct Local {
    }

    #[init]
    fn init(_: init::Context) -> (Shared, Local) {
        rtt_init_print!();
        rprintln!(env!("CARGO_CRATE_NAME"));

        let peripherals = Peripherals::take();
        let system = peripherals.SYSTEM.split();
        let clocks = ClockControl::max(system.clock_control).freeze();

        rprintln!("init works");
        let cmd: Option<Command> = None;

        (Shared{cmd}, Local{})
    }

    #[task(binds = UART0, shared = [cmd])]
    fn aggregate(cx: aggregate::Context) {
    }

    #[task(shared = [cmd])]
    async fn broker(cx: broker::Context) {
    }

    #[task]
    async fn set_blink_data(cx: set_blink_data::Context) {
    }

    #[task]
    async fn set_rgb_data(cx: set_rgb_data::Context) {
    }

    #[task]
    async fn set_date_time(cx: set_date_time::Context) {
    }

    #[task]
    async fn blink(cx: blink::Context) {
    }

    #[task]
    async fn update_rgb(cx: update_rgb::Context) {
    }
}
