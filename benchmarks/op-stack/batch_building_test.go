package opstack

import (
	"bytes"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"strings"
	"testing"

	"github.com/andybalholm/brotli"
	"github.com/klauspost/compress/zstd"
)

// ===========================================================================
// Batch Header Encoding (matches Montana's wire format)
// ===========================================================================

// BatchHeader represents the batch header in Montana's wire format
// Total size: 19 bytes (little-endian)
// - version: 1 byte
// - batch_number: 8 bytes
// - timestamp: 8 bytes
// - block_count: 2 bytes
type BatchHeader struct {
	Version     uint8
	BatchNumber uint64
	Timestamp   uint64
	BlockCount  uint16
}

func (h *BatchHeader) Encode() []byte {
	buf := make([]byte, 19)
	buf[0] = h.Version
	binary.LittleEndian.PutUint64(buf[1:9], h.BatchNumber)
	binary.LittleEndian.PutUint64(buf[9:17], h.Timestamp)
	binary.LittleEndian.PutUint16(buf[17:19], h.BlockCount)
	return buf
}

func DecodeBatchHeader(data []byte) (*BatchHeader, error) {
	if len(data) < 19 {
		return nil, nil
	}
	return &BatchHeader{
		Version:     data[0],
		BatchNumber: binary.LittleEndian.Uint64(data[1:9]),
		Timestamp:   binary.LittleEndian.Uint64(data[9:17]),
		BlockCount:  binary.LittleEndian.Uint16(data[17:19]),
	}, nil
}

// ===========================================================================
// Batch Body Encoding (matches Montana's wire format)
// ===========================================================================

// Block wire format:
// - timestamp_delta: 2 bytes (u16)
// - tx_count: 2 bytes (u16)
// - transactions: variable
//
// Transaction wire format:
// - tx_len: 3 bytes (u24)
// - tx_data: tx_len bytes

func encodeBatchBody(blocks []BlockData, baseTimestamp uint64) []byte {
	var buf bytes.Buffer

	for _, block := range blocks {
		// timestamp_delta (u16)
		delta := uint16(block.Timestamp - baseTimestamp)
		binary.Write(&buf, binary.LittleEndian, delta)

		// tx_count (u16)
		txCount := uint16(len(block.Transactions))
		binary.Write(&buf, binary.LittleEndian, txCount)

		// transactions
		for _, tx := range block.Transactions {
			txData := decodeHex(tx.Data)

			// tx_len (u24 - 3 bytes little-endian)
			txLen := uint32(len(txData))
			buf.WriteByte(byte(txLen & 0xFF))
			buf.WriteByte(byte((txLen >> 8) & 0xFF))
			buf.WriteByte(byte((txLen >> 16) & 0xFF))

			// tx_data
			buf.Write(txData)
		}
	}

	return buf.Bytes()
}

func decodeHex(s string) []byte {
	s = strings.TrimPrefix(s, "0x")
	data, _ := hex.DecodeString(s)
	return data
}

// ===========================================================================
// Full Batch Encoding Benchmarks
// ===========================================================================

func BenchmarkBatchEncode(b *testing.B) {
	blockCounts := []int{1, 10, 100}

	for _, count := range blockCounts {
		fixture := generateFixture(count, 10)

		b.Run(formatBlockCount(count), func(b *testing.B) {
			b.ReportAllocs()

			for i := 0; i < b.N; i++ {
				// Encode header
				header := BatchHeader{
					Version:     0,
					BatchNumber: 1,
					Timestamp:   fixture.Blocks[0].Timestamp,
					BlockCount:  uint16(len(fixture.Blocks)),
				}
				headerBytes := header.Encode()

				// Encode body
				bodyBytes := encodeBatchBody(fixture.Blocks, fixture.Blocks[0].Timestamp)

				// Combine
				batch := append(headerBytes, bodyBytes...)
				_ = batch
			}
		})
	}
}

func BenchmarkBatchEncodeWithCompression(b *testing.B) {
	blockCounts := []int{1, 10, 100}

	for _, count := range blockCounts {
		fixture := generateFixture(count, 10)

		b.Run(formatBlockCount(count)+"/Brotli", func(b *testing.B) {
			b.ReportAllocs()

			for i := 0; i < b.N; i++ {
				// Encode header
				header := BatchHeader{
					Version:     0,
					BatchNumber: 1,
					Timestamp:   fixture.Blocks[0].Timestamp,
					BlockCount:  uint16(len(fixture.Blocks)),
				}
				headerBytes := header.Encode()

				// Encode body
				bodyBytes := encodeBatchBody(fixture.Blocks, fixture.Blocks[0].Timestamp)

				// Combine
				batch := append(headerBytes, bodyBytes...)

				// Compress with Brotli (fast preset for benchmarks)
				var buf bytes.Buffer
				w := brotli.NewWriterLevel(&buf, brotli.BestSpeed)
				_, _ = w.Write(batch)
				_ = w.Close()
				_ = buf.Bytes()
			}
		})

		b.Run(formatBlockCount(count)+"/Zstd", func(b *testing.B) {
			encoder, _ := zstd.NewWriter(nil, zstd.WithEncoderLevel(zstd.SpeedFastest))
			defer encoder.Close()
			b.ReportAllocs()

			for i := 0; i < b.N; i++ {
				// Encode header
				header := BatchHeader{
					Version:     0,
					BatchNumber: 1,
					Timestamp:   fixture.Blocks[0].Timestamp,
					BlockCount:  uint16(len(fixture.Blocks)),
				}
				headerBytes := header.Encode()

				// Encode body
				bodyBytes := encodeBatchBody(fixture.Blocks, fixture.Blocks[0].Timestamp)

				// Combine
				batch := append(headerBytes, bodyBytes...)

				// Compress with Zstd
				_ = encoder.EncodeAll(batch, nil)
			}
		})
	}
}

// ===========================================================================
// Full Pipeline Benchmarks (Source -> Encode -> Compress -> Sink)
// ===========================================================================

func BenchmarkFullPipeline(b *testing.B) {
	blockCounts := []int{1, 10, 100}

	for _, count := range blockCounts {
		fixture := generateFixture(count, 10)
		fixtureJSON, _ := json.Marshal(fixture)

		b.Run(formatBlockCount(count)+"/Noop", func(b *testing.B) {
			b.SetBytes(int64(len(fixtureJSON)))
			b.ReportAllocs()

			for i := 0; i < b.N; i++ {
				// Source: parse JSON (simulates reading from source)
				var parsed BatchFixture
				_ = json.Unmarshal(fixtureJSON, &parsed)

				// Encode: build batch
				header := BatchHeader{
					Version:     0,
					BatchNumber: 1,
					Timestamp:   parsed.Blocks[0].Timestamp,
					BlockCount:  uint16(len(parsed.Blocks)),
				}
				headerBytes := header.Encode()
				bodyBytes := encodeBatchBody(parsed.Blocks, parsed.Blocks[0].Timestamp)
				batch := append(headerBytes, bodyBytes...)

				// Compress: noop (passthrough)
				compressed := batch

				// Sink: simulate submission (just compute length)
				_ = len(compressed)
			}
		})

		b.Run(formatBlockCount(count)+"/Brotli", func(b *testing.B) {
			b.SetBytes(int64(len(fixtureJSON)))
			b.ReportAllocs()

			for i := 0; i < b.N; i++ {
				// Source: parse JSON
				var parsed BatchFixture
				_ = json.Unmarshal(fixtureJSON, &parsed)

				// Encode: build batch
				header := BatchHeader{
					Version:     0,
					BatchNumber: 1,
					Timestamp:   parsed.Blocks[0].Timestamp,
					BlockCount:  uint16(len(parsed.Blocks)),
				}
				headerBytes := header.Encode()
				bodyBytes := encodeBatchBody(parsed.Blocks, parsed.Blocks[0].Timestamp)
				batch := append(headerBytes, bodyBytes...)

				// Compress: Brotli
				var buf bytes.Buffer
				w := brotli.NewWriterLevel(&buf, brotli.BestSpeed)
				_, _ = w.Write(batch)
				_ = w.Close()
				compressed := buf.Bytes()

				// Sink: simulate submission
				_ = len(compressed)
			}
		})

		b.Run(formatBlockCount(count)+"/Zstd", func(b *testing.B) {
			encoder, _ := zstd.NewWriter(nil, zstd.WithEncoderLevel(zstd.SpeedFastest))
			defer encoder.Close()
			b.SetBytes(int64(len(fixtureJSON)))
			b.ReportAllocs()

			for i := 0; i < b.N; i++ {
				// Source: parse JSON
				var parsed BatchFixture
				_ = json.Unmarshal(fixtureJSON, &parsed)

				// Encode: build batch
				header := BatchHeader{
					Version:     0,
					BatchNumber: 1,
					Timestamp:   parsed.Blocks[0].Timestamp,
					BlockCount:  uint16(len(parsed.Blocks)),
				}
				headerBytes := header.Encode()
				bodyBytes := encodeBatchBody(parsed.Blocks, parsed.Blocks[0].Timestamp)
				batch := append(headerBytes, bodyBytes...)

				// Compress: Zstd
				compressed := encoder.EncodeAll(batch, nil)

				// Sink: simulate submission
				_ = len(compressed)
			}
		})
	}
}

// ===========================================================================
// Helper Functions
// ===========================================================================

func generateFixture(blockCount, txPerBlock int) *BatchFixture {
	blocks := make([]BlockData, blockCount)
	for i := 0; i < blockCount; i++ {
		txs := make([]Transaction, txPerBlock)
		for j := 0; j < txPerBlock; j++ {
			// Generate simple transaction data
			txs[j] = Transaction{
				Data: "0x" + strings.Repeat("ab", 32+j),
			}
		}
		blocks[i] = BlockData{
			Timestamp:    uint64(1000 + i*12),
			Transactions: txs,
		}
	}

	return &BatchFixture{
		L1Origin: L1Origin{
			BlockNumber: 12345,
			HashPrefix:  "0x0102030405060708091011121314151617181920",
		},
		ParentHash: "0x2122232425262728293031323334353637383940",
		Blocks:     blocks,
	}
}

func formatBlockCount(count int) string {
	switch count {
	case 1:
		return "1_block"
	case 10:
		return "10_blocks"
	case 100:
		return "100_blocks"
	case 1000:
		return "1000_blocks"
	default:
		return ""
	}
}
