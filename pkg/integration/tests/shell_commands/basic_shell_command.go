package shell_commands

import (
	"github.com/quangdang46/slg/pkg/config"
	. "github.com/quangdang46/slg/pkg/integration/components"
)

var BasicShellCommand = NewIntegrationTest(NewIntegrationTestArgs{
	Description:  "Using a custom command provided at runtime to create a new file",
	ExtraCmdArgs: []string{},
	Skip:         false,
	SetupRepo: func(shell *Shell) {
		shell.EmptyCommit("blah")
	},
	SetupConfig: func(cfg *config.AppConfig) {},
	Run: func(t *TestDriver, keys config.KeybindingConfig) {
		t.Views().Files().
			IsEmpty().
			IsFocused().
			Press(keys.Universal.ExecuteShellCommand)

		t.ExpectPopup().Prompt().
			Title(Equals("Shell command:")).
			Type("touch file.txt").
			Confirm()

		t.GlobalPress(keys.Files.RefreshFiles)

		t.Views().Files().
			IsFocused().
			Lines(
				Contains("file.txt"),
			)
	},
})
