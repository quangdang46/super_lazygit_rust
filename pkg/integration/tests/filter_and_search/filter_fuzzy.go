package filter_and_search

import (
	"github.com/quangdang46/slg/pkg/config"
	. "github.com/quangdang46/slg/pkg/integration/components"
)

var FilterFuzzy = NewIntegrationTest(NewIntegrationTestArgs{
	Description:  "Verify that fuzzy filtering works (not just exact matches)",
	ExtraCmdArgs: []string{},
	Skip:         false,
	SetupConfig: func(config *config.AppConfig) {
		config.GetUserConfig().Gui.FilterMode = "fuzzy"
	},
	SetupRepo: func(shell *Shell) {
		shell.NewBranch("this-is-my-branch")
		shell.EmptyCommit("first commit")
		shell.NewBranch("other-branch")
	},
	Run: func(t *TestDriver, keys config.KeybindingConfig) {
		t.Views().Branches().
			Focus().
			Lines(
				Contains(`other-branch`).IsSelected(),
				Contains(`this-is-my-branch`),
			).
			FilterOrSearch("timb"). // using first letters of words
			Lines(
				Contains(`this-is-my-branch`).IsSelected(),
			).
			FilterOrSearch("brnch"). // allows missing letter
			Lines(
				Contains(`other-branch`).IsSelected(),
				Contains(`this-is-my-branch`),
			)
	},
})
