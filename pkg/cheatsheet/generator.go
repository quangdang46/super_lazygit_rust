//go:build ignore

package main

import (
	"fmt"

	"github.com/quangdang46/slg/pkg/cheatsheet"
)

func main() {
	fmt.Printf("Generating cheatsheets in %s...\n", cheatsheet.GetKeybindingsDir())
	cheatsheet.Generate()
}
