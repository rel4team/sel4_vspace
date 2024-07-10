use sel4_cspace::arch::cap_t;

use crate::{map_it_pt_cap, pptr_t, vptr_t};

#[no_mangle]
#[link_section = ".boot.text"]
pub fn create_it_pt_cap(vspace_cap: &cap_t, pptr: pptr_t, vptr: vptr_t, asid: usize) -> cap_t {
    let cap = cap_t::new_page_table_cap(asid, pptr, 1, vptr);
    map_it_pt_cap(vspace_cap, &cap);
    return cap;
}
