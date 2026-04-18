package stash

import (
	"github.com/quangdang46/slg/pkg/config"
	. "github.com/quangdang46/slg/pkg/integration/components"
)

var ShowWithBranchNamedStash = NewIntegrationTest(NewIntegrationTestArgs{
	Description:  "View stash when there is a branch also named 'stash'",
	ExtraCmdArgs: []string{},
	Skip:         false,
	SetupConfig:  func(config *config.AppConfig) {},
	SetupRepo: func(shell *Shell) {
		shell.EmptyCommit("initial commit")
		shell.CreateFile("file", "content")
		shell.GitAddAll()

		shell.NewBranch("stash")
	},
	Run: func(t *TestDriver, keys config.KeybindingConfig) {
		t.Views().Stash().
			IsEmpty()

		t.Views().Files().
			Lines(
				Contains("file"),
			).
			Press(keys.Files.StashAllChanges)

		t.ExpectPopup().Prompt().Title(Equals("Stash changes")).Type("my stashed file").Confirm()

		t.Views().Stash().
			Lines(
				MatchesRegexp(`\ds .* my stashed file`),
			)

		t.Views().Files().
			IsEmpty()

		t.Views().Stash().Focus()
		t.Views().Main().ContainsLines(Equals(" file | 1 +"))
	},
})
