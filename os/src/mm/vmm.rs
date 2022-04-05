
use super::{VirtAddr, VirtPageNum, PhysPageNum, MapType, frame_alloc, frame_check, MemorySet, GlobalFrameManager};
use crate::task::current_process;
use crate::drivers::{MAX_PAGES, ide_read, ide_write};
use crate::sync::UPSafeCell;
use crate::config::{PRA_IS_LOCAL, CHOSEN_PRA};
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::*;

pub struct IdeManager {
    current: usize,
    end: usize,
    recycled: Vec<usize>,
    map: BTreeMap<(usize, VirtPageNum), usize>,
}

impl IdeManager {
    pub fn new() -> Self {
        Self {
            current: 0,
            end: MAX_PAGES - 1,
            recycled: Vec::new(),
            map: BTreeMap::new(),
        }
    }
    pub fn swap_in(&mut self, token: usize, vpn: VirtPageNum, src: &mut [u8]) {
        if let Some(idx) = self.recycled.pop() {
            ide_write(idx, src);
            self.map.insert((token, vpn), idx);
        } else if self.current == self.end {
            panic!("[kernel] Virtual disk space is not enough for handling page fault.");
        } else {
            self.current += 1;
            ide_write(self.current - 1, src);
            self.map.insert((token, vpn), self.current - 1);
        }
    }
    pub fn swap_out(&mut self, token: usize, vpn: VirtPageNum, dst: &mut [u8]) {
        let idx = self.map.get(&(token, vpn)).unwrap();
        ide_read(*idx, dst);
        self.recycled.push(*idx);
        self.map.remove(&(token, vpn));
    }
    pub fn check(&mut self, token: usize, vpn: VirtPageNum) -> bool {
        self.map.get(&(token, vpn)) != None
    }
}

lazy_static! {
    pub static ref P2V_MAP: Arc<UPSafeCell<BTreeMap<PhysPageNum, VirtPageNum>>> =
        Arc::new(unsafe { UPSafeCell::new( BTreeMap::new()) });
    pub static ref IDE_MANAGER: Arc<UPSafeCell<IdeManager>> =
        Arc::new(unsafe { UPSafeCell::new( IdeManager::new()) });
    pub static ref GFM: Arc<UPSafeCell<GlobalFrameManager>> =
        Arc::new(unsafe { UPSafeCell::new( GlobalFrameManager::new(CHOSEN_PRA)) });
}

fn local_pra(memory_set: &mut MemorySet, vpn: VirtPageNum, token: usize) -> bool {
    let mut ide_manager = IDE_MANAGER.exclusive_access();
    for i in 0..memory_set.areas.len() {
        if vpn >= memory_set.areas[i].vpn_range.get_start() && vpn < memory_set.areas[i].vpn_range.get_end() {
            let ppn: PhysPageNum;
            match memory_set.areas[i].map_type {
                MapType::Identical => {
                    panic!("[kernel] Page fault MapType should not be Identical.");
                }
                MapType::Framed => {
                    if let Some(frame) = frame_alloc() { // enough frame
                        ppn = frame.ppn;
                        memory_set.areas[i].data_frames.insert(vpn, frame);
                        println!("[kernel] PAGE FAULT: (local) Frame enough, allocating ppn:{} frame.", ppn.0);
                    }
                    else {  // frame not enough, need to swap out a frame
                        ppn = memory_set.get_next_frame().unwrap();
                        let data_old = ppn.get_bytes_array();
                        let mut p2v_map = P2V_MAP.exclusive_access();
                        let vpn_old = *(p2v_map.get(&ppn).unwrap());
                        ide_manager.swap_in(token, vpn_old, data_old);
                        for j in 0..memory_set.areas.len() {
                            if vpn_old >= memory_set.areas[j].vpn_range.get_start() && vpn_old < memory_set.areas[j].vpn_range.get_end() {
                                memory_set.areas[j].unmap_one(&mut memory_set.page_table, vpn_old);
                            }
                        }
                        p2v_map.remove(&ppn);
                        println!("[kernel] PAGE FAULT: (local) Frame not enough, swapping out ppn:{} frame.", ppn.0);
        
                        let frame = frame_alloc().unwrap();
                        memory_set.areas[i].data_frames.insert(vpn, frame);
                    }
        
                    if ide_manager.check(token, vpn) {
                        let data = ppn.get_bytes_array();
                        ide_manager.swap_out(token, vpn, data);
                        println!("[kernel] PAGE FAULT: (local) Swapping in vpn:{} ppn:{} frame.", vpn.0, ppn.0);
                    }
                }
            }
            if !frame_check() {
                let ppn = memory_set.get_next_frame().unwrap();
                let data_old = ppn.get_bytes_array();
                let mut p2v_map = P2V_MAP.exclusive_access();
                let vpn_old = *(p2v_map.get(&ppn).unwrap());
                ide_manager.swap_in(token, vpn_old, data_old);
                for j in 0..memory_set.areas.len() {
                    if vpn_old >= memory_set.areas[j].vpn_range.get_start() && vpn_old < memory_set.areas[j].vpn_range.get_end() {
                        memory_set.areas[j].unmap_one(&mut memory_set.page_table, vpn_old);
                    }
                }
                p2v_map.remove(&ppn);
                println!("[kernel] PAGE FAULT: (local) Swapping out ppn:{} frame.", ppn.0);
            }
            println!("[kernel] PAGE FAULT: (local) Mapping vpn:{} to ppn:{}.", vpn.0, ppn.0);
            memory_set.page_table.map(vpn, ppn, memory_set.areas[i].get_flag_bits());
            P2V_MAP.exclusive_access().insert(ppn, vpn);
            memory_set.insert_frame(ppn);
            return true;
        }
    }
    false
}

fn global_pra(memory_set: &mut MemorySet, vpn: VirtPageNum, token: usize) -> bool {
    for i in 0..memory_set.areas.len() {
        if vpn >= memory_set.areas[i].vpn_range.get_start() && vpn < memory_set.areas[i].vpn_range.get_end() {
            GFM.exclusive_access().work(memory_set, token);
            let ppn: PhysPageNum;
            match memory_set.areas[i].map_type {
                MapType::Identical => {
                    panic!("[kernel] Page fault MapType should not be Identical.");
                }
                MapType::Framed => {
                    let frame = frame_alloc().unwrap();
                    ppn = frame.ppn;
                    memory_set.areas[i].data_frames.insert(vpn, frame);

                    if IDE_MANAGER.exclusive_access().check(token, vpn) {
                        let data = ppn.get_bytes_array();
                        IDE_MANAGER.exclusive_access().swap_out(token, vpn, data);
                        println!("[kernel] PAGE FAULT: (global) Swapping in vpn:{} ppn:{} frame.", vpn.0, ppn.0);
                    }
                }
            }
            println!("[kernel] PAGE FAULT: (global) Mapping vpn:{} to ppn:{}.", vpn.0, ppn.0);
            memory_set.page_table.map(vpn, ppn, memory_set.areas[i].get_flag_bits());
            P2V_MAP.exclusive_access().insert(ppn, vpn);
            memory_set.insert_frame(ppn);
            return true;
        }
    }
    false
}

pub fn do_pgfault(addr: usize, flag: usize) -> bool {
    let process = current_process();
    println!("[kernel] PAGE FAULT: pid {}", process.pid.0);
    let mut pcb = process.inner_exclusive_access();
    let token = pcb.get_user_token();
    let memory_set = &mut pcb.memory_set;
    let va: VirtAddr = addr.into();
    let vpn: VirtPageNum = va.into();
    println!("[kernel] PAGE FAULT: addr:{} vpn:{}", addr, vpn.0);
    if let Some(pte) = memory_set.page_table.translate(vpn) {
        if pte.is_valid() {
            if !pte.readable() && flag==0 {
                println!("[kernel] PAGE FAULT: Frame not readable.");
                return false;
            }
            if !pte.writable() && flag==1 {
                println!("[kernel] PAGE FAULT: Frame not writable.");
                return false;
            }
            if !pte.executable() && flag==2 {
                println!("[kernel] PAGE FAULT: Frame not executable.");
                return false;
            }
        }
    }
    if PRA_IS_LOCAL {
        local_pra(memory_set, vpn, token)
    }
    else {
        global_pra(memory_set, vpn, token)
    }
}
