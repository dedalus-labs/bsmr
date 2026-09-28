package main

import (
	"fmt"
	"runtime/debug"

	"example.com/greeting"
)

func main() {
	info, ok := debug.ReadBuildInfo()
	if !ok {
		panic("binary carries no build info")
	}
	fmt.Printf("%s\n%s", greeting.Subject, info)
}
