use topcoat::{
    Result,
    context::Cx,
    icon::{IconData, iconify::iconify_icon},
    view::{Attributes, View, component, view},
};

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Server,
    Layers,
    Box,
    Play,
    Stop,
    Restart,
    Pull,
    Update,
    Trash,
    Folder,
    FolderOpen,
    File,
    Plus,
    X,
    Refresh,
    Terminal,
    Home,
    Power,
    Chevron,
    Logout,
    Pencil,
    Save,
    Menu,
    Up,
    Check,
    Alert,
    Info,
    Settings,
    Package,
    HardDrive,
    Network,
}

pub const ALL: [Icon; 31] = [
    Icon::Server,
    Icon::Layers,
    Icon::Box,
    Icon::Play,
    Icon::Stop,
    Icon::Restart,
    Icon::Pull,
    Icon::Update,
    Icon::Trash,
    Icon::Folder,
    Icon::FolderOpen,
    Icon::File,
    Icon::Plus,
    Icon::X,
    Icon::Refresh,
    Icon::Terminal,
    Icon::Home,
    Icon::Power,
    Icon::Chevron,
    Icon::Logout,
    Icon::Pencil,
    Icon::Save,
    Icon::Menu,
    Icon::Up,
    Icon::Check,
    Icon::Alert,
    Icon::Info,
    Icon::Settings,
    Icon::Package,
    Icon::HardDrive,
    Icon::Network,
];

impl Icon {
    pub fn key(self) -> &'static str {
        match self {
            Icon::Server => "server",
            Icon::Layers => "layers",
            Icon::Box => "box",
            Icon::Play => "play",
            Icon::Stop => "stop",
            Icon::Restart => "restart",
            Icon::Pull => "pull",
            Icon::Update => "update",
            Icon::Trash => "trash",
            Icon::Folder => "folder",
            Icon::FolderOpen => "folder-open",
            Icon::File => "file",
            Icon::Plus => "plus",
            Icon::X => "x",
            Icon::Refresh => "refresh",
            Icon::Terminal => "terminal",
            Icon::Home => "home",
            Icon::Power => "power",
            Icon::Chevron => "chevron",
            Icon::Logout => "logout",
            Icon::Pencil => "pencil",
            Icon::Save => "save",
            Icon::Menu => "menu",
            Icon::Up => "up",
            Icon::Check => "check",
            Icon::Alert => "alert",
            Icon::Info => "info",
            Icon::Settings => "settings",
            Icon::Package => "package",
            Icon::HardDrive => "hard-drive",
            Icon::Network => "network",
        }
    }

    pub fn data(self) -> IconData {
        match self {
            Icon::Server => iconify_icon!("lucide:server"),
            Icon::Layers => iconify_icon!("lucide:layers"),
            Icon::Box => iconify_icon!("lucide:box"),
            Icon::Play => iconify_icon!("lucide:play"),
            Icon::Stop => iconify_icon!("lucide:square"),
            Icon::Restart => iconify_icon!("lucide:rotate-cw"),
            Icon::Pull => iconify_icon!("lucide:download"),
            Icon::Update => iconify_icon!("lucide:circle-arrow-up"),
            Icon::Trash => iconify_icon!("lucide:trash-2"),
            Icon::Folder => iconify_icon!("lucide:folder"),
            Icon::FolderOpen => iconify_icon!("lucide:folder-open"),
            Icon::File => iconify_icon!("lucide:file"),
            Icon::Plus => iconify_icon!("lucide:plus"),
            Icon::X => iconify_icon!("lucide:x"),
            Icon::Refresh => iconify_icon!("lucide:refresh-cw"),
            Icon::Terminal => iconify_icon!("lucide:terminal"),
            Icon::Home => iconify_icon!("lucide:house"),
            Icon::Power => iconify_icon!("lucide:power"),
            Icon::Chevron => iconify_icon!("lucide:chevron-right"),
            Icon::Logout => iconify_icon!("lucide:log-out"),
            Icon::Pencil => iconify_icon!("lucide:pencil"),
            Icon::Save => iconify_icon!("lucide:save"),
            Icon::Menu => iconify_icon!("lucide:menu"),
            Icon::Up => iconify_icon!("lucide:corner-left-up"),
            Icon::Check => iconify_icon!("lucide:circle-check"),
            Icon::Alert => iconify_icon!("lucide:circle-alert"),
            Icon::Info => iconify_icon!("lucide:info"),
            Icon::Settings => iconify_icon!("lucide:settings"),
            Icon::Package => iconify_icon!("lucide:package"),
            Icon::HardDrive => iconify_icon!("lucide:hard-drive"),
            Icon::Network => iconify_icon!("lucide:network"),
        }
    }
}

#[component]
pub async fn icon(cx: &Cx, name: Icon, #[default] size: u16) -> Result<impl View> {
    let size = if size == 0 { 16 } else { size };
    let mut attrs = Attributes::new();
    attrs.insert(cx, "class", "icon");
    Ok(view! {
        topcoat::icon::icon(data: name.data(), size: size, attrs: attrs)
    })
}

#[component]
pub async fn sprite(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        <svg hidden=(true) aria-hidden="true" width="0" height="0" style="position:absolute">
            for name in ALL {
                let data = name.data();
                <symbol id=(format!("i-{}", name.key())) viewBox=(data.view_box())>(data.into_body())</symbol>
            }
        </svg>
    })
}
