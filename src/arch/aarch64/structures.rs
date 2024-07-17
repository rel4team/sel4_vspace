use crate::{impl_multi, vm_attributes_t, PDE, PGDE, PTE, PUDE};
use sel4_common::{
    plus_define_bitfield, sel4_config::asidLowBits, structures::exception_t,
    utils::convert_to_mut_slice, BIT,
};

use super::machine::mair_types;

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

    pub fn get_attr_index(&self) -> mair_types {
        if self.get_armPageCacheable() {
            return mair_types::NORMAL;
        }

        mair_types::DEVICE_nGnRnE
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

#[repr(C)]
pub struct lookupFrame_ret_t {
    pub frameBase: usize,
    pub frameSize: usize,
    pub valid: bool,
}

/// 用于存放`asid`对应的根页表基址，是一个`usize`的数组，其中`asid`按低`asidLowBits`位进行索引
#[repr(C)]
#[derive(Copy, Clone)]
// pub struct asid_pool_t {
//     pub array: [asid_map_t; BIT!(asidLowBits)],
// }
pub struct asid_pool_t {
    pub array: [asid_map_t; BIT!(asidLowBits)],
}

impl asid_pool_t {
    pub fn new() -> Self {
        Self {
            array: [asid_map_t::from(0); BIT!(asidLowBits)],
        }
    }
}

impl From<usize> for asid_map_t {
    fn from(val: usize) -> Self {
        Self { words: [val; 1] }
    }
}

impl_multi!(asid_pool_t {
    #[inline]
    pub fn get_asid_map(&self, idx: usize) -> & asid_map_t {
        log::debug!("in get_asid_map");
        &self.array[idx]
    }

    #[inline]
    pub fn set_asid_map(&mut self, idx: usize, val: &asid_map_t) {
        log::debug!("in set_asid_map");
        self.array[idx] = val.clone();
    }
});

plus_define_bitfield! {
    pgde_t, 1, 0, 0, 0 => {
        new_pud, 0 => {
            pud_base_address, get_pud_base_address, set_pud_base_address, 0, 0, 48, 0, false
        }
    }
}

plus_define_bitfield! {
    pude_t, 1, 0, 0, 0 => {
        new_pd, 0 => {
            pd_base_address, get_pd_base_address, set_pd_base_address, 0, 0, 48, 0, false
        }
    }
}

plus_define_bitfield! {
    pde_t, 1, 0, 0, 0 => {
        new_small, 0 => {
            pud_base_address, get_pt_base_address, set_pt_base_address, 0, 0, 48, 0, false
        }
    }
}

plus_define_bitfield! {
    asid_map_t, 1, 0, 0, 1 => {
        new_none, 0 => {},
        new_vspace, 0 => {
            vspace_root , get_vspace_root , set_vspace_root , 0, 0, 36, 12 ,true
        }
    }
}
