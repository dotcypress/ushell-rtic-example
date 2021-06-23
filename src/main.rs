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

use hal::{gpio::*, prelude::*, serial, stm32, timer::*};
use ushell::ShellError;
use ushell::{autocomplete::StaticAutocomplete, history::LRUHistory, UShell};

const SHELL_PROMT: &str = "#> ";
const CR: &str = "\r\n";
const HELP: &str = "\r\n\
\x1b[31mL\x1b[32mE\x1b[34mD \x1b[33mBlinky Shell \x1b[0mv.0\r\n\r\n\
USAGE:\r\n\
\t command [arg]\r\n\r\n\
COMMANDS:\r\n\
\t set <Hz>\t Set animation frequency in Hertz [1-100]\r\n\
\t status\t\t Get animation status\r\n\
\t on\t\t Start animation\r\n\
\t off\t\t Stop animation\r\n\
\t clear\t\t Clear screen\r\n\
\t help\t\t Print this message\r\n\r\n";

pub type Serial = serial::Serial<stm32::USART2, serial::FullConfig>;
pub type BlinkTimer = Timer<stm32::TIM16>;
pub type Led = gpioa::PA5<Output<PushPull>>;

#[rtic::app(device = hal::stm32, peripherals = true)]
const APP: () = {
    struct Resources {
        led: Led,
        blink_freq: u8,
        blink_enabled: bool,
        blink_timer: BlinkTimer,
        shell: UShell<Serial, StaticAutocomplete<6>, LRUHistory<32, 4>, 32>,
    }

    #[init]
    fn init(ctx: init::Context) -> init::LateResources {
        let mut rcc = ctx.device.RCC.constrain();
        let port_a = ctx.device.GPIOA.split(&mut rcc);

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

        let led = port_a.pa5.into_push_pull_output();
        init::LateResources {
            led,
            shell,
            blink_timer,
            blink_freq: 2,
            blink_enabled: false,
        }
    }

    #[task(binds = TIM16, priority = 2, resources = [blink_timer, blink_enabled, led])]
    fn blink_timer_tick(ctx: blink_timer_tick::Context) {
        if *ctx.resources.blink_enabled {
            ctx.resources.led.toggle().expect("Failed to blink o_0");
        } else {
            ctx.resources
                .led
                .set_low()
                .expect("Failed to switch led off");
        }
        ctx.resources.blink_timer.clear_irq();
    }

    #[task(binds = USART2, priority = 1, resources = [shell, blink_timer, blink_freq, blink_enabled])]
    fn serial_data(ctx: serial_data::Context) {
        let shell = ctx.resources.shell;
        let blink_freq = ctx.resources.blink_freq;
        let mut blink_timer = ctx.resources.blink_timer;
        let mut blink_enabled = ctx.resources.blink_enabled;

        loop {
            match shell.poll() {
                Ok(Some((cmd, args))) => {
                    match cmd {
                        "help" => {
                            shell.write_str(HELP).ok();
                        }
                        "clear" => {
                            shell.clear().ok();
                        }
                        "on" => {
                            blink_enabled.lock(|e| *e = true);
                            shell.write_str(CR).ok();
                        }
                        "off" => {
                            blink_enabled.lock(|e| *e = false);
                            shell.write_str(CR).ok();
                        }
                        "status" => {
                            let on = blink_enabled.lock(|e| *e);
                            let status = if on { "On" } else { "Off" };
                            write!(
                                shell,
                                "{0:}Animation: {1:}{0:}Frequency: {2:}Hz{0:}",
                                CR, status, blink_freq
                            )
                            .ok();
                        }
                        "set" => match btoi::btoi(args.as_bytes()) {
                            Ok(freq) if freq > 0 && freq <= 100 => {
                                *blink_freq = freq;
                                blink_timer.lock(|t| {
                                    t.start((freq as u32 * 2).hz());
                                });
                                shell.write_str(CR).ok();
                            }
                            _ => {
                                write!(shell, "{0:}unsupported frequency{0:}", CR).ok();
                            }
                        },
                        "" => {
                            shell.write_str(CR).ok();
                        }
                        _ => {
                            write!(shell, "{0:}unsupported command{0:}", CR).ok();
                        }
                    }
                    shell.write_str(SHELL_PROMT).ok();
                }
                Err(ShellError::WouldBlock) => break,
                _ => {}
            }
        }
    }
};
