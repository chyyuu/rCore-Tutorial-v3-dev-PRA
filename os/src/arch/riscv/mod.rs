mod cpu;
mod sbi;
mod page_control;
pub mod stdout;

pub use sbi::{
    set_timer,
    shutdown,
    console_getchar,
};

pub use page_control::{
    setSUMAccessClose,
    setSUMAccessOpen,
};

core::arch::global_asm!(include_str!("boot/entry.asm"));

pub fn get_cpu_id() -> usize {
    cpu::id()
}
