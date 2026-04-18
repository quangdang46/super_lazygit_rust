package gui

import (
	"fmt"

	"github.com/quangdang46/slg/pkg/constants"
	"github.com/quangdang46/slg/pkg/gui/style"
	"github.com/quangdang46/slg/pkg/utils"
)

func (gui *Gui) informationStr() string {
	if activeMode, ok := gui.helpers.Mode.GetActiveMode(); ok {
		return activeMode.InfoLabel()
	}

	if gui.g.Mouse {
		donate := style.FgMagenta.Sprint(style.PrintHyperlink(gui.c.Tr.Donate, constants.Links.Donate))
		askQuestion := style.FgYellow.Sprint(style.PrintHyperlink(gui.c.Tr.AskQuestion, constants.Links.Discussions))
		return fmt.Sprintf("%s %s %s", donate, askQuestion, gui.Config.GetVersion())
	}

	return gui.Config.GetVersion()
}

func (gui *Gui) handleInfoClick() error {
	if !gui.g.Mouse {
		return nil
	}

	view := gui.Views.Information

	cx, _ := view.Cursor()
	width := view.Width()

	if activeMode, ok := gui.helpers.Mode.GetActiveMode(); ok {
		if width-cx > utils.StringWidth(gui.c.Tr.ResetInParentheses) {
			return nil
		}
		return activeMode.Reset()
	}

	return nil
}
