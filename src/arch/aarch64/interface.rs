use core::ops::{Deref, DerefMut};

use super::{machine::*, pte::PTEFlags};
use crate::{
    ap_from_vm_rights, asid_t, pptr_t, pptr_to_paddr, pte_t, vm_attributes_t, vptr_t, PDE, PUDE,
};
use sel4_common::{
    arch::{
        config::{PADDR_BASE, PADDR_TOP, PPTR_BASE, PPTR_TOP},
        vm_rights_t,
    },
    fault::lookup_fault_t,
    sel4_config::{seL4_LargePageBits, PT_INDEX_BITS},
    BIT,
};
use sel4_cspace::interface::cap_t;

use super::utils::{kpptr_to_paddr, GET_KPT_INDEX};

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
pub(crate) static mut armKSGlobalKernelPGD: PageAligned<pte_t> = PageAligned::new(pte_t(0));

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPUD: PageAligned<pte_t> = PageAligned::new(pte_t(0));

// #[no_mangle]
// #[link_section = ".page_table"]
// pub(crate) static mut armKSGlobalKernelPDs: [[pte_t; BIT!(PT_INDEX_BITS)]; BIT!(PT_INDEX_BITS)] =
//     [[pte_t(0); BIT!(PT_INDEX_BITS)]; BIT!(PT_INDEX_BITS)];
#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPDs: PageAligned<PageAligned<pte_t>> =
    PageAligned::new(PageAligned::new(pte_t(0)));

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalUserVSpace: PageAligned<pte_t> = PageAligned::new(pte_t(0));

#[no_mangle]
#[link_section = ".page_table"]
pub(crate) static mut armKSGlobalKernelPT: PageAligned<pte_t> = PageAligned::new(pte_t(0));

#[no_mangle]
pub fn rust_map_kernel_window() {
    unsafe {
        armKSGlobalKernelPGD[GET_KPT_INDEX(PPTR_BASE, 0)] =
            pte_t::pte_next_table(kpptr_to_paddr(armKSGlobalKernelPUD.as_ptr() as usize), true);
    }

    let mut idx = GET_KPT_INDEX(PPTR_BASE, 1);
    while idx < GET_KPT_INDEX(PPTR_TOP, 1) {
        unsafe {
            armKSGlobalKernelPUD[idx] = pte_t::pte_next_table(
                kpptr_to_paddr(armKSGlobalKernelPDs[idx].as_ptr() as usize),
                true,
            );
        }
        idx += 1;
    }

    let mut vaddr = PPTR_BASE;
    let mut paddr = PADDR_BASE;
    while paddr < PADDR_TOP {
        unsafe {
            let flag = PTEFlags::UXN | PTEFlags::AF | PTEFlags::NORMAL;
            armKSGlobalKernelPDs[GET_KPT_INDEX(vaddr, 1)][GET_KPT_INDEX(vaddr, 2)] =
                pte_t::new(paddr, flag);
            vaddr += BIT!(seL4_LargePageBits);
            paddr += BIT!(seL4_LargePageBits)
        }
    }

    unsafe {
        armKSGlobalKernelPUD[GET_KPT_INDEX(PPTR_TOP, 1)] = pte_t::pte_next_table(
            kpptr_to_paddr(armKSGlobalKernelPDs[BIT!(PT_INDEX_BITS) - 1].as_ptr() as usize),
            true,
        );
        armKSGlobalKernelPDs[BIT!(PT_INDEX_BITS) - 1][BIT!(PT_INDEX_BITS) - 1] =
            pte_t::pte_next_table(kpptr_to_paddr(armKSGlobalKernelPT.as_ptr() as usize), true);
    }

    //FIXME:: map_kernel_window not implemented;
}

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

pub fn unmap_page_table(asid: asid_t, vaddr: vptr_t, pt: &mut pte_t) {
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
) -> pte_t {
    let uxn = attributes.get_armExecuteNever();
    if attributes.get_armPageCacheable() {
        return pte_t::pte_new(
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

    pte_t::pte_new(
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
