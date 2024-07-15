use core::ops::{Deref, DerefMut};

use super::machine::*;
use crate::{
    ap_from_vm_rights, asid_t, find_map_for_asid, pptr_t, pptr_to_paddr, vm_attributes_t, vptr_t,
    PDE, PGDE, PTE, PUDE,
};
use sel4_common::arch::config::PPTR_BASE;
use sel4_common::structures::exception_t;
use sel4_common::utils::convert_to_type_ref;
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

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPUD: PageAligned<PTE> = PageAligned::new(PTE(0));

// #[no_mangle]
// #[link_section = ".page_table"]
// pub(crate) static mut armKSGlobalKernelPDs: [[PTE; BIT!(PT_INDEX_BITS)]; BIT!(PT_INDEX_BITS)] =
//     [[PTE(0); BIT!(PT_INDEX_BITS)]; BIT!(PT_INDEX_BITS)];
#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPDs: PageAligned<PageAligned<PTE>> =
    PageAligned::new(PageAligned::new(PTE(0)));

#[no_mangle]
#[link_section = ".page_table"]
pub static mut armKSGlobalUserVSpace: PageAligned<PTE> = PageAligned::new(PTE(0));

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPT: PageAligned<PTE> = PageAligned::new(PTE(0));

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

pub fn unmap_page_table(asid: asid_t, vaddr: vptr_t, pt: &mut PTE) {
    pt.unmap_page_table(asid, vaddr);
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn activate_kernel_vspace() {
    unsafe {
        clean_invalidate_l1_caches();
        setCurrentKernelVSpaceRoot(ttbr_new(
            0,
            armKSGlobalKernelPGD.as_ptr() as usize - PPTR_BASE,
        ));
        setCurrentUserVSpaceRoot(ttbr_new(
            0,
            armKSGlobalUserVSpace.as_ptr() as usize - PPTR_BASE,
        ));
        invalidate_local_tlb();
        /* A53 hardware does not support TLB locking */
    }
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

pub fn set_vm_root_for_flush_with_thread_root(
    vspace: usize,
    asid: asid_t,
    thread_root: &cap_t,
) -> bool {
    if thread_root.get_cap_type() == CapTag::CapPageGlobalDirectoryCap
        && thread_root.get_pgd_is_mapped() != 0
        && thread_root.get_pgd_base_ptr() == vspace
    {
        return false;
    }

    // armv_context_switch(vspace, asid);
    setCurrentUserVSpaceRoot(ttbr_new(asid, vspace));
    true
}

pub fn page_upper_directory_mapped(asid: asid_t, vaddr: vptr_t, pud: &PUDE) -> Option<*mut PGDE> {
    match find_map_for_asid(asid) {
        Some(asid_map) => {
            let look_up_ret =
                convert_to_type_ref::<PTE>(asid_map.get_vspace_root()).lookup_pgd_slot(vaddr);
            if look_up_ret.status != exception_t::EXCEPTION_NONE {
                return None;
            }

            let slot = unsafe { &mut (*look_up_ret.pgdSlot) };

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
            let look_up_ret =
                convert_to_type_ref::<PTE>(asid_map.get_vspace_root()).lookup_pud_slot(vaddr);
            if look_up_ret.status != exception_t::EXCEPTION_NONE {
                return None;
            }

            let slot = unsafe { &mut (*look_up_ret.pudSlot) };

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

pub fn page_table_mapped(asid: asid_t, vaddr: vptr_t, pt: &PTE) -> Option<*mut PDE> {
    match find_map_for_asid(asid) {
        Some(asid_map) => {
            let look_up_ret =
                convert_to_type_ref::<PTE>(asid_map.get_vspace_root()).lookup_pd_slot(vaddr);
            if look_up_ret.status != exception_t::EXCEPTION_NONE {
                return None;
            }

            let slot = unsafe { &mut (*look_up_ret.pdSlot) };

            if !slot.get_present()
                || slot.get_pt_base_address() != pptr_to_paddr(pt as *const _ as _)
            {
                return None;
            }

            return Some(slot);
        }
        None => None,
    }
}

#[inline]
pub fn invalidateTLBByASID(asid: asid_t) {
    invalidate_local_tlb_asid(asid);
}

#[inline]
pub fn invalidateTLBByASIDVA(asid: asid_t, vaddr: vptr_t) {
    invalidate_local_tlb_va_asid((asid << 48) | vaddr >> seL4_PageBits);
}
