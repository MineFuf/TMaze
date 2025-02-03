use crossterm::event::{
    Event as TermEvent, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use pad::PadStr;
use unicode_width::UnicodeWidthStr;

use std::{
    borrow::Cow,
    fmt::{self},
    io,
    ops::RangeInclusive,
};

use cmaze::dims::Dims;

use crate::{
    app::{
        activity::{Activity, ActivityHandler, Change},
        app::AppData,
        event::Event,
    },
    helpers::{is_release, strings::MbyStaticStr, LineDir},
    renderer::Frame,
    settings::theme::{Style, Theme, ThemeResolver},
};

use super::{center_box_in_screen, draw_box, Rect, Screen};

pub fn panic_on_menu_push() -> ! {
    panic!("menu should only be popping itself or staying");
}

pub struct SliderDef {
    pub text: MbyStaticStr,
    pub val: i32,
    pub range: RangeInclusive<i32>,
    #[allow(clippy::type_complexity)]
    // FIXME: take value instead of change direction (bool),
    // this should allow for mouse support
    pub fun: Box<dyn FnMut(bool, &mut i32, &mut AppData)>,
    pub as_num: bool,
}

pub struct OptionDef {
    pub text: MbyStaticStr,
    pub val: bool,
    #[allow(clippy::type_complexity)]
    // FIXME: return the bool instead
    pub fun: Box<dyn FnMut(&mut bool, &mut AppData)>,
}

// TODO: styling individual items
pub enum MenuItem {
    Text(MbyStaticStr),
    Option(OptionDef),
    Slider(SliderDef),
    Separator,
}

impl MenuItem {
    fn width(&self, special: usize) -> Option<usize> {
        match self {
            MenuItem::Text(text) => Some(text.width()),
            MenuItem::Option(OptionDef { text, .. }) => Some(text.width() + 4),
            MenuItem::Slider(SliderDef {
                text,
                range,
                as_num: show_as_number,
                ..
            }) => {
                assert!(range.start() <= range.end());
                assert!(
                    (*show_as_number || *range.start() >= 0),
                    "if range is not shown as number, it must be positive"
                );

                let text_width = text.width();
                if *show_as_number {
                    let min = range.start().to_string().len();
                    let max = range.end().to_string().len();
                    Some(text_width + min.max(max) + 3)
                } else {
                    let boxes = range.end() - range.start();
                    Some(text_width + boxes as usize + 4)
                }
            }
            MenuItem::Separator => None,
        }
        .map(|w| w + special)
    }

    // TODO: same as Display, make it get a buffer and write to it,
    // so we don't allocate a new string every time
    fn render(&self, width: usize) -> Cow<str> {
        match self {
            MenuItem::Text(text) => text.as_ref_cow(),
            MenuItem::Option(OptionDef { text, val, .. }) => {
                // TODO: this is not a prefix tho ?!?
                let prefix = if *val { "[▪]" } else { "[ ]" };
                let text_w = text.width();
                format!("{text} {prefix:>width$}", width = width - text_w - 1).into()
            }
            MenuItem::Slider(SliderDef {
                text,
                val,
                as_num,
                range,
                ..
            }) => {
                if *as_num {
                    format!("[{val}] {text}").into()
                } else {
                    // TODO: find the best character to use
                    // const FILLED: char = '█';
                    const FILLED: char = '#';
                    // const FILLED: char = '-';

                    let count = (range.end() - range.start()) as usize;

                    let filled = (*val - range.start()) as usize;
                    let empty = count - filled;

                    let filled = FILLED.to_string().repeat(filled);
                    let empty = " ".repeat(empty);

                    let progress = filled + &empty;
                    let text_width = text.width();

                    let indicator = format!(" [{progress}]");

                    format!("{text}{indicator:>width$}", width = width - text_width).into()
                }
            }
            MenuItem::Separator => LineDir::Horizontal.round().to_string().repeat(width).into(),
        }
    }
}

impl From<String> for MenuItem {
    fn from(s: String) -> Self {
        MenuItem::Text(s.into())
    }
}

impl From<&str> for MenuItem {
    fn from(s: &str) -> Self {
        MenuItem::Text(s.to_string().into())
    }
}

impl fmt::Debug for MenuItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MenuItem::Text(s) => write!(f, "Text({})", s),
            MenuItem::Option(OptionDef { text, val, .. }) => write!(f, "Option({}, {})", text, val),
            MenuItem::Slider(SliderDef {
                text, val, range, ..
            }) => write!(f, "Slider({}, {}, {:?})", text, val, range),
            MenuItem::Separator => write!(f, "Separator"),
        }
    }
}

pub struct MenuConfig {
    pub title: String,
    pub subtitles: Vec<String>,
    pub options: Vec<MenuItem>,
    pub default: Option<usize>,
    pub counted: bool,
    pub q_to_quit: bool,
    pub auto_select_single: bool,
    pub styles: MenuStyles,
}

impl MenuConfig {
    pub fn new_from_strings(title: impl Into<String>, options: impl Into<Vec<String>>) -> Self {
        let options: Vec<_> = Into::<Vec<_>>::into(options)
            .into_iter()
            .map(MenuItem::from)
            .collect();

        Self::new(title, options)
    }

    pub fn new(title: impl Into<String>, options: impl Into<Vec<MenuItem>>) -> Self {
        Self {
            title: title.into(),
            subtitles: vec![],
            options: options.into(),
            default: None,
            counted: false,
            q_to_quit: true,
            auto_select_single: false,
            styles: MenuStyles::default(),
        }
    }

    pub fn counted(mut self) -> Self {
        self.counted = true;
        self
    }

    pub fn default(mut self, default: usize) -> Self {
        self.default = Some(default);
        self
    }

    pub fn maybe_default(mut self, default: Option<usize>) -> Self {
        self.default = default;
        self
    }

    pub fn no_q(mut self) -> Self {
        self.q_to_quit = false;
        self
    }

    pub fn auto_select_single(mut self) -> Self {
        self.auto_select_single = true;
        self
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitles.push(subtitle.into());
        self
    }

    pub fn subtitles(mut self, subtitles: impl Into<Vec<String>>) -> Self {
        self.subtitles.extend(subtitles.into());
        self
    }

    pub fn with_styles(mut self, styles: MenuStyles) -> Self {
        self.styles = styles;
        self
    }

    fn special_width(&self) -> usize {
        let mut special = 2; // 2 is for cursor

        if self.default.is_some() {
            special += 2;
        }

        if self.counted {
            let max_num_w = (self.options.len() as f64).log10().ceil() as usize;
            special += max_num_w + 2;
        }

        special
    }

    fn map_options<'s, T>(
        &'s self,
        f: impl Fn(&'s MenuItem) -> T + 'static,
    ) -> impl Iterator<Item = T> + 's {
        self.options.iter().map(f)
    }
}

pub struct MenuStyles {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub text: &'static str,
    pub border: &'static str,
    pub separator: &'static str,
    pub selector: &'static str,
    pub number: &'static str,
}

impl MenuStyles {
    fn apply(&self, theme: &Theme) -> AppliedStyles {
        AppliedStyles {
            title: theme[self.title],
            subtitle: theme[self.subtitle],
            text: theme[self.text],
            border: theme[self.border],
            separator: theme[self.separator],
            selector: theme[self.selector],
            number: theme[self.number],
        }
    }
}

impl Default for MenuStyles {
    fn default() -> Self {
        Self {
            title: "ui_menu_title",
            subtitle: "ui_menu_subtitle",
            text: "ui_menu_text",
            border: "ui_menu_border",
            separator: "ui_menu_separator",
            selector: "ui_menu_selector",
            number: "ui_menu_number",
        }
    }
}

struct AppliedStyles {
    title: Style,
    subtitle: Style,
    text: Style,
    border: Style,
    separator: Style,
    selector: Style,
    number: Style,
}

pub struct Menu {
    config: MenuConfig,
    selected: usize, // isize for more readable code
    items_pos: Option<Rect>,
}

impl Menu {
    pub fn new(config: MenuConfig) -> Self {
        let MenuConfig { options, .. } = &config;

        let default = config.default.unwrap_or(0).clamp(0, options.len() - 1);

        Self {
            selected: default,
            config,
            items_pos: None,
        }
    }

    pub fn into_activity(self) -> Activity {
        Activity::new("tmaze", "menu", Box::new(self))
    }

    fn select(&mut self, down: bool) {
        let opt_count = self.config.options.len();
        loop {
            // negative numbers wrap around zero
            if down {
                self.selected = (self.selected + 1) % opt_count;
            } else {
                self.selected =
                    (self.selected as isize - 1).rem_euclid(opt_count as isize) as usize;
            }

            // skip separators
            if !matches!(self.config.options[self.selected], MenuItem::Separator) {
                break;
            }
        }
    }

    fn switch(&mut self, data: &mut AppData) -> Option<Change> {
        let selected_opt = &mut self.config.options[self.selected];

        match selected_opt {
            MenuItem::Text(_) => return Some(Change::pop_top_with(self.selected)),
            MenuItem::Option(OptionDef { val, fun, .. }) => fun(val, data),
            MenuItem::Slider(_) | MenuItem::Separator => {}
        }

        None
    }

    fn update_slider(&mut self, right: bool, data: &mut AppData) {
        if let MenuItem::Slider(SliderDef {
            val, range, fun, ..
        }) = &mut self.config.options[self.selected]
        {
            fun(right, val, data);
            *val = (*val).clamp(*range.start(), *range.end());
        }
    }

    fn get_opt_by_mouse_pos(&self, Dims(x, y): Dims) -> Option<usize> {
        let Rect { start, end } = self.items_pos?;
        let size = end - start;

        // TODO: check using ranges instead
        if y < start.1 || y > start.1 + size.1 || x < start.0 || x > start.0 + size.0 {
            return None;
        }

        let selected = (y - start.1) as usize;

        if matches!(self.config.options[selected], MenuItem::Separator) {
            return None;
        }

        Some(selected)
    }
}

impl ActivityHandler for Menu {
    fn update(&mut self, events: Vec<Event>, app_data: &mut AppData) -> Option<Change> {
        let opt_count = self.config.options.len() as isize;
        let non_sep_count = self
            .config
            .map_options(|opt| !matches!(opt, MenuItem::Separator))
            .count() as isize;

        if non_sep_count == 1 && self.config.auto_select_single {
            log::info!("Menu with only one option, returning that");
            let first_non_separator = self
                .config
                .options
                .iter()
                .position(|opt| !matches!(opt, MenuItem::Separator))
                .unwrap();
            return Some(Change::pop_top_with(first_non_separator));
        } else if non_sep_count == 0 {
            log::warn!("Empty menu, returning `None`");
            return Some(Change::pop_top());
        }

        /// Return if the expression is `Some`, otherwise do nothing.
        /// Can be thought of as a `?` or `try` for `Some` instead of `None`.
        macro_rules! return_if_some {
            ($change:expr) => {
                if let Some(change) = $change {
                    return Some(change);
                }
            };
        }

        let dims = MenuDimenstions::calc(&self.config);
        self.items_pos = Some(Rect::sized_at(dims.items_pos, dims.items_size));

        for event in events {
            match event {
                Event::Term(TermEvent::Key(KeyEvent { code, kind, .. })) if !is_release(kind) => {
                    match code {
                        KeyCode::Up | KeyCode::Char('w') => {
                            self.select(false);
                        }
                        KeyCode::Down | KeyCode::Char('s') => {
                            self.select(true);
                        }
                        KeyCode::Enter | KeyCode::Char(' ') => {
                            return_if_some!(self.switch(app_data));
                        }
                        KeyCode::Char('q') if !self.config.q_to_quit => {
                            return Some(Change::pop_top())
                        }
                        KeyCode::Char('q') if self.config.q_to_quit => {
                            return Some(Change::pop_all())
                        }
                        KeyCode::Char(ch @ '1'..='9') if self.config.counted => {
                            let old_sel = self.selected;
                            self.selected =
                                (ch as isize - '1' as isize).clamp(0, opt_count - 1) as usize;

                            if old_sel == self.selected {
                                return_if_some!(self.switch(app_data));
                            }
                        }
                        KeyCode::Esc => return Some(Change::pop_top()),
                        KeyCode::Left => {
                            self.update_slider(false, app_data);
                        }
                        KeyCode::Right => {
                            self.update_slider(true, app_data);
                        }
                        _ => {}
                    }
                }
                Event::Term(TermEvent::Mouse(MouseEvent {
                    kind,
                    column,
                    row,
                    modifiers,
                })) => {
                    let mouse_pos = (column, row).into();
                    match kind {
                        MouseEventKind::Moved => {
                            if let Some(selected) = self.get_opt_by_mouse_pos(mouse_pos) {
                                self.selected = selected;
                            }
                        }
                        MouseEventKind::ScrollDown => {
                            if modifiers.contains(KeyModifiers::CONTROL) {
                                self.update_slider(false, app_data);
                            } else {
                                self.select(true);
                            }
                        }
                        MouseEventKind::ScrollUp => {
                            if modifiers.contains(KeyModifiers::CONTROL) {
                                self.update_slider(true, app_data);
                            } else {
                                self.select(false);
                            }
                        }
                        MouseEventKind::Up(MouseButton::Left) => {
                            if let Some(selected) = self.get_opt_by_mouse_pos(mouse_pos) {
                                self.selected = selected;
                            }
                            return_if_some!(self.switch(app_data));
                        }

                        // TODO: Test these
                        MouseEventKind::ScrollLeft => {
                            self.update_slider(false, app_data);
                        }
                        MouseEventKind::ScrollRight => {
                            self.update_slider(true, app_data);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        None
    }

    fn screen(&self) -> &dyn Screen {
        self
    }
}

impl Screen for Menu {
    fn draw(&self, frame: &mut Frame, theme: &Theme) -> Result<(), io::Error> {
        let MenuConfig { title, counted, .. } = &self.config;

        let AppliedStyles {
            title: title_style,
            subtitle: subtitle_style,
            text: text_style,
            border: border_style,
            separator: separator_style,
            selector: selector_style,
            number: number_style,
        } = self.config.styles.apply(theme);

        let MenuDimenstions {
            size,
            title_pos,
            items_pos,
            subtitles_pos,
            items_size: _,
            count_pos,
            item_text_pos,
            item_text_len,
        } = MenuDimenstions::calc(&self.config);

        let max_item_width = size.0 as usize - 2 - self.config.special_width();

        let options = self
            .config
            .options
            .iter()
            .map(|opt| opt.render(max_item_width))
            .collect::<Vec<_>>();

        let pos = center_box_in_screen(size);
        let opt_count = options.len();

        let max_count = opt_count.to_string().len();

        draw_box(frame, pos, size, border_style);

        frame.draw(title_pos, title.as_str(), title_style);

        for (i, subtitle) in self.config.subtitles.iter().enumerate() {
            frame.draw(
                subtitles_pos + Dims(0, i as i32),
                subtitle.as_str(),
                subtitle_style,
            );
        }

        frame.draw(
            items_pos - Dims(0, 1),
            LineDir::Horizontal
                .round()
                .to_string()
                .repeat(size.0 as usize - 2),
            separator_style,
        );

        for (i, option) in options.iter().enumerate() {
            let prep_style = |style: Style| {
                if i == self.selected {
                    style.invert()
                } else {
                    style
                }
            };

            // selector
            if i == self.selected {
                frame.draw(
                    items_pos + Dims(0, i as i32),
                    "> ",
                    prep_style(selector_style),
                );
            } else {
                frame.draw(
                    items_pos + Dims(0, i as i32),
                    "  ",
                    prep_style(selector_style),
                );
            }

            if *counted {
                frame.draw(
                    count_pos.unwrap() + Dims(0, i as i32),
                    format!("{:width$}. ", i + 1, width = max_count),
                    prep_style(number_style),
                );
            }

            frame.draw(
                item_text_pos + Dims(0, i as i32),
                option.as_ref().pad_to_width(item_text_len),
                prep_style(text_style),
            );
        }

        Ok(())
    }
}

struct MenuDimenstions {
    size: Dims,
    title_pos: Dims,
    items_pos: Dims,
    items_size: Dims,
    subtitles_pos: Dims,
    count_pos: Option<Dims>,
    item_text_pos: Dims,
    item_text_len: usize,
}

impl MenuDimenstions {
    fn calc(config: &MenuConfig) -> Self {
        let menu_size = {
            let special = config.special_width();

            let subtitles_width = config
                .subtitles
                .iter()
                .map(|s| s.width())
                .max()
                .unwrap_or(0);

            let items_width = config
                .map_options(move |opt| opt.width(special).unwrap_or(0))
                .max()
                .unwrap_or(0);

            let width = subtitles_width
                .max(items_width)
                .max(config.title.width() + 2) // title is offseted by 2
                + 2;

            let width = width + 2;
            let height = config.options.len() + 4 + config.subtitles.len();

            Dims(width as i32, height as i32)
        };

        let pos = center_box_in_screen(menu_size);

        let items_pos = Dims(pos.0 + 1, pos.1 + config.subtitles.len() as i32 + 3);

        let mut item_text_len = menu_size.0 as usize - 4;

        let count_pos = if config.counted {
            let max_count = config.options.len().to_string().len();
            item_text_len -= max_count + 2;
            Some(Dims(items_pos.0 + 2, items_pos.1))
        } else {
            None
        };

        let item_text_pos = count_pos.map_or(Dims(items_pos.0 + 2, items_pos.1), |pos| {
            let max_count = config.options.len().to_string().len();
            Dims(pos.0 + max_count as i32 + 2, pos.1)
        });

        Self {
            size: menu_size,
            title_pos: pos + Dims(3, 1),
            items_pos,
            items_size: Dims(menu_size.0 - 2, config.options.len() as i32),
            subtitles_pos: pos + Dims(2, 2),
            count_pos,
            item_text_pos,
            item_text_len,
        }
    }
}

pub type MenuAction<R> = Box<dyn Fn(&mut AppData) -> R>;

#[macro_export]
macro_rules! menu_actions {
    ($($name:literal $(on $feature:literal)? -> $data:pat => $action:expr),* $(,)?) => {
        {
            let opts: Vec<(_, $crate::ui::menu::MenuAction<_>)> = vec![
                $(
                    $(#[cfg(feature = $feature)])?
                    { ($crate::ui::menu::MenuItem::from($name), Box::new(|$data: &mut AppData| $action)) },
                )*
            ];

            opts
        }
    };
}

pub fn split_menu_actions<R>(
    actions: Vec<(MenuItem, MenuAction<R>)>,
) -> (Vec<MenuItem>, Vec<MenuAction<R>>) {
    actions.into_iter().unzip()
}

pub fn menu_theme_resolver() -> ThemeResolver {
    let mut resolver = ThemeResolver::new();

    resolver
        .link("ui_menu_border", "border")
        .link("ui_menu_text", "text")
        .link("ui_menu_title", "ui_menu_text")
        .link("ui_menu_subtitle", "ui_menu_text")
        .link("ui_menu_separator", "ui_menu_border")
        .link("ui_menu_selector", "ui_menu_text")
        .link("ui_menu_number", "ui_menu_text");

    resolver
}
