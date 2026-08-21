//===----------------------------------------------------------------------===//
// Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
// Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package gobsmrifylib

import (
	"fmt"
	"slices"
	"strings"
)

// OSDeps is a map of Bsmr-OS to ArchDeps
type OSDeps struct {
	OS       string
	ArchDeps map[string]*ArchDeps
}

// ArchDeps is a map of Bsmr-Arch to set of go packages
type ArchDeps struct {
	Arch string
	Deps *StringSet
}

// BsmrTarget is a bsmr-friendly representation of a go package
type BsmrTarget struct {
	Name                 string
	ImportPath           string
	IsBinary             bool
	EmbedFiles           StringSet
	CommonDeps           []string
	PlatformDeps         map[string]*OSDeps
	TargetCompatibleWith map[string][]string // os => []arch
}

// Normalise prepares BsmrTarget to be written to a file:
// - Moves common dependencies to CommonDeps out of PlatformDeps
// - Removes empty PlatformDeps
// - Removes TargetCompatibleWith if all platforms are compatible
func (b *BsmrTarget) Normalise(totalPlatformNumber int) {
	freqmap := make(map[string]int)
	for _, osDeps := range b.PlatformDeps {
		for _, archDeps := range osDeps.ArchDeps {
			for dep := range *archDeps.Deps {
				freqmap[dep]++
			}
		}
	}

	// If a dependency is used in all platforms, it can be moved to CommonDeps
	for dep, freq := range freqmap {
		if freq == totalPlatformNumber {
			b.CommonDeps = append(b.CommonDeps, dep)
			for _, osDeps := range b.PlatformDeps {
				for _, archDeps := range osDeps.ArchDeps {
					archDeps.Deps.Remove(dep)
				}
			}
		}
	}

	// Remove empty PlatformDeps
	for os, osDeps := range b.PlatformDeps {
		for arch, archDeps := range osDeps.ArchDeps {
			if archDeps.Deps.Len() == 0 {
				delete(osDeps.ArchDeps, arch)
			}
		}
		if len(osDeps.ArchDeps) == 0 {
			delete(b.PlatformDeps, os)
		}
	}
	slices.Sort(b.CommonDeps)

	compatibleWithNumber := 0
	for _, archList := range b.TargetCompatibleWith {
		compatibleWithNumber += len(archList)
	}

	if compatibleWithNumber == totalPlatformNumber {
		b.TargetCompatibleWith = nil // all platforms are compatible
	}

	for _, archList := range b.TargetCompatibleWith {
		slices.Sort(archList) // sort arch list to render it deterministically
	}
}

// BsmrTargets is a map of bsmr targets keyed by import path
type BsmrTargets map[string]*BsmrTarget

// AddPackage adds a package to the bsmr targets map
func (b *BsmrTargets) AddPackage(pkg *Package, bsmrOS, bsmrArch string) {
	// If package with the same import path and os/arch already added, its data be replaced
	var target *BsmrTarget
	var ok bool
	if target, ok = (*b)[pkg.ImportPath]; !ok {
		target = &BsmrTarget{
			Name:                 TargetNameFromImportPath(pkg.ImportPath),
			ImportPath:           pkg.ImportPath,
			PlatformDeps:         make(map[string]*OSDeps),
			EmbedFiles:           *NewSet(),
			IsBinary:             pkg.Name == "main",
			TargetCompatibleWith: map[string][]string{},
		}
		(*b)[pkg.ImportPath] = target
	}

	target.EmbedFiles.AddList(pkg.EmbedFiles)
	target.TargetCompatibleWith[bsmrOS] = append(target.TargetCompatibleWith[bsmrOS], bsmrArch)

	if target.PlatformDeps[bsmrOS] == nil {
		target.PlatformDeps[bsmrOS] = &OSDeps{
			OS:       bsmrOS,
			ArchDeps: make(map[string]*ArchDeps),
		}
	}

	target.PlatformDeps[bsmrOS].ArchDeps[bsmrArch] = &ArchDeps{
		Arch: bsmrArch,
		Deps: NewSet(),
	}

	for _, dep := range pkg.Imports {
		if !strings.ContainsRune(dep, '.') {
			continue // skip stdlib deps
		}
		target.PlatformDeps[bsmrOS].ArchDeps[bsmrArch].Deps.Add(dep)
	}
}

func TargetNameFromImportPath(importPath string) string {
	lastSlash := strings.LastIndex(importPath, "/")
	var targetName string
	if lastSlash == -1 {
		targetName = importPath
	} else {
		targetName = importPath[lastSlash+1:]
	}
	return targetName
}

func TargetLabelFromImportPath(targetLabelPrefix, importPath string) string {
	return fmt.Sprintf("%s%s:%s", targetLabelPrefix, importPath, TargetNameFromImportPath(importPath))
}
