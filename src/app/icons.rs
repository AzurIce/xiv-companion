use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::*;
use dioxus_free_icons::{Icon as FreeIcon, IconShape};

#[derive(Clone, Copy, PartialEq)]
pub enum IconKind {
    BookOpen,
    ChevronDown,
    ChevronRight,
    CircleCheck,
    Coins,
    Copy,
    Database,
    ExternalLink,
    FilePlus2,
    Fish,
    Folder,
    FolderPlus,
    Hammer,
    Home,
    Info,
    LayoutDashboard,
    Leaf,
    LoaderCircle,
    MoreHorizontal,
    PackageSearch,
    PanelLeftClose,
    PanelLeftOpen,
    Pencil,
    Plus,
    RotateCcw,
    Search,
    Shuffle,
    Sparkles,
    Trash2,
    Wrench,
    X,
    ZoomIn,
    ZoomOut,
}

#[component]
pub fn Icon(kind: IconKind, #[props(default = "h-4 w-4")] class: &'static str) -> Element {
    match kind {
        IconKind::BookOpen => render_icon(LdBookOpen, class),
        IconKind::ChevronDown => render_icon(LdChevronDown, class),
        IconKind::ChevronRight => render_icon(LdChevronRight, class),
        IconKind::CircleCheck => render_icon(LdCircleCheck, class),
        IconKind::Coins => render_icon(LdCoins, class),
        IconKind::Copy => render_icon(LdCopy, class),
        IconKind::Database => render_icon(LdDatabase, class),
        IconKind::ExternalLink => render_icon(LdExternalLink, class),
        IconKind::FilePlus2 => render_icon(LdFilePlus2, class),
        IconKind::Fish => render_icon(LdFish, class),
        IconKind::Folder => render_icon(LdFolder, class),
        IconKind::FolderPlus => render_icon(LdFolderPlus, class),
        IconKind::Hammer => render_icon(LdHammer, class),
        IconKind::Home => render_icon(LdHome, class),
        IconKind::Info => render_icon(LdInfo, class),
        IconKind::LayoutDashboard => render_icon(LdLayoutDashboard, class),
        IconKind::Leaf => render_icon(LdLeaf, class),
        IconKind::LoaderCircle => render_icon(LdLoaderCircle, class),
        IconKind::MoreHorizontal => render_icon(LdEllipsis, class),
        IconKind::PackageSearch => render_icon(LdPackageSearch, class),
        IconKind::PanelLeftClose => render_icon(LdPanelLeftClose, class),
        IconKind::PanelLeftOpen => render_icon(LdPanelLeftOpen, class),
        IconKind::Pencil => render_icon(LdPencil, class),
        IconKind::Plus => render_icon(LdPlus, class),
        IconKind::RotateCcw => render_icon(LdRotateCcw, class),
        IconKind::Search => render_icon(LdSearch, class),
        IconKind::Shuffle => render_icon(LdShuffle, class),
        IconKind::Sparkles => render_icon(LdSparkles, class),
        IconKind::Trash2 => render_icon(LdTrash2, class),
        IconKind::Wrench => render_icon(LdWrench, class),
        IconKind::X => render_icon(LdX, class),
        IconKind::ZoomIn => render_icon(LdZoomIn, class),
        IconKind::ZoomOut => render_icon(LdZoomOut, class),
    }
}

fn render_icon<T>(icon: T, class: &'static str) -> Element
where
    T: IconShape + Clone + PartialEq + 'static,
{
    rsx! {
        FreeIcon {
            icon,
            class: class.to_string(),
            width: 16,
            height: 16,
        }
    }
}
