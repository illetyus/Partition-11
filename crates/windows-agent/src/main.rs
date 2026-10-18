//! Non-elevated diagnostic agent, not wired to Electron or IPC.
//! P2-I2a enumerates present Windows disk interfaces using SetupAPI.
//! P2-I2b provides a separate, non-elevated, metadata-only diagnostic; it
//! opens GENERIC_READ handles, queries bounded drive-layout metadata, but
//! never reads raw sectors or writes to disks.
//! All Electron/runtime capabilities remain false.
#![deny(unsafe_code)]

// Win32 FFI has its own reviewed and bounded safety boundary. Do not add
// unsafe code elsewhere in the agent or expose raw paths to the renderer.
#[cfg(windows)]
#[allow(unsafe_code)]
mod disk_discovery;
#[cfg(windows)]
#[allow(unsafe_code)]
mod disk_metadata;
#[cfg(windows)]
mod scan_identity;
#[cfg(windows)]
#[allow(unsafe_code)]
mod pnp_notifications;

use partition11_core::PHASE_0_CAPABILITIES;

fn main() {
    match (std::env::args().nth(1).as_deref(), std::env::args().nth(2)) {
        (Some("capabilities"), None) => {
            let c = PHASE_0_CAPABILITIES;
            println!(
                "{{\"protocol_version\":1,\"phase\":\"P0\",\"inspect_images\":{},\"inspect_physical_disks\":{},\"write_images\":{},\"write_physical_disks\":{}}}",
                c.inspect_images, c.inspect_physical_disks, c.write_images, c.write_physical_disks
            );
        }
        #[cfg(windows)]
        (Some("discovery-count"), None) => {
            // Local diagnostic only: no device path, serial, or handle returned.
            // Device inventory is not a stable identity and grants no capability.
            match disk_discovery::enumerate_disk_interfaces() {
                Ok(paths) => println!(
                    r#"{{"protocol_version":1,"mode":"diagnostic-only","disk_interface_count":{},"disk_access_enabled":false}}"#,
                    paths.len()
                ),
                Err(err) => {
                    eprintln!("Windows read-only interface enumeration failed: {err}");
                    std::process::exit(3);
                }
            }
        }
        #[cfg(windows)]
        (Some("metadata-summary"), None) => {
            // Deliberately aggregate-only: no PnP path, serial, temporary disk
            // number or raw handle is sent to the renderer or stdout.
            match disk_metadata::inspect_discovered_devices() {
                Ok(results) => {
                    let mut opened = 0_usize;
                    let mut capacities = 0_usize;
                    let mut logical_sectors = 0_usize;
                    let mut physical_sectors = 0_usize;
                    let mut bus_types = 0_usize;
                    let mut layouts = 0_usize;
                    let mut verified_partitions = 0_usize;
                    let mut unavailable = 0_usize;
                    for result in &results {
                        match result {
                            disk_metadata::ProbeOutcome::Ready(metadata) => {
                                // Device number is only a temporary OS label.
                                debug_assert_ne!(metadata.device_number, u32::MAX);
                                opened += 1;
                                capacities += usize::from(metadata.capacity_bytes.is_ok());
                                if let Ok(sectors) = &metadata.sectors {
                                    logical_sectors += usize::from(sectors.logical.is_some());
                                    physical_sectors += usize::from(sectors.physical.is_some());
                                }
                                bus_types += usize::from(matches!(metadata.bus_type, Ok(Some(_))));
                                if let Ok(layout) = &metadata.partition_layout {
                                    layouts += 1;
                                    verified_partitions += layout.occupied.len();
                                }
                            }
                            disk_metadata::ProbeOutcome::Unavailable(error) => {
                                // Individual Win32 failures remain internal.
                                let _ = error.to_string();
                                unavailable += 1;
                            }
                        }
                    }
                    println!(
                        r#"{{"protocol_version":1,"mode":"diagnostic-only","disk_interface_count":{},"temporarily_opened":{},"capacity_known":{},"logical_sector_known":{},"physical_sector_known":{},"bus_type_known":{},"layout_known":{},"verified_partition_count":{},"unavailable":{},"ui_disk_access_enabled":false}}"#,
                        results.len(),
                        opened,
                        capacities,
                        logical_sectors,
                        physical_sectors,
                        bus_types,
                        layouts,
                        verified_partitions,
                        unavailable
                    );
                }
                Err(error) => {
                    eprintln!("Windows read-only metadata diagnostic rejected: {error}");
                    std::process::exit(3);
                }
            }
        }
        _ => {
            eprintln!("Partition 11 agent: 'capabilities', and on Windows 'discovery-count' / 'metadata-summary' diagnostics only. No GUI disk access or disk writes exist.");
            std::process::exit(2);
        }
    }
}
