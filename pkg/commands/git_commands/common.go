package git_commands

import (
	"github.com/quangdang46/slg/pkg/commands/oscommands"
	"github.com/quangdang46/slg/pkg/common"
	"github.com/quangdang46/slg/pkg/config"
)

type GitCommon struct {
	*common.Common
	version     *GitVersion
	cmd         oscommands.ICmdObjBuilder
	os          *oscommands.OSCommand
	repoPaths   *RepoPaths
	config      *ConfigCommands
	pagerConfig *config.PagerConfig
}

func NewGitCommon(
	cmn *common.Common,
	version *GitVersion,
	cmd oscommands.ICmdObjBuilder,
	osCommand *oscommands.OSCommand,
	repoPaths *RepoPaths,
	config *ConfigCommands,
	pagerConfig *config.PagerConfig,
) *GitCommon {
	return &GitCommon{
		Common:      cmn,
		version:     version,
		cmd:         cmd,
		os:          osCommand,
		repoPaths:   repoPaths,
		config:      config,
		pagerConfig: pagerConfig,
	}
}
