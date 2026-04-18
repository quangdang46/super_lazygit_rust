package branch

import (
	"github.com/quangdang46/slg/pkg/config"
	. "github.com/quangdang46/slg/pkg/integration/components"
	"github.com/samber/lo"
)

func checkRemoteBranches(t *TestDriver, keys config.KeybindingConfig, remoteName string, expectedBranches []string) {
	t.Views().Remotes().
		Focus().
		NavigateToLine(Contains(remoteName)).
		PressEnter()

	t.Views().
		RemoteBranches().
		Lines(
			lo.Map(expectedBranches, func(branch string, _ int) *TextMatcher { return Equals(branch) })...,
		).
		Press(keys.Universal.Return)

	t.Views().
		Branches().
		Focus()
}
