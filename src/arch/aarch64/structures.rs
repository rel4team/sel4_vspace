use crate::{pte_t, vm_attributes_t, PDE, PGDE, PTE, PUDE};
use sel4_common::structures::exception_t;

pub type hw_asid_t = u8;

impl vm_attributes_t {
    pub fn get_armExecuteNever(&self) -> bool {
        if (self.0 & 0x4) != 0 {
            true
        } else {
            false
        }
    }

    pub fn get_armPageCacheable(&self) -> bool {
        if (self.0 & 0x1) != 0 {
            true
        } else {
            false
        }
    }
}

///lookup_pt_slot函数的返回值，
/// `ptSlot`：找到的虚地址对应的`pte`的存放槽
/// `ptBitsLeft`:找到叶子节点时，虚地址剩余未被索引的位置
#[repr(C)]
#[derive(Copy, Clone)]
pub struct lookupPTSlot_ret_t {
    pub status: exception_t,
    pub ptSlot: *mut PTE,
}

#[repr(C)]
pub struct lookupPGDSlot_ret_t {
    pub status: exception_t,
    pub pgdSlot: *mut PGDE, // *mut pgde_t
}

#[repr(C)]
pub struct lookupPDSlot_ret_t {
    pub status: exception_t,
    pub pdSlot: *mut PDE, // *mut pde_t
}

#[repr(C)]
pub struct lookupPUDSlot_ret_t {
    pub status: exception_t,
    pub pudSlot: *mut PUDE, // *mut pude_t
}
