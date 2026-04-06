use std::collections::HashMap;
use std::sync::LazyLock;

use super::git_icons::LINKED_WORKTREE_ICON;
use super::IconProperties;

pub const DEFAULT_FILE_ICON: IconProperties = IconProperties {
    icon: "\u{f15b}",
    color: "#878787",
};
pub const DEFAULT_SUBMODULE_ICON: IconProperties = IconProperties {
    icon: "\u{f02a2}",
    color: "#FF4F00",
};
pub const DEFAULT_DIRECTORY_ICON: IconProperties = IconProperties {
    icon: "\u{f07b}",
    color: "#878787",
};

static NAME_ICON_MAP: LazyLock<HashMap<&'static str, IconProperties>> = LazyLock::new(|| {
    HashMap::from([
        (
            ".atom",
            IconProperties {
                icon: "\u{e764}",
                color: "#EED9B7",
            },
        ),
        (
            ".babelrc",
            IconProperties {
                icon: "\u{e639}",
                color: "#FED836",
            },
        ),
        (
            ".bash_profile",
            IconProperties {
                icon: "\u{e615}",
                color: "#89E051",
            },
        ),
        (
            ".bashprofile",
            IconProperties {
                icon: "\u{e615}",
                color: "#89E051",
            },
        ),
        (
            ".bashrc",
            IconProperties {
                icon: "\u{e795}",
                color: "#89E051",
            },
        ),
        (
            ".clang-format",
            IconProperties {
                icon: "\u{e615}",
                color: "#86806D",
            },
        ),
        (
            ".clang-tidy",
            IconProperties {
                icon: "\u{e615}",
                color: "#86806D",
            },
        ),
        (
            ".codespellrc",
            IconProperties {
                icon: "\u{f04c6}",
                color: "#35DA60",
            },
        ),
        (
            ".condarc",
            IconProperties {
                icon: "\u{e715}",
                color: "#43B02A",
            },
        ),
        (
            ".dockerignore",
            IconProperties {
                icon: "\u{f0868}",
                color: "#458EE6",
            },
        ),
        (
            ".ds_store",
            IconProperties {
                icon: "\u{f302}",
                color: "#78919C",
            },
        ),
        (
            ".editorconfig",
            IconProperties {
                icon: "\u{e652}",
                color: "#FFFFFF",
            },
        ),
        (
            ".env",
            IconProperties {
                icon: "\u{f066a}",
                color: "#FBC02D",
            },
        ),
        (
            ".eslintignore",
            IconProperties {
                icon: "\u{f0c7a}",
                color: "#3F52B5",
            },
        ),
        (
            ".eslintrc",
            IconProperties {
                icon: "\u{f0c7a}",
                color: "#3F52B5",
            },
        ),
        (
            ".git",
            IconProperties {
                icon: "\u{f02a2}",
                color: "#E64A19",
            },
        ),
        (
            ".git-blame-ignore-revs",
            IconProperties {
                icon: "\u{f02a2}",
                color: "#E64A19",
            },
        ),
        (
            ".gitattributes",
            IconProperties {
                icon: "\u{f02a2}",
                color: "#E64A19",
            },
        ),
        (
            ".gitconfig",
            IconProperties {
                icon: "\u{f02a2}",
                color: "#E64A19",
            },
        ),
        (
            ".github",
            IconProperties {
                icon: "\u{f408}",
                color: "#333333",
            },
        ),
        (
            ".gitignore",
            IconProperties {
                icon: "\u{f02a2}",
                color: "#E64A19",
            },
        ),
        (
            ".gitlab-ci.yml",
            IconProperties {
                icon: "\u{f296}",
                color: "#F54D27",
            },
        ),
        (
            ".gitmodules",
            IconProperties {
                icon: "\u{f02a2}",
                color: "#E64A19",
            },
        ),
        (
            ".gtkrc-2.0",
            IconProperties {
                icon: "\u{f362}",
                color: "#FFFFFF",
            },
        ),
        (
            ".gvimrc",
            IconProperties {
                icon: "\u{e62b}",
                color: "#019833",
            },
        ),
        (
            ".idea",
            IconProperties {
                icon: "\u{e7b5}",
                color: "#626262",
            },
        ),
        (
            ".justfile",
            IconProperties {
                icon: "\u{f0ad}",
                color: "#6D8086",
            },
        ),
        (
            ".luacheckrc",
            IconProperties {
                icon: "\u{e615}",
                color: "#868F9D",
            },
        ),
        (
            ".luaurc",
            IconProperties {
                icon: "\u{e615}",
                color: "#00A2FF",
            },
        ),
        (
            ".mailmap",
            IconProperties {
                icon: "\u{f01ee}",
                color: "#42A5F5",
            },
        ),
        (
            ".nanorc",
            IconProperties {
                icon: "\u{e838}",
                color: "#440077",
            },
        ),
        (
            ".npmignore",
            IconProperties {
                icon: "\u{ed0e}",
                color: "#CC3837",
            },
        ),
        (
            ".npmrc",
            IconProperties {
                icon: "\u{ed0e}",
                color: "#CC3837",
            },
        ),
        (
            ".nuxtrc",
            IconProperties {
                icon: "\u{f1106}",
                color: "#00C58E",
            },
        ),
        (
            ".nvmrc",
            IconProperties {
                icon: "\u{ed0d}",
                color: "#4CAF51",
            },
        ),
        (
            ".pre-commit-config.yaml",
            IconProperties {
                icon: "\u{f06e2}",
                color: "#F8B424",
            },
        ),
        (
            ".prettierignore",
            IconProperties {
                icon: "\u{e6b4}",
                color: "#4285F4",
            },
        ),
        (
            ".prettierrc",
            IconProperties {
                icon: "\u{e6b4}",
                color: "#4285F4",
            },
        ),
        (
            ".prettierrc.json",
            IconProperties {
                icon: "\u{e6b4}",
                color: "#4285F4",
            },
        ),
        (
            ".prettierrc.json5",
            IconProperties {
                icon: "\u{e6b4}",
                color: "#4285F4",
            },
        ),
        (
            ".prettierrc.toml",
            IconProperties {
                icon: "\u{e6b4}",
                color: "#4285F4",
            },
        ),
        (
            ".prettierrc.yaml",
            IconProperties {
                icon: "\u{e6b4}",
                color: "#4285F4",
            },
        ),
        (
            ".prettierrc.yml",
            IconProperties {
                icon: "\u{e6b4}",
                color: "#4285F4",
            },
        ),
        (
            ".pylintrc",
            IconProperties {
                icon: "\u{e615}",
                color: "#968F6D",
            },
        ),
        (
            ".rvm",
            IconProperties {
                icon: "\u{e21e}",
                color: "#D70000",
            },
        ),
        (
            ".settings.json",
            IconProperties {
                icon: "\u{e70c}",
                color: "#854CC7",
            },
        ),
        (
            ".SRCINFO",
            IconProperties {
                icon: "\u{f129}",
                color: "#0F94D2",
            },
        ),
        (
            ".tmux.conf",
            IconProperties {
                icon: "\u{ebc8}",
                color: "#14BA19",
            },
        ),
        (
            ".tmux.conf.local",
            IconProperties {
                icon: "\u{ebc8}",
                color: "#14BA19",
            },
        ),
        (
            ".Trash",
            IconProperties {
                icon: "\u{f1f8}",
                color: "#ACBCEF",
            },
        ),
        (
            ".vimrc",
            IconProperties {
                icon: "\u{e62b}",
                color: "#019833",
            },
        ),
        (
            ".vscode",
            IconProperties {
                icon: "\u{e70c}",
                color: "#007ACC",
            },
        ),
        (
            ".Xauthority",
            IconProperties {
                icon: "\u{f369}",
                color: "#E54D18",
            },
        ),
        (
            ".Xresources",
            IconProperties {
                icon: "\u{f369}",
                color: "#E54D18",
            },
        ),
        (
            ".xinitrc",
            IconProperties {
                icon: "\u{f369}",
                color: "#E54D18",
            },
        ),
        (
            ".xsession",
            IconProperties {
                icon: "\u{f369}",
                color: "#E54D18",
            },
        ),
        (
            ".zprofile",
            IconProperties {
                icon: "\u{e615}",
                color: "#89E051",
            },
        ),
        (
            ".zshenv",
            IconProperties {
                icon: "\u{e615}",
                color: "#89E051",
            },
        ),
        (
            ".zshrc",
            IconProperties {
                icon: "\u{e795}",
                color: "#89E051",
            },
        ),
        (
            "_gvimrc",
            IconProperties {
                icon: "\u{e62b}",
                color: "#019833",
            },
        ),
        (
            "_vimrc",
            IconProperties {
                icon: "\u{e62b}",
                color: "#019833",
            },
        ),
        (
            "AUTHORS",
            IconProperties {
                icon: "\u{edca}",
                color: "#A172FF",
            },
        ),
        (
            "AUTHORS.txt",
            IconProperties {
                icon: "\u{edca}",
                color: "#A172FF",
            },
        ),
        (
            "bin",
            IconProperties {
                icon: "\u{f12a7}",
                color: "#25A79A",
            },
        ),
        (
            "brewfile",
            IconProperties {
                icon: "\u{e791}",
                color: "#701516",
            },
        ),
        (
            "bspwmrc",
            IconProperties {
                icon: "\u{f355}",
                color: "#2F2F2F",
            },
        ),
        (
            "BUILD",
            IconProperties {
                icon: "\u{e63a}",
                color: "#89E051",
            },
        ),
        (
            "build.gradle",
            IconProperties {
                icon: "\u{e660}",
                color: "#005F87",
            },
        ),
        (
            "build.zig.zon",
            IconProperties {
                icon: "\u{e6a9}",
                color: "#F69A1B",
            },
        ),
        (
            "bun.lockb",
            IconProperties {
                icon: "\u{e76f}",
                color: "#EADCD1",
            },
        ),
        (
            "cantorrc",
            IconProperties {
                icon: "\u{f373}",
                color: "#1C99F3",
            },
        ),
        (
            "Cargo.lock",
            IconProperties {
                icon: "\u{e7a8}",
                color: "#DEA584",
            },
        ),
        (
            "Cargo.toml",
            IconProperties {
                icon: "\u{e7a8}",
                color: "#DEA584",
            },
        ),
        (
            "checkhealth",
            IconProperties {
                icon: "\u{f04d9}",
                color: "#75B4FB",
            },
        ),
        (
            "CMakeLists.txt",
            IconProperties {
                icon: "\u{e794}",
                color: "#DCE3EB",
            },
        ),
        (
            "CODE_OF_CONDUCT",
            IconProperties {
                icon: "\u{f4ae}",
                color: "#E41662",
            },
        ),
        (
            "CODE_OF_CONDUCT.md",
            IconProperties {
                icon: "\u{f4ae}",
                color: "#E41662",
            },
        ),
        (
            "CODE-OF-CONDUCT.md",
            IconProperties {
                icon: "\u{f4ae}",
                color: "#E41662",
            },
        ),
        (
            "commit_editmsg",
            IconProperties {
                icon: "\u{e702}",
                color: "#F54D27",
            },
        ),
        (
            "COMMIT_EDITMSG",
            IconProperties {
                icon: "\u{e702}",
                color: "#E54D18",
            },
        ),
        (
            "commitlint.config.js",
            IconProperties {
                icon: "\u{f0718}",
                color: "#039688",
            },
        ),
        (
            "commitlint.config.ts",
            IconProperties {
                icon: "\u{f0718}",
                color: "#039688",
            },
        ),
        (
            "compose.yaml",
            IconProperties {
                icon: "\u{f21f}",
                color: "#0088C9",
            },
        ),
        (
            "compose.yml",
            IconProperties {
                icon: "\u{f21f}",
                color: "#0088C9",
            },
        ),
        (
            "config",
            IconProperties {
                icon: "\u{f013}",
                color: "#696969",
            },
        ),
        (
            "containerfile",
            IconProperties {
                icon: "\u{f21f}",
                color: "#0088C9",
            },
        ),
        (
            "copying",
            IconProperties {
                icon: "\u{f0124}",
                color: "#FF5821",
            },
        ),
        (
            "copying.lesser",
            IconProperties {
                icon: "\u{e60a}",
                color: "#CBCB41",
            },
        ),
        (
            "docker-compose.yaml",
            IconProperties {
                icon: "\u{f21f}",
                color: "#0088C9",
            },
        ),
        (
            "docker-compose.yml",
            IconProperties {
                icon: "\u{f21f}",
                color: "#0088C9",
            },
        ),
        (
            "dockerfile",
            IconProperties {
                icon: "\u{f21f}",
                color: "#0088C9",
            },
        ),
        (
            "Dockerfile",
            IconProperties {
                icon: "\u{f308}",
                color: "#458EE6",
            },
        ),
        (
            "ds_store",
            IconProperties {
                icon: "\u{f179}",
                color: "#DDDDDD",
            },
        ),
        (
            "eslint.config.cjs",
            IconProperties {
                icon: "\u{f0c7a}",
                color: "#3F52B5",
            },
        ),
        (
            "eslint.config.js",
            IconProperties {
                icon: "\u{f0c7a}",
                color: "#3F52B5",
            },
        ),
        (
            "eslint.config.mjs",
            IconProperties {
                icon: "\u{f0c7a}",
                color: "#3F52B5",
            },
        ),
        (
            "eslint.config.ts",
            IconProperties {
                icon: "\u{f0c7a}",
                color: "#3F52B5",
            },
        ),
        (
            "ext_typoscript_setup.txt",
            IconProperties {
                icon: "\u{e772}",
                color: "#FF8700",
            },
        ),
        (
            "favicon.ico",
            IconProperties {
                icon: "\u{e623}",
                color: "#CBCB41",
            },
        ),
        (
            "fp-info-cache",
            IconProperties {
                icon: "\u{f34c}",
                color: "#FFFFFF",
            },
        ),
        (
            "fp-lib-table",
            IconProperties {
                icon: "\u{f34c}",
                color: "#FFFFFF",
            },
        ),
        (
            "FreeCAD.conf",
            IconProperties {
                icon: "\u{f336}",
                color: "#CB333B",
            },
        ),
        (
            "gemfile$",
            IconProperties {
                icon: "\u{e791}",
                color: "#701516",
            },
        ),
        (
            "gitignore_global",
            IconProperties {
                icon: "\u{f02a2}",
                color: "#E64A19",
            },
        ),
        (
            "gnumakefile",
            IconProperties {
                icon: "\u{eba2}",
                color: "#EF5351",
            },
        ),
        (
            "GNUmakefile",
            IconProperties {
                icon: "\u{e779}",
                color: "#6D8086",
            },
        ),
        (
            "go.mod",
            IconProperties {
                icon: "\u{e627}",
                color: "#02ACC1",
            },
        ),
        (
            "go.sum",
            IconProperties {
                icon: "\u{e627}",
                color: "#02ACC1",
            },
        ),
        (
            "go.work",
            IconProperties {
                icon: "\u{e627}",
                color: "#02ACC1",
            },
        ),
        (
            "gradle",
            IconProperties {
                icon: "\u{e660}",
                color: "#005F87",
            },
        ),
        (
            "gradle-wrapper.properties",
            IconProperties {
                icon: "\u{e660}",
                color: "#005F87",
            },
        ),
        (
            "gradle.properties",
            IconProperties {
                icon: "\u{e660}",
                color: "#005F87",
            },
        ),
        (
            "gradlew",
            IconProperties {
                icon: "\u{e660}",
                color: "#005F87",
            },
        ),
        (
            "gruntfile.babel.js",
            IconProperties {
                icon: "\u{e611}",
                color: "#E37933",
            },
        ),
        (
            "gruntfile.coffee",
            IconProperties {
                icon: "\u{e611}",
                color: "#E37933",
            },
        ),
        (
            "gruntfile.js",
            IconProperties {
                icon: "\u{e611}",
                color: "#E37933",
            },
        ),
        (
            "gruntfile.ls",
            IconProperties {
                icon: "\u{e611}",
                color: "#E37933",
            },
        ),
        (
            "gruntfile.ts",
            IconProperties {
                icon: "\u{e611}",
                color: "#E37933",
            },
        ),
        (
            "gtkrc",
            IconProperties {
                icon: "\u{f362}",
                color: "#FFFFFF",
            },
        ),
        (
            "gulpfile.babel.js",
            IconProperties {
                icon: "\u{e610}",
                color: "#CC3E44",
            },
        ),
        (
            "gulpfile.coffee",
            IconProperties {
                icon: "\u{e610}",
                color: "#CC3E44",
            },
        ),
        (
            "gulpfile.js",
            IconProperties {
                icon: "\u{e610}",
                color: "#CC3E44",
            },
        ),
        (
            "gulpfile.ls",
            IconProperties {
                icon: "\u{e610}",
                color: "#CC3E44",
            },
        ),
        (
            "gulpfile.ts",
            IconProperties {
                icon: "\u{e610}",
                color: "#CC3E44",
            },
        ),
        (
            "hidden",
            IconProperties {
                icon: "\u{f023}",
                color: "#555555",
            },
        ),
        (
            "hypridle.conf",
            IconProperties {
                icon: "\u{f359}",
                color: "#00AAAE",
            },
        ),
        (
            "hyprland.conf",
            IconProperties {
                icon: "\u{f359}",
                color: "#00AAAE",
            },
        ),
        (
            "hyprlock.conf",
            IconProperties {
                icon: "\u{f359}",
                color: "#00AAAE",
            },
        ),
        (
            "hyprpaper.conf",
            IconProperties {
                icon: "\u{f359}",
                color: "#00AAAE",
            },
        ),
        (
            "i3blocks.conf",
            IconProperties {
                icon: "\u{f35a}",
                color: "#E8EBEE",
            },
        ),
        (
            "i3status.conf",
            IconProperties {
                icon: "\u{f35a}",
                color: "#E8EBEE",
            },
        ),
        (
            "include",
            IconProperties {
                icon: "\u{e5fc}",
                color: "#EEEEEE",
            },
        ),
        (
            "index.theme",
            IconProperties {
                icon: "\u{ee72}",
                color: "#2DB96F",
            },
        ),
        (
            "ionic.config.json",
            IconProperties {
                icon: "\u{e66b}",
                color: "#508FF7",
            },
        ),
        (
            "justfile",
            IconProperties {
                icon: "\u{f0ad}",
                color: "#6D8086",
            },
        ),
        (
            "kalgebrarc",
            IconProperties {
                icon: "\u{f373}",
                color: "#1C99F3",
            },
        ),
        (
            "kdeglobals",
            IconProperties {
                icon: "\u{f373}",
                color: "#1C99F3",
            },
        ),
        (
            "kdenlive-layoutsrc",
            IconProperties {
                icon: "\u{f33c}",
                color: "#83B8F2",
            },
        ),
        (
            "kdenliverc",
            IconProperties {
                icon: "\u{f33c}",
                color: "#83B8F2",
            },
        ),
        (
            "kritadisplayrc",
            IconProperties {
                icon: "\u{f33d}",
                color: "#F245FB",
            },
        ),
        (
            "kritarc",
            IconProperties {
                icon: "\u{f33d}",
                color: "#F245FB",
            },
        ),
        (
            "lib",
            IconProperties {
                icon: "\u{f1517}",
                color: "#8BC34A",
            },
        ),
        (
            "LICENSE",
            IconProperties {
                icon: "\u{f02d}",
                color: "#EDEDED",
            },
        ),
        (
            "LICENSE.md",
            IconProperties {
                icon: "\u{f02d}",
                color: "#EDEDED",
            },
        ),
        (
            "localized",
            IconProperties {
                icon: "\u{f179}",
                color: "#DDDDDD",
            },
        ),
        (
            "lxde-rc.xml",
            IconProperties {
                icon: "\u{f363}",
                color: "#909090",
            },
        ),
        (
            "lxqt.conf",
            IconProperties {
                icon: "\u{f364}",
                color: "#0192D3",
            },
        ),
        (
            "Makefile",
            IconProperties {
                icon: "\u{e673}",
                color: "#FEFEFE",
            },
        ),
        (
            "mix.lock",
            IconProperties {
                icon: "\u{e62d}",
                color: "#A074C4",
            },
        ),
        (
            "mpv.conf",
            IconProperties {
                icon: "\u{f36e}",
                color: "#3B1342",
            },
        ),
        (
            "node_modules",
            IconProperties {
                icon: "\u{e718}",
                color: "#E8274B",
            },
        ),
        (
            "npmignore",
            IconProperties {
                icon: "\u{e71e}",
                color: "#E8274B",
            },
        ),
        (
            "nuxt.config.cjs",
            IconProperties {
                icon: "\u{f1106}",
                color: "#00C58E",
            },
        ),
        (
            "nuxt.config.js",
            IconProperties {
                icon: "\u{f1106}",
                color: "#00C58E",
            },
        ),
        (
            "nuxt.config.mjs",
            IconProperties {
                icon: "\u{f1106}",
                color: "#00C58E",
            },
        ),
        (
            "nuxt.config.ts",
            IconProperties {
                icon: "\u{f1106}",
                color: "#00C58E",
            },
        ),
        (
            "package-lock.json",
            IconProperties {
                icon: "\u{ed0d}",
                color: "#F54436",
            },
        ),
        (
            "package.json",
            IconProperties {
                icon: "\u{ed0d}",
                color: "#4CAF51",
            },
        ),
        (
            "PKGBUILD",
            IconProperties {
                icon: "\u{f303}",
                color: "#0F94D2",
            },
        ),
        (
            "platformio.ini",
            IconProperties {
                icon: "\u{e682}",
                color: "#F6822B",
            },
        ),
        (
            "pom.xml",
            IconProperties {
                icon: "\u{f06d3}",
                color: "#FF7043",
            },
        ),
        (
            "prettier.config.cjs",
            IconProperties {
                icon: "\u{e6b4}",
                color: "#4285F4",
            },
        ),
        (
            "prettier.config.js",
            IconProperties {
                icon: "\u{e6b4}",
                color: "#4285F4",
            },
        ),
        (
            "prettier.config.mjs",
            IconProperties {
                icon: "\u{e6b4}",
                color: "#4285F4",
            },
        ),
        (
            "prettier.config.ts",
            IconProperties {
                icon: "\u{e6b4}",
                color: "#4285F4",
            },
        ),
        (
            "PrusaSlicer.ini",
            IconProperties {
                icon: "\u{f351}",
                color: "#EC6B23",
            },
        ),
        (
            "PrusaSlicerGcodeViewer.ini",
            IconProperties {
                icon: "\u{f351}",
                color: "#EC6B23",
            },
        ),
        (
            "py.typed",
            IconProperties {
                icon: "\u{e606}",
                color: "#ffbc03",
            },
        ),
        (
            "QtProject.conf",
            IconProperties {
                icon: "\u{f375}",
                color: "#40CD52",
            },
        ),
        (
            "R",
            IconProperties {
                icon: "\u{f07d4}",
                color: "#2266BA",
            },
        ),
        (
            "README",
            IconProperties {
                icon: "\u{f00ba}",
                color: "#EDEDED",
            },
        ),
        (
            "README.md",
            IconProperties {
                icon: "\u{f00ba}",
                color: "#EDEDED",
            },
        ),
        (
            "robots.txt",
            IconProperties {
                icon: "\u{f06a9}",
                color: "#5D7096",
            },
        ),
        (
            "rubydoc",
            IconProperties {
                icon: "\u{e73b}",
                color: "#F32C24",
            },
        ),
        (
            "SECURITY",
            IconProperties {
                icon: "\u{f0483}",
                color: "#BEC4C9",
            },
        ),
        (
            "SECURITY.md",
            IconProperties {
                icon: "\u{f0483}",
                color: "#BEC4C9",
            },
        ),
        (
            "settings.gradle",
            IconProperties {
                icon: "\u{e660}",
                color: "#005F87",
            },
        ),
        (
            "svelte.config.js",
            IconProperties {
                icon: "\u{e697}",
                color: "#FF5821",
            },
        ),
        (
            "sxhkdrc",
            IconProperties {
                icon: "\u{f355}",
                color: "#2F2F2F",
            },
        ),
        (
            "sym-lib-table",
            IconProperties {
                icon: "\u{f34c}",
                color: "#FFFFFF",
            },
        ),
        (
            "tailwind.config.js",
            IconProperties {
                icon: "\u{f13ff}",
                color: "#4DB6AC",
            },
        ),
        (
            "tailwind.config.mjs",
            IconProperties {
                icon: "\u{f13ff}",
                color: "#4DB6AC",
            },
        ),
        (
            "tailwind.config.ts",
            IconProperties {
                icon: "\u{f13ff}",
                color: "#4DB6AC",
            },
        ),
        (
            "tmux.conf",
            IconProperties {
                icon: "\u{ebc8}",
                color: "#14BA19",
            },
        ),
        (
            "tmux.conf.local",
            IconProperties {
                icon: "\u{ebc8}",
                color: "#14BA19",
            },
        ),
        (
            "tsconfig.json",
            IconProperties {
                icon: "\u{e628}",
                color: "#0188D1",
            },
        ),
        (
            "unlicense",
            IconProperties {
                icon: "\u{e60a}",
                color: "#D0BF41",
            },
        ),
        (
            "vagrantfile$",
            IconProperties {
                icon: "\u{f2b8}",
                color: "#1868F2",
            },
        ),
        (
            "vlcrc",
            IconProperties {
                icon: "\u{f057c}",
                color: "#E85E00",
            },
        ),
        (
            "webpack",
            IconProperties {
                icon: "\u{f072b}",
                color: "#519ABA",
            },
        ),
        (
            "weston.ini",
            IconProperties {
                icon: "\u{f367}",
                color: "#FFBB01",
            },
        ),
        (
            "WORKSPACE",
            IconProperties {
                icon: "\u{e63a}",
                color: "#89E051",
            },
        ),
        (
            "WORKSPACE.bzlmod",
            IconProperties {
                icon: "\u{e63a}",
                color: "#89E051",
            },
        ),
        (
            "xmobarrc",
            IconProperties {
                icon: "\u{f35e}",
                color: "#FD4D5D",
            },
        ),
        (
            "xmobarrc.hs",
            IconProperties {
                icon: "\u{f35e}",
                color: "#FD4D5D",
            },
        ),
        (
            "xmonad.hs",
            IconProperties {
                icon: "\u{f35e}",
                color: "#FD4D5D",
            },
        ),
        (
            "xorg.conf",
            IconProperties {
                icon: "\u{f369}",
                color: "#E54D18",
            },
        ),
        (
            "xsettingsd.conf",
            IconProperties {
                icon: "\u{f369}",
                color: "#E54D18",
            },
        ),
        (
            "yarn.lock",
            IconProperties {
                icon: "\u{e6a7}",
                color: "#0188D1",
            },
        ),
    ])
});

static EXT_ICON_MAP: LazyLock<HashMap<&'static str, IconProperties>> = LazyLock::new(|| {
    HashMap::from([
        (
            ".3gp",
            IconProperties {
                icon: "\u{f03d}",
                color: "#F6822B",
            },
        ),
        (
            ".3mf",
            IconProperties {
                icon: "\u{f01a7}",
                color: "#888888",
            },
        ),
        (
            ".7z",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".DS_store",
            IconProperties {
                icon: "\u{f179}",
                color: "#A2AAAD",
            },
        ),
        (
            ".a",
            IconProperties {
                icon: "\u{f1517}",
                color: "#8BC34A",
            },
        ),
        (
            ".aac",
            IconProperties {
                icon: "\u{f001}",
                color: "#20C2E3",
            },
        ),
        (
            ".adb",
            IconProperties {
                icon: "\u{e6b5}",
                color: "#22FFFF",
            },
        ),
        (
            ".ads",
            IconProperties {
                icon: "\u{e6b5}",
                color: "#22FFFF",
            },
        ),
        (
            ".ai",
            IconProperties {
                icon: "\u{e7b4}",
                color: "#D0BF41",
            },
        ),
        (
            ".aif",
            IconProperties {
                icon: "\u{f001}",
                color: "#00AFFF",
            },
        ),
        (
            ".aiff",
            IconProperties {
                icon: "\u{f0386}",
                color: "#EE534F",
            },
        ),
        (
            ".android",
            IconProperties {
                icon: "\u{e70e}",
                color: "#66AF3D",
            },
        ),
        (
            ".ape",
            IconProperties {
                icon: "\u{f001}",
                color: "#00AFFF",
            },
        ),
        (
            ".apk",
            IconProperties {
                icon: "\u{e70e}",
                color: "#8BC34A",
            },
        ),
        (
            ".app",
            IconProperties {
                icon: "\u{eae8}",
                color: "#9F0500",
            },
        ),
        (
            ".apple",
            IconProperties {
                icon: "\u{e635}",
                color: "#A2AAAD",
            },
        ),
        (
            ".applescript",
            IconProperties {
                icon: "\u{f302}",
                color: "#78919C",
            },
        ),
        (
            ".asc",
            IconProperties {
                icon: "\u{f0306}",
                color: "#25A79A",
            },
        ),
        (
            ".asm",
            IconProperties {
                icon: "\u{e637}",
                color: "#0091BD",
            },
        ),
        (
            ".ass",
            IconProperties {
                icon: "\u{f0a16}",
                color: "#FFB713",
            },
        ),
        (
            ".astro",
            IconProperties {
                icon: "\u{e6b3}",
                color: "#FF6D00",
            },
        ),
        (
            ".avi",
            IconProperties {
                icon: "\u{f0381}",
                color: "#FF9800",
            },
        ),
        (
            ".avif",
            IconProperties {
                icon: "\u{f021f}",
                color: "#25A6A0",
            },
        ),
        (
            ".avro",
            IconProperties {
                icon: "\u{e60b}",
                color: "#965824",
            },
        ),
        (
            ".awk",
            IconProperties {
                icon: "\u{f018d}",
                color: "#FF7043",
            },
        ),
        (
            ".azcli",
            IconProperties {
                icon: "\u{ebd8}",
                color: "#2088E5",
            },
        ),
        (
            ".bak",
            IconProperties {
                icon: "\u{f006f}",
                color: "#6D8086",
            },
        ),
        (
            ".bash",
            IconProperties {
                icon: "\u{ebca}",
                color: "#FF7043",
            },
        ),
        (
            ".bash_history",
            IconProperties {
                icon: "\u{e795}",
                color: "#8DC149",
            },
        ),
        (
            ".bash_profile",
            IconProperties {
                icon: "\u{e795}",
                color: "#8DC149",
            },
        ),
        (
            ".bashrc",
            IconProperties {
                icon: "\u{e795}",
                color: "#8DC149",
            },
        ),
        (
            ".bat",
            IconProperties {
                icon: "\u{f018d}",
                color: "#FF7043",
            },
        ),
        (
            ".bats",
            IconProperties {
                icon: "\u{f0b5f}",
                color: "#D2D2D2",
            },
        ),
        (
            ".bazel",
            IconProperties {
                icon: "\u{e63a}",
                color: "#44A047",
            },
        ),
        (
            ".bib",
            IconProperties {
                icon: "\u{f1517}",
                color: "#8BC34A",
            },
        ),
        (
            ".bicep",
            IconProperties {
                icon: "\u{f0fd7}",
                color: "#FBC02D",
            },
        ),
        (
            ".bicepparam",
            IconProperties {
                icon: "\u{e63b}",
                color: "#797DAC",
            },
        ),
        (
            ".blade.php",
            IconProperties {
                icon: "\u{f2f7}",
                color: "#FF5252",
            },
        ),
        (
            ".blend",
            IconProperties {
                icon: "\u{f00ab}",
                color: "#ED8F30",
            },
        ),
        (
            ".blp",
            IconProperties {
                icon: "\u{f0ebe}",
                color: "#458EE6",
            },
        ),
        (
            ".bmp",
            IconProperties {
                icon: "\u{f021f}",
                color: "#25A6A0",
            },
        ),
        (
            ".brep",
            IconProperties {
                icon: "\u{f0eeb}",
                color: "#839463",
            },
        ),
        (
            ".bz",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".bz2",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".bz3",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".bzl",
            IconProperties {
                icon: "\u{e63a}",
                color: "#44A047",
            },
        ),
        (
            ".c",
            IconProperties {
                icon: "\u{e61e}",
                color: "#0188D1",
            },
        ),
        (
            ".c++",
            IconProperties {
                icon: "\u{e61d}",
                color: "#0188D1",
            },
        ),
        (
            ".cab",
            IconProperties {
                icon: "\u{e70f}",
                color: "#626262",
            },
        ),
        (
            ".cache",
            IconProperties {
                icon: "\u{f49b}",
                color: "#FFFFFF",
            },
        ),
        (
            ".cast",
            IconProperties {
                icon: "\u{f03d}",
                color: "#EA8220",
            },
        ),
        (
            ".cbl",
            IconProperties {
                icon: "\u{2699}",
                color: "#005CA5",
            },
        ),
        (
            ".cc",
            IconProperties {
                icon: "\u{e61d}",
                color: "#0188D1",
            },
        ),
        (
            ".ccm",
            IconProperties {
                icon: "\u{e61d}",
                color: "#F34B7D",
            },
        ),
        (
            ".cfg",
            IconProperties {
                icon: "\u{f013}",
                color: "#42A5F5",
            },
        ),
        (
            ".cjs",
            IconProperties {
                icon: "\u{e60c}",
                color: "#CBCB41",
            },
        ),
        (
            ".class",
            IconProperties {
                icon: "\u{f0f4}",
                color: "#2088E5",
            },
        ),
        (
            ".clj",
            IconProperties {
                icon: "\u{e642}",
                color: "#2AB6F6",
            },
        ),
        (
            ".cljc",
            IconProperties {
                icon: "\u{e642}",
                color: "#2AB6F6",
            },
        ),
        (
            ".cljd",
            IconProperties {
                icon: "\u{e76a}",
                color: "#519ABA",
            },
        ),
        (
            ".cljs",
            IconProperties {
                icon: "\u{e642}",
                color: "#2AB6F6",
            },
        ),
        (
            ".cls",
            IconProperties {
                icon: "\u{e69b}",
                color: "#4B5163",
            },
        ),
        (
            ".cmake",
            IconProperties {
                icon: "\u{e794}",
                color: "#DCE3EB",
            },
        ),
        (
            ".cmd",
            IconProperties {
                icon: "\u{ebc4}",
                color: "#FF7043",
            },
        ),
        (
            ".cob",
            IconProperties {
                icon: "\u{2699}",
                color: "#005CA5",
            },
        ),
        (
            ".cobol",
            IconProperties {
                icon: "\u{2699}",
                color: "#005CA5",
            },
        ),
        (
            ".coffee",
            IconProperties {
                icon: "\u{e61b}",
                color: "#6F4E38",
            },
        ),
        (
            ".conda",
            IconProperties {
                icon: "\u{e715}",
                color: "#43B02A",
            },
        ),
        (
            ".conf",
            IconProperties {
                icon: "\u{f013}",
                color: "#696969",
            },
        ),
        (
            ".config.ru",
            IconProperties {
                icon: "\u{e791}",
                color: "#701516",
            },
        ),
        (
            ".cp",
            IconProperties {
                icon: "\u{e646}",
                color: "#0188D1",
            },
        ),
        (
            ".cpio",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".cpp",
            IconProperties {
                icon: "\u{e61d}",
                color: "#0188D1",
            },
        ),
        (
            ".cppm",
            IconProperties {
                icon: "\u{e61d}",
                color: "#519ABA",
            },
        ),
        (
            ".cpy",
            IconProperties {
                icon: "\u{2699}",
                color: "#005CA5",
            },
        ),
        (
            ".cr",
            IconProperties {
                icon: "\u{e62f}",
                color: "#CFD8DD",
            },
        ),
        (
            ".crdownload",
            IconProperties {
                icon: "\u{f019}",
                color: "#44CDA8",
            },
        ),
        (
            ".cs",
            IconProperties {
                icon: "\u{f031b}",
                color: "#0188D1",
            },
        ),
        (
            ".csh",
            IconProperties {
                icon: "\u{f018d}",
                color: "#FF7043",
            },
        ),
        (
            ".cshtml",
            IconProperties {
                icon: "\u{f486}",
                color: "#42A5F5",
            },
        ),
        (
            ".cson",
            IconProperties {
                icon: "\u{e61b}",
                color: "#6F4E38",
            },
        ),
        (
            ".csproj",
            IconProperties {
                icon: "\u{f0610}",
                color: "#AB48BC",
            },
        ),
        (
            ".css",
            IconProperties {
                icon: "\u{e749}",
                color: "#42A5F5",
            },
        ),
        (
            ".csv",
            IconProperties {
                icon: "\u{f021b}",
                color: "#8BC34A",
            },
        ),
        (
            ".csx",
            IconProperties {
                icon: "\u{f031b}",
                color: "#0188D1",
            },
        ),
        (
            ".cts",
            IconProperties {
                icon: "\u{e628}",
                color: "#519ABA",
            },
        ),
        (
            ".cu",
            IconProperties {
                icon: "\u{e64b}",
                color: "#89E051",
            },
        ),
        (
            ".cue",
            IconProperties {
                icon: "\u{f0cb9}",
                color: "#ED95AE",
            },
        ),
        (
            ".cuh",
            IconProperties {
                icon: "\u{e64b}",
                color: "#A074C4",
            },
        ),
        (
            ".cxx",
            IconProperties {
                icon: "\u{e646}",
                color: "#0188D1",
            },
        ),
        (
            ".cxxm",
            IconProperties {
                icon: "\u{e61d}",
                color: "#519ABA",
            },
        ),
        (
            ".d",
            IconProperties {
                icon: "\u{e7af}",
                color: "#B03931",
            },
        ),
        (
            ".d.ts",
            IconProperties {
                icon: "\u{e628}",
                color: "#0188D1",
            },
        ),
        (
            ".dart",
            IconProperties {
                icon: "\u{e64c}",
                color: "#59B6F0",
            },
        ),
        (
            ".db",
            IconProperties {
                icon: "\u{f1c0}",
                color: "#FFCA29",
            },
        ),
        (
            ".dconf",
            IconProperties {
                icon: "\u{e706}",
                color: "#DAD8D8",
            },
        ),
        (
            ".deb",
            IconProperties {
                icon: "\u{ebc5}",
                color: "#D80651",
            },
        ),
        (
            ".desktop",
            IconProperties {
                icon: "\u{f108}",
                color: "#56347C",
            },
        ),
        (
            ".diff",
            IconProperties {
                icon: "\u{f4d2}",
                color: "#4262A2",
            },
        ),
        (
            ".djvu",
            IconProperties {
                icon: "\u{f02d}",
                color: "#624262",
            },
        ),
        (
            ".dll",
            IconProperties {
                icon: "\u{f107c}",
                color: "#42A5F5",
            },
        ),
        (
            ".doc",
            IconProperties {
                icon: "\u{f022c}",
                color: "#0188D1",
            },
        ),
        (
            ".docx",
            IconProperties {
                icon: "\u{f022c}",
                color: "#0188D1",
            },
        ),
        (
            ".dot",
            IconProperties {
                icon: "\u{f1049}",
                color: "#005F87",
            },
        ),
        (
            ".download",
            IconProperties {
                icon: "\u{f019}",
                color: "#44CDA8",
            },
        ),
        (
            ".drl",
            IconProperties {
                icon: "\u{e28c}",
                color: "#FFAFAF",
            },
        ),
        (
            ".dropbox",
            IconProperties {
                icon: "\u{e707}",
                color: "#2E63FF",
            },
        ),
        (
            ".ds_store",
            IconProperties {
                icon: "\u{f179}",
                color: "#A2AAAD",
            },
        ),
        (
            ".dump",
            IconProperties {
                icon: "\u{f1c0}",
                color: "#DAD8D8",
            },
        ),
        (
            ".dwg",
            IconProperties {
                icon: "\u{f0eeb}",
                color: "#839463",
            },
        ),
        (
            ".dxf",
            IconProperties {
                icon: "\u{f0eeb}",
                color: "#839463",
            },
        ),
        (
            ".ebook",
            IconProperties {
                icon: "\u{e28b}",
                color: "#EAB16D",
            },
        ),
        (
            ".ebuild",
            IconProperties {
                icon: "\u{f30d}",
                color: "#4C416E",
            },
        ),
        (
            ".editorconfig",
            IconProperties {
                icon: "\u{e615}",
                color: "#626262",
            },
        ),
        (
            ".edn",
            IconProperties {
                icon: "\u{e76a}",
                color: "#519ABA",
            },
        ),
        (
            ".eex",
            IconProperties {
                icon: "\u{e62d}",
                color: "#9575CE",
            },
        ),
        (
            ".ejs",
            IconProperties {
                icon: "\u{e618}",
                color: "#CBCB41",
            },
        ),
        (
            ".el",
            IconProperties {
                icon: "\u{e632}",
                color: "#805EB7",
            },
        ),
        (
            ".elc",
            IconProperties {
                icon: "\u{e632}",
                color: "#805EB7",
            },
        ),
        (
            ".elf",
            IconProperties {
                icon: "\u{eae8}",
                color: "#9F0500",
            },
        ),
        (
            ".elm",
            IconProperties {
                icon: "\u{e62c}",
                color: "#60B6CC",
            },
        ),
        (
            ".eln",
            IconProperties {
                icon: "\u{e632}",
                color: "#8172BE",
            },
        ),
        (
            ".env",
            IconProperties {
                icon: "\u{f462}",
                color: "#FAF743",
            },
        ),
        (
            ".eot",
            IconProperties {
                icon: "\u{e659}",
                color: "#F54436",
            },
        ),
        (
            ".epp",
            IconProperties {
                icon: "\u{e631}",
                color: "#FFA61A",
            },
        ),
        (
            ".epub",
            IconProperties {
                icon: "\u{e28b}",
                color: "#EAB16D",
            },
        ),
        (
            ".erb",
            IconProperties {
                icon: "\u{f0d2d}",
                color: "#F54436",
            },
        ),
        (
            ".erl",
            IconProperties {
                icon: "\u{f23f}",
                color: "#F54436",
            },
        ),
        (
            ".ex",
            IconProperties {
                icon: "\u{e62d}",
                color: "#9575CE",
            },
        ),
        (
            ".exe",
            IconProperties {
                icon: "\u{f2d0}",
                color: "#E64A19",
            },
        ),
        (
            ".exs",
            IconProperties {
                icon: "\u{e62d}",
                color: "#9575CE",
            },
        ),
        (
            ".f#",
            IconProperties {
                icon: "\u{e7a7}",
                color: "#519ABA",
            },
        ),
        (
            ".f3d",
            IconProperties {
                icon: "\u{f0eeb}",
                color: "#839463",
            },
        ),
        (
            ".f90",
            IconProperties {
                icon: "\u{f121a}",
                color: "#FF7043",
            },
        ),
        (
            ".fbx",
            IconProperties {
                icon: "\u{ea8c}",
                color: "#2AB6F6",
            },
        ),
        (
            ".fcbak",
            IconProperties {
                icon: "\u{f336}",
                color: "#6D8086",
            },
        ),
        (
            ".fcmacro",
            IconProperties {
                icon: "\u{f336}",
                color: "#CB333B",
            },
        ),
        (
            ".fcmat",
            IconProperties {
                icon: "\u{f336}",
                color: "#CB333B",
            },
        ),
        (
            ".fcparam",
            IconProperties {
                icon: "\u{f336}",
                color: "#CB333B",
            },
        ),
        (
            ".fcscript",
            IconProperties {
                icon: "\u{f336}",
                color: "#CB333B",
            },
        ),
        (
            ".fcstd",
            IconProperties {
                icon: "\u{f336}",
                color: "#CB333B",
            },
        ),
        (
            ".fcstd1",
            IconProperties {
                icon: "\u{f336}",
                color: "#CB333B",
            },
        ),
        (
            ".fctb",
            IconProperties {
                icon: "\u{f336}",
                color: "#CB333B",
            },
        ),
        (
            ".fctl",
            IconProperties {
                icon: "\u{f336}",
                color: "#CB333B",
            },
        ),
        (
            ".fdmdownload",
            IconProperties {
                icon: "\u{f019}",
                color: "#44CDA8",
            },
        ),
        (
            ".fish",
            IconProperties {
                icon: "\u{f023a}",
                color: "#FF7043",
            },
        ),
        (
            ".flac",
            IconProperties {
                icon: "\u{f0386}",
                color: "#EE534F",
            },
        ),
        (
            ".flc",
            IconProperties {
                icon: "\u{f031}",
                color: "#ECECEC",
            },
        ),
        (
            ".flf",
            IconProperties {
                icon: "\u{f031}",
                color: "#ECECEC",
            },
        ),
        (
            ".flv",
            IconProperties {
                icon: "\u{f0381}",
                color: "#FF9800",
            },
        ),
        (
            ".fnl",
            IconProperties {
                icon: "\u{e6af}",
                color: "#FFF3D7",
            },
        ),
        (
            ".fodg",
            IconProperties {
                icon: "\u{f379}",
                color: "#FFFB57",
            },
        ),
        (
            ".fodp",
            IconProperties {
                icon: "\u{f37a}",
                color: "#FE9C45",
            },
        ),
        (
            ".fods",
            IconProperties {
                icon: "\u{f378}",
                color: "#78FC4E",
            },
        ),
        (
            ".fodt",
            IconProperties {
                icon: "\u{f37c}",
                color: "#2DCBFD",
            },
        ),
        (
            ".font",
            IconProperties {
                icon: "\u{e659}",
                color: "#F54436",
            },
        ),
        (
            ".fs",
            IconProperties {
                icon: "\u{e7a7}",
                color: "#31B9DB",
            },
        ),
        (
            ".fsi",
            IconProperties {
                icon: "\u{e7a7}",
                color: "#31B9DB",
            },
        ),
        (
            ".fsscript",
            IconProperties {
                icon: "\u{e7a7}",
                color: "#519ABA",
            },
        ),
        (
            ".fsx",
            IconProperties {
                icon: "\u{e7a7}",
                color: "#31B9DB",
            },
        ),
        (
            ".gcode",
            IconProperties {
                icon: "\u{f0af4}",
                color: "#505075",
            },
        ),
        (
            ".gd",
            IconProperties {
                icon: "\u{e65f}",
                color: "#42A5F5",
            },
        ),
        (
            ".gdoc",
            IconProperties {
                icon: "\u{f1c2}",
                color: "#01D000",
            },
        ),
        (
            ".gem",
            IconProperties {
                icon: "\u{e21e}",
                color: "#C90F02",
            },
        ),
        (
            ".gemfile",
            IconProperties {
                icon: "\u{eb48}",
                color: "#E63936",
            },
        ),
        (
            ".gemspec",
            IconProperties {
                icon: "\u{e21e}",
                color: "#C90F02",
            },
        ),
        (
            ".gform",
            IconProperties {
                icon: "\u{f298}",
                color: "#01D000",
            },
        ),
        (
            ".gif",
            IconProperties {
                icon: "\u{f021f}",
                color: "#25A6A0",
            },
        ),
        (
            ".git",
            IconProperties {
                icon: "\u{f02a2}",
                color: "#EC6B23",
            },
        ),
        (
            ".glb",
            IconProperties {
                icon: "\u{f1b2}",
                color: "#FFA61A",
            },
        ),
        (
            ".gnumakefile",
            IconProperties {
                icon: "\u{eba2}",
                color: "#EF5351",
            },
        ),
        (
            ".go",
            IconProperties {
                icon: "\u{e627}",
                color: "#02ACC1",
            },
        ),
        (
            ".godot",
            IconProperties {
                icon: "\u{e65f}",
                color: "#42A5F5",
            },
        ),
        (
            ".gpr",
            IconProperties {
                icon: "\u{e6b5}",
                color: "#22FFFF",
            },
        ),
        (
            ".gql",
            IconProperties {
                icon: "\u{f0877}",
                color: "#EC417A",
            },
        ),
        (
            ".gradle",
            IconProperties {
                icon: "\u{e660}",
                color: "#0397A7",
            },
        ),
        (
            ".graphql",
            IconProperties {
                icon: "\u{f0877}",
                color: "#EC417A",
            },
        ),
        (
            ".gresource",
            IconProperties {
                icon: "\u{f362}",
                color: "#FFFFFF",
            },
        ),
        (
            ".groovy",
            IconProperties {
                icon: "\u{e775}",
                color: "#005F87",
            },
        ),
        (
            ".gsheet",
            IconProperties {
                icon: "\u{f1c3}",
                color: "#97BA6A",
            },
        ),
        (
            ".gslides",
            IconProperties {
                icon: "\u{f1c4}",
                color: "#FFFF00",
            },
        ),
        (
            ".guardfile",
            IconProperties {
                icon: "\u{e21e}",
                color: "#626262",
            },
        ),
        (
            ".gv",
            IconProperties {
                icon: "\u{f1049}",
                color: "#005F87",
            },
        ),
        (
            ".gz",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".h",
            IconProperties {
                icon: "\u{f0fd}",
                color: "#A074C4",
            },
        ),
        (
            ".haml",
            IconProperties {
                icon: "\u{e664}",
                color: "#F4521E",
            },
        ),
        (
            ".hbs",
            IconProperties {
                icon: "\u{f15de}",
                color: "#FF7043",
            },
        ),
        (
            ".hc",
            IconProperties {
                icon: "\u{f00a2}",
                color: "#FAF743",
            },
        ),
        (
            ".heex",
            IconProperties {
                icon: "\u{e62d}",
                color: "#9575CE",
            },
        ),
        (
            ".hex",
            IconProperties {
                icon: "\u{f12a7}",
                color: "#25A79A",
            },
        ),
        (
            ".hh",
            IconProperties {
                icon: "\u{f0fd}",
                color: "#A074C4",
            },
        ),
        (
            ".hpp",
            IconProperties {
                icon: "\u{f0fd}",
                color: "#A074C4",
            },
        ),
        (
            ".hrl",
            IconProperties {
                icon: "\u{e7b1}",
                color: "#B83998",
            },
        ),
        (
            ".hs",
            IconProperties {
                icon: "\u{e61f}",
                color: "#FFA726",
            },
        ),
        (
            ".htm",
            IconProperties {
                icon: "\u{f13b}",
                color: "#E44E27",
            },
        ),
        (
            ".html",
            IconProperties {
                icon: "\u{f13b}",
                color: "#E44E27",
            },
        ),
        (
            ".huff",
            IconProperties {
                icon: "\u{f0858}",
                color: "#CFD8DD",
            },
        ),
        (
            ".hurl",
            IconProperties {
                icon: "\u{f0ec}",
                color: "#FF0288",
            },
        ),
        (
            ".hx",
            IconProperties {
                icon: "\u{e666}",
                color: "#F68713",
            },
        ),
        (
            ".hxx",
            IconProperties {
                icon: "\u{f0fd}",
                color: "#A074C4",
            },
        ),
        (
            ".ical",
            IconProperties {
                icon: "\u{f073}",
                color: "#2B9EF3",
            },
        ),
        (
            ".icalendar",
            IconProperties {
                icon: "\u{f073}",
                color: "#2B9EF3",
            },
        ),
        (
            ".ico",
            IconProperties {
                icon: "\u{f021f}",
                color: "#25A6A0",
            },
        ),
        (
            ".ics",
            IconProperties {
                icon: "\u{f01ee}",
                color: "#42A5F5",
            },
        ),
        (
            ".ifb",
            IconProperties {
                icon: "\u{f073}",
                color: "#2B9EF3",
            },
        ),
        (
            ".ifc",
            IconProperties {
                icon: "\u{f0eeb}",
                color: "#839463",
            },
        ),
        (
            ".ige",
            IconProperties {
                icon: "\u{f0eeb}",
                color: "#839463",
            },
        ),
        (
            ".iges",
            IconProperties {
                icon: "\u{f0eeb}",
                color: "#839463",
            },
        ),
        (
            ".igs",
            IconProperties {
                icon: "\u{f0eeb}",
                color: "#839463",
            },
        ),
        (
            ".image",
            IconProperties {
                icon: "\u{f1c5}",
                color: "#CBCB41",
            },
        ),
        (
            ".img",
            IconProperties {
                icon: "\u{f021f}",
                color: "#25A6A0",
            },
        ),
        (
            ".iml",
            IconProperties {
                icon: "\u{f022e}",
                color: "#8BC34A",
            },
        ),
        (
            ".import",
            IconProperties {
                icon: "\u{f0c6}",
                color: "#ECECEC",
            },
        ),
        (
            ".info",
            IconProperties {
                icon: "\u{f129}",
                color: "#FFF3D7",
            },
        ),
        (
            ".ini",
            IconProperties {
                icon: "\u{f013}",
                color: "#42A5F5",
            },
        ),
        (
            ".ino",
            IconProperties {
                icon: "\u{f34b}",
                color: "#01979D",
            },
        ),
        (
            ".ipynb",
            IconProperties {
                icon: "\u{e80f}",
                color: "#F57D01",
            },
        ),
        (
            ".iso",
            IconProperties {
                icon: "\u{ede9}",
                color: "#B1BEC5",
            },
        ),
        (
            ".ixx",
            IconProperties {
                icon: "\u{e61d}",
                color: "#519ABA",
            },
        ),
        (
            ".j2c",
            IconProperties {
                icon: "\u{f1c5}",
                color: "#4B5163",
            },
        ),
        (
            ".j2k",
            IconProperties {
                icon: "\u{f1c5}",
                color: "#4B5163",
            },
        ),
        (
            ".jad",
            IconProperties {
                icon: "\u{e256}",
                color: "#F19210",
            },
        ),
        (
            ".jar",
            IconProperties {
                icon: "\u{f06ca}",
                color: "#F19210",
            },
        ),
        (
            ".java",
            IconProperties {
                icon: "\u{f0f4}",
                color: "#F19210",
            },
        ),
        (
            ".jfi",
            IconProperties {
                icon: "\u{f1c5}",
                color: "#626262",
            },
        ),
        (
            ".jfif",
            IconProperties {
                icon: "\u{f021f}",
                color: "#25A6A0",
            },
        ),
        (
            ".jif",
            IconProperties {
                icon: "\u{f1c5}",
                color: "#626262",
            },
        ),
        (
            ".jl",
            IconProperties {
                icon: "\u{e624}",
                color: "#338A23",
            },
        ),
        (
            ".jmd",
            IconProperties {
                icon: "\u{f48a}",
                color: "#519ABA",
            },
        ),
        (
            ".jp2",
            IconProperties {
                icon: "\u{f1c5}",
                color: "#626262",
            },
        ),
        (
            ".jpe",
            IconProperties {
                icon: "\u{f1c5}",
                color: "#626262",
            },
        ),
        (
            ".jpeg",
            IconProperties {
                icon: "\u{f021f}",
                color: "#25A6A0",
            },
        ),
        (
            ".jpg",
            IconProperties {
                icon: "\u{f021f}",
                color: "#25A6A0",
            },
        ),
        (
            ".jpx",
            IconProperties {
                icon: "\u{f1c5}",
                color: "#626262",
            },
        ),
        (
            ".js",
            IconProperties {
                icon: "\u{f031e}",
                color: "#FFCA29",
            },
        ),
        (
            ".json",
            IconProperties {
                icon: "\u{e60b}",
                color: "#FAA825",
            },
        ),
        (
            ".json5",
            IconProperties {
                icon: "\u{e60b}",
                color: "#FAA825",
            },
        ),
        (
            ".jsonc",
            IconProperties {
                icon: "\u{e60b}",
                color: "#FAA825",
            },
        ),
        (
            ".jsx",
            IconProperties {
                icon: "\u{ed46}",
                color: "#FFCA29",
            },
        ),
        (
            ".jwmrc",
            IconProperties {
                icon: "\u{f35b}",
                color: "#007AC2",
            },
        ),
        (
            ".jxl",
            IconProperties {
                icon: "\u{f1c5}",
                color: "#727252",
            },
        ),
        (
            ".kbx",
            IconProperties {
                icon: "\u{f0bc4}",
                color: "#537662",
            },
        ),
        (
            ".kdb",
            IconProperties {
                icon: "\u{f23e}",
                color: "#529B34",
            },
        ),
        (
            ".kdbx",
            IconProperties {
                icon: "\u{f23e}",
                color: "#529B34",
            },
        ),
        (
            ".kdenlive",
            IconProperties {
                icon: "\u{f33c}",
                color: "#83B8F2",
            },
        ),
        (
            ".kdenlivetitle",
            IconProperties {
                icon: "\u{f33c}",
                color: "#83B8F2",
            },
        ),
        (
            ".kicad_dru",
            IconProperties {
                icon: "\u{f34c}",
                color: "#FFFFFF",
            },
        ),
        (
            ".kicad_mod",
            IconProperties {
                icon: "\u{f34c}",
                color: "#FFFFFF",
            },
        ),
        (
            ".kicad_pcb",
            IconProperties {
                icon: "\u{f34c}",
                color: "#FFFFFF",
            },
        ),
        (
            ".kicad_prl",
            IconProperties {
                icon: "\u{f34c}",
                color: "#FFFFFF",
            },
        ),
        (
            ".kicad_pro",
            IconProperties {
                icon: "\u{f34c}",
                color: "#FFFFFF",
            },
        ),
        (
            ".kicad_sch",
            IconProperties {
                icon: "\u{f34c}",
                color: "#FFFFFF",
            },
        ),
        (
            ".kicad_sym",
            IconProperties {
                icon: "\u{f34c}",
                color: "#FFFFFF",
            },
        ),
        (
            ".kicad_wks",
            IconProperties {
                icon: "\u{f34c}",
                color: "#FFFFFF",
            },
        ),
        (
            ".ko",
            IconProperties {
                icon: "\u{f17c}",
                color: "#DDDDDD",
            },
        ),
        (
            ".kpp",
            IconProperties {
                icon: "\u{f33d}",
                color: "#F245FB",
            },
        ),
        (
            ".kra",
            IconProperties {
                icon: "\u{f33d}",
                color: "#F245FB",
            },
        ),
        (
            ".krz",
            IconProperties {
                icon: "\u{f33d}",
                color: "#F245FB",
            },
        ),
        (
            ".ksh",
            IconProperties {
                icon: "\u{f018d}",
                color: "#FF7043",
            },
        ),
        (
            ".kt",
            IconProperties {
                icon: "\u{e634}",
                color: "#1A95D9",
            },
        ),
        (
            ".kts",
            IconProperties {
                icon: "\u{e634}",
                color: "#1A95D9",
            },
        ),
        (
            ".latex",
            IconProperties {
                icon: "\u{e69b}",
                color: "#626262",
            },
        ),
        (
            ".lck",
            IconProperties {
                icon: "\u{e672}",
                color: "#BBBBBB",
            },
        ),
        (
            ".leex",
            IconProperties {
                icon: "\u{e62d}",
                color: "#9575CE",
            },
        ),
        (
            ".less",
            IconProperties {
                icon: "\u{ed48}",
                color: "#0277BD",
            },
        ),
        (
            ".lff",
            IconProperties {
                icon: "\u{f031}",
                color: "#ECECEC",
            },
        ),
        (
            ".lhs",
            IconProperties {
                icon: "\u{e777}",
                color: "#A074C4",
            },
        ),
        (
            ".license",
            IconProperties {
                icon: "\u{f0124}",
                color: "#FFCA29",
            },
        ),
        (
            ".liquid",
            IconProperties {
                icon: "\u{f043}",
                color: "#2AB6F6",
            },
        ),
        (
            ".localized",
            IconProperties {
                icon: "\u{f179}",
                color: "#A2AAAD",
            },
        ),
        (
            ".lock",
            IconProperties {
                icon: "\u{f023}",
                color: "#FFD550",
            },
        ),
        (
            ".log",
            IconProperties {
                icon: "\u{f0f6}",
                color: "#ECA517",
            },
        ),
        (
            ".lrc",
            IconProperties {
                icon: "\u{f0a16}",
                color: "#FFA61A",
            },
        ),
        (
            ".lua",
            IconProperties {
                icon: "\u{e620}",
                color: "#42A5F5",
            },
        ),
        (
            ".luac",
            IconProperties {
                icon: "\u{e620}",
                color: "#519ABA",
            },
        ),
        (
            ".luau",
            IconProperties {
                icon: "\u{e620}",
                color: "#519ABA",
            },
        ),
        (
            ".lz",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".lz4",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".lzh",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".lzma",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".lzo",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".m",
            IconProperties {
                icon: "\u{e61e}",
                color: "#599EFF",
            },
        ),
        (
            ".m3u",
            IconProperties {
                icon: "\u{f0cb9}",
                color: "#ED95AE",
            },
        ),
        (
            ".m3u8",
            IconProperties {
                icon: "\u{f0cb9}",
                color: "#ED95AE",
            },
        ),
        (
            ".m4a",
            IconProperties {
                icon: "\u{f0386}",
                color: "#EE534F",
            },
        ),
        (
            ".m4v",
            IconProperties {
                icon: "\u{f0381}",
                color: "#FF9800",
            },
        ),
        (
            ".magnet",
            IconProperties {
                icon: "\u{f076}",
                color: "#9F0500",
            },
        ),
        (
            ".makefile",
            IconProperties {
                icon: "\u{e673}",
                color: "#FEFEFE",
            },
        ),
        (
            ".markdown",
            IconProperties {
                icon: "\u{eb1d}",
                color: "#42A5F5",
            },
        ),
        (
            ".material",
            IconProperties {
                icon: "\u{f0509}",
                color: "#B83998",
            },
        ),
        (
            ".md",
            IconProperties {
                icon: "\u{eb1d}",
                color: "#42A5F5",
            },
        ),
        (
            ".md5",
            IconProperties {
                icon: "\u{f0565}",
                color: "#8C86AF",
            },
        ),
        (
            ".mdx",
            IconProperties {
                icon: "\u{eb1d}",
                color: "#FFCA29",
            },
        ),
        (
            ".mint",
            IconProperties {
                icon: "\u{e7a4}",
                color: "#44A047",
            },
        ),
        (
            ".mjs",
            IconProperties {
                icon: "\u{f031e}",
                color: "#FFCA29",
            },
        ),
        (
            ".mk",
            IconProperties {
                icon: "\u{e795}",
                color: "#626262",
            },
        ),
        (
            ".mkd",
            IconProperties {
                icon: "\u{f48a}",
                color: "#519ABA",
            },
        ),
        (
            ".mkv",
            IconProperties {
                icon: "\u{f0381}",
                color: "#FF9800",
            },
        ),
        (
            ".ml",
            IconProperties {
                icon: "\u{e67a}",
                color: "#FF9800",
            },
        ),
        (
            ".mli",
            IconProperties {
                icon: "\u{e67a}",
                color: "#FF9800",
            },
        ),
        (
            ".mm",
            IconProperties {
                icon: "\u{e61d}",
                color: "#599EFF",
            },
        ),
        (
            ".mo",
            IconProperties {
                icon: "\u{f05ca}",
                color: "#7986CB",
            },
        ),
        (
            ".mobi",
            IconProperties {
                icon: "\u{e28b}",
                color: "#EAB16D",
            },
        ),
        (
            ".mojo",
            IconProperties {
                icon: "\u{e780}",
                color: "#FF7043",
            },
        ),
        (
            ".mov",
            IconProperties {
                icon: "\u{f0381}",
                color: "#FF9800",
            },
        ),
        (
            ".mp3",
            IconProperties {
                icon: "\u{f0386}",
                color: "#EE534F",
            },
        ),
        (
            ".mp4",
            IconProperties {
                icon: "\u{f0381}",
                color: "#FF9800",
            },
        ),
        (
            ".mpp",
            IconProperties {
                icon: "\u{e61d}",
                color: "#519ABA",
            },
        ),
        (
            ".msf",
            IconProperties {
                icon: "\u{f370}",
                color: "#137BE1",
            },
        ),
        (
            ".msi",
            IconProperties {
                icon: "\u{f2d0}",
                color: "#E64A19",
            },
        ),
        (
            ".mts",
            IconProperties {
                icon: "\u{e628}",
                color: "#519ABA",
            },
        ),
        (
            ".mustache",
            IconProperties {
                icon: "\u{f15de}",
                color: "#FF7043",
            },
        ),
        (
            ".nfo",
            IconProperties {
                icon: "\u{f129}",
                color: "#FFF3D7",
            },
        ),
        (
            ".nim",
            IconProperties {
                icon: "\u{e677}",
                color: "#FFCA29",
            },
        ),
        (
            ".nix",
            IconProperties {
                icon: "\u{f313}",
                color: "#5175C2",
            },
        ),
        (
            ".node",
            IconProperties {
                icon: "\u{f0399}",
                color: "#E8274B",
            },
        ),
        (
            ".npmignore",
            IconProperties {
                icon: "\u{e71e}",
                color: "#E8274B",
            },
        ),
        (
            ".nswag",
            IconProperties {
                icon: "\u{e60b}",
                color: "#85EA2D",
            },
        ),
        (
            ".nu",
            IconProperties {
                icon: "\u{f018d}",
                color: "#FF7043",
            },
        ),
        (
            ".o",
            IconProperties {
                icon: "\u{ea8c}",
                color: "#2AB6F6",
            },
        ),
        (
            ".obj",
            IconProperties {
                icon: "\u{ea8c}",
                color: "#2AB6F6",
            },
        ),
        (
            ".odin",
            IconProperties {
                icon: "\u{f07e2}",
                color: "#3882D2",
            },
        ),
        (
            ".odf",
            IconProperties {
                icon: "\u{f37b}",
                color: "#FF5A96",
            },
        ),
        (
            ".odg",
            IconProperties {
                icon: "\u{f379}",
                color: "#FFFB57",
            },
        ),
        (
            ".odp",
            IconProperties {
                icon: "\u{f37a}",
                color: "#FE9C45",
            },
        ),
        (
            ".ods",
            IconProperties {
                icon: "\u{f378}",
                color: "#78FC4E",
            },
        ),
        (
            ".odt",
            IconProperties {
                icon: "\u{f37c}",
                color: "#2DCBFD",
            },
        ),
        (
            ".ogg",
            IconProperties {
                icon: "\u{f0381}",
                color: "#FF9800",
            },
        ),
        (
            ".ogv",
            IconProperties {
                icon: "\u{f0381}",
                color: "#FF9800",
            },
        ),
        (
            ".opus",
            IconProperties {
                icon: "\u{f0223}",
                color: "#EA8220",
            },
        ),
        (
            ".org",
            IconProperties {
                icon: "\u{e633}",
                color: "#56B6C2",
            },
        ),
        (
            ".otf",
            IconProperties {
                icon: "\u{e659}",
                color: "#F54436",
            },
        ),
        (
            ".out",
            IconProperties {
                icon: "\u{eae8}",
                color: "#9F0500",
            },
        ),
        (
            ".part",
            IconProperties {
                icon: "\u{f43a}",
                color: "#628262",
            },
        ),
        (
            ".patch",
            IconProperties {
                icon: "\u{f440}",
                color: "#4262A2",
            },
        ),
        (
            ".pck",
            IconProperties {
                icon: "\u{f487}",
                color: "#5D8096",
            },
        ),
        (
            ".pdf",
            IconProperties {
                icon: "\u{f1c1}",
                color: "#EF5351",
            },
        ),
        (
            ".php",
            IconProperties {
                icon: "\u{f031f}",
                color: "#2088E5",
            },
        ),
        (
            ".pl",
            IconProperties {
                icon: "\u{f03d2}",
                color: "#EF5351",
            },
        ),
        (
            ".pls",
            IconProperties {
                icon: "\u{f0cb9}",
                color: "#ED95AE",
            },
        ),
        (
            ".ply",
            IconProperties {
                icon: "\u{f01a7}",
                color: "#888888",
            },
        ),
        (
            ".pm",
            IconProperties {
                icon: "\u{e769}",
                color: "#9575CE",
            },
        ),
        (
            ".png",
            IconProperties {
                icon: "\u{f021f}",
                color: "#25A6A0",
            },
        ),
        (
            ".po",
            IconProperties {
                icon: "\u{f05ca}",
                color: "#7986CB",
            },
        ),
        (
            ".pot",
            IconProperties {
                icon: "\u{f05ca}",
                color: "#7986CB",
            },
        ),
        (
            ".pp",
            IconProperties {
                icon: "\u{e631}",
                color: "#FFA61A",
            },
        ),
        (
            ".ppt",
            IconProperties {
                icon: "\u{f0227}",
                color: "#D14525",
            },
        ),
        (
            ".pptx",
            IconProperties {
                icon: "\u{f0227}",
                color: "#D14525",
            },
        ),
        (
            ".prisma",
            IconProperties {
                icon: "\u{e684}",
                color: "#00BFA5",
            },
        ),
        (
            ".pro",
            IconProperties {
                icon: "\u{f03d2}",
                color: "#EF5351",
            },
        ),
        (
            ".procfile",
            IconProperties {
                icon: "\u{e607}",
                color: "#6964BA",
            },
        ),
        (
            ".properties",
            IconProperties {
                icon: "\u{f013}",
                color: "#42A5F5",
            },
        ),
        (
            ".ps1",
            IconProperties {
                icon: "\u{f0a0a}",
                color: "#04A9F4",
            },
        ),
        (
            ".psb",
            IconProperties {
                icon: "\u{f021f}",
                color: "#25A6A0",
            },
        ),
        (
            ".psd",
            IconProperties {
                icon: "\u{e7b8}",
                color: "#25A6A0",
            },
        ),
        (
            ".psd1",
            IconProperties {
                icon: "\u{f0a0a}",
                color: "#04A9F4",
            },
        ),
        (
            ".psm1",
            IconProperties {
                icon: "\u{f0a0a}",
                color: "#04A9F4",
            },
        ),
        (
            ".pub",
            IconProperties {
                icon: "\u{f0306}",
                color: "#25A79A",
            },
        ),
        (
            ".pxd",
            IconProperties {
                icon: "\u{e606}",
                color: "#00AFFF",
            },
        ),
        (
            ".pxi",
            IconProperties {
                icon: "\u{e606}",
                color: "#00AFFF",
            },
        ),
        (
            ".pxm",
            IconProperties {
                icon: "\u{f1c5}",
                color: "#626262",
            },
        ),
        (
            ".py",
            IconProperties {
                icon: "\u{ed1b}",
                color: "#FED836",
            },
        ),
        (
            ".pyc",
            IconProperties {
                icon: "\u{e606}",
                color: "#FFA61A",
            },
        ),
        (
            ".pyd",
            IconProperties {
                icon: "\u{e606}",
                color: "#E3C58E",
            },
        ),
        (
            ".pyi",
            IconProperties {
                icon: "\u{e606}",
                color: "#FFA61A",
            },
        ),
        (
            ".pyo",
            IconProperties {
                icon: "\u{e606}",
                color: "#E3C58E",
            },
        ),
        (
            ".pyw",
            IconProperties {
                icon: "\u{e606}",
                color: "#00AFFF",
            },
        ),
        (
            ".pyx",
            IconProperties {
                icon: "\u{e606}",
                color: "#00AFFF",
            },
        ),
        (
            ".qm",
            IconProperties {
                icon: "\u{f05ca}",
                color: "#2596BE",
            },
        ),
        (
            ".qml",
            IconProperties {
                icon: "\u{f375}",
                color: "#42CD52",
            },
        ),
        (
            ".qrc",
            IconProperties {
                icon: "\u{f375}",
                color: "#40CD52",
            },
        ),
        (
            ".qss",
            IconProperties {
                icon: "\u{f375}",
                color: "#40CD52",
            },
        ),
        (
            ".query",
            IconProperties {
                icon: "\u{e21c}",
                color: "#90A850",
            },
        ),
        (
            ".r",
            IconProperties {
                icon: "\u{e68a}",
                color: "#1976D3",
            },
        ),
        (
            ".rake",
            IconProperties {
                icon: "\u{e791}",
                color: "#701516",
            },
        ),
        (
            ".rakefile",
            IconProperties {
                icon: "\u{e21e}",
                color: "#C90F02",
            },
        ),
        (
            ".rar",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".razor",
            IconProperties {
                icon: "\u{f1fa}",
                color: "#207245",
            },
        ),
        (
            ".rb",
            IconProperties {
                icon: "\u{f0d2d}",
                color: "#F54436",
            },
        ),
        (
            ".rdata",
            IconProperties {
                icon: "\u{f25d}",
                color: "#458EE6",
            },
        ),
        (
            ".rdb",
            IconProperties {
                icon: "\u{e76d}",
                color: "#C90F02",
            },
        ),
        (
            ".rdoc",
            IconProperties {
                icon: "\u{f48a}",
                color: "#519ABA",
            },
        ),
        (
            ".rds",
            IconProperties {
                icon: "\u{f25d}",
                color: "#458EE6",
            },
        ),
        (
            ".readme",
            IconProperties {
                icon: "\u{f05a}",
                color: "#42A5F5",
            },
        ),
        (
            ".res",
            IconProperties {
                icon: "\u{e688}",
                color: "#EF5351",
            },
        ),
        (
            ".resi",
            IconProperties {
                icon: "\u{e688}",
                color: "#FFB300",
            },
        ),
        (
            ".rlib",
            IconProperties {
                icon: "\u{e7a8}",
                color: "#DEA584",
            },
        ),
        (
            ".rmd",
            IconProperties {
                icon: "\u{e68a}",
                color: "#1976D3",
            },
        ),
        (
            ".rpm",
            IconProperties {
                icon: "\u{e7bb}",
                color: "#EE0000",
            },
        ),
        (
            ".rproj",
            IconProperties {
                icon: "\u{f05c6}",
                color: "#358A5B",
            },
        ),
        (
            ".rs",
            IconProperties {
                icon: "\u{e68b}",
                color: "#FF7043",
            },
        ),
        (
            ".rspec",
            IconProperties {
                icon: "\u{e21e}",
                color: "#C90F02",
            },
        ),
        (
            ".rspec_parallel",
            IconProperties {
                icon: "\u{e21e}",
                color: "#C90F02",
            },
        ),
        (
            ".rspec_status",
            IconProperties {
                icon: "\u{e21e}",
                color: "#C90F02",
            },
        ),
        (
            ".rss",
            IconProperties {
                icon: "\u{f09e}",
                color: "#965824",
            },
        ),
        (
            ".rtf",
            IconProperties {
                icon: "\u{f022c}",
                color: "#0188D1",
            },
        ),
        (
            ".ru",
            IconProperties {
                icon: "\u{e21e}",
                color: "#C90F02",
            },
        ),
        (
            ".rubydoc",
            IconProperties {
                icon: "\u{e73b}",
                color: "#C90F02",
            },
        ),
        (
            ".s",
            IconProperties {
                icon: "\u{e637}",
                color: "#0091BD",
            },
        ),
        (
            ".sass",
            IconProperties {
                icon: "\u{e603}",
                color: "#EC417A",
            },
        ),
        (
            ".sbt",
            IconProperties {
                icon: "\u{e68d}",
                color: "#0277BD",
            },
        ),
        (
            ".sc",
            IconProperties {
                icon: "\u{e68e}",
                color: "#F54436",
            },
        ),
        (
            ".scad",
            IconProperties {
                icon: "\u{f34e}",
                color: "#F9D72C",
            },
        ),
        (
            ".scala",
            IconProperties {
                icon: "\u{e68e}",
                color: "#F54436",
            },
        ),
        (
            ".scm",
            IconProperties {
                icon: "\u{f0627}",
                color: "#F54436",
            },
        ),
        (
            ".scss",
            IconProperties {
                icon: "\u{e603}",
                color: "#EC417A",
            },
        ),
        (
            ".sh",
            IconProperties {
                icon: "\u{f018d}",
                color: "#FF7043",
            },
        ),
        (
            ".sha1",
            IconProperties {
                icon: "\u{f0565}",
                color: "#8C86AF",
            },
        ),
        (
            ".sha224",
            IconProperties {
                icon: "\u{f0565}",
                color: "#8C86AF",
            },
        ),
        (
            ".sha256",
            IconProperties {
                icon: "\u{f0565}",
                color: "#8C86AF",
            },
        ),
        (
            ".sha384",
            IconProperties {
                icon: "\u{f0565}",
                color: "#8C86AF",
            },
        ),
        (
            ".sha512",
            IconProperties {
                icon: "\u{f0565}",
                color: "#8C86AF",
            },
        ),
        (
            ".shell",
            IconProperties {
                icon: "\u{e795}",
                color: "#89E051",
            },
        ),
        (
            ".sig",
            IconProperties {
                icon: "\u{03bb}",
                color: "#DC682E",
            },
        ),
        (
            ".signature",
            IconProperties {
                icon: "\u{03bb}",
                color: "#DC682E",
            },
        ),
        (
            ".skp",
            IconProperties {
                icon: "\u{ea8c}",
                color: "#2AB6F6",
            },
        ),
        (
            ".sldasm",
            IconProperties {
                icon: "\u{f0eeb}",
                color: "#839463",
            },
        ),
        (
            ".sldprt",
            IconProperties {
                icon: "\u{f0eeb}",
                color: "#839463",
            },
        ),
        (
            ".slim",
            IconProperties {
                icon: "\u{e692}",
                color: "#F57F19",
            },
        ),
        (
            ".sln",
            IconProperties {
                icon: "\u{f0610}",
                color: "#AB48BC",
            },
        ),
        (
            ".slvs",
            IconProperties {
                icon: "\u{f0eeb}",
                color: "#839463",
            },
        ),
        (
            ".sml",
            IconProperties {
                icon: "\u{03bb}",
                color: "#DC682E",
            },
        ),
        (
            ".so",
            IconProperties {
                icon: "\u{f107c}",
                color: "#42A5F5",
            },
        ),
        (
            ".sol",
            IconProperties {
                icon: "\u{e656}",
                color: "#0188D1",
            },
        ),
        (
            ".spec.js",
            IconProperties {
                icon: "\u{f499}",
                color: "#FFCA29",
            },
        ),
        (
            ".spec.jsx",
            IconProperties {
                icon: "\u{f499}",
                color: "#FFCA29",
            },
        ),
        (
            ".spec.ts",
            IconProperties {
                icon: "\u{f499}",
                color: "#519ABA",
            },
        ),
        (
            ".spec.tsx",
            IconProperties {
                icon: "\u{f499}",
                color: "#0188D1",
            },
        ),
        (
            ".sql",
            IconProperties {
                icon: "\u{f1c0}",
                color: "#CFCA99",
            },
        ),
        (
            ".sqlite",
            IconProperties {
                icon: "\u{f1c0}",
                color: "#CFCA99",
            },
        ),
        (
            ".sqlite3",
            IconProperties {
                icon: "\u{f1c0}",
                color: "#CFCA99",
            },
        ),
        (
            ".srt",
            IconProperties {
                icon: "\u{f0a16}",
                color: "#FFA61A",
            },
        ),
        (
            ".ssa",
            IconProperties {
                icon: "\u{f0a16}",
                color: "#FFA61A",
            },
        ),
        (
            ".ste",
            IconProperties {
                icon: "\u{f0eeb}",
                color: "#839463",
            },
        ),
        (
            ".step",
            IconProperties {
                icon: "\u{f0eeb}",
                color: "#839463",
            },
        ),
        (
            ".stl",
            IconProperties {
                icon: "\u{ea8c}",
                color: "#2AB6F6",
            },
        ),
        (
            ".stp",
            IconProperties {
                icon: "\u{ea8c}",
                color: "#2AB6F6",
            },
        ),
        (
            ".strings",
            IconProperties {
                icon: "\u{f05ca}",
                color: "#2596BE",
            },
        ),
        (
            ".sty",
            IconProperties {
                icon: "\u{e69b}",
                color: "#42A5F5",
            },
        ),
        (
            ".styl",
            IconProperties {
                icon: "\u{e759}",
                color: "#C0CA33",
            },
        ),
        (
            ".stylus",
            IconProperties {
                icon: "\u{e600}",
                color: "#83C837",
            },
        ),
        (
            ".sub",
            IconProperties {
                icon: "\u{f0a16}",
                color: "#FFA61A",
            },
        ),
        (
            ".sublime",
            IconProperties {
                icon: "\u{e7aa}",
                color: "#DC682E",
            },
        ),
        (
            ".suo",
            IconProperties {
                icon: "\u{f0610}",
                color: "#AB48BC",
            },
        ),
        (
            ".sv",
            IconProperties {
                icon: "\u{f035b}",
                color: "#FF7043",
            },
        ),
        (
            ".svelte",
            IconProperties {
                icon: "\u{e697}",
                color: "#FF5821",
            },
        ),
        (
            ".svg",
            IconProperties {
                icon: "\u{f0721}",
                color: "#FFB300",
            },
        ),
        (
            ".svh",
            IconProperties {
                icon: "\u{f035b}",
                color: "#FF7043",
            },
        ),
        (
            ".swift",
            IconProperties {
                icon: "\u{f06e5}",
                color: "#FE5E2F",
            },
        ),
        (
            ".t",
            IconProperties {
                icon: "\u{e769}",
                color: "#519ABA",
            },
        ),
        (
            ".tar",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".taz",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".tbc",
            IconProperties {
                icon: "\u{f06d3}",
                color: "#005CA5",
            },
        ),
        (
            ".tbz",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".tbz2",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".tcl",
            IconProperties {
                icon: "\u{f06d3}",
                color: "#EF5351",
            },
        ),
        (
            ".templ",
            IconProperties {
                icon: "\u{f05c0}",
                color: "#FFD550",
            },
        ),
        (
            ".terminal",
            IconProperties {
                icon: "\u{f489}",
                color: "#14BA19",
            },
        ),
        (
            ".test.js",
            IconProperties {
                icon: "\u{f499}",
                color: "#FFCA29",
            },
        ),
        (
            ".test.jsx",
            IconProperties {
                icon: "\u{f499}",
                color: "#FFCA29",
            },
        ),
        (
            ".test.ts",
            IconProperties {
                icon: "\u{f499}",
                color: "#519ABA",
            },
        ),
        (
            ".test.tsx",
            IconProperties {
                icon: "\u{f499}",
                color: "#0188D1",
            },
        ),
        (
            ".tex",
            IconProperties {
                icon: "\u{e69b}",
                color: "#42A5F5",
            },
        ),
        (
            ".tf",
            IconProperties {
                icon: "\u{e69a}",
                color: "#5D6BC0",
            },
        ),
        (
            ".tfvars",
            IconProperties {
                icon: "\u{e69a}",
                color: "#5D6BC0",
            },
        ),
        (
            ".tgz",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".tiff",
            IconProperties {
                icon: "\u{f021f}",
                color: "#25A6A0",
            },
        ),
        (
            ".tlz",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".tmux",
            IconProperties {
                icon: "\u{ebc8}",
                color: "#14BA19",
            },
        ),
        (
            ".toml",
            IconProperties {
                icon: "\u{e6b2}",
                color: "#9C4221",
            },
        ),
        (
            ".torrent",
            IconProperties {
                icon: "\u{e275}",
                color: "#4C90E8",
            },
        ),
        (
            ".tres",
            IconProperties {
                icon: "\u{e65f}",
                color: "#42A5F5",
            },
        ),
        (
            ".ts",
            IconProperties {
                icon: "\u{f06e6}",
                color: "#0188D1",
            },
        ),
        (
            ".tscn",
            IconProperties {
                icon: "\u{e65f}",
                color: "#42A5F5",
            },
        ),
        (
            ".tsconfig",
            IconProperties {
                icon: "\u{e772}",
                color: "#EA8220",
            },
        ),
        (
            ".tsv",
            IconProperties {
                icon: "\u{f021b}",
                color: "#8BC34A",
            },
        ),
        (
            ".tsx",
            IconProperties {
                icon: "\u{ed46}",
                color: "#04BCD4",
            },
        ),
        (
            ".ttf",
            IconProperties {
                icon: "\u{e659}",
                color: "#F54436",
            },
        ),
        (
            ".twig",
            IconProperties {
                icon: "\u{e61c}",
                color: "#9BB92F",
            },
        ),
        (
            ".txt",
            IconProperties {
                icon: "\u{f0219}",
                color: "#42A5F5",
            },
        ),
        (
            ".txz",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".typ",
            IconProperties {
                icon: "\u{f37f}",
                color: "#0DBCC0",
            },
        ),
        (
            ".typoscript",
            IconProperties {
                icon: "\u{e772}",
                color: "#EA8220",
            },
        ),
        (
            ".tz",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".tzo",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".ui",
            IconProperties {
                icon: "\u{f2d0}",
                color: "#015BF0",
            },
        ),
        (
            ".v",
            IconProperties {
                icon: "\u{e6ac}",
                color: "#009CE5",
            },
        ),
        (
            ".vala",
            IconProperties {
                icon: "\u{e8d1}",
                color: "#7B3DB9",
            },
        ),
        (
            ".vh",
            IconProperties {
                icon: "\u{f035b}",
                color: "#009900",
            },
        ),
        (
            ".vhd",
            IconProperties {
                icon: "\u{f035b}",
                color: "#FF7043",
            },
        ),
        (
            ".vhdl",
            IconProperties {
                icon: "\u{f035b}",
                color: "#009900",
            },
        ),
        (
            ".video",
            IconProperties {
                icon: "\u{f03d}",
                color: "#626262",
            },
        ),
        (
            ".vi",
            IconProperties {
                icon: "\u{e81e}",
                color: "#FEC60A",
            },
        ),
        (
            ".vim",
            IconProperties {
                icon: "\u{e62b}",
                color: "#44A047",
            },
        ),
        (
            ".vsh",
            IconProperties {
                icon: "\u{e6ac}",
                color: "#5D87BF",
            },
        ),
        (
            ".vsix",
            IconProperties {
                icon: "\u{f0a1e}",
                color: "#2296F3",
            },
        ),
        (
            ".vue",
            IconProperties {
                icon: "\u{e6a0}",
                color: "#40B883",
            },
        ),
        (
            ".war",
            IconProperties {
                icon: "\u{e256}",
                color: "#F54436",
            },
        ),
        (
            ".wasm",
            IconProperties {
                icon: "\u{e6a1}",
                color: "#7D4DFF",
            },
        ),
        (
            ".wav",
            IconProperties {
                icon: "\u{f0386}",
                color: "#76B900",
            },
        ),
        (
            ".webm",
            IconProperties {
                icon: "\u{f0381}",
                color: "#FF9800",
            },
        ),
        (
            ".webmanifest",
            IconProperties {
                icon: "\u{e60b}",
                color: "#CBCB41",
            },
        ),
        (
            ".webp",
            IconProperties {
                icon: "\u{f021f}",
                color: "#25A6A0",
            },
        ),
        (
            ".webpack",
            IconProperties {
                icon: "\u{f072b}",
                color: "#519ABA",
            },
        ),
        (
            ".windows",
            IconProperties {
                icon: "\u{f17a}",
                color: "#00A4EF",
            },
        ),
        (
            ".wma",
            IconProperties {
                icon: "\u{f0386}",
                color: "#EE534F",
            },
        ),
        (
            ".woff",
            IconProperties {
                icon: "\u{e659}",
                color: "#F54436",
            },
        ),
        (
            ".woff2",
            IconProperties {
                icon: "\u{e659}",
                color: "#F54436",
            },
        ),
        (
            ".wrl",
            IconProperties {
                icon: "\u{f01a7}",
                color: "#778899",
            },
        ),
        (
            ".wrz",
            IconProperties {
                icon: "\u{f01a7}",
                color: "#778899",
            },
        ),
        (
            ".wv",
            IconProperties {
                icon: "\u{f001}",
                color: "#00AFFF",
            },
        ),
        (
            ".wvc",
            IconProperties {
                icon: "\u{f001}",
                color: "#00AFFF",
            },
        ),
        (
            ".x",
            IconProperties {
                icon: "\u{e691}",
                color: "#599EFF",
            },
        ),
        (
            ".xaml",
            IconProperties {
                icon: "\u{f0673}",
                color: "#42A5F5",
            },
        ),
        (
            ".xcf",
            IconProperties {
                icon: "\u{f338}",
                color: "#635b46",
            },
        ),
        (
            ".xcplayground",
            IconProperties {
                icon: "\u{e755}",
                color: "#DC682E",
            },
        ),
        (
            ".xcstrings",
            IconProperties {
                icon: "\u{f05ca}",
                color: "#2596BE",
            },
        ),
        (
            ".xhtml",
            IconProperties {
                icon: "\u{f13b}",
                color: "#E44E27",
            },
        ),
        (
            ".xls",
            IconProperties {
                icon: "\u{f021b}",
                color: "#8BC34A",
            },
        ),
        (
            ".xlsx",
            IconProperties {
                icon: "\u{f021b}",
                color: "#8BC34A",
            },
        ),
        (
            ".xm",
            IconProperties {
                icon: "\u{e691}",
                color: "#519ABA",
            },
        ),
        (
            ".xml",
            IconProperties {
                icon: "\u{f022e}",
                color: "#8BC34A",
            },
        ),
        (
            ".xpi",
            IconProperties {
                icon: "\u{eae6}",
                color: "#375A8E",
            },
        ),
        (
            ".xul",
            IconProperties {
                icon: "\u{f121}",
                color: "#DC682E",
            },
        ),
        (
            ".xz",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".yaml",
            IconProperties {
                icon: "\u{e6a8}",
                color: "#a074b3",
            },
        ),
        (
            ".yml",
            IconProperties {
                icon: "\u{e6a8}",
                color: "#a074b3",
            },
        ),
        (
            ".zig",
            IconProperties {
                icon: "\u{e6a9}",
                color: "#FAA825",
            },
        ),
        (
            ".zip",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
        (
            ".zsh",
            IconProperties {
                icon: "\u{f018d}",
                color: "#FF7043",
            },
        ),
        (
            ".zsh-theme",
            IconProperties {
                icon: "\u{e795}",
                color: "#89E051",
            },
        ),
        (
            ".zshrc",
            IconProperties {
                icon: "\u{e795}",
                color: "#89E051",
            },
        ),
        (
            ".zst",
            IconProperties {
                icon: "\u{f410}",
                color: "#ECA517",
            },
        ),
    ])
});

pub fn icon_for_file(
    name: &str,
    is_submodule: bool,
    is_linked_worktree: bool,
    is_directory: bool,
) -> IconProperties {
    let base = std::path::Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(name);

    if let Some(icon) = NAME_ICON_MAP.get(base) {
        return *icon;
    }

    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()));

    if let Some(ext) = &ext {
        if let Some(icon) = EXT_ICON_MAP.get(ext.as_str()) {
            return *icon;
        }
    }

    if is_submodule {
        DEFAULT_SUBMODULE_ICON
    } else if is_linked_worktree {
        IconProperties {
            icon: LINKED_WORKTREE_ICON,
            color: "#4E4E4E",
        }
    } else if is_directory {
        DEFAULT_DIRECTORY_ICON
    } else {
        DEFAULT_FILE_ICON
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_filename_returns_correct_icon() {
        let icon = icon_for_file("Cargo.toml", false, false, false);
        assert_eq!(icon.icon, "\u{e7a8}");
        assert_eq!(icon.color, "#DEA584");
    }

    #[test]
    fn known_extension_returns_correct_icon() {
        let icon = icon_for_file("main.rs", false, false, false);
        assert_eq!(icon.icon, "\u{e68b}");
        assert_eq!(icon.color, "#FF7043");
    }

    #[test]
    fn extension_lookup_is_case_insensitive() {
        let lower = icon_for_file("photo.jpg", false, false, false);
        let upper = icon_for_file("photo.JPG", false, false, false);
        assert_eq!(lower, upper);
    }

    #[test]
    fn unknown_file_returns_default() {
        let icon = icon_for_file("mystery.zzzzz", false, false, false);
        assert_eq!(icon, DEFAULT_FILE_ICON);
    }

    #[test]
    fn submodule_returns_submodule_icon() {
        let icon = icon_for_file("some_module", true, false, false);
        assert_eq!(icon, DEFAULT_SUBMODULE_ICON);
    }

    #[test]
    fn directory_returns_directory_icon() {
        let icon = icon_for_file("src", false, false, true);
        assert_eq!(icon, DEFAULT_DIRECTORY_ICON);
    }

    #[test]
    fn linked_worktree_returns_worktree_icon() {
        let icon = icon_for_file("wt", false, true, false);
        assert_eq!(icon.icon, LINKED_WORKTREE_ICON);
        assert_eq!(icon.color, "#4E4E4E");
    }

    #[test]
    fn name_match_takes_priority_over_extension() {
        let icon = icon_for_file("Makefile", false, false, false);
        assert_eq!(icon.icon, "\u{e673}");
    }
}
