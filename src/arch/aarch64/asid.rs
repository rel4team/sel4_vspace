use sel4_common::{
    fault::lookup_fault_t,
    sel4_config::{asidHighBits, asidLowBits, IT_ASID},
    structures::exception_t,
    utils::{convert_ref_type_to_usize, convert_to_mut_type_ref, convert_to_option_mut_type_ref},
    BIT, MASK,
};
use sel4_cspace::arch::cap_t;

use crate::{asid_map_t, asid_pool_t, asid_t, findVSpaceForASID_ret, set_vm_root, PTE};

use super::machine::invalidate_local_tlb_asid;

pub const asid_map_asid_map_none: usize = 0;
pub const asid_map_asid_map_vspace: usize = 1;

pub(crate) static mut armKSASIDTable: [usize; BIT!(asidHighBits)] = [0; BIT!(asidHighBits)];

#[no_mangle]
pub fn find_map_for_asid(asid: usize) -> Option<&'static asid_map_t> {
    let poolPtr = unsafe {
        convert_to_option_mut_type_ref::<asid_pool_t>(armKSASIDTable[asid >> asidLowBits])
    };
    if let Some(pool) = poolPtr {
        return Some(&pool.asid_map_slice()[asid & MASK!(asidLowBits)]);
    }
    None
}

#[no_mangle]
pub fn find_vspace_for_asid(asid: usize) -> findVSpaceForASID_ret {
    let mut ret: findVSpaceForASID_ret = findVSpaceForASID_ret {
        status: exception_t::EXCEPTION_LOOKUP_FAULT,
        vspace_root: None,
        lookup_fault: Some(lookup_fault_t::new_root_invalid()),
    };

    match find_map_for_asid(asid) {
        Some(asid_map) => {
            ret.vspace_root = Some(asid_map.get_vspace_root() as *mut PTE);
            ret.status = exception_t::EXCEPTION_NONE;
        }
        None => {}
    }
    ret
}

#[no_mangle]
pub fn delete_asid(asid: usize, vspace: *mut PTE, cap: &cap_t) -> Result<(), lookup_fault_t> {
    let ptr = unsafe {
        convert_to_option_mut_type_ref::<asid_pool_t>(armKSASIDTable[asid >> asidLowBits])
    };
    if let Some(pool) = ptr {
        let asid_map: asid_map_t = pool.asid_map_slice()[asid & MASK!(asidLowBits)];
        if asid_map.get_type() == asid_map_asid_map_vspace
            && asid_map.get_vspace_root() == vspace as usize
        {
            invalidate_local_tlb_asid(asid);
            pool.set_asid_map(asid & MASK!(asidLowBits), asid_map_t::new_none());
            return set_vm_root(cap);
        }
    }
    Ok(())
}

#[no_mangle]
pub fn delete_asid_pool(
    asid_base: asid_t,
    pool: *mut asid_pool_t,
    default_vspace_cap: &cap_t,
) -> Result<(), lookup_fault_t> {
    let pool_in_table = unsafe { armKSASIDTable[asid_base >> asidLowBits] };
    if pool as usize == pool_in_table {
        // clear all asid in target asid pool
        let pool = convert_to_mut_type_ref::<asid_pool_t>(pool_in_table);
        for offset in 0..BIT!(asidLowBits) {
            let asid_map = pool.get_asid_map(offset);
            if asid_map.get_type() == asid_map_asid_map_vspace {
                invalidate_local_tlb_asid(asid_base + offset);
            }
        }
        unsafe {
            armKSASIDTable[asid_base >> asidLowBits] = 0;
        }
        return set_vm_root(default_vspace_cap);
    }
    Ok(())
}

pub fn set_asid_pool_by_index(index: usize, pool_ptr: usize) {
    // assert!(index < BIT!(asidHighBits));
    unsafe {
        armKSASIDTable[index] = pool_ptr;
    }
}

#[no_mangle]
pub fn write_it_asid_pool(it_ap_cap: &cap_t, it_vspace_cap: &cap_t) {
    let ap = convert_to_mut_type_ref::<asid_pool_t>(it_ap_cap.get_asid_pool());
    let asid_map = asid_map_t::new_vspace(it_vspace_cap.get_pgd_base_ptr());
    ap.set_asid_map(IT_ASID, asid_map);
    unsafe {
        armKSASIDTable[IT_ASID >> asidLowBits] = convert_ref_type_to_usize(ap);
    }
}
