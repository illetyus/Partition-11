//! Phase 0 diagnostic agent. Not a Windows service and never requests UAC.
//! It neither enumerates disks nor opens disk images. Every unexpected
//! command is rejected. This binary is NOT yet exposed to Electron.
#![forbid(unsafe_code)]

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
        _ => {
            eprintln!("Partition 11 agent Phase 0: only 'capabilities' is supported; no disk operations are available.");
            std::process::exit(2);
        }
    }
}
