//! Human names for virtual keys, as printed on a Spanish keyboard.

use std::borrow::Cow;

use crate::Vk;

pub fn key_name(vk: Vk) -> Cow<'static, str> {
    let fixed = match vk {
        0x08 => "Retroceso",
        0x09 => "Tab",
        0x0D => "Enter",
        0x10 => "Shift",
        0x11 => "Ctrl",
        0x12 => "Alt",
        0x13 => "Pausa",
        0x14 => "Bloq Mayús",
        0x1B => "Esc",
        0x20 => "Espacio",
        0x21 => "Re Pág",
        0x22 => "Av Pág",
        0x23 => "Fin",
        0x24 => "Inicio",
        0x25 => "←",
        0x26 => "↑",
        0x27 => "→",
        0x28 => "↓",
        0x2C => "Impr Pant",
        0x2D => "Insert",
        0x2E => "Supr",
        0x5B => "Win izq.",
        0x5C => "Win der.",
        0x5D => "Menú",
        0x6A => "Num *",
        0x6B => "Num +",
        0x6D => "Num −",
        0x6E => "Num .",
        0x6F => "Num /",
        0x90 => "Bloq Num",
        0x91 => "Bloq Despl",
        0xA0 => "Shift izq.",
        0xA1 => "Shift der.",
        0xA2 => "Ctrl izq.",
        0xA3 => "Ctrl der.",
        0xA4 => "Alt",
        0xA5 => "Alt Gr",
        0xA6 => "Atrás",
        0xA7 => "Adelante",
        0xA8 => "Recargar",
        0xAD => "Silenciar",
        0xAE => "Volumen −",
        0xAF => "Volumen +",
        0xB0 => "Siguiente",
        0xB1 => "Anterior",
        0xB2 => "Detener",
        0xB3 => "Reproducir",
        _ => "",
    };
    if !fixed.is_empty() {
        return fixed.into();
    }
    match vk {
        b'0'..=b'9' | b'A'..=b'Z' => char::from(vk).to_string().into(),
        0x60..=0x69 => format!("Num {}", vk - 0x60).into(),
        0x70..=0x87 => format!("F{}", vk - 0x6F).into(),
        // OEM keys depend on the layout: ask Windows what this one types.
        _ => crate::hook::layout_char(vk)
            .map(|c| c.to_uppercase().to_string().into())
            .unwrap_or_else(|| format!("Tecla {vk:#04X}").into()),
    }
}

#[cfg(test)]
mod tests {
    use super::key_name;

    #[test]
    fn names_common_keys() {
        assert_eq!(key_name(0x14), "Bloq Mayús");
        assert_eq!(key_name(b'Q'), "Q");
        assert_eq!(key_name(0x7B), "F12");
        assert_eq!(key_name(0x87), "F24");
        assert_eq!(key_name(0x65), "Num 5");
    }
}
