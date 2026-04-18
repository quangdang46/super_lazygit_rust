package config

import (
	"github.com/quangdang46/slg/pkg/config"
	. "github.com/quangdang46/slg/pkg/integration/components"
)

var NegativeRefspec = NewIntegrationTest(NewIntegrationTestArgs{
	Description:  "Having a config with a negative refspec",
	ExtraCmdArgs: []string{},
	SetupRepo: func(shell *Shell) {
		shell.
			SetConfig("remote.origin.fetch", "^refs/heads/test").
			CreateNCommits(2)
	},
	SetupConfig: func(cfg *config.AppConfig) {},
	Run: func(t *TestDriver, keys config.KeybindingConfig) {
		// the failure case with an unpatched go-git is that no branches display
		t.Views().Branches().
			Lines(
				Contains("master"),
			)
	},
})
