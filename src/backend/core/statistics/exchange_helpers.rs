fn builtin_cny_based_rates() -> Vec<(&'static str, f64)> {
    vec![
        ("USD", 0.139),
        ("EUR", 0.128),
        ("GBP", 0.110),
        ("JPY", 20.76),
        ("HKD", 1.087),
        ("KRW", 183.33),
        ("AUD", 0.211),
        ("CAD", 0.189),
        ("SGD", 0.186),
        ("TWD", 4.35),
        ("MYR", 0.646),
        ("THB", 4.86),
        ("VND", 3425.0),
        ("CHF", 0.123),
        ("NZD", 0.231),
    ]
}

fn format_rate_text(value: f64) -> String {
    let rounded = (value * 1_000_000.0).round() / 1_000_000.0;
    let mut text = format!("{rounded:.6}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if !text.contains('.') {
        text.push_str(".0");
    }
    text
}

fn chinese_currency_name_map() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("人民币", "CNY"),
        ("美元", "USD"),
        ("欧元", "EUR"),
        ("英镑", "GBP"),
        ("日元", "JPY"),
        ("港币", "HKD"),
        ("韩元", "KRW"),
        ("韩国元", "KRW"),
        ("澳大利亚元", "AUD"),
        ("澳币", "AUD"),
        ("加拿大元", "CAD"),
        ("加拿大币", "CAD"),
        ("新加坡元", "SGD"),
        ("新加坡币", "SGD"),
        ("新台币", "TWD"),
        ("林吉特", "MYR"),
        ("马来币", "MYR"),
        ("泰国铢", "THB"),
        ("泰币", "THB"),
        ("越南盾", "VND"),
        ("瑞士法郎", "CHF"),
        ("新西兰元", "NZD"),
        ("纽元", "NZD"),
    ])
}
