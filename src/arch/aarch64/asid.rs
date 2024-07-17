use sel4_common::{
    fault::lookup_fault_t,
    sel4_config::{asidHighBits, asidLowBits, IT_ASID},
    structures::exception_t,
    utils::convert_to_mut_type_ref,
    BIT, MASK,
};
use sel4_cspace::arch::cap_t;

use crate::{asid_map_t, asid_pool_t, asid_t, findVSpaceForASID_ret, set_vm_root, PGDE, PTE};

use super::machine::invalidate_local_tlb_asid;

pub const asid_map_asid_map_none: usize = 0;
pub const asid_map_asid_map_vspace: usize = 1;

pub(crate) static mut armKSASIDTable: [Option<asid_pool_t>; BIT!(asidHighBits)] =
    [None; BIT!(asidHighBits)];

#[inline]
fn get_asid_table() -> &'static mut [Option<asid_pool_t>] {
    unsafe { core::slice::from_raw_parts_mut(armKSASIDTable.as_mut_ptr(), BIT!(asidHighBits)) }
}

#[inline]
fn get_asid_pool_by_index(idx: usize) -> Option<&'static mut asid_pool_t> {
    log::debug!("in get_asid_pool_by_index");
    unsafe { armKSASIDTable.get_mut(idx).and_then(|pool| pool.as_mut()) }
}

#[inline]
pub fn set_asid_pool_by_index(idx: usize, val: Option<asid_pool_t>) {
    log::debug!("set_asid_pool_by_index");
    unsafe {
        armKSASIDTable[idx] = val;
    }
}

#[no_mangle]
pub fn find_map_for_asid(asid: usize) -> Option<&'static asid_map_t> {
    log::debug!("in find_map_for_asid");
    let poolPtr = get_asid_pool_by_index(asid >> asidLowBits);
    if let Some(pool) = poolPtr {
        return Some(&pool.get_asid_map(asid & MASK!(asidLowBits)));
    }
    None
}

#[no_mangle]
pub fn find_vspace_for_asid(asid: usize) -> findVSpaceForASID_ret {
    log::debug!("in find_vspace_for_asid asid:{}", asid);
    let mut ret: findVSpaceForASID_ret = findVSpaceForASID_ret {
        status: exception_t::EXCEPTION_LOOKUP_FAULT,
        vspace_root: None,
        lookup_fault: Some(lookup_fault_t::new_root_invalid()),
    };

    match find_map_for_asid(asid) {
        Some(asid_map) => {
            ret.vspace_root = Some(asid_map.get_vspace_root() as *mut PGDE);
            ret.status = exception_t::EXCEPTION_NONE;
            ret.lookup_fault = None;
            log::debug!("in find_vspace_for_asid , find vspace_root:{:#x}",asid_map.get_vspace_root());
        }
        None => {}
    }
    
    ret
}

#[no_mangle]
pub fn delete_asid(asid: usize, vspace: *mut PTE, cap: &cap_t) -> Result<(), lookup_fault_t> {
    log::debug!("in delete_asid");
    let ptr = get_asid_pool_by_index(asid >> asidLowBits);
    if let Some(pool) = ptr {
        let asid_map = pool.get_asid_map(asid & MASK!(asidLowBits));
        if asid_map.get_type() == asid_map_asid_map_vspace
            && asid_map.get_vspace_root() == vspace as usize
        {
            invalidate_local_tlb_asid(asid);
            pool.set_asid_map(asid & MASK!(asidLowBits), &asid_map_t::new_none());
            return set_vm_root(cap);
        }
    }
    Ok(())
}

#[no_mangle]
pub fn delete_asid_pool(
    asid_base: asid_t,
    pool: &mut asid_pool_t,
    default_vspace_cap: &cap_t,
) -> Result<(), lookup_fault_t> {
    log::debug!("in delete_asid_pool");
    let pool_in_table = get_asid_pool_by_index(asid_base >> asidLowBits).unwrap_or_else(|| {
        panic!("Invalid asid pool found");
    });
    if pool.array == pool_in_table.array {
        // clear all asid in target asid pool
        let pool = pool_in_table;
        for offset in 0..BIT!(asidLowBits) {
            let asid_map = pool.get_asid_map(offset);
            if asid_map.get_type() == asid_map_asid_map_vspace {
                invalidate_local_tlb_asid(asid_base + offset);
            }
        }
        set_asid_pool_by_index(asid_base >> asidLowBits, None);
        return set_vm_root(default_vspace_cap);
    }
    Ok(())
}

#[no_mangle]
#[inline]
pub fn write_it_asid_pool(it_ap_cap: &cap_t, it_vspace_cap: &cap_t) {
    log::debug!("in write_it_asid_pool");
    let ap = convert_to_mut_type_ref::<asid_pool_t>(it_ap_cap.get_asid_pool());
    let asid_map = asid_map_t::new_vspace(it_vspace_cap.get_pgd_base_ptr());
    log::info!(
        "debug:{:#x} asid_map:{:#x}",
        it_vspace_cap.get_pgd_base_ptr(),
        asid_map.get_vspace_root()
    );
    ap.set_asid_map(IT_ASID, &asid_map);
    set_asid_pool_by_index(IT_ASID >> asidLowBits, Some(ap.clone()));
}
