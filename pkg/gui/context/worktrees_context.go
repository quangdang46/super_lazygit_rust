package context

import (
	"github.com/quangdang46/slg/pkg/commands/models"
	"github.com/quangdang46/slg/pkg/gui/presentation"
	"github.com/quangdang46/slg/pkg/gui/types"
)

type WorktreesContext struct {
	*FilteredListViewModel[*models.Worktree]
	*ListContextTrait
}

var _ types.IListContext = (*WorktreesContext)(nil)

func NewWorktreesContext(c *ContextCommon) *WorktreesContext {
	viewModel := NewFilteredListViewModel(
		func() []*models.Worktree { return c.Model().Worktrees },
		func(Worktree *models.Worktree) []string {
			return []string{Worktree.Name}
		},
	)

	getDisplayStrings := func(_ int, _ int) [][]string {
		return presentation.GetWorktreeDisplayStrings(
			c.Tr,
			viewModel.GetFilteredList(),
		)
	}

	return &WorktreesContext{
		FilteredListViewModel: viewModel,
		ListContextTrait: &ListContextTrait{
			Context: NewSimpleContext(NewBaseContext(NewBaseContextOpts{
				View:       c.Views().Worktrees,
				WindowName: "files",
				Key:        WORKTREES_CONTEXT_KEY,
				Kind:       types.SIDE_CONTEXT,
				Focusable:  true,
			})),
			ListRenderer: ListRenderer{
				list:              viewModel,
				getDisplayStrings: getDisplayStrings,
			},
			c: c,
		},
	}
}
