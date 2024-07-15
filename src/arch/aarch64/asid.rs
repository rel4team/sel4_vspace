use sel4_common::{
    fault::{lookup_fault_invalid_root, lookup_fault_t},
    sel4_config::{asidHighBits, asidLowBits},
    structures::exception_t,
    utils::{convert_to_mut_type_ref, convert_to_option_mut_type_ref},
    BIT, MASK,
};
use sel4_cspace::arch::cap_t;

use crate::{asid_map_t, asid_pool_t, findVSpaceForASID_ret, PTE};

pub const asid_map_asid_map_none: usize = 0;
pub const asid_map_asid_map_vspace: usize = 1;

pub(crate) const armKSASIDTable: [usize; BIT!(asidHighBits)] = [0; BIT!(asidHighBits)];

#[no_mangle]
pub fn find_map_for_asid(asid: usize) -> asid_map_t {
    let poolPtr =
        convert_to_option_mut_type_ref::<asid_pool_t>(armKSASIDTable[asid >> asidLowBits]);
    if let Some(pool) = poolPtr {
        return pool.asid_map_slice()[asid & MASK!(asidLowBits)];
    }
    asid_map_t::new_none()
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
    //FIXME::this PTE should be pgde_t;
    ret.vspace_root = Some(asid_map.get_vspace_root() as *mut PTE);
    ret.status = exception_t::EXCEPTION_NONE;
    ret
}

#[no_mangle]
pub fn delete_asid(asid: usize, vspace: *mut PTE, cap: &cap_t)->Result<(), lookup_fault_t> {
    let ptr = convert_to_option_mut_type_ref::<asid_pool_t>(armKSASIDTable[asid >> asidLowBits]);
    if let Some(pool) = ptr {
        // let asid_map = convert_to_mut_type_ref()
        let asid_map: asid_map_t = pool.asid_map_slice()[asid & MASK!(asidLowBits)];
        if asid_map.get_type() == asid_map_asid_map_vspace {}
    }
    Ok(())
}
// void deleteASID(asid_t asid, vspace_root_t *vspace)
// {
//     asid_pool_t *poolPtr;

//     poolPtr = armKSASIDTable[asid >> asidLowBits];

//     if (poolPtr != NULL) {
//         asid_map_t asid_map = poolPtr->array[asid & MASK(asidLowBits)];
//         if (asid_map_get_type(asid_map) == asid_map_asid_map_vspace &&
//             (vspace_root_t *)asid_map_asid_map_vspace_get_vspace_root(asid_map) == vspace) {
//             invalidateTLBByASID(asid);
//             poolPtr->array[asid & MASK(asidLowBits)] = asid_map_asid_map_none_new();
//             setVMRoot(NODE_STATE(ksCurThread));
//         }
//     }
// }
