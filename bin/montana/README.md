# montana

The Montana batch submitter execution extension.

An execution extension that submits L2 batches to L1 as part of the OP Stack batcher pipeline.

## Status

**This is currently a stub implementation.** The batch submitter functionality is not yet implemented.

## Usage

```bash
montana [OPTIONS]
```

### Options

- `-v, --verbose`: Increase logging verbosity (can be used multiple times)
  - No flag (default): WARN level
  - `-v`: INFO level
  - `-vv`: DEBUG level
  - `-vvv`: TRACE level
- `--help`: Print help information
- `--version`: Print version information

### Examples

Run the montana binary with INFO logging:
```bash
montana -v
```

Run with DEBUG logging:
```bash
montana -vv
```

Run with TRACE logging:
```bash
montana -vvv
```

## Planned Implementation

The following features are planned for implementation:

- Connect to L2 node
- Subscribe to new blocks
- Compress and batch transactions
- Submit batches to L1

See the source code TODO comments for more details.
