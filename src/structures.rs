/// 在`PSpace`段的虚拟地址空间中的指针
/// 
/// Virtual pointer used in PSpace
pub type pptr_t = usize;
/// 物理地址空间指针
/// 
/// Physical pointer
pub type paddr_t = usize;
/// 用戶地址地址空间中虚拟的指针
/// 
/// Virtual pointer in user space
pub type vptr_t = usize;

/// 进行系统调用时，应用程序向内核传递信息的消息格式
/// 
/// vm_attributes_t is a message type. When program pass message to kernel , it uses vm_attributes_t.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct vm_attributes_t {
    pub words: [usize; 1],
}


impl vm_attributes_t {
    pub fn new(value: usize) -> Self {
        Self {
            words: [value & 0x1usize],
        }
    }

    pub fn from_word(w: usize) -> Self {
        Self {
            words: [w]
        }
    }

    pub fn get_execute_never(&self) -> usize {
        self.words[0] & 0x1usize
    }

    pub fn set_execute_never(&mut self, v64: usize) {
        self.words[0] &= !0x1usize;
        self.words[0] |= (v64 << 0) & 0x1usize;
    }
}