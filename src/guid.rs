use std::fmt::Display;
use windows::core::GUID;

pub trait GuidExt {
    fn display(&self) -> GuidDisplay;
}

impl GuidExt for GUID {
    fn display(&self) -> GuidDisplay {
        GuidDisplay(self)
    }
}

pub struct GuidDisplay<'a>(&'a GUID);

impl Display for GuidDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let guid = self.0;
        write!(
            f,
            "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
            guid.data1,
            guid.data2,
            guid.data3,
            guid.data4[0],
            guid.data4[1],
            guid.data4[2],
            guid.data4[3],
            guid.data4[4],
            guid.data4[5],
            guid.data4[6],
            guid.data4[7]
        )
    }
}
