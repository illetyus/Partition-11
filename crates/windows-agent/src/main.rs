//! Non-elevated agent with a bounded, sanitized stdin/stdout inventory protocol
//! consumed by Electron. It queries Windows disk metadata through GENERIC_READ
//! handles but never reads raw sectors or writes to disks.
//! No image or physical-disk write capability is enabled.
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
mod inventory_protocol;
#[cfg(windows)]
#[allow(unsafe_code)]
mod metadata_query;
#[cfg(windows)]
#[allow(unsafe_code)]
mod metadata_worker;
#[cfg(any(windows, test))]
mod metadata_worker_protocol;
#[cfg(any(windows, test))]
#[allow(unsafe_code)]
mod overlapped_ioctl;
#[cfg(windows)]
#[allow(unsafe_code)]
mod pnp_notifications;
#[cfg(windows)]
mod scan_identity;

use partition11_core::PHASE_0_CAPABILITIES;
use std::thread::JoinHandle;
use std::time::Duration;

const INVENTORY_HARD_DEADLINE: Duration = Duration::from_secs(32);

fn spawn_hard_deadline(
    timeout: Duration,
    on_expire: impl FnOnce() + Send + 'static,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("p11-inventory-deadline".into())
        .spawn(move || {
            std::thread::sleep(timeout);
            on_expire();
        })
}

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
        (Some("__metadata-worker"), None) => {
            let status = metadata_worker::run_stdio_worker();
            if status != 0 {
                std::process::exit(status);
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
        #[cfg(windows)]
        (Some("inventory"), None) => {
            use std::io::{self, Write};
            let _hard_deadline = spawn_hard_deadline(INVENTORY_HARD_DEADLINE, || {
                std::process::exit(124);
            })
            .unwrap_or_else(|_| std::process::exit(3));
            let request = match inventory_protocol::read_request(&mut io::stdin().lock()) {
                Ok(request) => request,
                Err(_) => std::process::exit(2),
            };
            let response = match disk_metadata::inspect_discovered_devices_with_timeout(
                std::time::Duration::from_secs(30),
            ) {
                Ok(disks) => inventory_protocol::encode_success(&request, &disks),
                Err(disk_metadata::ScanError::TimedOut) => {
                    Ok(inventory_protocol::encode_failure(&request, "scan_timeout"))
                }
                Err(disk_metadata::ScanError::Security(
                    disk_metadata::MetadataError::AccessDenied,
                )) => Ok(inventory_protocol::encode_failure(
                    &request,
                    "access_denied",
                )),
                Err(_) => Ok(inventory_protocol::encode_failure(
                    &request,
                    "agent_failure",
                )),
            };
            let Ok(response) = response else {
                std::process::exit(3);
            };
            if inventory_protocol::write_response(&mut io::stdout().lock(), &response).is_err() {
                std::process::exit(3);
            }
            let _ = io::stdout().flush();
        }
        _ => {
            eprintln!("Partition 11 agent: bounded diagnostics and one-shot read-only inventory only. No disk writes exist.");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod hard_deadline_tests {
    use super::spawn_hard_deadline;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn hard_deadline_fires_independently_of_the_inventory_call() {
        let (sender, receiver) = mpsc::channel();
        let _deadline = spawn_hard_deadline(Duration::from_millis(10), move || {
            let _ = sender.send(());
        })
        .expect("deadline thread should start");

        receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("deadline callback should fire without caller cooperation");
    }
}
