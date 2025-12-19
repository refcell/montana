//go:build ignore

// This program generates test fixtures for benchmarking.
// Run with: go run generate_fixtures.go
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"
)

type BlockData struct {
	Timestamp    uint64        `json:"timestamp"`
	Transactions []Transaction `json:"transactions"`
}

type Transaction struct {
	Data string `json:"data"`
}

type BatchFixture struct {
	L1Origin   L1Origin    `json:"l1_origin"`
	ParentHash string      `json:"parent_hash"`
	Blocks     []BlockData `json:"blocks"`
}

type L1Origin struct {
	BlockNumber uint64 `json:"block_number"`
	HashPrefix  string `json:"hash_prefix"`
}

func generateFixture(blockCount, txPerBlock int) *BatchFixture {
	blocks := make([]BlockData, blockCount)
	for i := 0; i < blockCount; i++ {
		txs := make([]Transaction, txPerBlock)
		for j := 0; j < txPerBlock; j++ {
			// Generate realistic transaction data (variable length)
			dataLen := 32 + (j % 64) // 32-96 bytes
			txs[j] = Transaction{
				Data: "0x" + strings.Repeat(fmt.Sprintf("%02x", (i+j)%256), dataLen),
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

func main() {
	fixtures := []struct {
		name       string
		blockCount int
		txPerBlock int
	}{
		{"blocks_1.json", 1, 5},
		{"blocks_10.json", 10, 10},
		{"blocks_100.json", 100, 10},
		{"blocks_1000.json", 1000, 10},
	}

	// Ensure fixtures directory exists
	os.MkdirAll("../fixtures", 0o755)

	for _, f := range fixtures {
		fixture := generateFixture(f.blockCount, f.txPerBlock)
		data, err := json.MarshalIndent(fixture, "", "  ")
		if err != nil {
			fmt.Printf("Error marshaling %s: %v\n", f.name, err)
			continue
		}

		path := "../fixtures/" + f.name
		if err := os.WriteFile(path, data, 0o644); err != nil {
			fmt.Printf("Error writing %s: %v\n", path, err)
			continue
		}
		fmt.Printf("Generated %s (%d bytes)\n", path, len(data))
	}

	fmt.Println("\nDone!")
}
