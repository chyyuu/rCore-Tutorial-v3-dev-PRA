#![no_std]
#![no_main]
#![feature(panic_info_message)]
<<<<<<< HEAD
#![feature(default_alloc_error_handler)]
=======
#![feature(alloc_error_handler)]

extern crate alloc;

#[macro_use]
extern crate bitflags;

#[cfg(feature = "board_k210")]
#[path = "boards/k210.rs"]
mod board;
#[cfg(not(any(feature = "board_k210")))]
#[path = "boards/qemu.rs"]
mod board;
>>>>>>> ch4

#[cfg(feature = "board_k210")]
#[path = "boards/k210.rs"]
mod board;
#[cfg(not(any(feature = "board_k210")))]
#[path = "boards/qemu.rs"]
mod board;
#[macro_use]
mod console;
mod config;
mod lang_items;
<<<<<<< HEAD
mod memory;
mod timer;
mod loader;
=======
mod loader;
mod mm;
mod sbi;
mod sync;
mod syscall;
mod task;
mod timer;
mod trap;

use core::arch::global_asm;
>>>>>>> ch4

pub mod syscall;
pub mod task;
pub mod trap;

<<<<<<< HEAD
#[cfg(target_arch = "riscv64")]
#[path = "arch/riscv/mod.rs"]
mod arch;

core::arch::global_asm!(include_str!("link_app.S"));

use core::sync::atomic::{Ordering, AtomicBool, AtomicUsize};
use core::hint::spin_loop;

extern crate lazy_static;

/// 是否已经有核在进行全局初始化
static GLOBAL_INIT_STARTED: AtomicBool = AtomicBool::new(false);
/// 全局初始化是否已结束
static GLOBAL_INIT_FINISHED: AtomicBool = AtomicBool::new(false);

lazy_static::lazy_static! {
    static ref BOOTED_CPU_NUM: AtomicUsize = AtomicUsize::new(0);
}

#[no_mangle]
pub extern "C" fn start_kernel(_arg0: usize, _arg1: usize) -> ! {
    let cpu_id = arch::get_cpu_id();
    // 只有一个核能进入这个 if 并执行全局初始化操作
    if can_do_global_init() {
        println!("I am the first CPU [{}].", cpu_id);
        memory::clear_bss(); // 清空 bss 段
        memory::init();
        loader::load_apps();
        mark_global_init_finished(); // 通知全局初始化已完成
    }

    // 等待第一个核执行完上面的全局初始化
    wait_global_init_finished();
    println!("I'm CPU [{}].", cpu_id);

    trap::init();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();

    mark_bootstrap_finish();
    wait_all_cpu_started();

    if config::IS_SINGLE_CORE {
        if cpu_id == config::BOOTSTRAP_CPU_ID {
            task::run_first_task();
        } else {
            loop {}
        }
    } else {
        task::run_first_task();
    }
    unreachable!();
}

/// 是否还没有核进行全局初始化，如是则返回 true
fn can_do_global_init() -> bool {
    GLOBAL_INIT_STARTED.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok()
}

/// 标记那些全局只执行一次的启动步骤已完成。
/// 内核必须由 cpu_id 等于 AP_CAN_INIT 初始值的核先启动并执行这些全局只需要一次的操作，然后其他的核才能启动 
fn mark_global_init_finished() {
    GLOBAL_INIT_FINISHED.compare_exchange(false, true, Ordering::Release, Ordering::Relaxed).unwrap();
}

/// 等待那些全局只执行一次的启动步骤是否完成
fn wait_global_init_finished() {
    while GLOBAL_INIT_FINISHED.load(Ordering::Acquire) == false {
        spin_loop();
    }
}

/// 确认当前核已启动(BOOTSTRAP_CPU 也需要调用)
fn mark_bootstrap_finish() {
    BOOTED_CPU_NUM.fetch_add(1, Ordering::Release);
}

/// 等待所有核已启动
fn wait_all_cpu_started() {
    while BOOTED_CPU_NUM.load(Ordering::Acquire) < config::CPU_NUM {
        spin_loop();
    }
=======
fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(sbss as usize as *mut u8, ebss as usize - sbss as usize)
            .fill(0);
    }
}

#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    println!("[kernel] Hello, world!");
    mm::init();
    println!("[kernel] back to world!");
    mm::remap_test();
    trap::init();
    //trap::enable_interrupt();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    task::run_first_task();
    panic!("Unreachable in rust_main!");
>>>>>>> ch4
}
