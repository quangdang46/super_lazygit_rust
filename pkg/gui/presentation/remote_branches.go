package presentation

import (
	"github.com/quangdang46/slg/pkg/commands/models"
	"github.com/quangdang46/slg/pkg/gui/presentation/icons"
	"github.com/quangdang46/slg/pkg/theme"
	"github.com/samber/lo"
)

func GetRemoteBranchListDisplayStrings(branches []*models.RemoteBranch, diffName string) [][]string {
	return lo.Map(branches, func(branch *models.RemoteBranch, _ int) []string {
		diffed := branch.FullName() == diffName
		return getRemoteBranchDisplayStrings(branch, diffed)
	})
}

// getRemoteBranchDisplayStrings returns the display string of branch
func getRemoteBranchDisplayStrings(b *models.RemoteBranch, diffed bool) []string {
	textStyle := GetBranchTextStyle(b.Name)
	if diffed {
		textStyle = theme.DiffTerminalColor
	}

	res := make([]string, 0, 2)
	if icons.IsIconEnabled() {
		res = append(res, textStyle.Sprint(icons.IconForRemoteBranch(b)))
	}
	res = append(res, textStyle.Sprint(b.Name))
	return res
}
