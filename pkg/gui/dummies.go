package gui

import (
	"github.com/quangdang46/slg/pkg/commands/git_commands"
	"github.com/quangdang46/slg/pkg/commands/oscommands"
	"github.com/quangdang46/slg/pkg/common"
	"github.com/quangdang46/slg/pkg/config"
	"github.com/quangdang46/slg/pkg/updates"
)

func NewDummyUpdater() *updates.Updater {
	newAppConfig := config.NewDummyAppConfig()
	dummyUpdater, _ := updates.NewUpdater(common.NewDummyCommon(), newAppConfig, oscommands.NewDummyOSCommand())
	return dummyUpdater
}

// NewDummyGui creates a new dummy GUI for testing
func NewDummyGui() *Gui {
	newAppConfig := config.NewDummyAppConfig()
	dummyGui, _ := NewGui(common.NewDummyCommon(), newAppConfig, &git_commands.GitVersion{Major: 2, Minor: 0, Patch: 0}, NewDummyUpdater(), false, "", nil)
	return dummyGui
}
