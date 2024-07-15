use sel4_common::{
    arch::{
        config::{PADDR_BASE, PADDR_TOP, PPTR_BASE, PPTR_TOP},
        vm_rights_t,
    },
    sel4_config::{seL4_LargePageBits, ARM_Large_Page, ARM_Small_Page, PT_INDEX_BITS},
    utils::convert_to_mut_type_ref,
    BIT,
};
use sel4_cspace::arch::cap_t;

use crate::{
    arch::VAddr, asid_t, kpptr_to_paddr, pptr_t, pptr_to_paddr, vm_attributes_t, vptr_t, PTEFlags,
    GET_KPT_INDEX, GET_PT_INDEX, PDE, PGDE, PTE, PUDE,
};

use super::interface::{
    armKSGlobalKernelPDs, armKSGlobalKernelPGD, armKSGlobalKernelPT, armKSGlobalKernelPUD,
};

use super::page_slice;

#[derive(PartialEq, Eq, Debug)]
enum find_type {
    PDE,
    PUDE,
    PTE,
}

/// TODO: Write the comments.
pub(crate) enum mair_types {
    DEVICE_nGnRnE = 0,
    DEVICE_nGnRE = 1,
    DEVICE_GRE = 2,
    NORMAL_NC = 3,
    NORMAL = 4,
    NORMAL_WT = 5,
}

pub const RESERVED: usize = 3;

#[no_mangle]
#[link_section = ".boot.text"]
pub fn rust_map_kernel_window() {
    unsafe {
        armKSGlobalKernelPGD[GET_KPT_INDEX(PPTR_BASE, 0)] =
            PTE::pte_next_table(kpptr_to_paddr(armKSGlobalKernelPUD.as_ptr() as usize), true);
    }

    let mut idx = GET_KPT_INDEX(PPTR_BASE, 1);
    while idx < GET_KPT_INDEX(PPTR_TOP, 1) {
        unsafe {
            armKSGlobalKernelPUD[idx] = PTE::pte_next_table(
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
                PTE::new(paddr, flag);
            vaddr += BIT!(seL4_LargePageBits);
            paddr += BIT!(seL4_LargePageBits)
        }
    }

    unsafe {
        armKSGlobalKernelPUD[GET_KPT_INDEX(PPTR_TOP, 1)] = PTE::pte_next_table(
            kpptr_to_paddr(armKSGlobalKernelPDs[BIT!(PT_INDEX_BITS) - 1].as_ptr() as usize),
            true,
        );
        armKSGlobalKernelPDs[BIT!(PT_INDEX_BITS) - 1][BIT!(PT_INDEX_BITS) - 1] =
            PTE::pte_next_table(kpptr_to_paddr(armKSGlobalKernelPT.as_ptr() as usize), true);
    }
}

#[no_mangle]
pub fn map_kernel_frame(
    paddr: usize,
    vaddr: usize,
    vm_rights: vm_rights_t,
    attributes: vm_attributes_t,
) {
    let uxn = 1;
    let attr_index: usize;
    let shareable: usize;
    if attributes.get_page_cacheable() != 0 {
        attr_index = mair_types::NORMAL as usize;
        shareable = 0;
    } else {
        attr_index = mair_types::DEVICE_nGnRnE as usize;
        shareable = 0;
    }
    unsafe {
        armKSGlobalKernelPT[GET_PT_INDEX(vaddr)] = PTE::pte_new(
            uxn,
            paddr,
            0,
            1,
            shareable,
            PTE::ap_from_vm_rights_t(vm_rights).bits() >> 6,
            attr_index,
            RESERVED,
        )
    }
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_it_pt_cap(vspace_cap: &cap_t, pt_cap: &cap_t) {
    let vspace_root = vspace_cap.get_cap_ptr();
    let vptr = pt_cap.get_pt_mapped_address();
    let pt = pt_cap.get_pt_base_ptr();
    let target_pte =
        convert_to_mut_type_ref::<PDE>(find_pt(vspace_root, vptr.into(), find_type::PDE));
    target_pte.set_next_level_paddr(pptr_to_paddr(pt));
    // TODO: move 0x3 into a proper position.
    target_pte.set_attr(3);
}

/// TODO: Write the comments.
#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_it_pd_cap(vspace_cap: &cap_t, pd_cap: &cap_t) {
    let pgd = page_slice::<PGDE>(vspace_cap.get_cap_ptr());
    let pd_addr = pd_cap.get_pd_base_ptr();
    let vptr: VAddr = pd_cap.get_pd_mapped_address().into();
    assert_eq!(pd_cap.get_pd_is_mapped(), 1);
    // TODO: move 0x3 into a proper position.
    assert_eq!(pgd[vptr.pgd_index()].attr(), 0x3);
    let pud = pgd[vptr.pgd_index()].next_level_slice::<PUDE>();
    pud[vptr.pud_index()] = PUDE::new_page(pptr_to_paddr(pd_addr), 0x3);
}

/// TODO: Write the comments.
pub fn map_it_pud_cap(vspace_cap: &cap_t, pud_cap: &cap_t) {
    let pgd = page_slice::<PGDE>(vspace_cap.get_cap_ptr());
    let pud_addr = pud_cap.get_pud_base_ptr();
    let vptr: VAddr = pud_cap.get_pud_mapped_address().into();
    assert_eq!(pud_cap.get_pud_is_mapped(), 1);

    // TODO: move 0x3 into a proper position.
    pgd[vptr.pgd_index()] = PGDE::new_page(pptr_to_paddr(pud_addr), 0x3);
}

/// TODO: Write the comments.
#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_it_frame_cap(vspace_cap: &cap_t, frame_cap: &cap_t, exec: bool) {
    let pte = convert_to_mut_type_ref::<PTE>(find_pt(
        vspace_cap.get_cap_ptr(),
        frame_cap.get_frame_mapped_address().into(),
        find_type::PTE,
    ));
    // TODO: Make set_attr usage more efficient.
    // TIPS: exec true will be cast to 1 and false to 0.
    pte.set_attr(PTE::pte_new((!exec) as usize, 0, 1, 1, 0, 1, 0, 3).0);
    pte.set_next_level_paddr(pptr_to_paddr(frame_cap.get_frame_base_ptr()));
}

/// TODO: Write the comments.
#[link_section = ".boot.text"]
fn find_pt(vspace_root: usize, vptr: VAddr, ftype: find_type) -> usize {
    let pgd = page_slice::<PGDE>(vspace_root);
    let pud = pgd[vptr.pgd_index()].next_level_slice::<PUDE>();
    if ftype == find_type::PUDE {
        return pud[vptr.pud_index()].self_addr();
    }
    let pd = pud[vptr.pud_index()].next_level_slice::<PDE>();
    if ftype == find_type::PDE {
        return pd[vptr.pd_index()].self_addr();
    }
    let pt = pd[vptr.pd_index()].next_level_slice::<PTE>();
    assert_eq!(ftype, find_type::PTE);
    pt[vptr.pt_index()].self_addr()
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn create_it_pd_cap(vspace_cap: &cap_t, pptr: usize, vptr: usize, asid: usize) -> cap_t {
    let cap = cap_t::new_page_directory_cap(asid, pptr, 1, vptr);
    map_it_pd_cap(vspace_cap, &cap);
    return cap;
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn create_unmapped_it_frame_cap(pptr: pptr_t, use_large: bool) -> cap_t {
    return create_it_frame_cap(pptr, 0, 0, use_large);
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn create_it_frame_cap(pptr: pptr_t, vptr: vptr_t, asid: asid_t, use_large: bool) -> cap_t {
    let frame_size;
    if use_large {
        frame_size = ARM_Large_Page;
    } else {
        frame_size = ARM_Small_Page;
    }
    cap_t::new_frame_cap(
        0,
        vm_rights_t::VMReadWrite as usize,
        vptr,
        frame_size,
        asid,
        pptr,
    )
}

#[no_mangle]
pub fn create_mapped_it_frame_cap(
    pd_cap: &cap_t,
    pptr: usize,
    vptr: usize,
    asid: usize,
    use_large: bool,
    exec: bool,
) -> cap_t {
    let cap = create_it_frame_cap(pptr, vptr, asid, use_large);
    map_it_frame_cap(pd_cap, &cap, exec);
    cap
}
