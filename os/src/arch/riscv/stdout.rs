use core::fmt::{Arguments, Result, Write};

use spin::Mutex;
use lazy_static;

fn putchar_raw(c: u8) {
    super::sbi::console_putchar(c as usize);
}

pub struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> Result {
        for c in s.bytes() {
            if c == 127 {
                putchar_raw(8);
                putchar_raw(b' ');
                putchar_raw(8);
            } else {
                putchar_raw(c);
            }
        }
        Ok(())
    }
}

lazy_static::lazy_static! {
    pub static ref STDOUT: Mutex<Stdout> = Mutex::new(Stdout);
    pub static ref STDERR: Mutex<Stdout> = Mutex::new(Stdout);
}

/// 输出到 stdout
pub fn stdout_puts(fmt: Arguments) {
    STDOUT.lock().write_fmt(fmt).unwrap();
}
