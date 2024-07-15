use core::ops::{Deref, DerefMut};

use super::utils::{kpptr_to_paddr, GET_KPT_INDEX};
use super::{machine::*, pte::PTEFlags};
use crate::{
    ap_from_vm_rights, asid_t, findVSpaceForASID, find_vspace_for_asid, pptr_t, pptr_to_paddr, vm_attributes_t, vptr_t, PDE, PTE, PUDE, TCB_PTR_CTE_PTR
};
use sel4_common::arch::config::PPTR_BASE;
use sel4_common::structures::exception_t;
use sel4_common::utils::convert_to_type_ref;
use sel4_common::{
    arch::vm_rights_t,
    fault::lookup_fault_t,
    sel4_config::{seL4_PageBits, tcbVTable, PT_INDEX_BITS},
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
    // TODO: Implement the vspace_root check like sel4 below.
    /*
        cap_t threadRoot;
        asid_t asid;
        vspace_root_t *vspaceRoot;
        findVSpaceForASID_ret_t find_ret;
        threadRoot = TCB_PTR_CTE_PTR(tcb, tcbVTable)->cap;
        if (!isValidNativeRoot(threadRoot)) {
            setCurrentUserVSpaceRoot(ttbr_new(0, addrFromKPPtr(armKSGlobalUserVSpace)));
            return;
        }
        vspaceRoot = VSPACE_PTR(cap_vtable_root_get_basePtr(threadRoot));
        asid = cap_vtable_root_get_mappedASID(threadRoot);
        find_ret = findVSpaceForASID(asid);
        if (unlikely(find_ret.status != EXCEPTION_NONE || find_ret.vspace_root != vspaceRoot)) {
            setCurrentUserVSpaceRoot(ttbr_new(0, addrFromKPPtr(armKSGlobalUserVSpace)));
            return;
        }
        armv_contextSwitch(vspaceRoot, asid);
    */
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

pub fn makeUser1stLevel(
    paddr: pptr_t,
    vm_rights: vm_rights_t,
    attributes: vm_attributes_t,
) -> PUDE {
    let uxn = attributes.get_armExecuteNever();
    if attributes.get_armPageCacheable() {
        return PUDE::new_1g(
            uxn as usize,
            paddr,
            1,
            1,
            0,
            ap_from_vm_rights(vm_rights),
            mair_types::NORMAL as usize,
        );
    }

    return PUDE::new_1g(
        uxn as usize,
        paddr,
        1,
        1,
        0,
        ap_from_vm_rights(vm_rights),
        mair_types::DEVICE_nGnRnE as usize,
    );
}

pub fn makeUser2ndLevel(paddr: pptr_t, vm_rights: vm_rights_t, attributes: vm_attributes_t) -> PDE {
    let uxn = attributes.get_armExecuteNever();
    if attributes.get_armPageCacheable() {
        return PDE::new_large(
            uxn as usize,
            paddr,
            1,
            1,
            0,
            ap_from_vm_rights(vm_rights),
            mair_types::NORMAL as usize,
        );
    }
    PDE::new_large(
        uxn as usize,
        paddr,
        1,
        1,
        0,
        ap_from_vm_rights(vm_rights),
        mair_types::DEVICE_nGnRnE as usize,
    )
}

pub fn makeUser3rdLevel(
    paddr: pptr_t,
    vm_rights: vm_rights_t,
    attributes: vm_attributes_t,
) -> PTE {
    let uxn = attributes.get_armExecuteNever();
    if attributes.get_armPageCacheable() {
        return PTE::pte_new(
            uxn as usize,
            paddr,
            1,
            1,
            0,
            ap_from_vm_rights(vm_rights),
            mair_types::NORMAL as usize,
            3, // RESERVED
        );
    }

    PTE::pte_new(
        uxn as usize,
        paddr,
        1,
        1,
        0,
        ap_from_vm_rights(vm_rights),
        mair_types::DEVICE_nGnRnE as usize,
        3, // RESERVED
    )
}

pub fn setVMRootForFlush(vspace: usize, asid: asid_t) -> bool {
    extern "C" {
        fn ksCurThread(); // from sel4 task
    }

    let threadRoot = unsafe { (*TCB_PTR_CTE_PTR(ksCurThread as usize, tcbVTable)).cap };

    if threadRoot.get_cap_type() == CapTag::CapPageGlobalDirectoryCap
        && threadRoot.get_pgd_is_mapped() > 0
        && threadRoot.get_pgd_base_ptr() == vspace
    {
        return false;
    }

    // armv_context_switch(vspace, asid);
    setCurrentUserVSpaceRoot(ttbr_new(asid, vspace));
    return true;
}

pub fn pageUpperDirectoryMapped(asid: asid_t, vaddr: vptr_t, pud: &PUDE) -> Option<*mut PTE> {
    let find_ret = findVSpaceForASID(asid);
    if find_ret.status != exception_t::EXCEPTION_NONE {
        return None;
    }

    let lu_ret =
        convert_to_type_ref::<PTE>(find_ret.vspace_root.unwrap() as usize).lookup_pgd_slot(vaddr);

    if lu_ret.status != exception_t::EXCEPTION_NONE {
        return None;
    }

    let pgde = unsafe { (*lu_ret.pgdSlot).as_pgde() };
    if pgde.pud_ptr_get_present()
        && pgde.get_pud_base_address() == pptr_to_paddr(pud as *const PUDE as usize)
    {
        return Some(lu_ret.pgdSlot);
    }

    None
}

pub fn pageDirectoryMapped(asid: asid_t, vaddr: vptr_t, pd: &PDE) -> Option<*mut PTE> {
    let find_ret = find_vspace_for_asid(asid);
    if find_ret.status != exception_t::EXCEPTION_NONE {
        return None;
    }

    let lu_ret =
        convert_to_type_ref::<PTE>(find_ret.vspace_root.unwrap() as usize).lookup_pud_slot(vaddr);

    if lu_ret.status != exception_t::EXCEPTION_NONE {
        return None;
    }

    let pude = unsafe { (*lu_ret.pudSlot).as_pude() };
    if pude.pd_ptr_get_present()
        && pude.pude_pd_ptr_get_pd_base_address() == pptr_to_paddr(pd as *const PDE as usize)
    {
        return Some(lu_ret.pudSlot);
    }

    None
}

pub fn pageTableMapped(asid: asid_t, vaddr: vptr_t, pt: &PTE) -> Option<*mut PTE> {
    let find_ret = findVSpaceForASID(asid);
    if find_ret.status != exception_t::EXCEPTION_NONE {
        return None;
    }

    let lu_ret =
        convert_to_type_ref::<PTE>(find_ret.vspace_root.unwrap() as usize).lookup_pd_slot(vaddr);

    if lu_ret.status != exception_t::EXCEPTION_NONE {
        return None;
    }

    let pde = unsafe { (*lu_ret.pdSlot).as_pde() };
    if pde.small_ptr_get_present()
        && pde.pde_small_ptr_get_pt_base_address() == pptr_to_paddr(pt as *const PTE as usize)
    {
        return Some(lu_ret.pdSlot);
    }

    None
}

#[inline]
pub fn invalidateTLBByASID(asid: asid_t) {
    invalidate_local_tlb_asid(asid);
}

#[inline]
pub fn invalidateTLBByASIDVA(asid: asid_t, vaddr: vptr_t) {
    invalidate_local_tlb_va_asid((asid << 48) | vaddr >> seL4_PageBits);
}
