//! Non-elevated diagnostic agent, not wired to Electron or IPC.
//! P2-I2a can enumerate PRESENT Windows disk interfaces using SetupAPI,
//! but opens NO disk handles and reads/writes NO disk content. All runtime
//! capabilities remain false. All unrecognized commands are rejected.
#![deny(unsafe_code)]

// Win32 FFI has its own reviewed and bounded safety boundary. Do not add
// unsafe code elsewhere in the agent or expose raw paths to the renderer.
#[cfg(windows)]
#[allow(unsafe_code)]
mod disk_discovery;

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
        _ => {
            eprintln!("Partition 11 agent: only 'capabilities' and the Windows-only 'discovery-count' diagnostic are supported; no disk content access is available.");
            std::process::exit(2);
        }
    }
}
