package conflicts

import (
	"github.com/quangdang46/slg/pkg/config"
	. "github.com/quangdang46/slg/pkg/integration/components"
	"github.com/quangdang46/slg/pkg/integration/tests/shared"
)

var ResolveExternally = NewIntegrationTest(NewIntegrationTestArgs{
	Description:  "Ensures that when merge conflicts are resolved outside of slg, slg prompts you to continue",
	ExtraCmdArgs: []string{},
	Skip:         false,
	SetupConfig:  func(config *config.AppConfig) {},
	SetupRepo: func(shell *Shell) {
		shared.CreateMergeConflictFile(shell)
	},
	Run: func(t *TestDriver, keys config.KeybindingConfig) {
		t.Views().Files().
			IsFocused().
			Lines(
				Contains("UU file").IsSelected(),
			).
			Tap(func() {
				t.Shell().UpdateFile("file", "resolved content")
			}).
			Press(keys.Universal.Refresh)

		t.Common().ContinueOnConflictsResolved("merge")

		t.Views().Files().
			IsEmpty()
	},
})
