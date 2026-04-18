package constants

type Docs struct {
	CustomPagers      string
	CustomCommands    string
	CustomKeybindings string
	Keybindings       string
	Undoing           string
	Config            string
	Tutorial          string
	CustomPatchDemo   string
}

var Links = struct {
	Docs        Docs
	Issues      string
	Donate      string
	Discussions string
	RepoUrl     string
	Releases    string
}{
	RepoUrl:     "https://github.com/quangdang46/slg",
	Issues:      "https://github.com/quangdang46/slg/issues",
	Donate:      "https://github.com/sponsors/quangdang46",
	Discussions: "https://github.com/quangdang46/slg/discussions",
	Releases:    "https://github.com/quangdang46/slg/releases",
	Docs: Docs{
		CustomPagers:      "https://github.com/quangdang46/slg/blob/master/docs/Custom_Pagers.md",
		CustomKeybindings: "https://github.com/quangdang46/slg/blob/master/docs/keybindings/Custom_Keybindings.md",
		CustomCommands:    "https://github.com/quangdang46/slg/wiki/Custom-Commands-Compendium",
		Keybindings:       "https://github.com/quangdang46/slg/blob/%s/docs/keybindings",
		Undoing:           "https://github.com/quangdang46/slg/blob/master/docs/Undoing.md",
		Config:            "https://github.com/quangdang46/slg/blob/%s/docs/Config.md",
		Tutorial:          "https://youtu.be/VDXvbHZYeKY",
		CustomPatchDemo:   "https://github.com/quangdang46/slg#rebase-magic-custom-patches",
	},
}
