//! Partition 11 core: safe-by-default policy layer.
//!
//! Phase 0 exposes no storage I/O to the app. P2 provides an internal,
//! bounded read-only regular image-file adapter. No writes can be authorized.
//! Any future write support MUST be gated by a versioned capability and a
//! separate privileged Windows agent with independent device validation.
#![forbid(unsafe_code)]

pub mod image;
pub mod partition;
pub mod windows_layout;

/// A named image is a logical target; it is NOT opened by Phase 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Image { name: String },
    PhysicalDisk { opaque_id: String },
}

/// Planned operations. This enum is not a command interpreter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Inspect,
    FormatExt4,
    CheckAndRepair,
    ResizeFilesystem,
    RewritePartitionTable,
}

impl Operation {
    pub fn is_destructive(self) -> bool {
        !matches!(self, Operation::Inspect)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    AllowedReadOnly,
    Denied(&'static str),
}

/// Deny all mutating operations until their implementation, audit, and
/// operation-specific image/hardware test gates are completed.
pub fn authorize(target: &Target, operation: Operation) -> Decision {
    if operation.is_destructive() {
        return Decision::Denied("write operations are disabled in Phase 0");
    }

    match target {
        Target::Image { .. } | Target::PhysicalDisk { .. } => Decision::AllowedReadOnly,
    }
}

/// Enumerated capabilities are deliberately different from method presence:
/// a UI MUST NOT infer that the core can perform an action merely because a
/// button or enum value exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub inspect_images: bool,
    pub inspect_physical_disks: bool,
    pub write_images: bool,
    pub write_physical_disks: bool,
}

pub const PHASE_0_CAPABILITIES: Capabilities = Capabilities {
    inspect_images: false,
    inspect_physical_disks: false,
    write_images: false,
    write_physical_disks: false,
};

/// Capabilities exposed by the local Windows read-only preview. This does not
/// authorize formatting, filesystem repair, resizing, or physical writes.
pub const READ_ONLY_PREVIEW_CAPABILITIES: Capabilities = Capabilities {
    inspect_images: true,
    inspect_physical_disks: true,
    write_images: false,
    write_physical_disks: false,
};
pub fn read_only_preview_capabilities() -> Capabilities {
    READ_ONLY_PREVIEW_CAPABILITIES
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets() -> [Target; 2] {
        [
            Target::Image {
                name: "scratch.img".into(),
            },
            Target::PhysicalDisk {
                opaque_id: "test-device".into(),
            },
        ]
    }

    #[test]
    fn inspection_is_policy_allowed_without_granting_mutations() {
        for target in targets() {
            assert_eq!(
                authorize(&target, Operation::Inspect),
                Decision::AllowedReadOnly
            );
        }
    }

    #[test]
    fn read_only_preview_exposes_only_inspection_capabilities() {
        let capabilities = read_only_preview_capabilities();
        assert!(capabilities.inspect_images);
        assert!(capabilities.inspect_physical_disks);
        assert!(!capabilities.write_images);
        assert!(!capabilities.write_physical_disks);
    }

    #[test]
    fn every_mutating_operation_is_denied_for_every_target() {
        for target in targets() {
            for operation in [
                Operation::FormatExt4,
                Operation::CheckAndRepair,
                Operation::ResizeFilesystem,
                Operation::RewritePartitionTable,
            ] {
                assert!(matches!(authorize(&target, operation), Decision::Denied(_)));
                assert!(operation.is_destructive());
            }
        }
    }
}
