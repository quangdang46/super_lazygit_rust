package commit

import (
	"github.com/quangdang46/slg/pkg/config"
	. "github.com/quangdang46/slg/pkg/integration/components"
)

var Highlight = NewIntegrationTest(NewIntegrationTestArgs{
	Description:  "Verify that the commit view highlights the correct lines",
	ExtraCmdArgs: []string{},
	Skip:         false,
	SetupConfig: func(config *config.AppConfig) {
		config.GetUserConfig().Git.Log.ShowGraph = "always"
		config.GetUserConfig().Gui.AuthorColors = map[string]string{
			"CI": "red",
		}
	},
	SetupRepo: func(shell *Shell) {
		shell.EmptyCommit("one")
		shell.EmptyCommit("two")
		shell.EmptyCommit("three")
	},
	Run: func(t *TestDriver, keys config.KeybindingConfig) {
		highlightedColor := "#ffffff"

		t.Views().Commits().
			DoesNotContainColoredText(highlightedColor, "◯").
			Focus().
			ContainsColoredText(highlightedColor, "◯")

		t.Views().Files().
			Focus()

		t.Views().Commits().
			DoesNotContainColoredText(highlightedColor, "◯")
	},
})
