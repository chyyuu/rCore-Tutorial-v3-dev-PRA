use super::{frame_alloc, FrameTracker};
use super::{PTEFlags, PageTable, PageTableEntry};
use super::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use super::{StepByOne, VPNRange};
use super::{P2V_MAP, PRA, LocalFrameManager};
use crate::config::{MEMORY_END, MMIO, PAGE_SIZE, TRAMPOLINE};
use crate::task::current_process;
use crate::sync::UPSafeCell;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::asm;
use lazy_static::*;
use riscv::register::satp;

extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss_with_stack();
    fn ebss();
    fn ekernel();
    fn strampoline();
}

lazy_static! {
    pub static ref KERNEL_SPACE: Arc<UPSafeCell<MemorySet>> =
        Arc::new(unsafe { UPSafeCell::new(MemorySet::new_kernel()) });
}

pub fn kernel_token() -> usize {
    KERNEL_SPACE.exclusive_access().token()
}

pub struct MemorySet {
    pub page_table: PageTable,
    pub areas: Vec<MapArea>,
    pub frame_manager: LocalFrameManager,
}

impl MemorySet {
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
            frame_manager: LocalFrameManager::new(PRA::ClockImproved),
        }
    }
    pub fn token(&self) -> usize {
        self.page_table.token()
    }
    /// Assume that no conflicts.
    pub fn insert_framed_area_kernel(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
    ) {
        self.kpush(
            MapArea::new(start_va, end_va, MapType::Framed, permission),
        );
    }
    pub fn insert_framed_area(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
    ) {
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed, permission),
            None,
        );
    }
    pub fn remove_area_with_start_vpn(&mut self, start_vpn: VirtPageNum) {
        if let Some((idx, area)) = self
            .areas
            .iter_mut()
            .enumerate()
            .find(|(_, area)| area.vpn_range.get_start() == start_vpn)
        {
            area.unmap(&mut self.page_table);
            self.areas.remove(idx);
        }
    }
    fn kpush(&mut self, mut map_area: MapArea) {
        map_area.map(&mut self.page_table);
        self.areas.push(map_area);
    }
    fn push(&mut self, mut map_area: MapArea, data: Option<&[u8]>) {
        if let Some(data) = data {
            map_area.map(&mut self.page_table);
            map_area.copy_data(&mut self.page_table, data);
        }
        self.areas.push(map_area);
    }
    /// Mention that trampoline is not collected by areas.
    fn map_trampoline(&mut self) {
        self.page_table.map(
            VirtAddr::from(TRAMPOLINE).into(),
            PhysAddr::from(strampoline as usize).into(),
            PTEFlags::R | PTEFlags::X,
        );
    }
    /// Without kernel stacks.
    pub fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // map kernel sections
        println!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
        println!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
        println!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
        println!(
            ".bss [{:#x}, {:#x})",
            sbss_with_stack as usize, ebss as usize
        );
        println!("mapping .text section");
        memory_set.kpush(
            MapArea::new(
                (stext as usize).into(),
                (etext as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::X,
            ),
        );
        println!("mapping .rodata section");
        memory_set.kpush(
            MapArea::new(
                (srodata as usize).into(),
                (erodata as usize).into(),
                MapType::Identical,
                MapPermission::R,
            ),
        );
        println!("mapping .data section");
        memory_set.kpush(
            MapArea::new(
                (sdata as usize).into(),
                (edata as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
        );
        println!("mapping .bss section");
        memory_set.kpush(
            MapArea::new(
                (sbss_with_stack as usize).into(),
                (ebss as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
        );
        println!("mapping physical memory");
        memory_set.kpush(
            MapArea::new(
                (ekernel as usize).into(),
                MEMORY_END.into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
        );
        println!("mapping memory-mapped registers");
        for pair in MMIO {
            memory_set.kpush(
                MapArea::new(
                    (*pair).0.into(),
                    ((*pair).0 + (*pair).1).into(),
                    MapType::Identical,
                    MapPermission::R | MapPermission::W,
                ),
            );
        }
        memory_set
    }
    /// Include sections in elf and trampoline,
    /// also returns user_sp_base and entry point.
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize) {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // map program headers of elf, with U flag
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");
        let ph_count = elf_header.pt2.ph_count();
        let mut max_end_vpn = VirtPageNum(0);
        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                let start_va: VirtAddr = (ph.virtual_addr() as usize).into();
                let end_va: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
                let mut map_perm = MapPermission::U;
                let ph_flags = ph.flags();
                if ph_flags.is_read() {
                    map_perm |= MapPermission::R;
                }
                if ph_flags.is_write() {
                    map_perm |= MapPermission::W;
                }
                if ph_flags.is_execute() {
                    map_perm |= MapPermission::X;
                }
                let map_area = MapArea::new(start_va, end_va, MapType::Framed, map_perm);
                max_end_vpn = map_area.vpn_range.get_end();
                memory_set.push(
                    map_area,
                    Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]),
                );
            }
        }
        let max_end_va: VirtAddr = max_end_vpn.into();
        let mut user_stack_base: usize = max_end_va.into();
        user_stack_base += PAGE_SIZE;
        (
            memory_set,
            user_stack_base,
            elf.header.pt2.entry_point() as usize,
        )
    }
    pub fn from_existed_user(user_space: &MemorySet) -> MemorySet {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // copy data sections/trap_context/user_stack
        for area in user_space.areas.iter() {
            let new_area = MapArea::from_another(area);
            memory_set.kpush(new_area);
            // copy data from another space
            for vpn in area.vpn_range {
                let src_ppn = user_space.translate(vpn).unwrap().ppn();
                let dst_ppn = memory_set.translate(vpn).unwrap().ppn();
                dst_ppn
                    .get_bytes_array()
                    .copy_from_slice(src_ppn.get_bytes_array());
            }
        }
        memory_set
    }
    pub fn activate(&self) {
        let satp = self.page_table.token();
        unsafe {
            satp::write(satp);
            asm!("sfence.vma");
        }
    }
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.page_table.translate(vpn)
    }
    pub fn recycle_data_pages(&mut self) {
        //*self = Self::new_bare();
        self.areas.clear();
    }
    pub fn get_next_frame(&mut self) -> Option<PhysPageNum> {
        self.frame_manager.get_next_frame(&mut self.page_table)
    }
    pub fn insert_frame(&mut self, ppn: PhysPageNum) {
        self.frame_manager.insert_frame(ppn)
    }
}

pub struct MapArea {
    pub vpn_range: VPNRange,
    pub data_frames: BTreeMap<VirtPageNum, FrameTracker>,
    pub map_type: MapType,
    pub map_perm: MapPermission,
}

impl MapArea {
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        map_perm: MapPermission,
    ) -> Self {
        let start_vpn: VirtPageNum = start_va.floor();
        let end_vpn: VirtPageNum = end_va.ceil();
        Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            data_frames: BTreeMap::new(),
            map_type,
            map_perm,
        }
    }
    pub fn from_another(another: &MapArea) -> Self {
        Self {
            vpn_range: VPNRange::new(another.vpn_range.get_start(), another.vpn_range.get_end()),
            data_frames: BTreeMap::new(),
            map_type: another.map_type,
            map_perm: another.map_perm,
        }
    }
    pub fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        let ppn: PhysPageNum;
        match self.map_type {
            MapType::Identical => {
                ppn = PhysPageNum(vpn.0);
            }
            MapType::Framed => {
                let frame = frame_alloc().unwrap();
                ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
            }
        }
        let pte_flags = PTEFlags::from_bits(self.map_perm.bits).unwrap();
        page_table.map(vpn, ppn, pte_flags);
        P2V_MAP.exclusive_access().insert(ppn, vpn);
    }
    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        if self.map_type == MapType::Framed {
            // println!("unmapping vpn:{}", vpn.0);
            self.data_frames.remove(&vpn);
        }
        page_table.unmap(vpn);
    }
    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn);
        }
    }
    pub fn unmap(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn);
        }
    }
    /// data: start-aligned but maybe with shorter length
    /// assume that all frames were cleared before
    pub fn copy_data(&mut self, page_table: &mut PageTable, data: &[u8]) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let mut current_vpn = self.vpn_range.get_start();
        let len = data.len();
        loop {
            let src = &data[start..len.min(start + PAGE_SIZE)];
            let dst = &mut page_table
                .translate(current_vpn)
                .unwrap()
                .ppn()
                .get_bytes_array()[..src.len()];
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len {
                break;
            }
            current_vpn.step();
        }
    }
    pub fn get_flag_bits(&mut self) -> PTEFlags {
        PTEFlags::from_bits(self.map_perm.bits).unwrap()
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum MapType {
    Identical,
    Framed,
}

bitflags! {
    pub struct MapPermission: u8 {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}

pub fn memory_alloc(start: usize, len: usize, port: usize) -> isize {
    if len == 0 {
        return 0;
    }
    if (len > 1073741824) || ((port & (!0x7)) != 0) || ((port & 0x7) == 0) || ((start % 4096) != 0) {
        return -1;
    }
    let process = current_process();
    let mut pcb = process.inner_exclusive_access();
    let memory_set = &mut pcb.memory_set;
    let l: VirtAddr = start.into();
    let r: VirtAddr = (start + len).into();
    let lvpn = l.floor();
    let rvpn = r.ceil();
    // println!("L:{:?} R:{:?}", L, R);
    for area in &memory_set.areas {
        // println!("{:?} {:?}", area.vpn_range.l, area.vpn_range.r);
        if (lvpn <= area.vpn_range.get_start()) && (rvpn > area.vpn_range.get_start()) {
            return -1;
        }
    }
    let mut permission = MapPermission::from_bits((port as u8) << 1).unwrap();
    permission.set(MapPermission::U, true);
    // inner.tasks[current].memory_set.insert_framed_area(start.into(), (start + len).into(), permission);
    let mut start = start;
    let end = start + len;
    while start < end {
        let mut endr = start + PAGE_SIZE;
        if endr > end {
            endr = end;
        }
        memory_set.insert_framed_area(start.into(), endr.into(), permission);
        start = endr;
    }
    // return ((len - 1 + PAGE_SIZE) / PAGE_SIZE * PAGE_SIZE) as isize;
    return 0;
}

pub fn memory_free(start: usize, len: usize) -> isize {
    if len == 0 {
        return 0;
    }
    if start % 4096 != 0 {
        return -1;
    }
    let process = current_process();
    let mut pcb = process.inner_exclusive_access();
    let memory_set = &mut pcb.memory_set;
    let l: VirtAddr = start.into();
    let r: VirtAddr = (start + len).into();
    let lvpn = l.floor();
    let rvpn = r.ceil();
    let mut cnt = 0;
    for area in &memory_set.areas {
        if (lvpn <= area.vpn_range.get_start()) && (rvpn > area.vpn_range.get_start()) {
            cnt += 1;
        }
    }
    if cnt < rvpn.0-lvpn.0 {
        return -1;
    }
    for i in 0..memory_set.areas.len() {
        if !memory_set.areas.get(i).is_some() {
            continue;
        }
        if (lvpn <= memory_set.areas[i].vpn_range.get_start()) && (rvpn > memory_set.areas[i].vpn_range.get_start()) {
            memory_set.areas[i].unmap(&mut memory_set.page_table);
            memory_set.areas.remove(i);
        }
    }
    // return ((len - 1 + PAGE_SIZE) / PAGE_SIZE * PAGE_SIZE) as isize;
    return 0;
}

#[allow(unused)]
pub fn remap_test() {
    let mut kernel_space = KERNEL_SPACE.exclusive_access();
    let mid_text: VirtAddr = ((stext as usize + etext as usize) / 2).into();
    let mid_rodata: VirtAddr = ((srodata as usize + erodata as usize) / 2).into();
    let mid_data: VirtAddr = ((sdata as usize + edata as usize) / 2).into();
    assert!(
        !kernel_space
            .page_table
            .translate(mid_text.floor())
            .unwrap()
            .writable(),
    );
    assert!(
        !kernel_space
            .page_table
            .translate(mid_rodata.floor())
            .unwrap()
            .writable(),
    );
    assert!(
        !kernel_space
            .page_table
            .translate(mid_data.floor())
            .unwrap()
            .executable(),
    );
    println!("remap_test passed!");
}
