pub fn cx(values: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    values
        .into_iter()
        .map(|value| value.as_ref().trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn format_integer(value: impl Into<f64>) -> String {
    let value = value.into();
    if !value.is_finite() {
        return "-".to_string();
    }
    let text = value.round().abs().to_string();
    let mut result = String::new();
    for (index, ch) in text.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    if value < 0.0 {
        result.push('-');
    }
    result.chars().rev().collect()
}
