use memoffset::offset_of;
use std::mem;
use std::time::Duration;
use std::{cell::Cell, ptr::addr_of_mut};
use windows::core::Result;
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::NetworkManagement::IpHelper::{GetIfTable, MIB_IFROW, MIB_IFTABLE};

#[derive(Default)]
pub struct State {
    prev_byte_count: Cell<u64>,
}

impl State {
    pub fn fetch_mbit(&self, time_delta: Option<Duration>) -> Result<f64> {
        /// Identical to MIB_IFTABLE but with more rows.
        #[repr(C)]
        struct BIG_MIB_IFTABLE {
            dw_num_entries: u32,
            table: [MIB_IFROW; 128],
        }
        assert!(mem::align_of::<BIG_MIB_IFTABLE>() == mem::align_of::<MIB_IFTABLE>());
        assert!(
            offset_of!(BIG_MIB_IFTABLE, dw_num_entries) == offset_of!(MIB_IFTABLE, dwNumEntries)
        );
        assert!(offset_of!(BIG_MIB_IFTABLE, table) == offset_of!(MIB_IFTABLE, table));

        // SAFETY: MIB_IFTABLE can be safely zero-initialized
        let mut interfaces: BIG_MIB_IFTABLE = unsafe { mem::zeroed() };
        let mut size_of_interfaces = mem::size_of_val(&interfaces).try_into().unwrap();

        // SAFETY: BIG_MIB_IFTABLE is layout-compatible with MIB_IFTABLE, but with a larger table
        unsafe {
            WIN32_ERROR(GetIfTable(
                Some(addr_of_mut!(interfaces).cast::<MIB_IFTABLE>()),
                &mut size_of_interfaces,
                false,
            ))
            .ok()?
        };

        let interfaces = &mut interfaces.table[..interfaces.dw_num_entries as usize];

        // Windows has many internal copies of the same interface, which results in double-counting.
        //
        // For example:
        // status=INTERNAL_IF_OPER_STATUS(5) type=6 addr=[4, 217, 245, 51, 50, 182, 0, 0] bytes=2288317722 - \DEVICE\TCPIP_{438B8BC2-XXXX-XXXX-XXXX-XXXXXXXXXXXX} Realtek PCIe 2.5GbE Family Controller-WFP Native MAC Layer LightWeight Filter-0000
        // status=INTERNAL_IF_OPER_STATUS(5) type=6 addr=[4, 217, 245, 51, 50, 182, 0, 0] bytes=2288317722 - \DEVICE\TCPIP_{8C3238C4-XXXX-XXXX-XXXX-XXXXXXXXXXXX} Realtek PCIe 2.5GbE Family Controller-Npcap Packet Driver (NPCAP)-0000
        // status=INTERNAL_IF_OPER_STATUS(5) type=6 addr=[4, 217, 245, 51, 50, 182, 0, 0] bytes=2288317722 - \DEVICE\TCPIP_{438B8BC4-XXXX-XXXX-XXXX-XXXXXXXXXXXX} Realtek PCIe 2.5GbE Family Controller-QoS Packet Scheduler-0000
        // status=INTERNAL_IF_OPER_STATUS(5) type=6 addr=[4, 217, 245, 51, 50, 182, 0, 0] bytes=2288317722 - \DEVICE\TCPIP_{438B8BC7-XXXX-XXXX-XXXX-XXXXXXXXXXXX} Realtek PCIe 2.5GbE Family Controller-WFP 802.3 MAC Layer LightWeight Filter-0000
        //
        // To avoid this, deduplicate interfaces by address.

        interfaces.sort_unstable_by_key(|if_row| if_row.bPhysAddr);

        let mut cur_network_byte_count = 0;

        let mut last_address = Default::default();
        for if_row in interfaces {
            if if_row.bPhysAddr == last_address {
                continue;
            }
            last_address = if_row.bPhysAddr;
            cur_network_byte_count += u64::from(if_row.dwInOctets) + u64::from(if_row.dwOutOctets);
        }

        // On first sample, just store the current byte count and return zero.
        let mbit = match time_delta {
            Some(time_delta) => {
                let bits_per_byte = 8;
                let bits =
                    cur_network_byte_count.wrapping_sub(self.prev_byte_count.get()) * bits_per_byte;
                (bits as f64) / 1_000_000.0 / time_delta.as_secs_f64()
            }
            None => 0.0,
        };

        self.prev_byte_count.set(cur_network_byte_count);

        Ok(mbit)
    }
}
