use sel4_common::{
    arch::vm_rights_t,
    sel4_config::{seL4_PageBits, RISCVMegaPageBits, RISCVPageBits},
    utils::convert_to_mut_type_ref,
};
use sel4_cspace::arch::cap_t;

use crate::{pptr_t, pptr_to_paddr, PTE, sfence, PTEFlags};

#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_it_pt_cap(_vspace_cap: &cap_t, _pt_cap: &cap_t) {
    let vptr = _pt_cap.get_pt_mapped_address();
    let lvl1pt = convert_to_mut_type_ref::<PTE>(_vspace_cap.get_cap_ptr());
    let pt = _pt_cap.get_cap_ptr();
    let pt_ret = lvl1pt.lookup_pt_slot(vptr);
    let targetSlot = convert_to_mut_type_ref::<PTE>(pt_ret.ptSlot as usize);
    *targetSlot = PTE::new(pptr_to_paddr(pt) >> seL4_PageBits, PTEFlags::V);
    sfence();
}

#[no_mangle]
pub fn map_it_frame_cap(_vspace_cap: &cap_t, _frame_cap: &cap_t) {
    let vptr = _frame_cap.get_frame_mapped_address();
    let lvl1pt = convert_to_mut_type_ref::<PTE>(_vspace_cap.get_cap_ptr());
    let frame_pptr: usize = _frame_cap.get_cap_ptr();
    let pt_ret = lvl1pt.lookup_pt_slot(vptr);

    let targetSlot = convert_to_mut_type_ref::<PTE>(pt_ret.ptSlot as usize);

    *targetSlot = PTE::new(
        pptr_to_paddr(frame_pptr) >> seL4_PageBits,
        PTEFlags::ADUVRWX,
    );
    sfence();
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn create_mapped_it_frame_cap(
    pd_cap: &cap_t,
    pptr: usize,
    vptr: usize,
    asid: usize,
    use_large: bool,
    _exec: bool,
) -> cap_t {
    let frame_size: usize;
    if use_large {
        frame_size = RISCVMegaPageBits;
    } else {
        frame_size = RISCVPageBits;
    }
    let cap = cap_t::new_frame_cap(
        asid,
        pptr,
        frame_size,
        vm_rights_t::VMReadWrite as usize,
        0,
        vptr,
    );
    map_it_frame_cap(pd_cap, &cap);
    cap
}

pub fn create_unmapped_it_frame_cap(pptr: pptr_t, _use_large: bool) -> cap_t {
    cap_t::new_frame_cap(0, pptr, 0, 0, 0, 0)
}
