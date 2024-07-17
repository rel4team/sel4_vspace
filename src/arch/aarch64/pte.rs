use core::intrinsics::unlikely;

use crate::{
    arch::aarch64::{
        machine::{clean_by_va_pou, invalidate_local_tlb_asid},
        structures::{
            lookupPDSlot_ret_t, lookupPGDSlot_ret_t, lookupPTSlot_ret_t, lookupPUDSlot_ret_t,
        },
        utils::{GET_PD_INDEX, GET_PGD_INDEX, GET_PT_INDEX, GET_UPUD_INDEX},
    },
    asid_t, find_vspace_for_asid, lookupFrame_ret_t, pptr_to_paddr, vm_attributes_t, vptr_t, PDE,
    PGDE, PTE, PUDE,
};
use sel4_common::{
    arch::vm_rights_t,
    fault::lookup_fault_t,
    sel4_config::{
        seL4_PageBits, seL4_PageTableBits, ARM_Huge_Page, ARM_Large_Page, ARM_Small_Page,
        PT_INDEX_BITS,
    },
    structures::exception_t,
    utils::{convert_ref_type_to_usize, convert_to_mut_type_ref, convert_to_type_ref},
    BIT,
};

use super::utils::{paddr_to_pptr, GET_UPT_INDEX};

pub enum vm_page_size {
    ARMSmallPage,
    ARMLargePage,
    ARMHugePage,
}

enum pte_tag_t {
    pte_table = 3,
    pte_page = 1,
    pte_4k_page = 7,
    pte_invalid = 0,
}


pub const UPT_LEVELS: usize = 4;
bitflags::bitflags! {
    /// Possible flags for a page table entry.
    pub struct PTEFlags: usize {
        // Attribute fields in stage 1 VMSAv8-64 Block and Page descriptors:
        /// Whether the descriptor is valid.
        const VALID =       BIT!(0);
        /// The descriptor gives the address of the next level of translation table or 4KB page.
        /// (not a 2M, 1G block)
        const NON_BLOCK =   BIT!(1);
        /// Memory attributes index field.
        const ATTR_INDX =   0b111 << 2;
        const NORMAL_NONCACHE = 0b010 << 2;
        const NORMAL =      0b100 << 2;
        /// Non-secure bit. For memory accesses from Secure state, specifies whether the output
        /// address is in Secure or Non-secure memory.
        const NS =          BIT!(5);
        /// Access permission: accessable at EL0.
        const AP_EL0 =      BIT!(6);
        /// Access permission: read-only.
        const AP_RO =       BIT!(7);
        /// Shareability: Inner Shareable (otherwise Outer Shareable).
        const INNER =       BIT!(8);
        /// Shareability: Inner or Outer Shareable (otherwise Non-shareable).
        const SHAREABLE =   BIT!(9);
        /// The Access flag.
        const AF =          BIT!(10);
        /// The not global bit.
        const NG =          BIT!(11);
        /// Indicates that 16 adjacent translation table entries point to contiguous memory regions.
        const CONTIGUOUS =  BIT!(52);
        /// The Privileged execute-never field.
        const PXN =         BIT!(53);
        /// The Execute-never or Unprivileged execute-never field.
        const UXN =         BIT!(54);

        // Next-level attributes in stage 1 VMSAv8-64 Table descriptors:

        /// PXN limit for subsequent levels of lookup.
        const PXN_TABLE =           BIT!(59);
        /// XN limit for subsequent levels of lookup.
        const XN_TABLE =            BIT!(60);
        /// Access permissions limit for subsequent levels of lookup: access at EL0 not permitted.
        const AP_NO_EL0_TABLE =     BIT!(61);
        /// Access permissions limit for subsequent levels of lookup: write access not permitted.
        const AP_NO_WRITE_TABLE =   BIT!(62);
        /// For memory accesses from Secure state, specifies the Security state for subsequent
        /// levels of lookup.
        const NS_TABLE =            BIT!(63);

    }
}

impl PTE {
    pub fn new(addr: usize, flags: PTEFlags) -> Self {
        Self((addr & 0xfffffffff000) | flags.bits())
    }
    pub fn pte_next_table(addr: usize, _: bool) -> Self {
        Self::new(addr, PTEFlags::VALID | PTEFlags::NON_BLOCK)
    }
    fn new_4k_page(addr: usize, flags: PTEFlags) -> Self {
        Self((addr & 0xfffffffff000) | flags.bits() | 0x400000000000003)
    }

    pub fn get_pte_from_ppn_mut(&self) -> &mut PTE {
        convert_to_mut_type_ref::<PTE>(paddr_to_pptr(self.get_ppn() << seL4_PageTableBits))
    }

    pub fn get_ppn(&self) -> usize {
        (self.0 & 0xfffffffff000) >> 10
    }

    pub fn as_pgde(&self) -> PGDE {
        PGDE::new_from_pte(self.0)
    }

    pub fn as_pude(&self) -> PUDE {
        PUDE::new_from_pte(self.0)
    }

    pub fn as_pde(&self) -> PDE {
        PDE::new_from_pte(self.0)
    }

    pub fn is_pte_table(&self) -> bool {
        self.get_type() != pte_tag_t::pte_table as usize
    }
    pub fn get_valid(&self) -> usize {
        (self.get_type() != pte_tag_t::pte_invalid as usize) as usize
    }

    pub fn pte_table_get_present(&self) -> bool {
        self.get_type() != pte_tag_t::pte_table as usize
    }

    pub fn new_invalid() -> Self {
        Self::new(0, PTEFlags::empty())
    }

    #[inline]
    pub fn update(&mut self, pte: Self) {
        *self = pte;
        clean_by_va_pou(
            convert_ref_type_to_usize(self),
            convert_ref_type_to_usize(self),
        );
    }

    pub fn ap_from_vm_rights_t(rights: vm_rights_t) -> PTEFlags {
        match rights {
            vm_rights_t::VMKernelOnly => PTEFlags::empty(),
            vm_rights_t::VMReadWrite => PTEFlags::AP_EL0,
            vm_rights_t::VMReadOnly => PTEFlags::AP_EL0 | PTEFlags::AP_RO,
        }
    }

    pub fn make_user_pte(
        paddr: usize,
        rights: vm_rights_t,
        attr: vm_attributes_t,
        page_size: usize,
    ) -> Self {
        let nonexecutable = attr.get_armExecuteNever();
        let cacheable = attr.get_armPageCacheable();
        let mut flags = PTEFlags::NG;
        if cacheable {
            flags |= PTEFlags::NORMAL;
        }
        if nonexecutable {
            flags |= PTEFlags::UXN;
        }
        flags |= Self::ap_from_vm_rights_t(rights);
        if vm_page_size::ARMSmallPage as usize == page_size {
            PTE::new_4k_page(paddr, flags)
        } else {
            PTE::new(paddr, flags)
        }
    }

    pub fn unmap_page_table(&mut self, asid: asid_t, vptr: vptr_t) {
        // let target_pt = self as *mut PTE;
        // let find_ret = find_vspace_for_asid(asid);
        // if unlikely(find_ret.status != exception_t::EXCEPTION_NONE) {
        //     return;
        // }
        // let pt: *mut PTE = find_ret.vspace_root.unwrap();
        // let mut ptSlot = unsafe { &mut *(pt.add(GET_UPT_INDEX(vptr, 0))) };
        // assert_ne!(find_ret.vspace_root.unwrap(), target_pt);
        // for i in 0..UPT_LEVELS - 1 {
        //     if pt == target_pt {
        //         break;
        //     }
        //     ptSlot = unsafe { &mut *(pt.add(GET_UPT_INDEX(vptr, i))) };
        //     if unlikely(ptSlot.pte_table_get_present()) {
        //         return;
        //     }
        // }

        // if pt != target_pt {
        //     return;
        // }
        // *ptSlot = PTE::new_invalid();
        // invalidate_local_tlb_asid(asid);
        // clean_by_va_pou(
        //     convert_ref_type_to_usize(ptSlot),
        //     pptr_to_paddr(convert_ref_type_to_usize(ptSlot)),
        // )
    }

    

    pub fn pte_new(
        UXN: usize,
        page_base_address: usize,
        nG: usize,
        AF: usize,
        SH: usize,
        AP: usize,
        AttrIndx: usize,
        reserved: usize,
    ) -> PTE {
        let val = 0
            | (UXN & 0x1) << 54
            | (page_base_address & 0xfffffffff000) >> 0
            | (nG & 0x1) << 11
            | (AF & 0x1) << 10
            | (SH & 0x3) << 8
            | (AP & 0x3) << 6
            | (AttrIndx & 0x7) << 2
            | (reserved & 0x3) << 0;

        PTE(val)
    }
}
