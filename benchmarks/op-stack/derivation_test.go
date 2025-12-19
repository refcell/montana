package opstack

import (
	"bytes"
	"encoding/binary"
	"io"
	"testing"

	"github.com/andybalholm/brotli"
	"github.com/klauspost/compress/zstd"
)

// ===========================================================================
// Batch Decoding Benchmarks
// ===========================================================================

func decodeBatchBody(data []byte, blockCount int) ([]BlockData, error) {
	blocks := make([]BlockData, 0, blockCount)
	offset := 0

	for i := 0; i < blockCount && offset < len(data); i++ {
		if offset+4 > len(data) {
			break
		}

		// timestamp_delta (u16)
		timestampDelta := binary.LittleEndian.Uint16(data[offset : offset+2])
		offset += 2

		// tx_count (u16)
		txCount := binary.LittleEndian.Uint16(data[offset : offset+2])
		offset += 2

		txs := make([]Transaction, 0, txCount)
		for j := uint16(0); j < txCount; j++ {
			if offset+3 > len(data) {
				break
			}

			// tx_len (u24 - 3 bytes little-endian)
			txLen := uint32(data[offset]) |
				uint32(data[offset+1])<<8 |
				uint32(data[offset+2])<<16
			offset += 3

			if offset+int(txLen) > len(data) {
				break
			}

			// tx_data
			txData := data[offset : offset+int(txLen)]
			offset += int(txLen)

			txs = append(txs, Transaction{
				Data: "0x" + string(txData),
			})
		}

		blocks = append(blocks, BlockData{
			Timestamp:    uint64(timestampDelta),
			Transactions: txs,
		})
	}

	return blocks, nil
}

func BenchmarkBatchDecode(b *testing.B) {
	blockCounts := []int{1, 10, 100}

	for _, count := range blockCounts {
		// Generate and encode a batch
		fixture := generateFixture(count, 10)
		header := BatchHeader{
			Version:     0,
			BatchNumber: 1,
			Timestamp:   fixture.Blocks[0].Timestamp,
			BlockCount:  uint16(len(fixture.Blocks)),
		}
		headerBytes := header.Encode()
		bodyBytes := encodeBatchBody(fixture.Blocks, fixture.Blocks[0].Timestamp)
		batch := append(headerBytes, bodyBytes...)

		b.Run(formatBlockCount(count), func(b *testing.B) {
			b.SetBytes(int64(len(batch)))
			b.ReportAllocs()

			for i := 0; i < b.N; i++ {
				// Decode header
				h, _ := DecodeBatchHeader(batch)

				// Decode body
				body := batch[19:]
				blocks, _ := decodeBatchBody(body, int(h.BlockCount))
				_ = blocks
			}
		})
	}
}

// ===========================================================================
// Decompression + Decode Benchmarks (Derivation Pipeline)
// ===========================================================================

func BenchmarkDerivationPipeline(b *testing.B) {
	blockCounts := []int{1, 10, 100}

	for _, count := range blockCounts {
		// Generate, encode, and compress a batch
		fixture := generateFixture(count, 10)
		header := BatchHeader{
			Version:     0,
			BatchNumber: 1,
			Timestamp:   fixture.Blocks[0].Timestamp,
			BlockCount:  uint16(len(fixture.Blocks)),
		}
		headerBytes := header.Encode()
		bodyBytes := encodeBatchBody(fixture.Blocks, fixture.Blocks[0].Timestamp)
		batch := append(headerBytes, bodyBytes...)

		// Pre-compress with Brotli
		var brotliBuf bytes.Buffer
		brotliWriter := brotli.NewWriterLevel(&brotliBuf, brotli.BestSpeed)
		_, _ = brotliWriter.Write(batch)
		_ = brotliWriter.Close()
		brotliCompressed := brotliBuf.Bytes()

		// Pre-compress with Zstd
		zstdEncoder, _ := zstd.NewWriter(nil, zstd.WithEncoderLevel(zstd.SpeedFastest))
		zstdCompressed := zstdEncoder.EncodeAll(batch, nil)
		zstdEncoder.Close()

		zstdDecoder, _ := zstd.NewReader(nil)
		defer zstdDecoder.Close()

		b.Run(formatBlockCount(count)+"/Brotli", func(b *testing.B) {
			b.SetBytes(int64(len(brotliCompressed)))
			b.ReportAllocs()

			for i := 0; i < b.N; i++ {
				// Decompress
				r := brotli.NewReader(bytes.NewReader(brotliCompressed))
				decompressed, _ := io.ReadAll(r)

				// Decode header
				h, _ := DecodeBatchHeader(decompressed)

				// Decode body
				body := decompressed[19:]
				blocks, _ := decodeBatchBody(body, int(h.BlockCount))
				_ = blocks
			}
		})

		b.Run(formatBlockCount(count)+"/Zstd", func(b *testing.B) {
			b.SetBytes(int64(len(zstdCompressed)))
			b.ReportAllocs()

			for i := 0; i < b.N; i++ {
				// Decompress
				decompressed, _ := zstdDecoder.DecodeAll(zstdCompressed, nil)

				// Decode header
				h, _ := DecodeBatchHeader(decompressed)

				// Decode body
				body := decompressed[19:]
				blocks, _ := decodeBatchBody(body, int(h.BlockCount))
				_ = blocks
			}
		})
	}
}

// ===========================================================================
// Channel Assembly Benchmarks
// ===========================================================================

// Frame represents an OP Stack frame in a channel
type Frame struct {
	ChannelID [16]byte
	FrameNum  uint16
	Data      []byte
	IsLast    bool
}

func encodeFrame(f *Frame) []byte {
	// Frame encoding:
	// - channel_id: 16 bytes
	// - frame_number: 2 bytes (big-endian)
	// - frame_data_length: 4 bytes (big-endian)
	// - frame_data: variable
	// - is_last: 1 byte
	buf := make([]byte, 16+2+4+len(f.Data)+1)
	copy(buf[0:16], f.ChannelID[:])
	binary.BigEndian.PutUint16(buf[16:18], f.FrameNum)
	binary.BigEndian.PutUint32(buf[18:22], uint32(len(f.Data)))
	copy(buf[22:22+len(f.Data)], f.Data)
	if f.IsLast {
		buf[22+len(f.Data)] = 1
	}
	return buf
}

func decodeFrame(data []byte) (*Frame, int, error) {
	if len(data) < 23 {
		return nil, 0, nil
	}

	var channelID [16]byte
	copy(channelID[:], data[0:16])
	frameNum := binary.BigEndian.Uint16(data[16:18])
	dataLen := binary.BigEndian.Uint32(data[18:22])

	if len(data) < 22+int(dataLen)+1 {
		return nil, 0, nil
	}

	frameData := make([]byte, dataLen)
	copy(frameData, data[22:22+dataLen])
	isLast := data[22+dataLen] == 1

	return &Frame{
		ChannelID: channelID,
		FrameNum:  frameNum,
		Data:      frameData,
		IsLast:    isLast,
	}, 22 + int(dataLen) + 1, nil
}

func BenchmarkChannelAssembly(b *testing.B) {
	// Create a channel split into multiple frames
	frameCounts := []int{1, 5, 10}

	for _, frameCount := range frameCounts {
		// Generate channel data
		channelData := generateTestData(10 * 1024) // 10KB channel
		frameSize := len(channelData) / frameCount
		var channelID [16]byte
		copy(channelID[:], []byte("test-channel-id!"))

		// Create frames
		frames := make([][]byte, frameCount)
		for i := 0; i < frameCount; i++ {
			start := i * frameSize
			end := start + frameSize
			if i == frameCount-1 {
				end = len(channelData)
			}
			frame := &Frame{
				ChannelID: channelID,
				FrameNum:  uint16(i),
				Data:      channelData[start:end],
				IsLast:    i == frameCount-1,
			}
			frames[i] = encodeFrame(frame)
		}

		// Concatenate all frames
		var allFrames []byte
		for _, f := range frames {
			allFrames = append(allFrames, f...)
		}

		b.Run(formatFrameCount(frameCount), func(b *testing.B) {
			b.SetBytes(int64(len(allFrames)))
			b.ReportAllocs()

			for i := 0; i < b.N; i++ {
				// Simulate channel assembly: decode all frames and reassemble
				var assembledData []byte
				offset := 0
				for offset < len(allFrames) {
					frame, consumed, _ := decodeFrame(allFrames[offset:])
					if frame == nil || consumed == 0 {
						break
					}
					assembledData = append(assembledData, frame.Data...)
					offset += consumed
				}
				_ = assembledData
			}
		})
	}
}

func formatFrameCount(count int) string {
	switch count {
	case 1:
		return "1_frame"
	case 5:
		return "5_frames"
	case 10:
		return "10_frames"
	default:
		return ""
	}
}

// ===========================================================================
// Roundtrip Benchmarks (Encode -> Compress -> Decompress -> Decode)
// ===========================================================================

func BenchmarkRoundtrip(b *testing.B) {
	blockCounts := []int{1, 10, 100}

	for _, count := range blockCounts {
		fixture := generateFixture(count, 10)

		b.Run(formatBlockCount(count)+"/Brotli", func(b *testing.B) {
			b.ReportAllocs()

			for i := 0; i < b.N; i++ {
				// === Batch Submission (L2 -> L1) ===

				// Encode
				header := BatchHeader{
					Version:     0,
					BatchNumber: 1,
					Timestamp:   fixture.Blocks[0].Timestamp,
					BlockCount:  uint16(len(fixture.Blocks)),
				}
				headerBytes := header.Encode()
				bodyBytes := encodeBatchBody(fixture.Blocks, fixture.Blocks[0].Timestamp)
				batch := append(headerBytes, bodyBytes...)

				// Compress
				var buf bytes.Buffer
				w := brotli.NewWriterLevel(&buf, brotli.BestSpeed)
				_, _ = w.Write(batch)
				_ = w.Close()
				compressed := buf.Bytes()

				// === Derivation (L1 -> L2) ===

				// Decompress
				r := brotli.NewReader(bytes.NewReader(compressed))
				decompressed, _ := io.ReadAll(r)

				// Decode
				h, _ := DecodeBatchHeader(decompressed)
				body := decompressed[19:]
				blocks, _ := decodeBatchBody(body, int(h.BlockCount))
				_ = blocks
			}
		})

		b.Run(formatBlockCount(count)+"/Zstd", func(b *testing.B) {
			encoder, _ := zstd.NewWriter(nil, zstd.WithEncoderLevel(zstd.SpeedFastest))
			decoder, _ := zstd.NewReader(nil)
			defer encoder.Close()
			defer decoder.Close()
			b.ReportAllocs()

			for i := 0; i < b.N; i++ {
				// === Batch Submission (L2 -> L1) ===

				// Encode
				header := BatchHeader{
					Version:     0,
					BatchNumber: 1,
					Timestamp:   fixture.Blocks[0].Timestamp,
					BlockCount:  uint16(len(fixture.Blocks)),
				}
				headerBytes := header.Encode()
				bodyBytes := encodeBatchBody(fixture.Blocks, fixture.Blocks[0].Timestamp)
				batch := append(headerBytes, bodyBytes...)

				// Compress
				compressed := encoder.EncodeAll(batch, nil)

				// === Derivation (L1 -> L2) ===

				// Decompress
				decompressed, _ := decoder.DecodeAll(compressed, nil)

				// Decode
				h, _ := DecodeBatchHeader(decompressed)
				body := decompressed[19:]
				blocks, _ := decodeBatchBody(body, int(h.BlockCount))
				_ = blocks
			}
		})
	}
}
