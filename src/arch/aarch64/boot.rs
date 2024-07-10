use sel4_common::utils::{convert_to_mut_type_ptr, convert_to_mut_type_ref};
use sel4_cspace::arch::cap_t;

use crate::{
    arch::aarch64::utils::GET_PGD_INDEX, paddr_to_pptr, pde_t, pgde_t, pptr_to_paddr, pude_t,
    GET_PD_INDEX, GET_UPUD_INDEX,
};

#[no_mangle]
pub fn map_it_pt_cap(_vspace_cap: &cap_t, _pt_cap: &cap_t) {
    let vspace_root = convert_to_mut_type_ptr::<pgde_t>(_vspace_cap.get_cap_ptr());
    let vptr = _pt_cap.get_pt_mapped_address();
    let pt = _pt_cap.get_cap_ptr();
    let target_pde = find_pde(vspace_root as usize, vptr);
    unsafe {
        *target_pde = pde_t::new_small(pptr_to_paddr(pt));
    }
}

pub fn find_pde(vspace_root: usize, vptr: usize) -> *mut pde_t {
    let vspace_root = convert_to_mut_type_ptr::<pgde_t>(vspace_root);
    unsafe {
        let _ = vspace_root.add(GET_PGD_INDEX(vptr));
        let pud = convert_to_mut_type_ptr::<pude_t>((*vspace_root).get_pud_base_address());
        let _ = pud.add(GET_UPUD_INDEX(vptr));
        let pd = convert_to_mut_type_ptr::<pde_t>(paddr_to_pptr((*pud).get_pud_base_address()));
        let target_pde = pd.add(GET_PD_INDEX(vptr));
        target_pde
    }
}
