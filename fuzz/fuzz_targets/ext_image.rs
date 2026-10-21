#![no_main]

use libfuzzer_sys::fuzz_target;
use partition11_ext_reader::{ExtEntryType, ExtImageReader};
use std::fs;
use std::io;

const MAX_FUZZ_IMAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_FUZZ_FILE_BYTES: u64 = 1024 * 1024;

fuzz_target!(|input: &[u8]| {
    if input.len() > MAX_FUZZ_IMAGE_BYTES {
        return;
    }

    let image_path = std::env::temp_dir().join(format!(
        "partition11-ext-image-fuzz-{}.img",
        std::process::id()
    ));
    if fs::write(&image_path, input).is_err() {
        return;
    }

    if let Ok(reader) = ExtImageReader::open(&image_path) {
        if let Ok(entries) = reader.list_directory(b"/") {
            for entry in entries.iter().take(16) {
                let mut image_path = Vec::with_capacity(entry.name.len() + 1);
                image_path.push(b'/');
                image_path.extend_from_slice(&entry.name);
                match entry.kind {
                    ExtEntryType::Directory => {
                        let _ = reader.list_directory(&image_path);
                    }
                    ExtEntryType::File if entry.size_bytes <= MAX_FUZZ_FILE_BYTES => {
                        let mut sink = io::sink();
                        let _ = reader.copy_file_to(&image_path, MAX_FUZZ_FILE_BYTES, &mut sink);
                    }
                    _ => {}
                }
            }
        }
    }

    let _ = fs::remove_file(&image_path);
});
