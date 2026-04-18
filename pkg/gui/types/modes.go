package types

import (
	"github.com/quangdang46/slg/pkg/gui/modes/cherrypicking"
	"github.com/quangdang46/slg/pkg/gui/modes/diffing"
	"github.com/quangdang46/slg/pkg/gui/modes/filtering"
	"github.com/quangdang46/slg/pkg/gui/modes/marked_base_commit"
)

type Modes struct {
	Filtering        filtering.Filtering
	CherryPicking    *cherrypicking.CherryPicking
	Diffing          diffing.Diffing
	MarkedBaseCommit marked_base_commit.MarkedBaseCommit
}
