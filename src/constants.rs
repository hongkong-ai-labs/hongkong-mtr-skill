#[derive(Debug, Clone, Copy)]
pub struct KnownLine {
    pub id: i64,
    pub code: &'static str,
    pub name_en: &'static str,
    pub name_tc: &'static str,
    pub color: &'static str,
    pub real_time: bool,
}

pub const KNOWN_LINES: [KnownLine; 12] = [
    KnownLine {
        id: 10,
        code: "AEL",
        name_en: "Airport Express",
        name_tc: "機場快綫",
        color: "#1C7670",
        real_time: true,
    },
    KnownLine {
        id: 16,
        code: "DRL",
        name_en: "Disneyland Resort Line",
        name_tc: "迪士尼綫",
        color: "#F550A6",
        real_time: true,
    },
    KnownLine {
        id: 17,
        code: "EAL",
        name_en: "East Rail Line",
        name_tc: "東鐵綫",
        color: "#5EB6E4",
        real_time: true,
    },
    KnownLine {
        id: 13,
        code: "ISL",
        name_en: "Island Line",
        name_tc: "港島綫",
        color: "#0860A8",
        real_time: true,
    },
    KnownLine {
        id: 12,
        code: "KTL",
        name_en: "Kwun Tong Line",
        name_tc: "觀塘綫",
        color: "#1A9431",
        real_time: true,
    },
    KnownLine {
        id: 27,
        code: "SIL",
        name_en: "South Island Line",
        name_tc: "南港島綫",
        color: "#B5BD00",
        real_time: true,
    },
    KnownLine {
        id: 14,
        code: "TCL",
        name_en: "Tung Chung Line",
        name_tc: "東涌綫",
        color: "#FE7F1D",
        real_time: true,
    },
    KnownLine {
        id: 15,
        code: "TKL",
        name_en: "Tseung Kwan O Line",
        name_tc: "將軍澳綫",
        color: "#6B208B",
        real_time: true,
    },
    KnownLine {
        id: 28,
        code: "TML",
        name_en: "Tuen Ma Line",
        name_tc: "屯馬綫",
        color: "#923011",
        real_time: true,
    },
    KnownLine {
        id: 11,
        code: "TWL",
        name_en: "Tsuen Wan Line",
        name_tc: "荃灣綫",
        color: "#FF0000",
        real_time: true,
    },
    KnownLine {
        id: 91,
        code: "HSR",
        name_en: "High Speed Rail",
        name_tc: "高速鐵路",
        color: "#F6821F",
        real_time: false,
    },
    KnownLine {
        id: 0,
        code: "LR",
        name_en: "Light Rail",
        name_tc: "輕鐵",
        color: "#9F7A00",
        real_time: true,
    },
];

pub fn known_line(code: &str) -> Option<&'static KnownLine> {
    let upper = code.to_ascii_uppercase();
    KNOWN_LINES.iter().find(|line| line.code == upper)
}

pub fn line_name_for_code(code: &str) -> String {
    known_line(code)
        .map(|line| line.name_en.to_string())
        .unwrap_or_else(|| code.to_string())
}

pub fn is_next_train_line(code: &str) -> bool {
    matches!(
        code.to_ascii_uppercase().as_str(),
        "AEL" | "DRL" | "EAL" | "ISL" | "KTL" | "SIL" | "TCL" | "TKL" | "TML" | "TWL"
    )
}
