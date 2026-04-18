package demo

import (
	"github.com/quangdang46/slg/pkg/config"
	. "github.com/quangdang46/slg/pkg/integration/components"
)

var CommitGraph = NewIntegrationTest(NewIntegrationTestArgs{
	Description:  "Show commit graph",
	ExtraCmdArgs: []string{"log", "--screen-mode=full"},
	Skip:         false,
	IsDemo:       true,
	SetupConfig: func(config *config.AppConfig) {
		setDefaultDemoConfig(config)
		setGeneratedAuthorColours(config)
	},
	SetupRepo: func(shell *Shell) {
		shell.CreateRepoHistory()
	},
	Run: func(t *TestDriver, keys config.KeybindingConfig) {
		t.SetCaptionPrefix("View commit log")
		t.Wait(1000)

		t.Views().Commits().
			IsFocused().
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100).
			SelectNextItem().
			Wait(100)
	},
})
