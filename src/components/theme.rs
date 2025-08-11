use eframe::egui;

pub struct Theme {
    pub top_bar: String,
    pub central_panel: String,
    pub key: String,
    pub value: String,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            top_bar: "#1E1A78".to_string(),
            central_panel: "#1E2D5B".to_string(),
            key: "#B2B2D1".to_string(),
            value: "#FFCC00".to_string(),
        }
    }
}

impl Theme {
    fn parse_color(s: &str) -> egui::Color32 {
        let s = s.trim_start_matches('#');
        if s.len() == 6 {
            if let Ok(rgb) = u32::from_str_radix(s, 16) {
                let r = ((rgb >> 16) & 0xFF) as u8;
                let g = ((rgb >> 8) & 0xFF) as u8;
                let b = (rgb & 0xFF) as u8;
                return egui::Color32::from_rgb(r, g, b);
            }
        }
        // Fallback to white
        egui::Color32::WHITE
    }

    pub fn top_bar_color(&self) -> egui::Color32 {
        Self::parse_color(&self.top_bar)
    }

    pub fn central_panel_color(&self) -> egui::Color32 {
        Self::parse_color(&self.central_panel)
    }

    pub fn key_color(&self) -> egui::Color32 {
        Self::parse_color(&self.key)
    }

    pub fn value_color(&self) -> egui::Color32 {
        Self::parse_color(&self.value)
    }
}
