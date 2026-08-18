# RosterForge

`rf` is a cross-platform command-line tool for updating EA FC PS4 squad saves.

It accepts an Apollo `DATA` file, a generated `Squads*` file, or a compressed EA
RefPack squad file. It validates the save format, creates a backup, patches the
database, and verifies the output before writing it.

## Usage

Build the binary:

```bash
cargo build --release
```

Validate a save:

```bash
rf verify input/DATA
```

Inspect a local squad source:

```bash
rf inspect result/ps4
```

Download the latest PS4 squad source:

```bash
rf download --output-dir downloaded
```

Run a dry update using a local source:

```bash
rf update \
  --data input/DATA \
  --squad-file result/ps4/squads/464/Squads20260218000000 \
  --dry-run
```

Run an update without specifying `--data`:

```bash
rf update
```

The tool searches `input/` and common Linux mount roots for the Apollo layout:

```text
USB/PS4/APOLLO/<user>_<title>_Squads<timestamp>/DATA
```

Complete Apollo save folders are backed up and mirrored to the output. If the
tool finds more than one save, provide `--data` explicitly.

## Safety

- Input DATA files are validated before processing.
- RefPack streams use checked bounds validation.
- T3DB and BNRY markers are validated.
- Backups are created before output is written.
- Downloads and output files use temporary paths and atomic renames.
- `--dry-run` performs validation without changing files.
- HTTPS certificate verification remains enabled.

## Credits and Inspirations

This project is an independent implementation inspired by:

- [FIFASquadFileDownloader](https://github.com/xAranaktu/FIFASquadFileDownloader)
  for EA download and RefPack format research.
- [Apollo Save Tool](https://github.com/bucanero/apollo-ps4) for PS4 save export
  and restore workflows.
