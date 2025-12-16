# montana-local

Local file-based data source and sink implementations for Montana.

This crate provides implementations of the pipeline traits that read from and write to
local JSON files, useful for testing, debugging, and benchmarking.

## Features

- `LocalBatchSource`: Reads L2 block data from a JSON file
- `LocalBatchSink`: Writes compressed batches to a JSON file
- `NoopCompressor`: A passthrough compressor for benchmarking
