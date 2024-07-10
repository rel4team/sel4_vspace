use sel4_common::{sel4_config::seL4_PageBits, utils::convert_to_mut_type_ref};
use sel4_cspace::arch::cap_t;

use crate::{pptr_to_paddr, pte_t, sfence, PTEFlags};

#[no_mangle]
pub fn map_it_pt_cap(_vspace_cap: &cap_t, _pt_cap: &cap_t) {
    let vptr = _pt_cap.get_pt_mapped_address();
    let lvl1pt = convert_to_mut_type_ref::<pte_t>(_vspace_cap.get_cap_ptr());
    let pt = _pt_cap.get_cap_ptr();
    let pt_ret = lvl1pt.lookup_pt_slot(vptr);
    let targetSlot = convert_to_mut_type_ref::<pte_t>(pt_ret.ptSlot as usize);
    *targetSlot = pte_t::new(pptr_to_paddr(pt) >> seL4_PageBits, PTEFlags::V);
    sfence();
}
