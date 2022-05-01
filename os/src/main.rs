#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(default_alloc_error_handler)]

#[macro_use]
mod console;
mod lang_items;
mod config;
mod memory;

#[cfg(target_arch = "riscv64")]
#[path = "arch/riscv/mod.rs"]
mod arch;

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
        mark_global_init_finished(); // 通知全局初始化已完成
        extern "C" {
            fn stext();
            fn etext();
            fn srodata();
            fn erodata();
            fn sdata();
            fn edata();
            fn sbss();
            fn ebss();
            fn boot_stack();
            fn boot_stack_top();
        }
        println!("Hello, world!");
        println!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
        println!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
        println!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
        println!(
            "boot_stack [{:#x}, {:#x})",
            boot_stack as usize, boot_stack_top as usize
        );
        println!(".bss [{:#x}, {:#x})", sbss as usize, ebss as usize);    
    }

    // 等待第一个核执行完上面的全局初始化
    wait_global_init_finished();
    println!("I'm CPU [{}].", cpu_id);
    mark_bootstrap_finish();
    wait_all_cpu_started();

    if cpu_id == config::BOOTSTRAP_CPU_ID{
        panic!("Shutdown machine!");
    }
    else { loop{} }
}

/// 是否还没有核进行全局初始化，如是则返回 true
fn can_do_global_init() -> bool {
    GLOBAL_INIT_STARTED.compare_exchange(false, true, Ordering::Release, Ordering::Relaxed).is_ok()
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
}
