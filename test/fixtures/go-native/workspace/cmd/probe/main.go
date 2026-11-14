package main

import (
	"debug/elf"
	"debug/macho"
	_ "embed"
	"encoding/json"
	"example.com/cache-probe/platform"
	"fmt"
	"os"
	"strings"
)

//go:embed message.txt
var message string
var _ = platform.Name

func main() {
	if len(os.Args) == 3 {
		machine, dwarf := uint16(0), uint64(0)
		switch os.Args[1] {
		case "linux":
			file, err := elf.Open(os.Args[2])
			if err != nil {
				panic(err)
			}
			defer file.Close()
			machine = uint16(file.Machine)
			for _, section := range file.Sections {
				if strings.HasPrefix(section.Name, ".debug_") || strings.HasPrefix(section.Name, ".zdebug_") {
					dwarf += section.Size
				}
			}
		case "darwin":
			file, err := macho.Open(os.Args[2])
			if err != nil {
				panic(err)
			}
			defer file.Close()
			for _, section := range file.Sections {
				if section.Seg == "__DWARF" {
					dwarf += section.Size
				}
			}
		default:
			panic("unsupported binary format")
		}
		if err := json.NewEncoder(os.Stdout).Encode(map[string]any{"machine": machine, "dwarf_bytes": dwarf}); err != nil {
			panic(err)
		}
		return
	}
	fmt.Print(prefix + message)
}
