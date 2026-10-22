use std::path::Path;

pub fn toggle<T: PartialEq>(list: &mut Vec<T>, item: T) {
    if let Some(i) = list.iter().position(|x| *x == item) {
        list.remove(i);
    } else {
        list.push(item);
    }
}

pub fn tilde(path: &Path) -> String {
    let full = path.display().to_string();
    match std::env::home_dir().map(|h| h.display().to_string()) {
        Some(home) if full.starts_with(&home) => format!("~{}", &full[home.len()..]),
        _ => full,
    }
}
