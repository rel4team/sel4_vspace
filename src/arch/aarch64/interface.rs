use core::intrinsics::unlikely;
use core::ops::{Deref, DerefMut};

use super::machine::*;
use crate::{
    ap_from_vm_rights, asid_t, find_map_for_asid, find_vspace_for_asid, pptr_t, pptr_to_paddr,
    vm_attributes_t, vptr_t, PDE, PGDE, PTE, PUDE,
};
use sel4_common::arch::config::PPTR_BASE;
use sel4_common::sel4_config::{ARM_Large_Page, ARM_Small_Page};
use sel4_common::structures::exception_t;
use sel4_common::utils::{convert_to_mut_type_ref, convert_to_type_ref, ptr_to_mut, ptr_to_ref};
use sel4_common::{
    arch::vm_rights_t,
    fault::lookup_fault_t,
    sel4_config::{seL4_PageBits, PT_INDEX_BITS},
    BIT,
};
use sel4_cspace::{arch::CapTag, interface::cap_t};
pub const PageAlignedLen: usize = BIT!(PT_INDEX_BITS);
#[repr(align(4096))]
#[derive(Clone, Copy)]
pub struct PageAligned<T>([T; PageAlignedLen]);

impl<T: Copy> PageAligned<T> {
    pub const fn new(v: T) -> Self {
        Self([v; PageAlignedLen])
    }
}

impl<T> Deref for PageAligned<T> {
    type Target = [T; PageAlignedLen];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for PageAligned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPGD: PageAligned<PTE> = PageAligned::new(PTE(0));

#[inline]
pub fn get_kernel_page_global_directory_base() -> usize {
    unsafe { armKSGlobalKernelPGD.as_ptr() as usize }
}

#[inline]
pub fn set_kernel_page_global_directory_by_index(idx: usize, pte: PTE) {
    unsafe {
        armKSGlobalKernelPGD[idx] = pte;
    }
}

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPUD: PageAligned<PTE> = PageAligned::new(PTE(0));

#[inline]
pub fn get_kernel_page_upper_directory_base() -> usize {
    unsafe { armKSGlobalKernelPUD.as_ptr() as usize }
}

#[inline]
pub fn set_kernel_page_upper_directory_by_index(idx: usize, pte: PTE) {
    unsafe {
        armKSGlobalKernelPUD[idx] = pte;
    }
}
// #[no_mangle]
// #[link_section = ".page_table"]
// pub(crate) static mut armKSGlobalKernelPDs: [[PTE; BIT!(PT_INDEX_BITS)]; BIT!(PT_INDEX_BITS)] =
//     [[PTE(0); BIT!(PT_INDEX_BITS)]; BIT!(PT_INDEX_BITS)];
#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPDs: PageAligned<PageAligned<PTE>> =
    PageAligned::new(PageAligned::new(PTE(0)));

#[inline]
pub fn get_kernel_page_directory_base_by_index(idx: usize) -> usize {
    unsafe { armKSGlobalKernelPDs[idx].as_ptr() as usize }
}

#[inline]
pub fn set_kernel_page_directory_by_index(idx1: usize, idx2: usize, pte: PTE) {
    unsafe {
        armKSGlobalKernelPDs[idx1][idx2] = pte;
    }
}

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalUserVSpace: PageAligned<PTE> = PageAligned::new(PTE(0));

#[inline]
pub fn get_arm_global_user_vspace_base() -> usize {
    unsafe { armKSGlobalUserVSpace.as_ptr() as usize }
}

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPT: PageAligned<PTE> = PageAligned::new(PTE(0));

#[inline]
pub fn get_kernel_page_table_base() -> usize {
    unsafe { armKSGlobalKernelPT.as_ptr() as usize }
}

#[inline]
pub fn set_kernel_page_table_by_index(idx: usize, pte: PTE) {
    unsafe {
        armKSGlobalKernelPT[idx] = pte;
    }
}
/// 根据给定的`vspace_root`设置相应的页表，会检查`vspace_root`是否合法，如果不合法默认设置为内核页表
///
/// Use page table in vspace_root to set the satp register.
pub fn set_vm_root(vspace_root: &cap_t) -> Result<(), lookup_fault_t> {
    setCurrentUserVSpaceRoot(pptr_to_paddr(vspace_root.get_pgd_base_ptr()));
    Ok(())
}

pub fn activate_kernel_window() {
    todo!()
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn activate_kernel_vspace() {
    clean_invalidate_l1_caches();
    setCurrentKernelVSpaceRoot(ttbr_new(
        0,
        get_kernel_page_global_directory_base() - PPTR_BASE,
    ));
    setCurrentUserVSpaceRoot(ttbr_new(0, get_arm_global_user_vspace_base() - PPTR_BASE));
    invalidate_local_tlb();
    /* A53 hardware does not support TLB locking */
}

pub fn make_user_1st_level(
    paddr: pptr_t,
    vm_rights: vm_rights_t,
    attributes: vm_attributes_t,
) -> PUDE {
    PUDE::new_1g(
        attributes.get_armExecuteNever(),
        paddr,
        1,
        1,
        0,
        ap_from_vm_rights(vm_rights),
        attributes.get_attr_index(),
    )
}

pub fn make_user_2nd_level(
    paddr: pptr_t,
    vm_rights: vm_rights_t,
    attributes: vm_attributes_t,
) -> PDE {
    PDE::new_large(
        attributes.get_armExecuteNever(),
        paddr,
        1,
        1,
        0,
        ap_from_vm_rights(vm_rights),
        attributes.get_attr_index(),
    )
}

pub fn makeUser3rdLevel(paddr: pptr_t, vm_rights: vm_rights_t, attributes: vm_attributes_t) -> PTE {
    PTE::pte_new(
        attributes.get_armExecuteNever() as usize,
        paddr,
        1,
        1,
        0,
        ap_from_vm_rights(vm_rights),
        attributes.get_attr_index() as usize,
        3, // RESERVED
    )
}

#[no_mangle]
pub fn set_vm_root_for_flush_with_thread_root(
    vspace: *mut PTE,
    asid: asid_t,
    thread_root: &cap_t,
) -> bool {
    if thread_root.get_cap_type() == CapTag::CapPageGlobalDirectoryCap
        && thread_root.get_pgd_is_mapped() != 0
        && thread_root.get_pgd_base_ptr() == vspace as usize
    {
        return false;
    }

    // armv_context_switch(vspace, asid);
    setCurrentUserVSpaceRoot(ttbr_new(asid, vspace as usize));
    true
}

pub fn page_upper_directory_mapped(asid: asid_t, vaddr: vptr_t, pud: &PUDE) -> Option<*mut PGDE> {
    match find_map_for_asid(asid) {
        Some(asid_map) => {
            let lookup_ret = PGDE::new_from_pte(asid_map.get_vspace_root()).lookup_pgd_slot(vaddr);
            if lookup_ret.status != exception_t::EXCEPTION_NONE {
                return None;
            }

            let slot = unsafe { &mut (*lookup_ret.pgdSlot) };

            if !slot.get_present()
                || slot.get_pud_base_address() != pptr_to_paddr(pud as *const _ as _)
            {
                return None;
            }

            return Some(slot);
        }
        None => None,
    }
}

pub fn page_directory_mapped(asid: asid_t, vaddr: vptr_t, pd: &PDE) -> Option<*mut PUDE> {
    match find_map_for_asid(asid) {
        Some(asid_map) => {
            let lookup_ret = PGDE::new_from_pte(asid_map.get_vspace_root()).lookup_pud_slot(vaddr);
            if lookup_ret.status != exception_t::EXCEPTION_NONE {
                return None;
            }

            let slot = unsafe { &mut (*lookup_ret.pudSlot) };

            if !slot.get_present()
                || slot.get_pd_base_address() != pptr_to_paddr(pd as *const _ as _)
            {
                return None;
            }

            return Some(slot);
        }
        None => None,
    }
}

/// TODO: Make pt as usize of
pub fn page_table_mapped(asid: asid_t, vaddr: vptr_t, pt: &PTE) -> Option<*mut PDE> {
    match find_map_for_asid(asid) {
        Some(asid_map) => {
            let lookup_ret = PGDE::new_from_pte(asid_map.get_vspace_root()).lookup_pd_slot(vaddr);
            if lookup_ret.status != exception_t::EXCEPTION_NONE {
                return None;
            }

            let slot = unsafe { &mut (*lookup_ret.pdSlot) };

            if !slot.get_present() || slot.get_pt_base_address() != pptr_to_paddr(pt.0) {
                return None;
            }

            return Some(slot);
        }
        None => None,
    }
}

#[inline]
pub fn invalidate_tlb_by_asid(asid: asid_t) {
    invalidate_local_tlb_asid(asid);
}

#[inline]
pub fn invalidate_tlb_by_asid_va(asid: asid_t, vaddr: vptr_t) {
    invalidate_local_tlb_va_asid((asid << 48) | vaddr >> seL4_PageBits);
}

pub fn unmap_page_upper_directory(asid: asid_t, vaddr: vptr_t, pud: &PUDE) {
    match page_upper_directory_mapped(asid, vaddr, pud) {
        Some(slot) => {
            let slot = unsafe { &mut (*slot) };
            slot.invalidate();
            clean_by_va_pou(slot.get_ptr(),pptr_to_paddr(slot.get_ptr()));
            invalidate_tlb_by_asid(asid);
        }
        None => {}
    }
}

pub fn unmap_page_directory(asid: asid_t, vaddr: vptr_t, pd: &PDE) {
    match page_directory_mapped(asid, vaddr, pd) {
        Some(slot) => {
            let slot = unsafe { &mut (*slot) };
            slot.invalidate();
            clean_by_va_pou(slot.get_ptr(),pptr_to_paddr(slot.get_ptr()));
            invalidate_tlb_by_asid(asid);
        }
        None => {}
    }
}

pub fn unmap_page_table(asid: asid_t, vaddr: vptr_t, pt: &PTE) {
    match page_table_mapped(asid, vaddr, pt) {
        Some(slot) => {
            let slot = unsafe { &mut (*slot) };
            slot.invalidate();
            clean_by_va_pou(slot.get_ptr(),pptr_to_paddr(slot.get_ptr()));
            invalidate_tlb_by_asid(asid);
        }
        None => {}
    }
}

/// Unmap a page table
/// TODO: Remove result Result<(), lookup_fault_t>
pub fn unmapPage(
    page_size: usize,
    asid: asid_t,
    vptr: vptr_t,
    pptr: pptr_t,
) -> Result<(), lookup_fault_t> {
    let addr = pptr_to_paddr(pptr);
    let find_ret = find_vspace_for_asid(asid);
    if unlikely(find_ret.status != exception_t::EXCEPTION_NONE) {
        return Ok(());
    }
    match page_size {
        ARM_Small_Page => {
            let lu_ret =
                PGDE::new_from_pte(find_ret.vspace_root.unwrap() as usize).lookup_pt_slot(vptr);
            if unlikely(lu_ret.status != exception_t::EXCEPTION_NONE) {
                return Ok(());
            }
            let pte = ptr_to_mut(lu_ret.ptSlot);
            if pte.pte_ptr_get_present() && pte.pte_ptr_get_page_base_address() == addr {
                *pte = PTE(0);
                clean_by_va_pou(pte.get_ptr(),pptr_to_paddr(pte.get_ptr()));
            }
            Ok(())
        }
        ARM_Large_Page => {
            log::info!("unmap large page: {:#x?}", vptr);
            let lu_ret =
                PGDE::new_from_pte(find_ret.vspace_root.unwrap() as usize).lookup_pd_slot(vptr);
            if unlikely(lu_ret.status != exception_t::EXCEPTION_NONE) {
                return Ok(());
            }
            let pde = ptr_to_mut(lu_ret.pdSlot);
            // TODO: Rename get_pt_base_address to get_base_address
            if pde.get_present() && pde.get_pt_base_address() == addr {
                *pde = PDE(0);
                clean_by_va_pou(pde.get_ptr(),pptr_to_paddr(pde.get_ptr()));
            }
            Ok(())
        }
        _ => unimplemented!("unMapPage: {page_size}"),
    }
    /*
        switch (page_size) {
        case ARMLargePage: {
            lookupPDSlot_ret_t lu_ret;
            lu_ret = lookupPDSlot(find_ret.vspace_root, vptr);
            if (unlikely(lu_ret.status != EXCEPTION_NONE)) {
                return;
            }
            if (pde_pde_large_ptr_get_present(lu_ret.pdSlot) &&
                pde_pde_large_ptr_get_page_base_address(lu_ret.pdSlot) == addr) {
                *(lu_ret.pdSlot) = pde_invalid_new();
                cleanByVA_PoU((vptr_t)lu_ret.pdSlot, pptr_to_paddr(lu_ret.pdSlot));
            }
            break;
        }
        case ARMHugePage: {
            lookupPUDSlot_ret_t lu_ret;
            lu_ret = lookupPUDSlot(find_ret.vspace_root, vptr);
            if (unlikely(lu_ret.status != EXCEPTION_NONE)) {
                return;
            }
            if (pude_pude_1g_ptr_get_present(lu_ret.pudSlot) &&
                pude_pude_1g_ptr_get_page_base_address(lu_ret.pudSlot) == addr) {
                *(lu_ret.pudSlot) = pude_invalid_new();
                cleanByVA_PoU((vptr_t)lu_ret.pudSlot, pptr_to_paddr(lu_ret.pudSlot));
            }
            break;
        }
        default:
            fail("Invalid ARM page type");
        }
        assert(asid < BIT(16));
        invalidateTLBByASIDVA(asid, vptr);
    */
}
