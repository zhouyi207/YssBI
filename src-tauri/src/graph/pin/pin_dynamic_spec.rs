use super::PinKind;
use super::PinRole;
use super::PinTypeDesc;

pub struct PinDynamicSpec {
    pub role: PinRole,
    pub kind: PinKind,
    pub type_desc: PinTypeDesc,
    pub name: String,
}
