package presentation

import (
	"github.com/quangdang46/slg/pkg/commands/models"
	"github.com/quangdang46/slg/pkg/gui/presentation/icons"
	"github.com/quangdang46/slg/pkg/gui/style"
	"github.com/quangdang46/slg/pkg/i18n"
	"github.com/quangdang46/slg/pkg/theme"
	"github.com/quangdang46/slg/pkg/utils"
	"github.com/samber/lo"
)

func GetWorktreeDisplayStrings(tr *i18n.TranslationSet, worktrees []*models.Worktree) [][]string {
	return lo.Map(worktrees, func(worktree *models.Worktree, _ int) []string {
		return GetWorktreeDisplayString(
			tr,
			worktree)
	})
}

func GetWorktreeDisplayString(tr *i18n.TranslationSet, worktree *models.Worktree) []string {
	textStyle := theme.DefaultTextColor

	current := ""
	currentColor := style.FgCyan
	if worktree.IsCurrent {
		current = "  *"
		currentColor = style.FgGreen
	}

	icon := icons.IconForWorktree(false)
	if worktree.IsPathMissing {
		textStyle = style.FgRed
		icon = icons.IconForWorktree(true)
	}

	res := []string{}
	res = append(res, currentColor.Sprint(current))
	if icons.IsIconEnabled() {
		res = append(res, textStyle.Sprint(icon))
	}

	name := worktree.Name
	if worktree.IsPathMissing && !icons.IsIconEnabled() {
		name += " " + tr.MissingWorktree
	}
	res = append(res, textStyle.Sprint(name))
	var branch string
	if worktree.Branch != "" {
		branch = style.FgCyan.Sprint(worktree.Branch)
	} else if worktree.Head != "" {
		branch = style.FgYellow.Sprint("HEAD detached at " + utils.ShortHash(worktree.Head))
	}
	res = append(res, branch+mainWorktreeLabel(tr, worktree))
	return res
}

func mainWorktreeLabel(tr *i18n.TranslationSet, worktree *models.Worktree) string {
	if worktree.IsMain {
		return style.FgDefault.Sprint(" " + tr.MainWorktree)
	}
	return ""
}
