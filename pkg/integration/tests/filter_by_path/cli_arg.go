package filter_by_path

import (
	"github.com/quangdang46/slg/pkg/config"
	. "github.com/quangdang46/slg/pkg/integration/components"
)

var CliArg = NewIntegrationTest(NewIntegrationTestArgs{
	Description:  "Filter commits by file path, using CLI arg",
	ExtraCmdArgs: []string{"-f=filterFile"},
	Skip:         false,
	SetupConfig: func(config *config.AppConfig) {
	},
	SetupRepo: func(shell *Shell) {
		commonSetup(shell)
	},
	Run: func(t *TestDriver, keys config.KeybindingConfig) {
		postFilterTest(t)
	},
})
