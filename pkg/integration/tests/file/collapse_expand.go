package file

import (
	"github.com/quangdang46/slg/pkg/config"
	. "github.com/quangdang46/slg/pkg/integration/components"
)

var CollapseExpand = NewIntegrationTest(NewIntegrationTestArgs{
	Description:  "Collapsing and expanding all files in the file tree",
	ExtraCmdArgs: []string{},
	Skip:         false,
	SetupConfig: func(config *config.AppConfig) {
	},
	SetupRepo: func(shell *Shell) {
		shell.CreateDir("dir")
		shell.CreateFile("dir/file-one", "original content\n")
		shell.CreateDir("dir2")
		shell.CreateFile("dir2/file-two", "original content\n")
	},
	Run: func(t *TestDriver, keys config.KeybindingConfig) {
		t.Views().Files().
			IsFocused().
			Lines(
				Equals("▼ /").IsSelected(),
				Equals("  ▼ dir"),
				Equals("    ?? file-one"),
				Equals("  ▼ dir2"),
				Equals("    ?? file-two"),
			)

		t.Views().Files().
			Press(keys.Files.CollapseAll).
			Lines(
				Equals("▶ /"),
			)

		t.Views().Files().
			Press(keys.Files.ExpandAll).
			Lines(
				Equals("▼ /").IsSelected(),
				Equals("  ▼ dir"),
				Equals("    ?? file-one"),
				Equals("  ▼ dir2"),
				Equals("    ?? file-two"),
			)
	},
})
