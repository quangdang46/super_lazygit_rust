package context

import (
	"github.com/quangdang46/slg/pkg/common"
	"github.com/quangdang46/slg/pkg/gui/types"
)

type ContextCommon struct {
	*common.Common
	types.IGuiCommon
}
