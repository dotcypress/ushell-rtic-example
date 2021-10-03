use core::fmt::Write;

use crate::*;
use hal::prelude::*;
use rtic::Mutex;
use ushell::{control, Environment, SpinError};

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
";
const SHELL_PROMPT: &str = "#> ";
const CR: &str = "\r\n";

const CMD_MAX_LEN: usize = 32;
const HISTORY_MAX_LEN: usize = 4;

pub type Serial = serial::Serial<stm32::USART2, serial::FullConfig>;
pub type BlinkTimer = Timer<stm32::TIM16>;
pub type Led = gpioa::PA5<Output<PushPull>>;
pub type Autocomplete = StaticAutocomplete<6>;
pub type History = LRUHistory<{ CMD_MAX_LEN }, { HISTORY_MAX_LEN }>;
pub type Shell = UShell<Serial, Autocomplete, History, { CMD_MAX_LEN }>;
pub type Env<'a> = ushell_app::serial_data::SharedResources<'a>;

impl Environment<Serial, Autocomplete, History, (), { CMD_MAX_LEN }> for Env<'_> {
    fn control(
        &mut self,
        shell: &mut Shell,
        code: u8,
    ) -> Result<(), ushell::SpinError<Serial, ()>> {
        match code {
            control::CTRL_K => {
                shell.clear().map_err(SpinError::ShellError)?;
            }
            control::CTRL_D => {
                self.blink_enabled
                    .lock(|blink_enabled| *blink_enabled = true);
            }
            control::CTRL_C => {
                self.blink_enabled
                    .lock(|blink_enabled| *blink_enabled = false);
            }
            _ => {}
        }

        Ok(())
    }

    fn command(
        &mut self,
        shell: &mut Shell,
        cmd: &str,
        args: &str,
    ) -> Result<(), SpinError<Serial, ()>> {
        match cmd {
            "help" => {
                shell.write_str(HELP).ok();
            }
            "clear" => {
                shell.clear().ok();
            }
            "on" => {
                self.blink_enabled
                    .lock(|blink_enabled| *blink_enabled = true);
                shell.write_str(CR).ok();
            }
            "off" => {
                self.blink_enabled
                    .lock(|blink_enabled| *blink_enabled = false);
                shell.write_str(CR).ok();
            }
            "status" => {
                let status = if self.blink_enabled.lock(|blink_enabled| *blink_enabled) {
                    "On"
                } else {
                    "Off"
                };
                write!(
                    shell,
                    "{0:}Animation: {1:}{0:}Frequency: {2:}Hz{0:}",
                    CR,
                    status,
                    self.blink_freq.lock(|blink_freq| *blink_freq)
                )
                .ok();
            }
            "set" => match btoi::btoi(args.as_bytes()) {
                Ok(freq) if freq > 0 && freq <= 100 => {
                    self.blink_freq.lock(|blink_freq| *blink_freq = freq);
                    self.blink_timer
                        .lock(|blink_timer| blink_timer.start((freq as u32 * 2).hz()));
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

        Ok(())
    }
}
