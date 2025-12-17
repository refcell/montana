# analyze

The Montana compression analyzer binary.

A modular and extensible duplex pipeline for L2 batch submission and derivation. This binary compares compression algorithms (Brotli, Zstd, Zlib) for L2 batch data and outputs performance metrics.

## Usage

```bash
analyze [OPTIONS]
```

## CLI Flags

### Required Flags

None - all flags have sensible defaults.

### Optional Flags

- `-v, --verbose`: Verbosity level (can be specified multiple times: -v, -vv, -vvv)
  - No flag (0): WARN level logging
  - `-v` (1): INFO level logging
  - `-vv` (2): DEBUG level logging
  - `-vvv` (3+): TRACE level logging
- `-m, --mode <MODE>`: Operation mode (default: `batch`)
  - `batch`: Batch submission mode - compress and submit L2 blocks
  - `derivation`: Derivation mode - decompress and derive L2 blocks from compressed batches
  - `roundtrip`: Roundtrip mode - batch submission followed by derivation with validation
- `-i, --input <PATH>`: Input JSON file containing transactions to batch submit (default: `static/base_mainnet_blocks.json`)
- `-o, --output <PATH>`: Output JSON file for the submitted batch (default: `output.json`)
- `-c, --compression <ALGORITHM>`: Compression algorithm to use (default: `brotli`)
  - `brotli`: Brotli compression
  - `zlib`: Zlib (DEFLATE) compression
  - `zstd`: Zstandard compression
  - `all`: Run all compression algorithms and compare results (uses best algorithm for submission)

## Operation Modes

### Batch Mode (default)

Compresses L2 block data and simulates batch submission.

**Workflow:**
1. Loads source data from input file
2. Encodes L2 blocks into raw batch data
3. Compresses using specified algorithm(s)
4. Outputs compressed batch to output file with metrics

**Example:**
```bash
# Use default brotli compression
analyze --mode batch --input static/base_mainnet_blocks.json --output output.json

# Compare all compression algorithms
analyze --mode batch --compression all

# Use zstd compression with verbose output
analyze -vv --mode batch --compression zstd
```

### Derivation Mode

Decompresses batches from output file (created by batch mode).

**Workflow:**
1. Reads compressed batches from output file
2. Decompresses using specified algorithm
3. Outputs decompressed batch data and metrics

**Example:**
```bash
# Decompress batches using brotli
analyze --mode derivation --compression brotli --output output.json

# Decompress with debug logging
analyze -vvv --mode derivation --compression zstd
```

**Note:** The compression algorithm must match what was used during batch mode.

### Roundtrip Mode

Performs batch submission followed by derivation with data integrity validation.

**Workflow:**
1. Loads source data from input file
2. Encodes and compresses L2 blocks
3. Decompresses the data
4. Validates that decompressed data matches original
5. Outputs validation results and metrics

**Example:**
```bash
# Validate roundtrip with brotli
analyze --mode roundtrip --compression brotli

# Validate with all algorithms (tests each separately)
analyze --mode roundtrip --compression zstd -v
```

**Note:** Roundtrip mode does not support `--compression all`.

## Examples

```bash
# Basic usage with defaults (batch mode, brotli compression)
analyze

# Batch mode with all algorithms comparison
analyze --mode batch --compression all -v

# Derivation mode to decompress existing output
analyze --mode derivation --output output.json --compression brotli

# Roundtrip validation with verbose logging
analyze --mode roundtrip --compression zstd -vv

# Custom input/output paths
analyze --input /path/to/blocks.json --output /path/to/output.json

# Maximum verbosity for debugging
analyze -vvv --mode batch --compression all
```

## Output Format

The output JSON file contains:
- Batch number
- Compressed batch data (hex-encoded)
- L1 block number
- Transaction hash
- Optional blob hash (for EIP-4844 batches)

## Environment Variables

- `RUST_LOG`: Controls logging output (overrides verbosity flags if set)
  - Example: `RUST_LOG=debug analyze`

## Exit Codes

- `0`: Success
- `1`: Pipeline failure (error logged to stderr)
