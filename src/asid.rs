use crate::findVSpaceForASID_ret;

use crate::asid_t;

#[no_mangle]
pub fn findVSpaceForASID(_asid: asid_t) -> findVSpaceForASID_ret {
    panic!("should not be invoked!")
}