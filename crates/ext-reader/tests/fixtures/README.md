# Independent ext4 fixture

`ext4-e2fsprogs-1.47.3.img` was generated with Linux e2fsprogs 1.47.3
(`mke2fs -t ext4 -b 4096`) from a source tree containing `known.txt`, an
empty file, a 189-byte filename, `nested/deeper/payload.bin`, and the standard
`lost+found` directory. `payload.bin` is 4,097 bytes with byte `i` equal to
`(i * 73 + 19) % 256`; `known.txt` contains
`e2fsprogs 1.47.3 independent oracle` followed by LF.

The generated image was checked with `e2fsck -fn` from e2fsprogs 1.47.3 and
reported a clean filesystem. `debugfs` independently listed the expected root
entries and dumped both known files; their bytes matched the source tree.
The fixture SHA-256 is
`8ace927a36d0cfff8eb07108ceb7a8e67f50e7ff0041ccfbd4d7956338da4a3d`.

This exercises an independently generated ext4 layout and known source data;
it does not establish compatibility with every ext4 feature or corruption mode.