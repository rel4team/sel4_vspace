use sel4_common::{
    arch::vm_rights_t,
    sel4_config::{ARM_Large_Page, ARM_Small_Page},
    utils::convert_to_mut_type_ptr,
};
use sel4_cspace::arch::cap_t;

use crate::{
    arch::aarch64::utils::GET_PGD_INDEX, asid_t, paddr_to_pptr, pde_t, pgde_t, pptr_t,
    pptr_to_paddr, pte_t, pude_t, vptr_t, GET_PD_INDEX, GET_PT_INDEX, GET_UPUD_INDEX,
};

#[derive(PartialEq, Eq)]
enum find_type {
    pde_t,
    pude_t,
    pte_t,
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_it_pt_cap(_vspace_cap: &cap_t, _pt_cap: &cap_t) {
    let vspace_root = convert_to_mut_type_ptr::<pgde_t>(_vspace_cap.get_cap_ptr());
    let vptr = _pt_cap.get_pt_mapped_address();
    let pt = _pt_cap.get_cap_ptr();
    let target_pde =
        convert_to_mut_type_ptr::<pde_t>(find_pt(vspace_root as usize, vptr, find_type::pde_t));
    unsafe {
        *target_pde = pde_t::new_small(pptr_to_paddr(pt));
    }
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_it_pd_cap(vspace_cap: &cap_t, pd_cap: &cap_t) {
    let vspace_root = convert_to_mut_type_ptr::<pgde_t>(vspace_cap.get_cap_ptr());
    let vptr = pd_cap.get_cap_ptr();
    let pd = pd_cap.get_pd_mapped_address();
    let pud =
        convert_to_mut_type_ptr::<pude_t>(find_pt(vspace_root as usize, vptr, find_type::pude_t));
    unsafe {
        *pud = pude_t::new_pd(pptr_to_paddr(pd));
    }
}

#[no_mangle]
#[link_section = ".boot.text"]
pub fn map_it_frame_cap(vspace_cap: &cap_t, frame_cap: &cap_t, exec: bool) {
    let vspace_root = convert_to_mut_type_ptr::<pgde_t>(vspace_cap.get_cap_ptr());
    let vptr = frame_cap.get_frame_mapped_address();
    let pptr = frame_cap.get_frame_base_ptr();
    let pt =
        convert_to_mut_type_ptr::<pte_t>(find_pt(vspace_root as usize, vptr, find_type::pte_t));
    unsafe {
        *(pt) = pte_t::pte_new(
            (!exec) as usize,
            pptr_to_paddr(pptr),
            1,
            1,
            0,
            pte_t::ap_from_vm_rights_t(vm_rights_t::VMReadWrite).bits() >> 6,
            0x4,
            0x3,
        );
    }
}

//     *(pt + GET_PT_INDEX(vptr)) = pte_new(
//                                      !executable,                    /* unprivileged execute never */
//                                      pptr_to_paddr(pptr),            /* page_base_address    */
//                                      1,                              /* not global */
//                                      1,                              /* access flag */
//                                      SMP_TERNARY(SMP_SHARE, 0),              /* Inner-shareable if SMP enabled, otherwise unshared */
//                                      APFromVMRights(VMReadWrite),
//                                      NORMAL,
//                                      RESERVED
//                                  );

#[link_section = ".boot.text"]
fn find_pt(vspace_root: usize, vptr: usize, type_: find_type) -> usize {
    let vspace_root = convert_to_mut_type_ptr::<pgde_t>(vspace_root);
    unsafe {
        let _ = vspace_root.add(GET_PGD_INDEX(vptr));
        let pud = convert_to_mut_type_ptr::<pude_t>((*vspace_root).get_pud_base_address());
        let _ = pud.add(GET_UPUD_INDEX(vptr));
        if type_ == find_type::pude_t {
            return pud as usize;
        }
        let pd = convert_to_mut_type_ptr::<pde_t>(paddr_to_pptr((*pud).get_pud_base_address()));
        if type_ == find_type::pde_t {
            let target_pde = pd.add(GET_PD_INDEX(vptr));
            return target_pde as usize;
        }
        let pt = convert_to_mut_type_ptr::<pte_t>(paddr_to_pptr((*pd).get_pud_base_address()));
        if type_ == find_type::pte_t {
            let target_pte = pt.add(GET_PT_INDEX(vptr));
            return target_pte as usize;
        }
        0
    }
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
