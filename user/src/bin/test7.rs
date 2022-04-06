#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::{exit, fork, wait, mmap, get_time};

fn random(m: usize, last: usize) -> usize {
    (last * 942137 + 99995423) % m
}

#[no_mangle]
fn main() -> i32 {
    for i in 0..3 {
        let pid = fork();
        if pid == 0 {
            println!("I am child {}", i);
            if i == 0 {
                let start: usize = 0x10000000;
                let len: usize = 4096 * 400;
                let prot: usize = 3;
                let seed: usize = get_time() as usize;
                let mut v: usize = seed;
                assert_eq!(0, mmap(start, len, prot));
                for i in 0..64 {
                    for _j in 0..64 {
                        let k = random(len / 64, v) + len / 64 * i;
                        let addr: *mut u8 = (start + k) as *mut u8;
                        unsafe {
                            *addr = (k) as u8;
                        }
                        v = k;
                    }
                }
            
                let mut v: usize = seed;
                for i in 0..64 {
                    for _j in 0..64 {
                        let k = random(len / 64, v) + len / 64 * i;
                        let addr: *mut u8 = (start + k as usize) as *mut u8;
                        unsafe {
                            assert_eq!(*addr, (k as usize) as u8);
                        }
                        v = k;
                    }
                }
            }
            if i == 1 {
                let start: usize = 0x10000000;
                let len: usize = 4096 * 400;
                let prot: usize = 3;
                let seed: usize = get_time() as usize;
                let mut v: usize = seed;
                assert_eq!(0, mmap(start, len, prot));
                for _ in 0..4096 {
                    let k = random(len, v);
                    let addr: *mut u8 = (start + k) as *mut u8;
                    unsafe {
                        *addr = (k) as u8;
                    }
                    v = k;
                }
            
                let mut v: usize = seed;
                for _ in 0..4096 {
                    let k = random(len, v);
                    let addr: *mut u8 = (start + k as usize) as *mut u8;
                    unsafe {
                        assert_eq!(*addr, (k as usize) as u8);
                    }
                    v = k;
                }
            }
            if i == 2 {
                let start: usize = 0x10000000;
                let len: usize = 4096 * 400;
                let prot: usize = 3;
                assert_eq!(0, mmap(start, len, prot));
                for i in start..(start + len) {
                    let addr: *mut u8 = i as *mut u8;
                    unsafe {
                        *addr = i as u8;
                    }
                }
                for i in start..(start + len) {
                    let addr: *mut u8 = i as *mut u8;
                    unsafe {
                        assert_eq!(*addr, i as u8);
                    }
                }
            }
            exit(0);
        } else {
            println!("forked child pid = {}", pid);
        }
        assert!(pid > 0);
    }
    let mut exit_code: i32 = 0;
    for _ in 0..3 {
        if wait(&mut exit_code) <= 0 {
            panic!("wait stopped early");
        }
    }
    if wait(&mut exit_code) > 0 {
        panic!("wait got too many");
    }
    println!("Test7 OK!");
    0
}
