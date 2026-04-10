// Ported from ./references/lazygit-master/pkg/config/keynames.go

use std::collections::HashMap;

/// Maps key codes to their label representation
pub fn label_by_key() -> HashMap<u16, &'static str> {
    let mut m = HashMap::new();
    m.insert(0xF1, "<f1>");
    m.insert(0xF2, "<f2>");
    m.insert(0xF3, "<f3>");
    m.insert(0xF4, "<f4>");
    m.insert(0xF5, "<f5>");
    m.insert(0xF6, "<f6>");
    m.insert(0xF7, "<f7>");
    m.insert(0xF8, "<f8>");
    m.insert(0xF9, "<f9>");
    m.insert(0xF10, "<f10>");
    m.insert(0xF11, "<f11>");
    m.insert(0xF12, "<f12>");
    m.insert(0x101, "<insert>");
    m.insert(0x102, "<delete>");
    m.insert(0x103, "<home>");
    m.insert(0x104, "<end>");
    m.insert(0x105, "<pgup>");
    m.insert(0x106, "<pgdown>");
    m.insert(0x10B, "<up>");
    m.insert(0x10C, "<down>");
    m.insert(0x10D, "<left>");
    m.insert(0x10E, "<right>");
    m.insert(0x109, "<tab>");
    m.insert(0x10A, "<backtab>");
    m.insert(0x10D, "<enter>");
    m.insert(0x111, "<a-enter>");
    m.insert(0x1B, "<esc>");
    m.insert(0x108, "<backspace>");
    m.insert(0x10F, "<c-space>");
    m.insert(0x110, "<c-/>");
    m.insert(0x20, "<space>");
    m.insert(0x01, "<c-a>");
    m.insert(0x02, "<c-b>");
    m.insert(0x03, "<c-c>");
    m.insert(0x04, "<c-d>");
    m.insert(0x05, "<c-e>");
    m.insert(0x06, "<c-f>");
    m.insert(0x07, "<c-g>");
    m.insert(0x0A, "<c-j>");
    m.insert(0x0B, "<c-k>");
    m.insert(0x0C, "<c-l>");
    m.insert(0x0E, "<c-n>");
    m.insert(0x0F, "<c-o>");
    m.insert(0x10, "<c-p>");
    m.insert(0x11, "<c-q>");
    m.insert(0x12, "<c-r>");
    m.insert(0x13, "<c-s>");
    m.insert(0x14, "<c-t>");
    m.insert(0x15, "<c-u>");
    m.insert(0x16, "<c-v>");
    m.insert(0x17, "<c-w>");
    m.insert(0x18, "<c-x>");
    m.insert(0x19, "<c-y>");
    m.insert(0x1A, "<c-z>");
    m
}

/// Maps labels to their key codes
pub fn key_by_label() -> HashMap<&'static str, u16> {
    let mut m = HashMap::new();
    m.insert("<f1>", 0xF1);
    m.insert("<f2>", 0xF2);
    m.insert("<f3>", 0xF3);
    m.insert("<f4>", 0xF4);
    m.insert("<f5>", 0xF5);
    m.insert("<f6>", 0xF6);
    m.insert("<f7>", 0xF7);
    m.insert("<f8>", 0xF8);
    m.insert("<f9>", 0xF9);
    m.insert("<f10>", 0xF10);
    m.insert("<f11>", 0xF11);
    m.insert("<f12>", 0xF12);
    m.insert("<insert>", 0x101);
    m.insert("<delete>", 0x102);
    m.insert("<home>", 0x103);
    m.insert("<end>", 0x104);
    m.insert("<pgup>", 0x105);
    m.insert("<pgdown>", 0x106);
    m.insert("<up>", 0x10B);
    m.insert("<down>", 0x10C);
    m.insert("<left>", 0x10D);
    m.insert("<right>", 0x10E);
    m.insert("<tab>", 0x109);
    m.insert("<backtab>", 0x10A);
    m.insert("<enter>", 0x10D);
    m.insert("<a-enter>", 0x111);
    m.insert("<esc>", 0x1B);
    m.insert("<backspace>", 0x108);
    m.insert("<c-space>", 0x10F);
    m.insert("<c-/>", 0x110);
    m.insert("<space>", 0x20);
    m.insert("<c-a>", 0x01);
    m.insert("<c-b>", 0x02);
    m.insert("<c-c>", 0x03);
    m.insert("<c-d>", 0x04);
    m.insert("<c-e>", 0x05);
    m.insert("<c-f>", 0x06);
    m.insert("<c-g>", 0x07);
    m.insert("<c-j>", 0x0A);
    m.insert("<c-k>", 0x0B);
    m.insert("<c-l>", 0x0C);
    m.insert("<c-n>", 0x0E);
    m.insert("<c-o>", 0x0F);
    m.insert("<c-p>", 0x10);
    m.insert("<c-q>", 0x11);
    m.insert("<c-r>", 0x12);
    m.insert("<c-s>", 0x13);
    m.insert("<c-t>", 0x14);
    m.insert("<c-u>", 0x15);
    m.insert("<c-v>", 0x16);
    m.insert("<c-w>", 0x17);
    m.insert("<c-x>", 0x18);
    m.insert("<c-y>", 0x19);
    m.insert("<c-z>", 0x1A);
    m
}

/// Get the display character for a key code
pub fn key_to_char(key: u16) -> char {
    if let Some(c) = char::from_u32(key as u32) {
        return c;
    }
    // For special keys that can't be represented as char, return a placeholder
    '?'
}
