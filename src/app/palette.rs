//! The math palette's contents.

#[derive(Debug)]
pub struct PaletteItem {
    pub label: &'static str,
    pub insert: &'static str,
    /// How far to move the cursor back after inserting, to land inside the first `{}`.
    pub cursor_back: usize,
}

const fn item(label: &'static str, insert: &'static str, cursor_back: usize) -> PaletteItem {
    PaletteItem {
        label,
        insert,
        cursor_back,
    }
}

const NEEDS_ARGUMENT: [&str; 18] = [
    "\\sin", "\\cos", "\\tan", "\\cot", "\\sec", "\\csc", "\\arcsin", "\\arccos", "\\arctan",
    "\\sinh", "\\cosh", "\\tanh", "\\log", "\\ln", "\\exp", "\\cdot", "\\times", "\\div",
];

impl PaletteItem {
    /// True if the snippet must be followed by a value to be a complete expression.
    pub fn needs_argument(&self) -> bool {
        NEEDS_ARGUMENT.contains(&self.insert.trim_end()) || self.insert.starts_with("\\log_")
    }

    /// True if the snippet must follow a value, as `^{}` does.
    pub fn needs_left_operand(&self) -> bool {
        ["^", "_", "\\cdot", "\\times", "\\div"]
            .iter()
            .any(|p| self.insert.starts_with(p))
    }
}

#[derive(Debug)]
pub struct Category {
    pub name: &'static str,
    pub items: &'static [PaletteItem],
}

pub const CATEGORIES: &[Category] = &[
    Category {
        name: "基本",
        items: &[
            item("\\sin", "\\sin ", 0),
            item("\\log", "\\log ", 0),
            item("\\cos", "\\cos ", 0),
            item("\\tan", "\\tan ", 0),
            item("\\ln", "\\ln ", 0),
            item("分数", "\\frac{}{}", 3),
            item("√", "\\sqrt{}", 1),
            item("xⁿ", "^{}", 1),
            item("π", "\\pi ", 0),
            item("|x|", "|{}|", 2),
        ],
    },
    Category {
        name: "三角関数",
        items: &[
            item("\\sin", "\\sin ", 0),
            item("\\cos", "\\cos ", 0),
            item("\\tan", "\\tan ", 0),
            item("\\cot", "\\cot ", 0),
            item("\\sec", "\\sec ", 0),
            item("\\csc", "\\csc ", 0),
            item("\\arcsin", "\\arcsin ", 0),
            item("\\arccos", "\\arccos ", 0),
            item("\\arctan", "\\arctan ", 0),
            item("\\sinh", "\\sinh ", 0),
            item("\\cosh", "\\cosh ", 0),
            item("\\tanh", "\\tanh ", 0),
        ],
    },
    Category {
        name: "指数・対数",
        items: &[
            item("\\log", "\\log ", 0),
            item("\\log_b", "\\log_{}", 1),
            item("\\ln", "\\ln ", 0),
            item("\\exp", "\\exp ", 0),
            item("eˣ", "e^{}", 1),
            item("n乗根", "\\sqrt[3]{}", 1),
        ],
    },
    Category {
        name: "演算・記号",
        items: &[
            item("分数", "\\frac{}{}", 3),
            item("×", "\\times ", 0),
            item("·", "\\cdot ", 0),
            item("÷", "\\div ", 0),
            item("xⁿ", "^{}", 1),
            item("添字", "_{}", 1),
            item("( )", "({})", 2),
            item("床", "\\lfloor{}\\rfloor", 8),
            item("天井", "\\lceil{}\\rceil", 7),
            item("max", "\\max({},{})", 5),
            item("min", "\\min({},{})", 5),
        ],
    },
    Category {
        name: "ギリシャ文字",
        items: &[
            item("π", "\\pi ", 0),
            item("θ", "\\theta ", 0),
            item("α", "\\alpha ", 0),
            item("β", "\\beta ", 0),
            item("γ", "\\gamma ", 0),
            item("δ", "\\delta ", 0),
            item("λ", "\\lambda ", 0),
            item("μ", "\\mu ", 0),
            item("σ", "\\sigma ", 0),
            item("φ", "\\phi ", 0),
            item("ω", "\\omega ", 0),
        ],
    },
];
