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
    Download,
    ExternalLink,
    Eye,
    EyeOff,
    Fish,
    Folder,
    FolderPlus,
    GripVertical,
    Hammer,
    Home,
    Info,
    LayoutDashboard,
    Layers3,
    Leaf,
    ListTree,
    LoaderCircle,
    PackageSearch,
    PanelLeftClose,
    PanelLeftOpen,
    Pencil,
    Plus,
    PlugZap,
    RotateCcw,
    Search,
    Settings,
    Shuffle,
    Sparkles,
    Sword,
    Trash2,
    Upload,
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
        IconKind::Download => render_icon(LdDownload, class),
        IconKind::ExternalLink => render_icon(LdExternalLink, class),
        IconKind::Eye => render_icon(LdEye, class),
        IconKind::EyeOff => render_icon(LdEyeOff, class),
        IconKind::Fish => render_icon(LdFish, class),
        IconKind::Folder => render_icon(LdFolder, class),
        IconKind::FolderPlus => render_icon(LdFolderPlus, class),
        IconKind::GripVertical => render_icon(LdGripVertical, class),
        IconKind::Hammer => render_icon(LdHammer, class),
        IconKind::Home => render_icon(LdHome, class),
        IconKind::Info => render_icon(LdInfo, class),
        IconKind::LayoutDashboard => render_icon(LdLayoutDashboard, class),
        IconKind::Layers3 => render_icon(LdLayers3, class),
        IconKind::Leaf => render_icon(LdLeaf, class),
        IconKind::ListTree => render_icon(LdListTree, class),
        IconKind::LoaderCircle => render_icon(LdLoaderCircle, class),
        IconKind::PackageSearch => render_icon(LdPackageSearch, class),
        IconKind::PanelLeftClose => render_icon(LdPanelLeftClose, class),
        IconKind::PanelLeftOpen => render_icon(LdPanelLeftOpen, class),
        IconKind::Pencil => render_icon(LdPencil, class),
        IconKind::Plus => render_icon(LdPlus, class),
        IconKind::PlugZap => render_icon(LdPlugZap, class),
        IconKind::RotateCcw => render_icon(LdRotateCcw, class),
        IconKind::Search => render_icon(LdSearch, class),
        IconKind::Settings => render_icon(LdSettings, class),
        IconKind::Shuffle => render_icon(LdShuffle, class),
        IconKind::Sparkles => render_icon(LdSparkles, class),
        IconKind::Sword => render_icon(LdSword, class),
        IconKind::Trash2 => render_icon(LdTrash2, class),
        IconKind::Upload => render_icon(LdUpload, class),
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
