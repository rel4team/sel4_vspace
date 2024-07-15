use sel4_common::{
    arch::vm_rights_t,
    sel4_config::{ARM_Large_Page, ARM_Small_Page},
    utils::convert_to_mut_type_ref,
};
use sel4_cspace::arch::cap_t;

use crate::{arch::VAddr, asid_t, pptr_t, pptr_to_paddr, pte_t, vptr_t, PDE, PGDE, PTE, PUDE};

use super::page_slice;

#[derive(PartialEq, Eq, Debug)]
enum find_type {
    pde_t,
    pude_t,
    pte_t,
}

/// TODO: Write the comments.
#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_it_pt_cap(vspace_cap: &cap_t, pt_cap: &cap_t) {
    let vspace_root = vspace_cap.get_cap_ptr();
    let vptr = pt_cap.get_pt_mapped_address();
    let pt = pt_cap.get_pt_base_ptr();
    let target_pte =
        convert_to_mut_type_ref::<PDE>(find_pt(vspace_root, vptr.into(), find_type::pde_t));
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
    pud[vptr.pud_index()] = PUDE::new(pptr_to_paddr(pd_addr), 0x3);
}

/// TODO: Write the comments.
pub fn map_it_pud_cap(vspace_cap: &cap_t, pud_cap: &cap_t) {
    let pgd = page_slice::<PGDE>(vspace_cap.get_cap_ptr());
    let pud_addr = pud_cap.get_pud_base_ptr();
    let vptr: VAddr = pud_cap.get_pud_mapped_address().into();
    assert_eq!(pud_cap.get_pud_is_mapped(), 1);

    // TODO: move 0x3 into a proper position.
    pgd[vptr.pgd_index()] = PGDE::new(pptr_to_paddr(pud_addr), 0x3);
}

/// TODO: Write the comments.
#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_it_frame_cap(vspace_cap: &cap_t, frame_cap: &cap_t, exec: bool) {
    let pte = convert_to_mut_type_ref::<PTE>(find_pt(
        vspace_cap.get_cap_ptr(),
        frame_cap.get_frame_mapped_address().into(),
        find_type::pte_t,
    ));
    // TODO: Make set_attr usage more efficient.
    // TIPS: exec true will be cast to 1 and false to 0.
    pte.set_attr(pte_t::pte_new((!exec) as usize, 0, 1, 1, 0, 1, 0, 3).0);
    pte.set_next_level_paddr(pptr_to_paddr(frame_cap.get_frame_base_ptr()));
}

/// TODO: Write the comments.
#[link_section = ".boot.text"]
fn find_pt(vspace_root: usize, vptr: VAddr, ftype: find_type) -> usize {
    let pgd = page_slice::<PGDE>(vspace_root);
    let pud = pgd[vptr.pgd_index()].next_level_slice::<PUDE>();
    if ftype == find_type::pude_t {
        return pud[vptr.pud_index()].self_addr();
    }
    let pd = pud[vptr.pud_index()].next_level_slice::<PDE>();
    if ftype == find_type::pde_t {
        return pd[vptr.pd_index()].self_addr();
    }
    let pt = pd[vptr.pd_index()].next_level_slice::<PTE>();
    assert_eq!(ftype, find_type::pte_t);
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
