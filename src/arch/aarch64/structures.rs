use crate::{pte_t, vm_attributes_t};
use sel4_common::{plus_define_bitfield, sel4_config::asidLowBits, structures::exception_t, BIT};

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
    pub ptSlot: *mut pte_t,
}

#[repr(C)]
pub struct lookupPGDSlot_ret_t {
    pub status: exception_t,
    pub pgdSlot: *mut pte_t, // *mut pgde_t
}

#[repr(C)]
pub struct lookupPDSlot_ret_t {
    pub status: exception_t,
    pub pdSlot: *mut pte_t, // *mut pde_t
}

#[repr(C)]
pub struct lookupPUDSlot_ret_t {
    pub status: exception_t,
    pub pudSlot: *mut pte_t, // *mut pude_t
}

/// 用于存放`asid`对应的根页表基址，是一个`usize`的数组，其中`asid`按低`asidLowBits`位进行索引
#[derive(Copy, Clone)]
pub struct asid_pool_t {
    pub array: [asid_map_t; BIT!(asidLowBits)],
}

plus_define_bitfield! {
    pgde_t, 1, 0, 0, 0 => {
        new_pud, 0 => {
            pud_base_address, get_pud_base_address, set_pud_base_address, 0, 12, 36, 0, false
        }
    }
}

plus_define_bitfield! {
    pude_t, 1, 0, 0, 0 => {
        new_pd, 0 => {
            pud_base_address, get_pud_base_address, set_pud_base_address, 0, 12, 36, 0, false
        }
    }
}

plus_define_bitfield! {
    pde_t, 1, 0, 0, 0 => {
        new_small, 0 => {
            pud_base_address, get_pud_base_address, set_pud_base_address, 0, 12, 36, 0, false
        }
    }
}

plus_define_bitfield! {
    asid_map_t, 1, 0, 0, 1 => {
        new_none, 0 => {},
        new_vspace, 0 => {
            vspace_root , get_vspace_root , set_vspace_root , 0, 12, 36, 0 ,true
        }
    }
}