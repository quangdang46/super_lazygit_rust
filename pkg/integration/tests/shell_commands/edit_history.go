package shell_commands

import (
	"github.com/quangdang46/slg/pkg/config"
	. "github.com/quangdang46/slg/pkg/integration/components"
)

var EditHistory = NewIntegrationTest(NewIntegrationTestArgs{
	Description:  "Edit an entry from the custom commands history",
	ExtraCmdArgs: []string{},
	Skip:         false,
	SetupRepo:    func(shell *Shell) {},
	SetupConfig:  func(cfg *config.AppConfig) {},
	Run: func(t *TestDriver, keys config.KeybindingConfig) {
		t.GlobalPress(keys.Universal.ExecuteShellCommand)
		t.ExpectPopup().Prompt().
			Title(Equals("Shell command:")).
			Type("echo x").
			Confirm()

		t.GlobalPress(keys.Universal.ExecuteShellCommand)
		t.ExpectPopup().Prompt().
			Title(Equals("Shell command:")).
			Type("ec").
			SuggestionLines(
				Equals("echo x"),
			).
			EditSuggestion(Equals("echo x")).
			InitialText(Equals("echo x"))
	},
})
