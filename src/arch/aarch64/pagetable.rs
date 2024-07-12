use sel4_cspace::arch::cap_t;

use crate::{map_it_pud_cap, pptr_t, vptr_t, PageTable};
impl PageTable {
    pub(crate) const PTE_NUM_IN_PAGE: usize = 0x200;
}

/// Create a new pud cap in the vspace.
///
/// vptr is the virtual address of the pud cap will be created
/// pptr is the address to the physical address will be mapped
#[no_mangle]
#[link_section = ".boot.text"]
pub fn create_it_pud_cap(vspace_cap: &cap_t, pptr: pptr_t, vptr: vptr_t, asid: usize) -> cap_t {
    let cap = cap_t::new_page_upper_directory_cap(asid, pptr, 1, vptr);
    map_it_pud_cap(vspace_cap, &cap);
    return cap;
}
