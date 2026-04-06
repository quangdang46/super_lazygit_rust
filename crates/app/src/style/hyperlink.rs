// Ported from ./references/lazygit-master/pkg/gui/style/hyperlink.go

pub fn print_hyperlink(text: &str, link: &str) -> String {
    format!("\x1B]8;;{}\x1B\\{}\x1B]8;;\x1B\\", link, text)
}

pub fn print_simple_hyperlink(link: &str) -> String {
    print_hyperlink(link, link)
}
