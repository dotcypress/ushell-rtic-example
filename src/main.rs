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
use ushell::{
    autocomplete::StaticAutocomplete, control, history::LRUHistory, Input, ShellError, UShell,
};

const SHELL_PROMPT: &str = "#> ";
const CR: &str = "\r\n";
const HELP: &str = "\r\n\
\x1b[31mL\x1b[32mE\x1b[34mD \x1b[33mBlinky Shell \x1b[0mv.1\r\n\r\n\
USAGE:\r\n\
\tcommand [arg]\r\n\r\n\
COMMANDS:\r\n\
\ton        Start animation\r\n\
\toff       Stop animation\r\n\
\tstatus    Get animation status\r\n\
\tset <Hz>  Set animation frequency in Hertz [1-100]\r\n\
\tclear     Clear screen\r\n\
\thelp      Print this message\r\n\r\n
CONTROL KEYS:\r\n\
\tCtrl+D    Start animation\r\n\
\tCtrl+C    Stop animation\r\n\
\tCtrl+S    Increment animation frequency\r\n\
\tCtrl+X    Decrement animation frequency\r\n\
";

#[rtic::app(device = hal::stm32, peripherals = true)]
mod ushell_demo {
    use super::*;

    type Serial = serial::Serial<stm32::USART2, serial::FullConfig>;
    type BlinkTimer = Timer<stm32::TIM16>;
    type Led = gpioa::PA5<Output<PushPull>>;
    type Shell = UShell<Serial, StaticAutocomplete<6>, LRUHistory<32, 4>, 32>;

    #[shared]
    struct Shared {
        blink_enabled: bool,
        blink_timer: BlinkTimer,
    }

    #[local]
    struct Local {
        blink_freq: u8,
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
            },
            Local {
                shell,
                blink_freq: 2,
                led,
            },
            init::Monotonics(),
        )
    }

    #[task(binds = TIM16, priority = 2, shared = [blink_timer, blink_enabled], local = [led])]
    fn blink_timer_tick(ctx: blink_timer_tick::Context) {
        let led = ctx.local.led;
        let blink_timer_tick::SharedResources {
            mut blink_enabled,
            mut blink_timer,
        } = ctx.shared;

        if blink_enabled.lock(|e| *e) {
            led.toggle().expect("Failed to blink o_0");
        } else {
            led.set_low().expect("Failed to switch led off");
        }
        blink_timer.lock(|t| t.clear_irq());
    }

    #[task(binds = USART2, priority = 1, shared = [blink_timer, blink_enabled], local = [blink_freq, shell])]
    fn serial_data(ctx: serial_data::Context) {
        let serial_data::SharedResources {
            mut blink_enabled,
            mut blink_timer,
        } = ctx.shared;
        let serial_data::LocalResources { blink_freq, shell } = ctx.local;

        loop {
            match shell.poll() {
                Ok(Some(Input::Command((cmd, args)))) => {
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
                    shell.write_str(SHELL_PROMPT).ok();
                }
                Ok(Some(Input::Control(control::CTRL_D))) => {
                    blink_enabled.lock(|e| *e = true);
                }
                Ok(Some(Input::Control(control::CTRL_C))) => {
                    blink_enabled.lock(|e| *e = false);
                }
                Ok(Some(Input::Control(control::CTRL_S))) => {
                    if *blink_freq == 100 {
                        return;
                    }
                    *blink_freq += 1;
                    blink_timer.lock(|t| {
                        t.start((*blink_freq as u32 * 2).hz());
                    });
                }
                Ok(Some(Input::Control(control::CTRL_X))) => {
                    if *blink_freq == 1 {
                        return;
                    }
                    *blink_freq -= 1;
                    blink_timer.lock(|t| {
                        t.start((*blink_freq as u32 * 2).hz());
                    });
                }
                Err(ShellError::WouldBlock) => break,
                _ => {}
            }
        }
    }
}
