use lime_syntax::HighlightKind;
use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone)]
pub struct UiTheme {
    pub background: Color,
    pub foreground: Color,
    pub dim: Color,
    pub gutter: Color,
    pub gutter_active: Color,
    pub status_bg: Color,
    pub status_fg: Color,
    pub help_bg: Color,
    pub help_fg: Color,
    pub popup_bg: Color,
    pub popup_border: Color,
    pub selection_bg: Color,
    pub cursor: Color,
    pub keyword: Color,
    pub string: Color,
    pub comment: Color,
    pub function: Color,
    pub type_name: Color,
    pub number: Color,
    pub error: Color,
    pub punctuation: Color,
    pub heading: Color,
}

impl Default for UiTheme {
    fn default() -> Self {
        Self::lime_dark()
    }
}

impl UiTheme {
    pub fn lime_dark() -> Self {
        Self {
            background: Color::Rgb(18, 20, 18),
            foreground: Color::Rgb(220, 226, 218),
            dim: Color::Rgb(112, 121, 108),
            gutter: Color::Rgb(88, 96, 84),
            gutter_active: Color::Rgb(174, 210, 118),
            status_bg: Color::Rgb(53, 83, 45),
            status_fg: Color::Rgb(236, 244, 222),
            help_bg: Color::Rgb(28, 32, 27),
            help_fg: Color::Rgb(177, 187, 169),
            popup_bg: Color::Rgb(22, 25, 22),
            popup_border: Color::Rgb(135, 175, 95),
            selection_bg: Color::Rgb(57, 72, 54),
            cursor: Color::Rgb(203, 248, 111),
            keyword: Color::Rgb(145, 205, 255),
            string: Color::Rgb(177, 219, 139),
            comment: Color::Rgb(107, 122, 100),
            function: Color::Rgb(246, 207, 122),
            type_name: Color::Rgb(129, 221, 199),
            number: Color::Rgb(230, 172, 117),
            error: Color::Rgb(255, 111, 111),
            punctuation: Color::Rgb(157, 167, 148),
            heading: Color::Rgb(203, 248, 111),
        }
    }

    pub fn normal(&self) -> Style {
        Style::default().fg(self.foreground).bg(self.background)
    }

    pub fn dim(&self) -> Style {
        Style::default().fg(self.dim).bg(self.background)
    }

    pub fn syntax_style(&self, kind: HighlightKind) -> Style {
        let color = match kind {
            HighlightKind::Normal => self.foreground,
            HighlightKind::Keyword => self.keyword,
            HighlightKind::String => self.string,
            HighlightKind::Comment => self.comment,
            HighlightKind::Function => self.function,
            HighlightKind::TypeName => self.type_name,
            HighlightKind::Number => self.number,
            HighlightKind::Error => self.error,
            HighlightKind::Punctuation => self.punctuation,
            HighlightKind::Heading => self.heading,
        };

        let mut style = Style::default().fg(color).bg(self.background);
        if matches!(kind, HighlightKind::Heading) {
            style = style.add_modifier(Modifier::BOLD);
        }
        style
    }
}
