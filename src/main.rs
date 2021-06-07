#![no_std]
#![no_main]
#![deny(warnings)]

extern crate cortex_m;
extern crate cortex_m_rt as rt;
extern crate panic_halt;
extern crate rtic;
extern crate stm32g0xx_hal as hal;
extern crate ushell;

use core::fmt::Write;
use core::usize;

use hal::{
    gpio::{gpioa::PA5, *},
    prelude::*,
    serial, stm32,
    timer::*,
};
use ushell::{autocomplete::StaticAutocomplete, history::LRUHistory, UShell};

pub type Led = PA5<Output<PushPull>>;
pub type BlinkyTimer = Timer<stm32::TIM16>;
pub type Serial = serial::Serial<stm32::USART2, serial::FullConfig>;

#[rtic::app(device = hal::stm32, peripherals = true)]
const APP: () = {
    struct Resources {
        led: Led,
        status: bool,
        blinky_freq: u8,
        blinky_timer: BlinkyTimer,
        shell: UShell<Serial, StaticAutocomplete<6>, LRUHistory<32, 4>, 32>,
    }

    #[init]
    fn init(ctx: init::Context) -> init::LateResources {
        let mut rcc = ctx.device.RCC.constrain();
        let port_a = ctx.device.GPIOA.split(&mut rcc);

        let mut blinky_timer = ctx.device.TIM16.timer(&mut rcc);
        blinky_timer.start(4.hz());
        blinky_timer.listen();

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
        let shell = UShell::new(serial, autocomplete, LRUHistory::default());

        let led = port_a.pa5.into_push_pull_output();
        init::LateResources {
            led,
            shell,
            blinky_timer,
            blinky_freq: 2,
            status: false,
        }
    }

    #[task(binds = TIM16, priority = 2, resources = [blinky_timer, status, led])]
    fn blinky_timer_tick(ctx: blinky_timer_tick::Context) {
        if *ctx.resources.status {
            ctx.resources.led.toggle().expect("Failed to blink o_0");
        }
        ctx.resources.blinky_timer.clear_irq();
    }

    #[task(binds = USART2, priority = 1, resources = [shell, blinky_timer, blinky_freq, status])]
    fn serial_data(ctx: serial_data::Context) {
        const HELP: &str = "\r\n\
            LED Blinky Shell v.0\r\n\r\n\
            USAGE:\r\n\
            \t command [arg]\r\n\r\n\
            COMMANDS:\r\n\
            \t set <hz>\t Set animation frequency [1-100]\r\n\
            \t status\t\t Get animation status\r\n\
            \t on\t\t Start animation\r\n\
            \t off\t\t Stop animation\r\n\
            \t clear\t\t Clear screen\r\n\
            \t help\t\t Print this message\r\n\r\n";
        const CR: &str = "\r\n";
        const SHELL_PROMT: &str = "~> ";

        let shell = ctx.resources.shell;
        let blinky_freq = ctx.resources.blinky_freq;
        let mut blinky_timer = ctx.resources.blinky_timer;
        let mut status = ctx.resources.status;

        loop {
            match shell.poll() {
                Ok(None) => break,
                Ok(Some(ushell::Input::Command(_, command))) => {
                    match command {
                        "help" => {
                            shell.write_str(HELP).ok();
                        }
                        "clear" => {
                            shell.clear().ok();
                        }
                        "on" => {
                            status.lock(|s| *s = true);
                            shell.write_str(CR).ok();
                        }
                        "off" => {
                            status.lock(|s| *s = false);
                            shell.write_str(CR).ok();
                        }
                        "status" => {
                            let on = status.lock(|s| *s);
                            let status = if on { "enabled" } else { "disabled" };
                            write!(
                                shell,
                                "{0:}animation: {1:}{0:}frequency: {2:}Hz{0:}",
                                CR, status, blinky_freq
                            )
                            .ok();
                        }
                        _ => {
                            if command.len() == 0 {
                                shell.write_str(CR).ok();
                            } else if command.len() > 4 && command.starts_with("set ") {
                                let (_, arg) = command.split_at(4);
                                match btoi::btoi(arg.as_bytes()) {
                                    Ok(freq) if freq > 0 && freq <= 100 => {
                                        *blinky_freq = freq;
                                        blinky_timer.lock(|t| {
                                            t.start((freq as u32 * 2).hz());
                                        });
                                        shell.write_str(CR).ok();
                                    }
                                    _ => {
                                        write!(shell, "{0:}invalid frequency{0:}", CR).ok();
                                    }
                                }
                            } else {
                                write!(shell, "{0:}invalid command{0:}", CR).ok();
                            }
                        }
                    }
                    shell.write_str(SHELL_PROMT).ok();
                }
                _ => {}
            }
        }
    }
};
