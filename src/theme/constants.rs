use super::Theme;

pub const GRID_UNIT: f32 = 4.0;
pub const SPACING_SMALL: f32 = GRID_UNIT; // 4px
pub const SPACING_MEDIUM: f32 = 2.0 * GRID_UNIT; // 8px
pub const SPACING_LARGE: f32 = 4.0 * GRID_UNIT; // 16px
pub const TREE_INDENT: f32 = SPACING_LARGE;
pub const ROW_HEIGHT: f32 = 22.0;

pub const ROW_PADDING_H: f32 = 24.0; // outer left/right margin for section header and group title
pub const ROW_INNER_H: f32 = 16.0; // horizontal padding INSIDE card rows (matches design 16px)
pub const ROW_PADDING_V: f32 = 14.0; // vertical padding inside card rows (matches design 14px)
pub const GROUP_SPACING: f32 = 20.0;
pub const CARD_OUTER_H: f32 = 24.0; // card indented from panel edges
pub const CARD_RADIUS: f32 = 8.0;
pub const CONTROL_WIDTH: f32 = 220.0;
pub const DIRTY_DOT_RADIUS: f32 = 3.0;

impl Theme {
    pub fn mocha() -> Self {
        Self {
            name: "Catppuccin Mocha".to_string(),
            dark_mode: true,
            bg: "#1e1e2e".into(),
            bg_panel: "#181825".into(),
            bg_sunken: "#11111b".into(),

            surface: "#313244".into(),
            surface_raised: "#45475a".into(),
            surface_active: "#585b70".into(),

            fg: "#cdd6f4".into(),
            fg_muted: "#7f849c".into(),

            syntax_key: "#89b4fa".into(),
            syntax_string: "#a6e3a1".into(),
            syntax_number: "#fab387".into(),
            syntax_bool: "#cba6f7".into(),
            syntax_punctuation: "#9399b2".into(),

            success: "#a6e3a1".into(),
            warning: "#f9e2af".into(),
            error: "#f38ba8".into(),
            info: "#74c7ec".into(),

            accent: "#cba6f7".into(),
            accent_secondary: "#b4befe".into(),

            sidebar_hover: "#6c708633".into(),
            sidebar_header: "#9399b2".into(),
            indent_guide: "#45475a".into(),
        }
    }

    pub fn latte() -> Self {
        Self {
            name: "Catppuccin Latte".into(),
            dark_mode: false,
            bg: "#eff1f5".into(),
            bg_panel: "#e6e9ef".into(),
            bg_sunken: "#dce0e8".into(),

            surface: "#ccd0da".into(),
            surface_raised: "#bcc0cc".into(),
            surface_active: "#acb0be".into(),

            fg: "#4c4f69".into(),
            fg_muted: "#8c8fa1".into(),

            syntax_key: "#1e66f5".into(),
            syntax_string: "#40a02b".into(),
            syntax_number: "#fe640b".into(),
            syntax_bool: "#8839ef".into(),
            syntax_punctuation: "#7c7f93".into(),

            success: "#40a02b".into(),
            warning: "#df8e1d".into(),
            error: "#d20f39".into(),
            info: "#209fb5".into(),

            accent: "#8839ef".into(),
            accent_secondary: "#7287fd".into(),

            sidebar_hover: "#9ca0b033".into(),
            sidebar_header: "#7c7f93".into(),
            indent_guide: "#bcc0cc".into(),
        }
    }

    /// Catppuccin Frappé — medium-dark theme
    pub fn frappe() -> Self {
        Self {
            name: "Catppuccin Frappé".into(),
            dark_mode: true,
            bg: "#303446".into(),
            bg_panel: "#292c3c".into(),
            bg_sunken: "#232634".into(),

            surface: "#414559".into(),
            surface_raised: "#51576d".into(),
            surface_active: "#626880".into(),

            fg: "#c6d0f5".into(),
            fg_muted: "#838ba7".into(),

            syntax_key: "#8caaee".into(),         // Blue
            syntax_string: "#a6d189".into(),      // Green
            syntax_number: "#ef9f76".into(),      // Peach
            syntax_bool: "#ca9ee6".into(),        // Mauve
            syntax_punctuation: "#737994".into(), // Overlay2

            success: "#a6d189".into(),
            warning: "#e5c890".into(),
            error: "#e78284".into(),
            info: "#85c1dc".into(), // Sapphire

            accent: "#ca9ee6".into(),           // Mauve
            accent_secondary: "#babbf1".into(), // Lavender

            sidebar_hover: "#62688033".into(),
            sidebar_header: "#737994".into(), // Overlay2
            indent_guide: "#51576d".into(),   // Surface1
        }
    }

    /// Catppuccin Macchiato — dark theme (slightly lighter than Mocha)
    pub fn macchiato() -> Self {
        Self {
            name: "Catppuccin Macchiato".into(),
            dark_mode: true,
            bg: "#24273a".into(),
            bg_panel: "#1e2030".into(),
            bg_sunken: "#181926".into(),

            surface: "#363a4f".into(),
            surface_raised: "#494d64".into(),
            surface_active: "#5b6078".into(),

            fg: "#cad3f5".into(),
            fg_muted: "#8087a2".into(),

            syntax_key: "#8aadf4".into(),         // Blue
            syntax_string: "#a6da95".into(),      // Green
            syntax_number: "#f5a97f".into(),      // Peach
            syntax_bool: "#c6a0f6".into(),        // Mauve
            syntax_punctuation: "#939ab7".into(), // Overlay2

            success: "#a6da95".into(),
            warning: "#eed49f".into(),
            error: "#ed8796".into(),
            info: "#7dc4e4".into(), // Sapphire

            accent: "#c6a0f6".into(),           // Mauve
            accent_secondary: "#b7bdf8".into(), // Lavender

            sidebar_hover: "#6e738d33".into(),
            sidebar_header: "#939ab7".into(),
            indent_guide: "#494d64".into(),
        }
    }

    pub fn dracula() -> Self {
        Self {
            name: "Dracula".into(),
            dark_mode: true,
            bg: "#282a36".into(),
            bg_panel: "#21222c".into(),
            bg_sunken: "#191a21".into(),
            surface: "#44475a".into(),
            surface_raised: "#4f526b".into(),
            surface_active: "#595c74".into(),
            fg: "#f8f8f2".into(),
            fg_muted: "#6272a4".into(),
            syntax_key: "#8be9fd".into(),
            syntax_string: "#f1fa8c".into(),
            syntax_number: "#ffb86c".into(),
            syntax_bool: "#bd93f9".into(),
            syntax_punctuation: "#6272a4".into(),
            success: "#50fa7b".into(),
            warning: "#f1fa8c".into(),
            error: "#ff5555".into(),
            info: "#8be9fd".into(),
            accent: "#bd93f9".into(),
            accent_secondary: "#ff79c6".into(),
            sidebar_hover: "#44475a33".into(),
            sidebar_header: "#6272a4".into(),
            indent_guide: "#44475a".into(),
        }
    }

    pub fn nord() -> Self {
        Self {
            name: "Nord".into(),
            dark_mode: true,
            bg: "#2e3440".into(),
            bg_panel: "#272c36".into(),
            bg_sunken: "#222730".into(),
            surface: "#3b4252".into(),
            surface_raised: "#434c5e".into(),
            surface_active: "#4c566a".into(),
            fg: "#eceff4".into(),
            fg_muted: "#7b88a1".into(),
            syntax_key: "#81a1c1".into(),
            syntax_string: "#a3be8c".into(),
            syntax_number: "#d08770".into(),
            syntax_bool: "#b48ead".into(),
            syntax_punctuation: "#7b88a1".into(),
            success: "#a3be8c".into(),
            warning: "#ebcb8b".into(),
            error: "#bf616a".into(),
            info: "#88c0d0".into(),
            accent: "#88c0d0".into(),
            accent_secondary: "#81a1c1".into(),
            sidebar_hover: "#3b425233".into(),
            sidebar_header: "#7b88a1".into(),
            indent_guide: "#3b4252".into(),
        }
    }

    pub fn gruvbox_dark() -> Self {
        Self {
            name: "Gruvbox Dark".into(),
            dark_mode: true,
            bg: "#282828".into(),
            bg_panel: "#1d2021".into(),
            bg_sunken: "#161819".into(),
            surface: "#3c3836".into(),
            surface_raised: "#504945".into(),
            surface_active: "#665c54".into(),
            fg: "#ebdbb2".into(),
            fg_muted: "#928374".into(),
            syntax_key: "#83a598".into(),
            syntax_string: "#b8bb26".into(),
            syntax_number: "#fe8019".into(),
            syntax_bool: "#d3869b".into(),
            syntax_punctuation: "#928374".into(),
            success: "#b8bb26".into(),
            warning: "#d79921".into(),
            error: "#cc241d".into(),
            info: "#458588".into(),
            accent: "#d3869b".into(),
            accent_secondary: "#83a598".into(),
            sidebar_hover: "#3c383633".into(),
            sidebar_header: "#928374".into(),
            indent_guide: "#3c3836".into(),
        }
    }

    pub fn tokyo_night() -> Self {
        Self {
            name: "Tokyo Night".into(),
            dark_mode: true,
            bg: "#1a1b26".into(),
            bg_panel: "#16161e".into(),
            bg_sunken: "#13131a".into(),
            surface: "#292e42".into(),
            surface_raised: "#2f3549".into(),
            surface_active: "#364156".into(),
            fg: "#c0caf5".into(),
            fg_muted: "#565f89".into(),
            syntax_key: "#7aa2f7".into(),
            syntax_string: "#9ece6a".into(),
            syntax_number: "#ff9e64".into(),
            syntax_bool: "#bb9af7".into(),
            syntax_punctuation: "#565f89".into(),
            success: "#9ece6a".into(),
            warning: "#e0af68".into(),
            error: "#f7768e".into(),
            info: "#7dcfff".into(),
            accent: "#bb9af7".into(),
            accent_secondary: "#7aa2f7".into(),
            sidebar_hover: "#292e4233".into(),
            sidebar_header: "#565f89".into(),
            indent_guide: "#292e42".into(),
        }
    }

    pub fn rose_pine() -> Self {
        Self {
            name: "Rosé Pine".into(),
            dark_mode: true,
            bg: "#191724".into(),
            bg_panel: "#1f1d2e".into(),
            bg_sunken: "#17151f".into(),
            surface: "#26233a".into(),
            surface_raised: "#2d2b3d".into(),
            surface_active: "#393552".into(),
            fg: "#e0def4".into(),
            fg_muted: "#6e6a86".into(),
            syntax_key: "#9ccfd8".into(),
            syntax_string: "#31748f".into(),
            syntax_number: "#f6c177".into(),
            syntax_bool: "#c4a7e7".into(),
            syntax_punctuation: "#6e6a86".into(),
            success: "#31748f".into(),
            warning: "#f6c177".into(),
            error: "#eb6f92".into(),
            info: "#9ccfd8".into(),
            accent: "#c4a7e7".into(),
            accent_secondary: "#9ccfd8".into(),
            sidebar_hover: "#26233a33".into(),
            sidebar_header: "#6e6a86".into(),
            indent_guide: "#26233a".into(),
        }
    }

    pub fn github_light() -> Self {
        Self {
            name: "GitHub Light".into(),
            dark_mode: false,
            bg: "#ffffff".into(),
            bg_panel: "#f6f8fa".into(),
            bg_sunken: "#eaeef2".into(),
            surface: "#d0d7de".into(),
            surface_raised: "#bbc2ca".into(),
            surface_active: "#adb5bd".into(),
            fg: "#1f2328".into(),
            fg_muted: "#656d76".into(),
            syntax_key: "#0969da".into(),
            syntax_string: "#0a3069".into(),
            syntax_number: "#0550ae".into(),
            syntax_bool: "#8250df".into(),
            syntax_punctuation: "#656d76".into(),
            success: "#1a7f37".into(),
            warning: "#9a6700".into(),
            error: "#d1242f".into(),
            info: "#0969da".into(),
            accent: "#8250df".into(),
            accent_secondary: "#0969da".into(),
            sidebar_hover: "#d0d7de33".into(),
            sidebar_header: "#656d76".into(),
            indent_guide: "#d0d7de".into(),
        }
    }

    pub fn solarized_dark() -> Self {
        Self {
            name: "Solarized Dark".into(),
            dark_mode: true,
            bg: "#002b36".into(),
            bg_panel: "#073642".into(),
            bg_sunken: "#00212b".into(),
            surface: "#073642".into(),
            surface_raised: "#124652".into(),
            surface_active: "#1e5f6e".into(),
            fg: "#eee8d5".into(),
            fg_muted: "#657b83".into(),
            syntax_key: "#268bd2".into(),
            syntax_string: "#859900".into(),
            syntax_number: "#cb4b16".into(),
            syntax_bool: "#d33682".into(),
            syntax_punctuation: "#657b83".into(),
            success: "#859900".into(),
            warning: "#b58900".into(),
            error: "#dc322f".into(),
            info: "#268bd2".into(),
            accent: "#d33682".into(),
            accent_secondary: "#268bd2".into(),
            sidebar_hover: "#07364233".into(),
            sidebar_header: "#657b83".into(),
            indent_guide: "#073642".into(),
        }
    }

    pub fn solarized_light() -> Self {
        Self {
            name: "Solarized Light".into(),
            dark_mode: false,
            bg: "#fdf6e3".into(),
            bg_panel: "#eee8d5".into(),
            bg_sunken: "#ddd6c1".into(),
            surface: "#93a1a1".into(),
            surface_raised: "#839496".into(),
            surface_active: "#718e90".into(),
            fg: "#586e75".into(),
            fg_muted: "#839496".into(),
            syntax_key: "#268bd2".into(),
            syntax_string: "#859900".into(),
            syntax_number: "#cb4b16".into(),
            syntax_bool: "#d33682".into(),
            syntax_punctuation: "#839496".into(),
            success: "#859900".into(),
            warning: "#b58900".into(),
            error: "#dc322f".into(),
            info: "#268bd2".into(),
            accent: "#d33682".into(),
            accent_secondary: "#268bd2".into(),
            sidebar_hover: "#93a1a133".into(),
            sidebar_header: "#839496".into(),
            indent_guide: "#93a1a1".into(),
        }
    }
}
