# Technical Reference

This document records the binary formats that `rf` handles.
The formats are undocumented EA internals; this page is the project source of
truth.

## Apollo PS4 save container

Apollo Save Tool exports a decrypted save as a folder:

```text
PS4/APOLLO/<user>_<title>_Squads<timestamp>/
    DATA        squad payload, structure below
    NAME        save title text
    INDEX       Apollo metadata
    sce_sys/    param.sfo, icons, keystone
```

`rf` treats every file in the folder except `DATA` as a companion file. It
mirrors them to backups and output containers unchanged.

## DATA file layout

A valid squads `DATA` file has this logical layout:

```text
offset  size  content
0       4     save name length N (little endian u32, 1..=128)
16      N     save name bytes (UTF-8)
?       11    "Type_Squads\0" signature, before the T3DB marker
+0      4     checksum field, zeroed by rf before writing
?       4     T3DB marker: 44 42 00 08 ("DB\0\x08")
...     ...   T3DB database body
end-45985  45985  BNRY trailer block
```

Discovery rules used by `rf`:

- The T3DB marker search starts at offset 1000 to skip false positives in
  the header area. A real fixture places it at offset 1136.
- The `Type_Squads` signature must appear before the T3DB marker.
- The 4-byte checksum field directly follows the signature. Its input hash
  is unknown; patching zeroes it and the game accepts the save. See
  `src/patch.rs`.
- The BNRY trailer is exactly 45,985 bytes and starts with a fixed 32-byte
  magic (`BNRY_MAGIC` in `src/save_format.rs`). `rf` validates it on every
  read and rebuilds it on every write.

## Generated Squads file layout

EA squad downloads unpacked by community tools use a generated container:

```text
offset  size  content
0       1178  generation header (FBCHUNKS-era metadata)
1178    ...   T3DB database body, starting with the T3DB marker
end-45985  45985  BNRY trailer block
```

`rf` classifies any file whose name starts with `Squads` as this format and
extracts the database slice between header and trailer.

## Compressed EA squad files (RefPack)

Fresh EA downloads are RefPack compressed streams:

```text
offset  size  content
0       2     flags (ignored by rf)
2       3     uncompressed size, big endian, 24 bit
5       5     padding, ignored
10      ...   command stream
```

The decoder expands to exactly the declared size, prepends nothing, and the
result must start with the T3DB marker. Output size is capped at 64 MiB.

Command encoding, decoded by `src/refpack.rs`:

| Control top bits | Form | Layout |
|---|---|---|
| `0xxxxxxx` | small pointer | `control`, `b1`; literals = `control & 3`, length = `((control >> 2) & 7) + 3`, offset = `b1 + ((control & 0x60) << 3) + 1` |
| `10xxxxxx` | medium pointer | `control`, `b2`, `b3`; literals = `b2 >> 6`, length = `(control & 0x3f) + 4`, offset = `((b2 & 0x3f) << 8 | b3) + 1` |
| `110xxxxx` | large pointer | `control`, `b2`, `b3`, `b4`; literals = `control & 3`, length = `b4 + ((control & 0x0c) << 6) + 5`, offset = `((control & 0x10) << 12 | b2 << 8 | b3) + 1` |
| `111xxxxx` | literal run | literals = `(control & 0x1f) * 4 + 4`; values above 112 are stop codes |

Stop codes are `0xFC..=0xFF`. After the loop ends, the low two bits of the
last control byte count trailing literal bytes that follow the stop code.
Back-references may overlap their own destination; copies run forward one
byte at a time.

## Roster manifest

`fc/fclive/genxtitle/rosterupdate.xml` under the EA content base URL lists
one `squadInfo` element per platform:

```xml
<squadInfo platform="ps4">
    <dbMajor>464</dbMajor>
    <dbMajorLoc>fc/fclive/squads/464/Squads20260218000000</dbMajorLoc>
    <dbFUTVer>464.fut</dbFUTVer>
    <dbFUTLoc>fc/fclive/futsquads/464/FutSquads</dbFUTLoc>
</squadInfo>
```

- `platform` values handled by `rf`: `ps4`, `xbox` (case insensitive).
- `dbMajor` / `dbMajorLoc` are required per platform.
- `dbFUTVer` / `dbFUTLoc` are optional FUT squad entries.
- Manifest locations are joined onto the content base URL. `rf` rejects
  absolute paths and parent traversal before any download.
- The content base URL rotates per game year. Pass `--content-url` instead
  of editing source when EA moves it.

## Update algorithm

1. Read the target `DATA` and validate the full layout.
2. Create a backup container with the original folder and a
   `.source_sha256` marker unless `--dry-run`.
3. Resolve the new database: local squad source first, then EA download.
4. If the current database already matches, stop.
5. Rebuild `DATA`: original header up to T3DB, checksum zeroed, new
   database, fresh BNRY trailer.
6. Re-validate the patched bytes.
7. Write through a `.part` temporary file and an atomic rename.

## Restore algorithm

1. Locate `DATA` inside the backup container.
2. Compare its SHA-256 against `.source_sha256`; refuse on mismatch.
3. Refuse destinations outside `*_Squads` folders that are not valid squad
   DATA files.
4. Write atomically unless `--dry-run`.

## Testing strategy

- Unit tests cover the RefPack decoder command paths, error contracts,
  manifest parsing, and the patch invariants. See the `#[cfg(test)]`
  modules in `src/`.
- Fuzz targets assert the decoders never panic on untrusted input:

```bash
cargo install cargo-fuzz
RUSTUP_TOOLCHAIN=nightly-2025-11-15 cargo fuzz run refpack_decompress
RUSTUP_TOOLCHAIN=nightly-2025-11-15 cargo fuzz run save_format_validate
```

The pinned nightly works around a cargo-fuzz and newest-nightly mismatch.
