# montana-local

Local file-based data source and sink implementations for Montana.

This crate provides implementations of the pipeline traits that read from and write to
local JSON files, useful for testing, debugging, and benchmarking.

## Features

- `LocalBatchSource`: Reads L2 block data from a JSON file
- `LocalBatchSink`: Writes compressed batches to a JSON file
- `NoopCompressor`: A passthrough compressor for benchmarking

## Components

### LocalBatchSource

Implements the `BatchSource` trait for file-based L2 block data:

```rust
use montana_local::LocalBatchSource;

// Load from JSON file
let source = LocalBatchSource::from_file("blocks.json")?;

// Or load from JSON string
let json = r#"{ ... }"#;
let source = LocalBatchSource::from_json(json)?;
```

**JSON Format:**
```json
{
  "blocks": [
    {
      "timestamp": 1000,
      "transactions": [
        {"data": "0xf86c0a8502540be400825208"}
      ]
    }
  ]
}
```

### LocalBatchSink

Implements the `BatchSink` trait for file-based batch submission:

```rust
use montana_local::LocalBatchSink;

// Write to file
let sink = LocalBatchSink::new("output.json");

// Or use in-memory (no file output)
let sink = LocalBatchSink::in_memory();

// With custom capacity
let sink = LocalBatchSink::new("output.json").with_capacity(256 * 1024);
```

### NoopCompressor

A passthrough compressor for testing and benchmarking:

```rust
use montana_local::NoopCompressor;

let compressor = NoopCompressor;
// compress() and decompress() return data unchanged
```

## Use Cases

- **Testing**: Validate pipeline logic with known data
- **Debugging**: Inspect compressed batch output
- **Benchmarking**: Measure codec and compression performance
- **Development**: Prototype new features without L1/L2 infrastructure
