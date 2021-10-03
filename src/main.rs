#![no_std]
#![no_main]
#![deny(warnings)]

extern crate cortex_m;
extern crate cortex_m_rt as rt;
extern crate panic_halt;
extern crate rtic;
extern crate stm32g0xx_hal as hal;
extern crate ushell;

mod shell;

use core::usize;
use hal::{gpio::*, prelude::*, serial, stm32, timer::*};
use shell::*;
use ushell::{autocomplete::StaticAutocomplete, history::LRUHistory, UShell};

#[rtic::app(device = hal::stm32, peripherals = true)]
mod ushell_app {
    use super::*;

    #[shared]
    struct Shared {
        blink_enabled: bool,
        blink_timer: BlinkTimer,
        blink_freq: u8,
    }

    #[local]
    struct Local {
        led: Led,
        shell: Shell,
    }

    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let mut rcc = ctx.device.RCC.constrain();
        let port_a = ctx.device.GPIOA.split(&mut rcc);
        let led = port_a.pa5.into_push_pull_output();

        let mut blink_timer = ctx.device.TIM16.timer(&mut rcc);
        blink_timer.start(4.hz());
        blink_timer.listen();

        let mut serial = ctx
            .device
            .USART2
            .usart(
                port_a.pa2,
                port_a.pa3,
                serial::FullConfig::default(),
                &mut rcc,
            )
            .expect("Failed to init serial port");
        serial.listen(serial::Event::Rxne);

        let autocomplete = StaticAutocomplete(["clear", "help", "off", "on", "set ", "status"]);
        let history = LRUHistory::default();
        let shell = UShell::new(serial, autocomplete, history);

        (
            Shared {
                blink_timer,
                blink_enabled: false,
                blink_freq: 2,
            },
            Local { shell, led },
            init::Monotonics(),
        )
    }

    #[task(binds = TIM16, priority = 2, shared = [blink_enabled, blink_timer], local = [led])]
    fn blink_timer_tick(ctx: blink_timer_tick::Context) {
        let led = ctx.local.led;
        let mut blink_enabled = ctx.shared.blink_enabled;
        let mut blink_timer = ctx.shared.blink_timer;

        if blink_enabled.lock(|blink_enabled| *blink_enabled) {
            led.toggle().ok();
        } else {
            led.set_low().ok();
        }
        blink_timer.lock(|blink_timer| blink_timer.clear_irq());
    }

    #[task(binds = USART2, priority = 1, shared = [blink_enabled, blink_timer, blink_freq], local = [shell])]
    fn serial_data(mut ctx: serial_data::Context) {
        ctx.local.shell.spin(&mut ctx.shared).ok();
    }
}
