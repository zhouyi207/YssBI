use super::PinKind;
use super::PinRole;
use super::PinDataType;

pub struct PinDynamicSpec {
    pub role: PinRole,
    pub kind: PinKind,
    pub data_type: PinDataType,
    pub name: String,
}
