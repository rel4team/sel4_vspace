use sel4_common::{
    fault::{lookup_fault_invalid_root, lookup_fault_t},
    sel4_config::{asidHighBits, asidLowBits},
    structures::exception_t,
    BIT, MASK,
};

use crate::{asid_map_t, asid_pool_t, findVSpaceForASID_ret, pte_t};

pub const asid_map_asid_map_none: usize = 0;
pub const asid_map_asid_map_vspace: usize = 1;

pub(crate) const armKSASIDTable: [*mut asid_pool_t; BIT!(asidHighBits)] =
    [0 as *mut asid_pool_t; BIT!(asidHighBits)];

#[no_mangle]
pub fn find_map_for_asid(asid: usize) -> asid_map_t {
    let poolPtr = unsafe { armKSASIDTable[asid >> asidLowBits] };
    if poolPtr as usize == 0 {
        return asid_map_t::new_none();
    }
    unsafe { (*poolPtr).array[asid & MASK!(asidLowBits)] }
}

#[no_mangle]
pub fn find_vspace_for_asid(asid: usize) -> findVSpaceForASID_ret {
    let asid_map = find_map_for_asid(asid);
    let mut ret: findVSpaceForASID_ret = findVSpaceForASID_ret {
        status: exception_t::EXCEPTION_FAULT,
        vspace_root: None,
        lookup_fault: None,
    };
    if asid_map.get_type() != asid_map_asid_map_vspace {
        ret.lookup_fault = Some(lookup_fault_t::new_root_invalid());
        ret.status = exception_t::EXCEPTION_LOOKUP_FAULT;
    }
    //FIXME::this pte_t should be pgde_t;
    ret.vspace_root = Some(asid_map.get_vspace_root() as *mut pte_t);
    ret.status = exception_t::EXCEPTION_NONE;
    ret
}
