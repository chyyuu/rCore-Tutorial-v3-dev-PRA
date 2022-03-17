
use super::{VirtAddr, VirtPageNum, PhysPageNum, MapType, frame_alloc};
use crate::task::current_process;
use crate::drivers::{MAX_PAGES, ide_read, ide_write};
use crate::sync::UPSafeCell;
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
}

pub fn do_pgfault(addr: usize, flag: usize) -> bool {
    let process = current_process();
    let mut pcb = process.inner_exclusive_access();
    let token = pcb.get_user_token();
    let memory_set = &mut pcb.memory_set;
    let va: VirtAddr = addr.into();
    let vpn: VirtPageNum = va.into();
    println!("[kernel] page fault: addr:{} vpn:{}", addr, vpn.0);
    if let Some(pte) = memory_set.page_table.translate(vpn) {
        if pte.is_valid() {
            if !pte.readable() && flag==0 {
                println!("[kernel] Frame not readable.");
                return false;
            }
            if !pte.writable() && flag==1 {
                println!("[kernel] Frame not writable.");
                return false;
            }
            if !pte.executable() && flag==2 {
                println!("[kernel] Frame not executable.");
                return false;
            }
        }
    }
    for area in &mut memory_set.areas {
        // println!("{} {}", area.vpn_range.get_start().0, area.vpn_range.get_end().0);
        if vpn >= area.vpn_range.get_start() && vpn < area.vpn_range.get_end() {
            // println!("suc: {} {}", area.vpn_range.get_start().0, area.vpn_range.get_end().0);
            // let flags = area.get_flag_bits();
            // println!("{}", flags.bits() as usize);
            let ppn: PhysPageNum;
            match area.map_type {
                MapType::Identical => {
                    panic!("[kernel] Page fault MapType should not be Identical.");
                }
                MapType::Framed => {
                    let mut ide_manager = IDE_MANAGER.exclusive_access();

                    if let Some(frame) = frame_alloc() { // enough frame
                        ppn = frame.ppn;
                        area.data_frames.insert(vpn, frame);
                    }
                    else {  // need to swap out a frame
                        ppn = memory_set.frame_que.pop().unwrap();
                        let data_old = ppn.get_bytes_array();
                        let mut p2v_map = P2V_MAP.exclusive_access();
                        let vpn_old = p2v_map.get(&ppn).unwrap();
                        ide_manager.swap_in(token, *vpn_old, data_old);
                        area.unmap_one(&mut memory_set.page_table, *vpn_old);
                        p2v_map.remove(&ppn);

                        let frame = frame_alloc().unwrap();
                        area.data_frames.insert(vpn, frame);
                    }

                    if ide_manager.check(token, vpn) {
                        let data = ppn.get_bytes_array();
                        ide_manager.swap_out(token, vpn, data);
                    }
                }
            }
            println!("[kernel] mapping vpn:{} to ppn:{}", vpn.0, ppn.0);
            memory_set.page_table.map(vpn, ppn, area.get_flag_bits());
            P2V_MAP.exclusive_access().insert(ppn, vpn);
            return true;
        }
    }
    false
}
