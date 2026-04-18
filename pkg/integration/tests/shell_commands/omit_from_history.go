package shell_commands

import (
	"github.com/quangdang46/slg/pkg/config"
	. "github.com/quangdang46/slg/pkg/integration/components"
)

var OmitFromHistory = NewIntegrationTest(NewIntegrationTestArgs{
	Description:  "Omitting a runtime custom command from history if it begins with space",
	ExtraCmdArgs: []string{},
	Skip:         false,
	SetupRepo: func(shell *Shell) {
		shell.EmptyCommit("blah")
	},
	SetupConfig: func(cfg *config.AppConfig) {},
	Run: func(t *TestDriver, keys config.KeybindingConfig) {
		t.GlobalPress(keys.Universal.ExecuteShellCommand)
		t.ExpectPopup().Prompt().
			Title(Equals("Shell command:")).
			Type("echo aubergine").
			Confirm()

		t.GlobalPress(keys.Universal.ExecuteShellCommand)
		t.ExpectPopup().Prompt().
			Title(Equals("Shell command:")).
			SuggestionLines(Contains("aubergine")).
			SuggestionLines(DoesNotContain("tangerine")).
			Type(" echo tangerine").
			Confirm()

		t.GlobalPress(keys.Universal.ExecuteShellCommand)
		t.ExpectPopup().Prompt().
			Title(Equals("Shell command:")).
			SuggestionLines(Contains("aubergine")).
			SuggestionLines(DoesNotContain("tangerine")).
			Cancel()
	},
})
