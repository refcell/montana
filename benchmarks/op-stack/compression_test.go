// Package opstack provides benchmarks for OP Stack components
// to compare against Montana's Rust implementation.
package opstack

import (
	"bytes"
	"compress/zlib"
	"encoding/json"
	"io"
	"os"
	"testing"

	"github.com/andybalholm/brotli"
	"github.com/klauspost/compress/zstd"
)

// BlockData represents L2 block data for benchmarking
type BlockData struct {
	Timestamp    uint64        `json:"timestamp"`
	Transactions []Transaction `json:"transactions"`
}

// Transaction represents a transaction in a block
type Transaction struct {
	Data string `json:"data"`
}

// BatchFixture represents the test fixture format
type BatchFixture struct {
	L1Origin   L1Origin    `json:"l1_origin"`
	ParentHash string      `json:"parent_hash"`
	Blocks     []BlockData `json:"blocks"`
}

// L1Origin represents L1 block reference
type L1Origin struct {
	BlockNumber uint64 `json:"block_number"`
	HashPrefix  string `json:"hash_prefix"`
}

// generateTestData creates test data of the specified size
func generateTestData(size int) []byte {
	data := make([]byte, size)
	for i := range data {
		data[i] = byte(i % 256)
	}
	return data
}

// loadFixture loads a test fixture from the fixtures directory
func loadFixture(name string) (*BatchFixture, error) {
	path := "../fixtures/" + name
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var fixture BatchFixture
	if err := json.Unmarshal(data, &fixture); err != nil {
		return nil, err
	}
	return &fixture, nil
}

// ===========================================================================
// Brotli Compression Benchmarks
// ===========================================================================

func BenchmarkBrotliCompress(b *testing.B) {
	sizes := []struct {
		name string
		size int
	}{
		{"100B", 100},
		{"1KB", 1024},
		{"10KB", 10 * 1024},
		{"100KB", 100 * 1024},
	}

	levels := []struct {
		name  string
		level int
	}{
		{"Fast", brotli.BestSpeed},
		{"Balanced", 6},
	}

	for _, sz := range sizes {
		data := generateTestData(sz.size)

		for _, lvl := range levels {
			b.Run(sz.name+"/"+lvl.name, func(b *testing.B) {
				b.SetBytes(int64(sz.size))
				b.ReportAllocs()

				for i := 0; i < b.N; i++ {
					var buf bytes.Buffer
					w := brotli.NewWriterLevel(&buf, lvl.level)
					_, _ = w.Write(data)
					_ = w.Close()
					_ = buf.Bytes()
				}
			})
		}
	}
}

func BenchmarkBrotliDecompress(b *testing.B) {
	sizes := []struct {
		name string
		size int
	}{
		{"100B", 100},
		{"1KB", 1024},
		{"10KB", 10 * 1024},
		{"100KB", 100 * 1024},
	}

	for _, sz := range sizes {
		data := generateTestData(sz.size)

		// Pre-compress the data
		var compressed bytes.Buffer
		w := brotli.NewWriterLevel(&compressed, brotli.BestSpeed)
		_, _ = w.Write(data)
		_ = w.Close()
		compressedData := compressed.Bytes()

		b.Run(sz.name, func(b *testing.B) {
			b.SetBytes(int64(len(compressedData)))
			b.ReportAllocs()

			for i := 0; i < b.N; i++ {
				r := brotli.NewReader(bytes.NewReader(compressedData))
				_, _ = io.ReadAll(r)
			}
		})
	}
}

// ===========================================================================
// Zstd Compression Benchmarks
// ===========================================================================

func BenchmarkZstdCompress(b *testing.B) {
	sizes := []struct {
		name string
		size int
	}{
		{"100B", 100},
		{"1KB", 1024},
		{"10KB", 10 * 1024},
		{"100KB", 100 * 1024},
	}

	levels := []struct {
		name  string
		level zstd.EncoderLevel
	}{
		{"Fast", zstd.SpeedFastest},
		{"Balanced", zstd.SpeedDefault},
	}

	for _, sz := range sizes {
		data := generateTestData(sz.size)

		for _, lvl := range levels {
			b.Run(sz.name+"/"+lvl.name, func(b *testing.B) {
				b.SetBytes(int64(sz.size))
				b.ReportAllocs()

				encoder, _ := zstd.NewWriter(nil, zstd.WithEncoderLevel(lvl.level))
				defer encoder.Close()

				for i := 0; i < b.N; i++ {
					_ = encoder.EncodeAll(data, nil)
				}
			})
		}
	}
}

func BenchmarkZstdDecompress(b *testing.B) {
	sizes := []struct {
		name string
		size int
	}{
		{"100B", 100},
		{"1KB", 1024},
		{"10KB", 10 * 1024},
		{"100KB", 100 * 1024},
	}

	for _, sz := range sizes {
		data := generateTestData(sz.size)

		// Pre-compress the data
		encoder, _ := zstd.NewWriter(nil, zstd.WithEncoderLevel(zstd.SpeedFastest))
		compressedData := encoder.EncodeAll(data, nil)
		encoder.Close()

		decoder, _ := zstd.NewReader(nil)
		defer decoder.Close()

		b.Run(sz.name, func(b *testing.B) {
			b.SetBytes(int64(len(compressedData)))
			b.ReportAllocs()

			for i := 0; i < b.N; i++ {
				_, _ = decoder.DecodeAll(compressedData, nil)
			}
		})
	}
}

// ===========================================================================
// Zlib Compression Benchmarks
// ===========================================================================

func BenchmarkZlibCompress(b *testing.B) {
	sizes := []struct {
		name string
		size int
	}{
		{"100B", 100},
		{"1KB", 1024},
		{"10KB", 10 * 1024},
		{"100KB", 100 * 1024},
	}

	levels := []struct {
		name  string
		level int
	}{
		{"Fast", zlib.BestSpeed},
		{"Balanced", zlib.DefaultCompression},
	}

	for _, sz := range sizes {
		data := generateTestData(sz.size)

		for _, lvl := range levels {
			b.Run(sz.name+"/"+lvl.name, func(b *testing.B) {
				b.SetBytes(int64(sz.size))
				b.ReportAllocs()

				for i := 0; i < b.N; i++ {
					var buf bytes.Buffer
					w, _ := zlib.NewWriterLevel(&buf, lvl.level)
					_, _ = w.Write(data)
					_ = w.Close()
					_ = buf.Bytes()
				}
			})
		}
	}
}

func BenchmarkZlibDecompress(b *testing.B) {
	sizes := []struct {
		name string
		size int
	}{
		{"100B", 100},
		{"1KB", 1024},
		{"10KB", 10 * 1024},
		{"100KB", 100 * 1024},
	}

	for _, sz := range sizes {
		data := generateTestData(sz.size)

		// Pre-compress the data
		var compressed bytes.Buffer
		w, _ := zlib.NewWriterLevel(&compressed, zlib.BestSpeed)
		_, _ = w.Write(data)
		_ = w.Close()
		compressedData := compressed.Bytes()

		b.Run(sz.name, func(b *testing.B) {
			b.SetBytes(int64(len(compressedData)))
			b.ReportAllocs()

			for i := 0; i < b.N; i++ {
				r, _ := zlib.NewReader(bytes.NewReader(compressedData))
				_, _ = io.ReadAll(r)
				_ = r.Close()
			}
		})
	}
}

// ===========================================================================
// Compression Ratio Benchmarks
// ===========================================================================

func BenchmarkCompressionRatio(b *testing.B) {
	// Load realistic fixture if available
	fixture, err := loadFixture("blocks_100.json")
	if err != nil {
		// Use generated data as fallback
		b.Skip("Fixture not available, run generate_fixtures.go first")
		return
	}

	// Serialize fixture to bytes
	data, _ := json.Marshal(fixture)

	b.Run("Brotli", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			var buf bytes.Buffer
			w := brotli.NewWriterLevel(&buf, 6)
			_, _ = w.Write(data)
			_ = w.Close()
			compressed := buf.Bytes()
			b.ReportMetric(float64(len(compressed))/float64(len(data))*100, "ratio%")
		}
	})

	b.Run("Zstd", func(b *testing.B) {
		encoder, _ := zstd.NewWriter(nil)
		defer encoder.Close()
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			compressed := encoder.EncodeAll(data, nil)
			b.ReportMetric(float64(len(compressed))/float64(len(data))*100, "ratio%")
		}
	})

	b.Run("Zlib", func(b *testing.B) {
		b.ReportAllocs()
		for i := 0; i < b.N; i++ {
			var buf bytes.Buffer
			w, _ := zlib.NewWriterLevel(&buf, zlib.DefaultCompression)
			_, _ = w.Write(data)
			_ = w.Close()
			compressed := buf.Bytes()
			b.ReportMetric(float64(len(compressed))/float64(len(data))*100, "ratio%")
		}
	})
}
