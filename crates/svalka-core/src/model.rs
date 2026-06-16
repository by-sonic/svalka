use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ItemKind {
    File,
    Dir,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanItem {
    pub kind: ItemKind,
    pub path: PathBuf,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CategoryId {
    SystemTemp,
    RecycleBin,
    ThumbnailCache,
    CrashDumps,
    Wer,
    WindowsUpdate,
    BrowserCache,
    DevCache,
}

#[derive(Debug, Clone, Serialize)]
pub struct Category {
    pub id: CategoryId,
    pub label: String,
    pub items: Vec<CleanItem>,
}

impl Category {
    pub fn total(&self) -> u64 {
        self.items.iter().map(|i| i.size).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn category_total_sums_item_sizes() {
        let cat = Category {
            id: CategoryId::SystemTemp,
            label: "Временные файлы".into(),
            items: vec![
                CleanItem {
                    kind: ItemKind::Dir,
                    path: PathBuf::from("a"),
                    size: 100,
                },
                CleanItem {
                    kind: ItemKind::File,
                    path: PathBuf::from("b"),
                    size: 50,
                },
            ],
        };
        assert_eq!(cat.total(), 150);
    }
}
