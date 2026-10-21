use partition11_ext_reader::{ExtEntryType, ExtImageReader};
use std::fs;
use std::path::PathBuf;

const KNOWN_CONTENT: &[u8] = b"e2fsprogs 1.47.3 independent oracle\n";
const LONG_NAME: &str = concat!("long-", "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx", ".txt");

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ext4-e2fsprogs-1.47.3.img")
}

#[test]
fn independently_generated_e2fsprogs_image_matches_its_known_tree_and_content() {
    let path = fixture_path();
    let before = fs::read(&path).expect("read independent fixture before access");
    let reader = ExtImageReader::open(&path).expect("mount e2fsprogs-generated ext4 image");

    let root = reader.list_directory(b"/").expect("list root directory");
    let mut names: Vec<Vec<u8>> = root.iter().map(|entry| entry.name.clone()).collect();
    names.sort();
    let mut expected_names = vec![
        b"empty.bin".to_vec(),
        b"known.txt".to_vec(),
        LONG_NAME.as_bytes().to_vec(),
        b"lost+found".to_vec(),
        b"nested".to_vec(),
    ];
    expected_names.sort();
    assert_eq!(names, expected_names);

    let expect_kind_and_size = |name: &[u8], kind: ExtEntryType, size: u64| {
        let entry = root
            .iter()
            .find(|entry| entry.name == name)
            .expect("expected entry");
        assert_eq!(entry.kind, kind);
        assert_eq!(entry.size_bytes, size);
    };
    expect_kind_and_size(b"empty.bin", ExtEntryType::File, 0);
    expect_kind_and_size(b"known.txt", ExtEntryType::File, KNOWN_CONTENT.len() as u64);
    expect_kind_and_size(
        LONG_NAME.as_bytes(),
        ExtEntryType::File,
        b"long-name payload".len() as u64,
    );
    expect_kind_and_size(b"lost+found", ExtEntryType::Directory, 16 * 1024);
    expect_kind_and_size(b"nested", ExtEntryType::Directory, 4096);

    let nested = reader
        .list_directory(b"/nested")
        .expect("list nested directory");
    assert_eq!(nested.len(), 1);
    assert_eq!(nested[0].name, b"deeper");
    assert_eq!(nested[0].kind, ExtEntryType::Directory);
    let deeper = reader
        .list_directory(b"/nested/deeper")
        .expect("list deeper directory");
    assert_eq!(deeper.len(), 1);
    assert_eq!(deeper[0].name, b"payload.bin");
    assert_eq!(deeper[0].kind, ExtEntryType::File);
    assert_eq!(deeper[0].size_bytes, 4097);

    let mut known = Vec::new();
    assert_eq!(
        reader
            .copy_file_to(b"/known.txt", KNOWN_CONTENT.len() as u64, &mut known)
            .expect("read known file"),
        KNOWN_CONTENT.len() as u64
    );
    assert_eq!(known, KNOWN_CONTENT);

    let mut empty = Vec::new();
    assert_eq!(
        reader.copy_file_to(b"/empty.bin", 0, &mut empty).unwrap(),
        0
    );
    assert!(empty.is_empty());

    let mut long_name_content = Vec::new();
    assert_eq!(
        reader
            .copy_file_to(
                format!("/{LONG_NAME}").as_bytes(),
                b"long-name payload".len() as u64,
                &mut long_name_content,
            )
            .expect("read long-name file"),
        b"long-name payload".len() as u64
    );
    assert_eq!(long_name_content, b"long-name payload");

    let expected_payload: Vec<u8> = (0..4097)
        .map(|index| ((index * 73 + 19) % 256) as u8)
        .collect();
    let mut payload = Vec::new();
    assert_eq!(
        reader
            .copy_file_to(b"/nested/deeper/payload.bin", 4097, &mut payload)
            .expect("read multi-block payload"),
        4097
    );
    assert_eq!(payload, expected_payload);

    drop(reader);
    assert_eq!(fs::read(&path).expect("read fixture after access"), before);
}
