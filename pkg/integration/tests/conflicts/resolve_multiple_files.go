package conflicts

import (
	"github.com/quangdang46/slg/pkg/config"
	. "github.com/quangdang46/slg/pkg/integration/components"
	"github.com/quangdang46/slg/pkg/integration/tests/shared"
)

var ResolveMultipleFiles = NewIntegrationTest(NewIntegrationTestArgs{
	Description:  "Ensures that upon resolving conflicts for one file, the next file is selected",
	ExtraCmdArgs: []string{},
	Skip:         false,
	SetupConfig:  func(config *config.AppConfig) {},
	SetupRepo: func(shell *Shell) {
		shared.CreateMergeConflictFiles(shell)
	},
	Run: func(t *TestDriver, keys config.KeybindingConfig) {
		t.Views().Files().
			IsFocused().
			Lines(
				Equals("▼ /").IsSelected(),
				Equals("  UU file1"),
				Equals("  UU file2"),
			).
			SelectNextItem().
			PressEnter()

		t.Views().MergeConflicts().
			IsFocused().
			SelectedLines(
				Contains("<<<<<<< HEAD"),
				Contains("First Change"),
				Contains("======="),
			).
			PressPrimaryAction()

		t.Views().Files().
			IsFocused().
			Lines(
				Equals("UU file2").IsSelected(),
			).
			PressEnter()

		// coincidentally these files have the same conflict
		t.Views().MergeConflicts().
			IsFocused().
			SelectedLines(
				Contains("<<<<<<< HEAD"),
				Contains("First Change"),
				Contains("======="),
			).
			PressPrimaryAction()

		t.Common().ContinueOnConflictsResolved("merge")
	},
})
