# RosterForge

`rf` is a **hobby project**: a cross-platform command-line tool for updating
EA FC PS4 squad saves. It downloads the latest EA roster, decodes it, and
patches an Apollo-exported PS4 save, with validation and backups along the way.

No warranty. The save formats are undocumented and can break at any time.
Always keep your own backup; the tool creates one too, but trust nothing
blindly - especially not a hobby tool.

## Build

```bash
cargo build --release
```

## Usage

```bash
rf verify <DATA>              # validate a save file
rf inspect <path>             # inspect a save or squad source
rf download                   # fetch the latest PS4 squad source
rf update                     # patch a save with the latest roster
rf restore <backup> <DATA>    # roll a save back from a backup
```

Run `rf --help` for all options: `--dry-run`, `--platform`, `--fut`,
`--content-url`, shell completions, and more.

Binary format details: [docs/Technical-Reference.md](docs/Technical-Reference.md).

## Credits

Independent implementation inspired by
[FIFASquadFileDownloader](https://github.com/xAranaktu/FIFASquadFileDownloader)
and [Apollo Save Tool](https://github.com/bucanero/apollo-ps4).

## License

[BSD-3-Clause](LICENSE).

