#![allow(non_camel_case_types)]

use std::{
    ffi::CString,
    io::{self, BufReader, Read, Write},
    mem::MaybeUninit,
    os::raw::{c_char, c_int, c_long, c_uint, c_ulong, c_void},
    os::unix::net::UnixStream,
    process::Command,
    ptr,
    time::{Duration, Instant},
};

use kubuntu_lldp_core::{
    decode_response, AgentRequest, AgentResponse, LinkState, RuntimeSnapshot, DEFAULT_SOCKET_PATH,
};

#[allow(non_camel_case_types)]
mod xlib {
    use super::{c_char, c_int, c_long, c_uint, c_ulong, c_void};

    pub enum Display {}
    pub type Window = c_ulong;
    pub type GC = *mut c_void;
    pub type KeySym = c_ulong;
    pub type Font = c_ulong;

    #[repr(C)]
    pub struct XFontStruct {
        pub ext_data: *mut c_void,
        pub fid: Font,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct XButtonEvent {
        pub type_: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
        pub root: Window,
        pub subwindow: Window,
        pub time: c_ulong,
        pub x: c_int,
        pub y: c_int,
        pub x_root: c_int,
        pub y_root: c_int,
        pub state: c_uint,
        pub button: c_uint,
        pub same_screen: c_int,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct XKeyEvent {
        pub type_: c_int,
        pub serial: c_ulong,
        pub send_event: c_int,
        pub display: *mut Display,
        pub window: Window,
        pub root: Window,
        pub subwindow: Window,
        pub time: c_ulong,
        pub x: c_int,
        pub y: c_int,
        pub x_root: c_int,
        pub y_root: c_int,
        pub state: c_uint,
        pub keycode: c_uint,
        pub same_screen: c_int,
    }

    #[repr(C)]
    pub union XEvent {
        pub type_: c_int,
        pub button: XButtonEvent,
        pub key: XKeyEvent,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct XColor {
        pub pixel: c_ulong,
        pub red: u16,
        pub green: u16,
        pub blue: u16,
        pub flags: c_char,
        pub pad: c_char,
    }

    pub const EXPOSE: c_int = 12;
    pub const KEY_PRESS: c_int = 2;
    pub const BUTTON_PRESS: c_int = 4;

    pub const KEY_PRESS_MASK: c_long = 1;
    pub const BUTTON_PRESS_MASK: c_long = 1 << 2;
    pub const EXPOSURE_MASK: c_long = 1 << 15;
    pub const STRUCTURE_NOTIFY_MASK: c_long = 1 << 17;

    #[link(name = "X11")]
    extern "C" {
        pub fn XOpenDisplay(name: *const c_char) -> *mut Display;
        pub fn XDefaultScreen(display: *mut Display) -> c_int;
        pub fn XRootWindow(display: *mut Display, screen_number: c_int) -> Window;
        pub fn XBlackPixel(display: *mut Display, screen_number: c_int) -> c_ulong;
        pub fn XWhitePixel(display: *mut Display, screen_number: c_int) -> c_ulong;
        pub fn XDefaultColormap(display: *mut Display, screen_number: c_int) -> c_ulong;
        pub fn XDisplayWidth(display: *mut Display, screen_number: c_int) -> c_int;
        pub fn XDisplayHeight(display: *mut Display, screen_number: c_int) -> c_int;
        pub fn XCreateSimpleWindow(
            display: *mut Display,
            parent: Window,
            x: c_int,
            y: c_int,
            width: u32,
            height: u32,
            border_width: u32,
            border: c_ulong,
            background: c_ulong,
        ) -> Window;
        pub fn XSelectInput(display: *mut Display, w: Window, event_mask: c_long) -> c_int;
        pub fn XMapRaised(display: *mut Display, w: Window) -> c_int;
        pub fn XStoreName(display: *mut Display, w: Window, window_name: *const c_char) -> c_int;
        pub fn XCreateGC(
            display: *mut Display,
            d: Window,
            valuemask: c_ulong,
            values: *mut c_void,
        ) -> GC;
        pub fn XLoadQueryFont(display: *mut Display, name: *const c_char) -> *mut XFontStruct;
        pub fn XFreeFont(display: *mut Display, font_struct: *mut XFontStruct) -> c_int;
        pub fn XSetFont(display: *mut Display, gc: GC, font: Font) -> c_int;
        pub fn XSetForeground(display: *mut Display, gc: GC, foreground: c_ulong) -> c_int;
        pub fn XSetBackground(display: *mut Display, gc: GC, background: c_ulong) -> c_int;
        pub fn XAllocColor(
            display: *mut Display,
            colormap: c_ulong,
            screen_in_out: *mut XColor,
        ) -> c_int;
        pub fn XClearWindow(display: *mut Display, w: Window) -> c_int;
        pub fn XFillRectangle(
            display: *mut Display,
            d: Window,
            gc: GC,
            x: c_int,
            y: c_int,
            width: u32,
            height: u32,
        ) -> c_int;
        pub fn XFillArc(
            display: *mut Display,
            d: Window,
            gc: GC,
            x: c_int,
            y: c_int,
            width: u32,
            height: u32,
            angle1: c_int,
            angle2: c_int,
        ) -> c_int;
        pub fn XDrawArc(
            display: *mut Display,
            d: Window,
            gc: GC,
            x: c_int,
            y: c_int,
            width: u32,
            height: u32,
            angle1: c_int,
            angle2: c_int,
        ) -> c_int;
        pub fn XDrawLine(
            display: *mut Display,
            d: Window,
            gc: GC,
            x1: c_int,
            y1: c_int,
            x2: c_int,
            y2: c_int,
        ) -> c_int;
        pub fn XDrawRectangle(
            display: *mut Display,
            d: Window,
            gc: GC,
            x: c_int,
            y: c_int,
            width: u32,
            height: u32,
        ) -> c_int;
        pub fn XDrawString(
            display: *mut Display,
            d: Window,
            gc: GC,
            x: c_int,
            y: c_int,
            string: *const c_char,
            length: c_int,
        ) -> c_int;
        pub fn XPending(display: *mut Display) -> c_int;
        pub fn XNextEvent(display: *mut Display, event_return: *mut XEvent) -> c_int;
        pub fn XLookupString(
            event_struct: *mut XKeyEvent,
            buffer_return: *mut c_char,
            bytes_buffer: c_int,
            keysym_return: *mut KeySym,
            status_in_out: *mut c_void,
        ) -> c_int;
        pub fn XFreeGC(display: *mut Display, gc: GC) -> c_int;
        pub fn XDestroyWindow(display: *mut Display, w: Window) -> c_int;
        pub fn XCloseDisplay(display: *mut Display) -> c_int;
        pub fn XFlush(display: *mut Display) -> c_int;
    }
}

const MARGIN: i32 = 40;
const GUTTER: i32 = 28;
const HEADER_HEIGHT: i32 = 118;
const LINE_HEIGHT: i32 = 32;
const TOUCH_ROW_HEIGHT: i32 = 88;
const TOUCH_ROW_GAP: i32 = 12;
const BUTTON_HEIGHT: i32 = 72;
const STATUS_Y: i32 = 146;
const STATUS_CARD_HEIGHT: i32 = 86;
const BUTTON_Y: i32 = 258;
const MAIN_Y: i32 = 362;
const PANEL_HEADER_HEIGHT: i32 = 54;
const ROWS_Y: i32 = MAIN_Y + PANEL_HEADER_HEIGHT + 16;
const RADIUS: i32 = 18;

#[derive(Clone)]
struct Palette {
    background: c_ulong,
    surface: c_ulong,
    surface_alt: c_ulong,
    border: c_ulong,
    text: c_ulong,
    secondary_text: c_ulong,
    accent: c_ulong,
    accent_soft: c_ulong,
    success: c_ulong,
    warning: c_ulong,
    danger: c_ulong,
}

struct UiState {
    snapshot: Option<RuntimeSnapshot>,
    error: Option<String>,
    last_refresh: Instant,
    selected_row: Option<usize>,
}

#[derive(Copy, Clone)]
struct WindowGeometry {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

fn main() -> io::Result<()> {
    let display = open_display()?;
    let app = run_ui(display);
    app
}

fn run_ui(display: *mut xlib::Display) -> io::Result<()> {
    unsafe {
        let screen = xlib::XDefaultScreen(display);
        let root = xlib::XRootWindow(display, screen);
        let fallback = WindowGeometry {
            x: 0,
            y: 0,
            width: xlib::XDisplayWidth(display, screen) as u32,
            height: xlib::XDisplayHeight(display, screen) as u32,
        };
        let geometry = preferred_window_geometry().unwrap_or(fallback);
        let black = xlib::XBlackPixel(display, screen);
        let palette = load_palette(display, screen);

        let window = xlib::XCreateSimpleWindow(
            display,
            root,
            geometry.x,
            geometry.y,
            geometry.width,
            geometry.height,
            0,
            black,
            palette.background,
        );
        xlib::XSelectInput(
            display,
            window,
            xlib::EXPOSURE_MASK
                | xlib::KEY_PRESS_MASK
                | xlib::BUTTON_PRESS_MASK
                | xlib::STRUCTURE_NOTIFY_MASK,
        );

        let title = CString::new("KubuntuLLDP").expect("window title");
        xlib::XStoreName(display, window, title.as_ptr());
        xlib::XMapRaised(display, window);

        let gc = xlib::XCreateGC(display, window, 0, ptr::null_mut());
        xlib::XSetForeground(display, gc, palette.text);
        xlib::XSetBackground(display, gc, palette.background);
        let font = load_large_font(display);
        if let Some(font) = font {
            xlib::XSetFont(display, gc, (*font).fid);
        }

        let mut ui = UiState {
            snapshot: None,
            error: None,
            last_refresh: Instant::now() - Duration::from_secs(5),
            selected_row: None,
        };

        refresh_state(&mut ui);
        draw(
            display,
            window,
            gc,
            geometry.width as i32,
            geometry.height as i32,
            &ui,
            &palette,
        )?;

        loop {
            while xlib::XPending(display) > 0 {
                let mut event = MaybeUninit::<xlib::XEvent>::uninit();
                xlib::XNextEvent(display, event.as_mut_ptr());
                let event = event.assume_init();
                match event.type_ {
                    xlib::EXPOSE => {
                        draw(
                            display,
                            window,
                            gc,
                            geometry.width as i32,
                            geometry.height as i32,
                            &ui,
                            &palette,
                        )?;
                    }
                    xlib::BUTTON_PRESS => {
                        let button = event.button;
                        if hit_retry(button.x, button.y, geometry.width as i32) {
                            let _ =
                                send_request(DEFAULT_SOCKET_PATH, AgentRequest::RetryProvisioning);
                            refresh_state(&mut ui);
                            draw(
                                display,
                                window,
                                gc,
                                geometry.width as i32,
                                geometry.height as i32,
                                &ui,
                                &palette,
                            )?;
                        } else if hit_quit(button.x, button.y, geometry.width as i32) {
                            if let Some(font) = font {
                                xlib::XFreeFont(display, font);
                            }
                            xlib::XFreeGC(display, gc);
                            xlib::XDestroyWindow(display, window);
                            xlib::XCloseDisplay(display);
                            return Ok(());
                        } else if let Some(row) =
                            hit_test(button.x, button.y, geometry.width as i32, &ui)
                        {
                            if let Some(snapshot) = &ui.snapshot {
                                if let Some(iface) = snapshot.interfaces.get(row) {
                                    let request = AgentRequest::SelectInterface {
                                        name: iface.name.clone(),
                                    };
                                    match send_request(DEFAULT_SOCKET_PATH, request) {
                                        Ok(AgentResponse::State(snapshot)) => {
                                            ui.snapshot = Some(snapshot);
                                            ui.error = None;
                                            ui.selected_row = selected_row(&ui.snapshot);
                                        }
                                        Ok(AgentResponse::Error { message }) => {
                                            ui.error = Some(message);
                                        }
                                        Err(err) => ui.error = Some(err.to_string()),
                                    }
                                    draw(
                                        display,
                                        window,
                                        gc,
                                        geometry.width as i32,
                                        geometry.height as i32,
                                        &ui,
                                        &palette,
                                    )?;
                                }
                            }
                        }
                    }
                    xlib::KEY_PRESS => {
                        let mut key_event = event.key;
                        let mut buffer = [0 as c_char; 8];
                        let mut keysym: xlib::KeySym = 0;
                        let len = xlib::XLookupString(
                            &mut key_event,
                            buffer.as_mut_ptr(),
                            buffer.len() as c_int,
                            &mut keysym,
                            ptr::null_mut(),
                        );
                        if len > 0 {
                            let key = buffer[0] as u8 as char;
                            if key == 'q' || key == '\u{1b}' {
                                if let Some(font) = font {
                                    xlib::XFreeFont(display, font);
                                }
                                xlib::XFreeGC(display, gc);
                                xlib::XDestroyWindow(display, window);
                                xlib::XCloseDisplay(display);
                                return Ok(());
                            }
                            if key == 'r' {
                                let _ = send_request(
                                    DEFAULT_SOCKET_PATH,
                                    AgentRequest::RetryProvisioning,
                                );
                                refresh_state(&mut ui);
                                draw(
                                    display,
                                    window,
                                    gc,
                                    geometry.width as i32,
                                    geometry.height as i32,
                                    &ui,
                                    &palette,
                                )?;
                            }
                        }
                    }
                    _ => {}
                }
            }

            if ui.last_refresh.elapsed() >= Duration::from_secs(1) {
                refresh_state(&mut ui);
                draw(
                    display,
                    window,
                    gc,
                    geometry.width as i32,
                    geometry.height as i32,
                    &ui,
                    &palette,
                )?;
            }

            std::thread::sleep(Duration::from_millis(50));
            xlib::XFlush(display);
        }
    }
}

fn refresh_state(ui: &mut UiState) {
    match send_request(DEFAULT_SOCKET_PATH, AgentRequest::ListState) {
        Ok(AgentResponse::State(snapshot)) => {
            ui.selected_row = selected_row_from_snapshot(&snapshot);
            ui.snapshot = Some(snapshot);
            ui.error = None;
        }
        Ok(AgentResponse::Error { message }) => {
            ui.error = Some(message);
        }
        Err(err) => ui.error = Some(err.to_string()),
    }
    ui.last_refresh = Instant::now();
}

fn selected_row(snapshot: &Option<RuntimeSnapshot>) -> Option<usize> {
    snapshot.as_ref().and_then(selected_row_from_snapshot)
}

fn selected_row_from_snapshot(snapshot: &RuntimeSnapshot) -> Option<usize> {
    snapshot
        .interfaces
        .iter()
        .position(|iface| iface.is_selected)
}

fn hit_test(x: c_int, y: c_int, width: i32, ui: &UiState) -> Option<usize> {
    let snapshot = ui.snapshot.as_ref()?;
    let left_width = interface_panel_width(width);
    if x < MARGIN || x > MARGIN + left_width {
        return None;
    }

    let mut row_top = ROWS_Y;
    for row in 0..snapshot.interfaces.len() {
        if y >= row_top && y < row_top + TOUCH_ROW_HEIGHT {
            return Some(row);
        }
        row_top += TOUCH_ROW_HEIGHT + TOUCH_ROW_GAP;
    }
    None
}

fn hit_retry(x: c_int, y: c_int, width: i32) -> bool {
    let (retry_x, _, button_width) = button_layout(width);
    x >= retry_x && x <= retry_x + button_width && y >= BUTTON_Y && y <= BUTTON_Y + BUTTON_HEIGHT
}

fn hit_quit(x: c_int, y: c_int, width: i32) -> bool {
    let (_, quit_x, button_width) = button_layout(width);
    x >= quit_x && x <= quit_x + button_width && y >= BUTTON_Y && y <= BUTTON_Y + BUTTON_HEIGHT
}

fn draw(
    display: *mut xlib::Display,
    window: xlib::Window,
    gc: xlib::GC,
    width: i32,
    height: i32,
    ui: &UiState,
    palette: &Palette,
) -> io::Result<()> {
    unsafe {
        xlib::XClearWindow(display, window);
        set_color(display, gc, palette.background);
        xlib::XFillRectangle(display, window, gc, 0, 0, width as u32, height as u32);
        draw_header(display, window, gc, width, palette);

        if let Some(error) = &ui.error {
            draw_status_card(
                display,
                window,
                gc,
                MARGIN,
                STATUS_Y,
                width - MARGIN * 2,
                "Agent error",
                error,
                palette.danger,
                palette,
            );
        }

        let Some(snapshot) = &ui.snapshot else {
            draw_button(
                display,
                window,
                gc,
                MARGIN,
                STATUS_Y,
                width - MARGIN * 2,
                STATUS_CARD_HEIGHT,
                "Waiting for agent",
                palette.surface,
                palette.text,
                palette,
            );
            return Ok(());
        };

        let selected = snapshot.selected_interface.as_deref().unwrap_or("none");
        draw_status_cards(display, window, gc, width, snapshot, selected, palette);

        let (retry_x, quit_x, button_width) = button_layout(width);
        draw_button(
            display,
            window,
            gc,
            retry_x,
            BUTTON_Y,
            button_width,
            BUTTON_HEIGHT,
            "Retry",
            palette.accent,
            palette.surface,
            palette,
        );
        draw_button(
            display,
            window,
            gc,
            quit_x,
            BUTTON_Y,
            button_width,
            BUTTON_HEIGHT,
            "Quit",
            palette.surface_alt,
            palette.text,
            palette,
        );

        let left_width = interface_panel_width(width);
        let right_x = MARGIN + left_width + GUTTER;
        let right_width = (width - right_x - MARGIN).max(260);
        let panel_bottom = height - MARGIN;
        let panel_height = (panel_bottom - MAIN_Y).max(280);
        draw_panel(
            display,
            window,
            gc,
            MARGIN,
            MAIN_Y,
            left_width,
            panel_height,
            "Interfaces",
            palette,
        );

        let mut y = ROWS_Y;
        for iface in &snapshot.interfaces {
            draw_interface_row(
                display,
                window,
                gc,
                MARGIN + 16,
                y,
                left_width - 32,
                iface,
                palette,
            );
            y += TOUCH_ROW_HEIGHT + TOUCH_ROW_GAP;
        }

        let right_panel_gap = GUTTER;
        let right_available_height = (panel_height - right_panel_gap).max(240);
        let preferred_dhcp_height = ((right_available_height * 28) / 100).max(160);
        let dhcp_panel_height = preferred_dhcp_height.min((right_available_height - 220).max(120));
        let right_panel_height = right_available_height - dhcp_panel_height;
        draw_panel(
            display,
            window,
            gc,
            right_x,
            MAIN_Y,
            right_width,
            right_panel_height,
            "LLDP / CDP Neighbor",
            palette,
        );
        draw_neighbors(
            display,
            window,
            gc,
            right_x + 18,
            MAIN_Y + PANEL_HEADER_HEIGHT + 30,
            right_width - 36,
            snapshot,
            palette,
        );

        let dhcp_y = MAIN_Y + right_panel_height + right_panel_gap;
        draw_panel(
            display,
            window,
            gc,
            right_x,
            dhcp_y,
            right_width,
            panel_bottom - dhcp_y,
            "DHCP Options",
            palette,
        );
        draw_dhcp_options(
            display,
            window,
            gc,
            right_x + 18,
            dhcp_y + PANEL_HEADER_HEIGHT + 34,
            right_width - 36,
            snapshot,
            palette,
        );
    }
    Ok(())
}

fn draw_header(
    display: *mut xlib::Display,
    window: xlib::Window,
    gc: xlib::GC,
    width: i32,
    palette: &Palette,
) {
    unsafe {
        set_color(display, gc, palette.surface);
        xlib::XFillRectangle(
            display,
            window,
            gc,
            0,
            0,
            width as u32,
            HEADER_HEIGHT as u32,
        );
        set_color(display, gc, palette.border);
        xlib::XFillRectangle(display, window, gc, 0, HEADER_HEIGHT - 1, width as u32, 1);
        set_color(display, gc, palette.text);
        draw_text(display, window, gc, MARGIN, 48, "KubuntuLLDP");
        set_color(display, gc, palette.secondary_text);
        draw_text(display, window, gc, MARGIN, 86, "Network link discovery");
    }
}

fn draw_status_cards(
    display: *mut xlib::Display,
    window: xlib::Window,
    gc: xlib::GC,
    width: i32,
    snapshot: &RuntimeSnapshot,
    selected: &str,
    palette: &Palette,
) {
    let card_gap = 18;
    let card_width = ((width - MARGIN * 2 - card_gap * 3) / 4).max(180);
    let mut x = MARGIN;
    draw_status_card(
        display,
        window,
        gc,
        x,
        STATUS_Y,
        card_width,
        "Selected",
        selected,
        palette.accent,
        palette,
    );
    x += card_width + card_gap;
    draw_status_card(
        display,
        window,
        gc,
        x,
        STATUS_Y,
        card_width,
        "Discovery",
        &snapshot.discovery_status,
        status_color(&snapshot.discovery_status, palette),
        palette,
    );
    x += card_width + card_gap;
    draw_status_card(
        display,
        window,
        gc,
        x,
        STATUS_Y,
        card_width,
        "DHCP",
        &snapshot.dhcp_status,
        status_color(&snapshot.dhcp_status, palette),
        palette,
    );
    x += card_width + card_gap;
    let last_error = snapshot.last_error.as_deref().unwrap_or("none");
    draw_status_card(
        display,
        window,
        gc,
        x,
        STATUS_Y,
        width - x - MARGIN,
        "Last error",
        last_error,
        if last_error == "none" {
            palette.success
        } else {
            palette.danger
        },
        palette,
    );
}

fn draw_neighbors(
    display: *mut xlib::Display,
    window: xlib::Window,
    gc: xlib::GC,
    x: i32,
    mut y: i32,
    width: i32,
    snapshot: &RuntimeSnapshot,
    palette: &Palette,
) {
    set_color(display, gc, palette.secondary_text);
    if snapshot.neighbors.is_empty() {
        draw_text(display, window, gc, x, y, "No neighbor discovered yet");
        return;
    }

    for neighbor in &snapshot.neighbors {
        draw_field(
            display,
            window,
            gc,
            x,
            y,
            width,
            "Protocol",
            protocol_name(&neighbor.protocol),
            palette,
        );
        y += LINE_HEIGHT;
        draw_field(
            display,
            window,
            gc,
            x,
            y,
            width,
            "Device name",
            neighbor.system_name.as_deref().unwrap_or("n/a"),
            palette,
        );
        y += LINE_HEIGHT;
        draw_field(
            display,
            window,
            gc,
            x,
            y,
            width,
            "Description",
            neighbor.system_description.as_deref().unwrap_or("n/a"),
            palette,
        );
        y += LINE_HEIGHT;
        draw_field(
            display,
            window,
            gc,
            x,
            y,
            width,
            "Chassis ID",
            neighbor.chassis_id.as_deref().unwrap_or("n/a"),
            palette,
        );
        y += LINE_HEIGHT;
        draw_field(
            display,
            window,
            gc,
            x,
            y,
            width,
            "Remote port",
            neighbor.port_id.as_deref().unwrap_or("n/a"),
            palette,
        );
        y += LINE_HEIGHT;
        draw_field(
            display,
            window,
            gc,
            x,
            y,
            width,
            "Port description",
            neighbor.port_description.as_deref().unwrap_or("n/a"),
            palette,
        );
        y += LINE_HEIGHT;
        draw_field(
            display,
            window,
            gc,
            x,
            y,
            width,
            "Management IP",
            &display_list(&neighbor.management_addresses),
            palette,
        );
        y += LINE_HEIGHT;
        draw_field(
            display,
            window,
            gc,
            x,
            y,
            width,
            "Capabilities",
            &display_list(&neighbor.capabilities),
            palette,
        );
        y += LINE_HEIGHT;
        draw_field(
            display,
            window,
            gc,
            x,
            y,
            width,
            "TTL",
            &neighbor
                .ttl_seconds
                .map(|ttl| format!("{ttl} seconds"))
                .unwrap_or_else(|| "n/a".to_string()),
            palette,
        );
        y += LINE_HEIGHT;
        draw_field(
            display,
            window,
            gc,
            x,
            y,
            width,
            "Native VLAN",
            &neighbor
                .native_vlan
                .map(|vlan| vlan.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            palette,
        );
        y += LINE_HEIGHT;
        draw_field(
            display,
            window,
            gc,
            x,
            y,
            width,
            "Duplex",
            neighbor.duplex.as_deref().unwrap_or("n/a"),
            palette,
        );
        y += LINE_HEIGHT + 18;
    }
}

fn draw_dhcp_options(
    display: *mut xlib::Display,
    window: xlib::Window,
    gc: xlib::GC,
    x: i32,
    mut y: i32,
    width: i32,
    snapshot: &RuntimeSnapshot,
    palette: &Palette,
) {
    set_color(display, gc, palette.secondary_text);
    if snapshot.dhcp_options.is_empty() {
        draw_text(display, window, gc, x, y, "No DHCP options reported yet");
        return;
    }

    for option in &snapshot.dhcp_options {
        let code = option
            .code
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        draw_text(
            display,
            window,
            gc,
            x,
            y,
            &fit_text(
                &format!("option {code}  {} = {}", option.name, option.value),
                width,
            ),
        );
        y += LINE_HEIGHT;
    }
}

fn draw_panel(
    display: *mut xlib::Display,
    window: xlib::Window,
    gc: xlib::GC,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    title: &str,
    palette: &Palette,
) {
    unsafe {
        fill_rounded_rect(
            display,
            window,
            gc,
            x,
            y,
            width,
            height,
            RADIUS,
            palette.surface,
        );
        set_color(display, gc, palette.border);
        draw_rounded_rect(display, window, gc, x, y, width, height, RADIUS);
        set_color(display, gc, palette.text);
        draw_text(display, window, gc, x + 22, y + 36, title);
        set_color(display, gc, palette.border);
        xlib::XFillRectangle(
            display,
            window,
            gc,
            x + 18,
            y + PANEL_HEADER_HEIGHT,
            (width - 36) as u32,
            1,
        );
    }
}

fn draw_status_card(
    display: *mut xlib::Display,
    window: xlib::Window,
    gc: xlib::GC,
    x: i32,
    y: i32,
    width: i32,
    label: &str,
    value: &str,
    indicator: c_ulong,
    palette: &Palette,
) {
    unsafe {
        fill_rounded_rect(
            display,
            window,
            gc,
            x,
            y,
            width,
            STATUS_CARD_HEIGHT,
            RADIUS,
            palette.surface,
        );
        set_color(display, gc, palette.border);
        draw_rounded_rect(display, window, gc, x, y, width, STATUS_CARD_HEIGHT, RADIUS);
        set_color(display, gc, indicator);
        xlib::XFillArc(display, window, gc, x + 18, y + 18, 12, 12, 0, 360 * 64);
        set_color(display, gc, palette.secondary_text);
        draw_text(display, window, gc, x + 38, y + 30, label);
        set_color(display, gc, palette.text);
        draw_text(
            display,
            window,
            gc,
            x + 18,
            y + 64,
            &fit_text(value, width - 36),
        );
    }
}

fn draw_interface_row(
    display: *mut xlib::Display,
    window: xlib::Window,
    gc: xlib::GC,
    x: i32,
    y: i32,
    width: i32,
    iface: &kubuntu_lldp_core::InterfaceSnapshot,
    palette: &Palette,
) {
    unsafe {
        if iface.is_selected {
            fill_rounded_rect(
                display,
                window,
                gc,
                x,
                y,
                width,
                TOUCH_ROW_HEIGHT,
                14,
                palette.accent_soft,
            );
            set_color(display, gc, palette.accent);
            xlib::XFillRectangle(display, window, gc, x, y + 16, 4, 56);
            set_color(display, gc, palette.text);
        } else {
            fill_rounded_rect(
                display,
                window,
                gc,
                x,
                y,
                width,
                TOUCH_ROW_HEIGHT,
                14,
                palette.surface_alt,
            );
            set_color(display, gc, palette.border);
            draw_rounded_rect(display, window, gc, x, y, width, TOUCH_ROW_HEIGHT, 14);
            set_color(display, gc, palette.text);
        }

        let title = format!("{}  {}", iface.name, format_state(&iface.state));
        let detail = format!(
            "MAC {}   IP {}",
            iface.mac_address.as_deref().unwrap_or("n/a"),
            iface.ip_address.as_deref().unwrap_or("n/a"),
        );
        draw_text(
            display,
            window,
            gc,
            x + 18,
            y + 34,
            &fit_text(&title, width - 36),
        );
        if !iface.is_selected {
            set_color(display, gc, palette.secondary_text);
        }
        draw_text(
            display,
            window,
            gc,
            x + 18,
            y + 66,
            &fit_text(&detail, width - 36),
        );
    }
}

fn interface_panel_width(width: i32) -> i32 {
    let available = width - MARGIN * 2 - GUTTER;
    ((available * 45) / 100).max(420).min(available - 320)
}

fn button_layout(width: i32) -> (i32, i32, i32) {
    let button_width = ((width - MARGIN * 2 - GUTTER) / 2).max(220);
    let retry_x = MARGIN;
    let quit_x = MARGIN + button_width + GUTTER;
    (retry_x, quit_x, button_width)
}

fn fit_text(text: &str, width: i32) -> String {
    let max_chars = (width / 12).max(8) as usize;
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let keep = max_chars.saturating_sub(3);
    let mut value: String = text.chars().take(keep).collect();
    value.push_str("...");
    value
}

fn draw_field(
    display: *mut xlib::Display,
    window: xlib::Window,
    gc: xlib::GC,
    x: i32,
    y: i32,
    width: i32,
    label: &str,
    value: &str,
    palette: &Palette,
) {
    let label_width = 190;
    set_color(display, gc, palette.secondary_text);
    draw_text(display, window, gc, x, y, label);
    set_color(display, gc, palette.text);
    draw_text(
        display,
        window,
        gc,
        x + label_width,
        y,
        &fit_text(value, width - label_width),
    );
}

fn display_list(values: &[String]) -> String {
    if values.is_empty() {
        "n/a".to_string()
    } else {
        values.join(", ")
    }
}

fn protocol_name(protocol: &kubuntu_lldp_core::DiscoveryProtocol) -> &'static str {
    match protocol {
        kubuntu_lldp_core::DiscoveryProtocol::Cdp => "CDP",
        kubuntu_lldp_core::DiscoveryProtocol::Lldp => "LLDP",
    }
}

fn draw_button(
    display: *mut xlib::Display,
    window: xlib::Window,
    gc: xlib::GC,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    label: &str,
    fill: c_ulong,
    text: c_ulong,
    palette: &Palette,
) {
    fill_rounded_rect(display, window, gc, x, y, width, height, RADIUS, fill);
    if fill != palette.accent {
        set_color(display, gc, palette.border);
        draw_rounded_rect(display, window, gc, x, y, width, height, RADIUS);
    }
    set_color(display, gc, text);
    draw_text(display, window, gc, x + 24, y + 40, label);
}

fn set_color(display: *mut xlib::Display, gc: xlib::GC, color: c_ulong) {
    unsafe {
        xlib::XSetForeground(display, gc, color);
    }
}

fn load_palette(display: *mut xlib::Display, screen: c_int) -> Palette {
    let fallback_black = unsafe { xlib::XBlackPixel(display, screen) };
    let fallback_white = unsafe { xlib::XWhitePixel(display, screen) };

    Palette {
        background: alloc_color(display, screen, 0xf5, 0xf5, 0xf7).unwrap_or(fallback_white),
        surface: alloc_color(display, screen, 0xff, 0xff, 0xff).unwrap_or(fallback_white),
        surface_alt: alloc_color(display, screen, 0xeb, 0xeb, 0xf0).unwrap_or(fallback_white),
        border: alloc_color(display, screen, 0xd2, 0xd2, 0xd7).unwrap_or(fallback_black),
        text: alloc_color(display, screen, 0x1d, 0x1d, 0x1f).unwrap_or(fallback_black),
        secondary_text: alloc_color(display, screen, 0x6e, 0x6e, 0x73).unwrap_or(fallback_black),
        accent: alloc_color(display, screen, 0x00, 0x7a, 0xff).unwrap_or(fallback_black),
        accent_soft: alloc_color(display, screen, 0xd9, 0xec, 0xff).unwrap_or(fallback_white),
        success: alloc_color(display, screen, 0x34, 0xc7, 0x59).unwrap_or(fallback_black),
        warning: alloc_color(display, screen, 0xff, 0x95, 0x00).unwrap_or(fallback_black),
        danger: alloc_color(display, screen, 0xff, 0x3b, 0x30).unwrap_or(fallback_black),
    }
}

fn alloc_color(
    display: *mut xlib::Display,
    screen: c_int,
    red: u8,
    green: u8,
    blue: u8,
) -> Option<c_ulong> {
    let colormap = unsafe { xlib::XDefaultColormap(display, screen) };
    let mut color = xlib::XColor {
        pixel: 0,
        red: u16::from(red) * 257,
        green: u16::from(green) * 257,
        blue: u16::from(blue) * 257,
        flags: 0,
        pad: 0,
    };

    let result = unsafe { xlib::XAllocColor(display, colormap, &mut color) };
    (result != 0).then_some(color.pixel)
}

fn status_color(status: &str, palette: &Palette) -> c_ulong {
    let lower = status.to_ascii_lowercase();
    if lower.contains("error") || lower.contains("fail") {
        palette.danger
    } else if lower.contains("complete")
        || lower.contains("detected")
        || lower.contains("up")
        || lower.contains("neighbor")
    {
        palette.success
    } else {
        palette.warning
    }
}

fn fill_rounded_rect(
    display: *mut xlib::Display,
    window: xlib::Window,
    gc: xlib::GC,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    radius: i32,
    color: c_ulong,
) {
    let radius = radius.min(width / 2).min(height / 2).max(0);
    let diameter = radius * 2;

    unsafe {
        set_color(display, gc, color);
        xlib::XFillRectangle(
            display,
            window,
            gc,
            x + radius,
            y,
            (width - diameter).max(0) as u32,
            height as u32,
        );
        xlib::XFillRectangle(
            display,
            window,
            gc,
            x,
            y + radius,
            width as u32,
            (height - diameter).max(0) as u32,
        );

        if radius > 0 {
            xlib::XFillArc(
                display,
                window,
                gc,
                x,
                y,
                diameter as u32,
                diameter as u32,
                90 * 64,
                90 * 64,
            );
            xlib::XFillArc(
                display,
                window,
                gc,
                x + width - diameter,
                y,
                diameter as u32,
                diameter as u32,
                0,
                90 * 64,
            );
            xlib::XFillArc(
                display,
                window,
                gc,
                x,
                y + height - diameter,
                diameter as u32,
                diameter as u32,
                180 * 64,
                90 * 64,
            );
            xlib::XFillArc(
                display,
                window,
                gc,
                x + width - diameter,
                y + height - diameter,
                diameter as u32,
                diameter as u32,
                270 * 64,
                90 * 64,
            );
        }
    }
}

fn draw_rounded_rect(
    display: *mut xlib::Display,
    window: xlib::Window,
    gc: xlib::GC,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    radius: i32,
) {
    let radius = radius.min(width / 2).min(height / 2).max(0);
    let diameter = radius * 2;

    unsafe {
        if radius == 0 {
            xlib::XDrawRectangle(display, window, gc, x, y, width as u32, height as u32);
            return;
        }

        xlib::XDrawLine(display, window, gc, x + radius, y, x + width - radius, y);
        xlib::XDrawLine(
            display,
            window,
            gc,
            x + radius,
            y + height,
            x + width - radius,
            y + height,
        );
        xlib::XDrawLine(display, window, gc, x, y + radius, x, y + height - radius);
        xlib::XDrawLine(
            display,
            window,
            gc,
            x + width,
            y + radius,
            x + width,
            y + height - radius,
        );

        xlib::XDrawArc(
            display,
            window,
            gc,
            x,
            y,
            diameter as u32,
            diameter as u32,
            90 * 64,
            90 * 64,
        );
        xlib::XDrawArc(
            display,
            window,
            gc,
            x + width - diameter,
            y,
            diameter as u32,
            diameter as u32,
            0,
            90 * 64,
        );
        xlib::XDrawArc(
            display,
            window,
            gc,
            x,
            y + height - diameter,
            diameter as u32,
            diameter as u32,
            180 * 64,
            90 * 64,
        );
        xlib::XDrawArc(
            display,
            window,
            gc,
            x + width - diameter,
            y + height - diameter,
            diameter as u32,
            diameter as u32,
            270 * 64,
            90 * 64,
        );
    }
}

fn draw_text(
    display: *mut xlib::Display,
    window: xlib::Window,
    gc: xlib::GC,
    x: i32,
    y: i32,
    text: &str,
) {
    let text = CString::new(text).expect("text");
    unsafe {
        xlib::XDrawString(
            display,
            window,
            gc,
            x,
            y,
            text.as_ptr(),
            text.as_bytes().len() as c_int,
        );
    }
}

fn preferred_window_geometry() -> Option<WindowGeometry> {
    let output = Command::new("xrandr").arg("--query").output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let mut connected = Vec::new();

    for line in stdout.lines() {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else {
            continue;
        };
        let Some(state) = parts.next() else {
            continue;
        };
        if state != "connected" {
            continue;
        }

        for token in parts {
            if let Some(geometry) = parse_geometry_token(token) {
                connected.push((name.to_string(), geometry));
                break;
            }
        }
    }

    connected
        .iter()
        .find(|(name, _)| name.starts_with("eDP") || name.starts_with("LVDS"))
        .map(|(_, geometry)| *geometry)
        .or_else(|| {
            connected
                .into_iter()
                .min_by_key(|(_, geometry)| geometry.x)
                .map(|(_, g)| g)
        })
}

fn parse_geometry_token(token: &str) -> Option<WindowGeometry> {
    let (size, position) = token.split_once('+')?;
    let (width, height) = size.split_once('x')?;
    let (x, y) = position.split_once('+')?;

    Some(WindowGeometry {
        x: x.parse().ok()?,
        y: y.parse().ok()?,
        width: width.parse().ok()?,
        height: height.parse().ok()?,
    })
}

fn load_large_font(display: *mut xlib::Display) -> Option<*mut xlib::XFontStruct> {
    let candidates = [
        "-urw-nimbus sans l-regular-r-normal--24-0-0-0-p-0-iso8859-1",
        "-urw-nimbus sans l-bold-r-normal--24-0-0-0-p-0-iso8859-1",
        "-adobe-helvetica-medium-r-normal--24-0-0-0-p-0-iso8859-1",
        "-adobe-helvetica-bold-r-normal--24-0-0-0-p-0-iso8859-1",
        "-schumacher-clean-medium-r-normal--16-160-75-75-c-80-iso646.1991-irv",
        "-schumacher-clean-medium-r-normal--14-140-75-75-c-80-iso646.1991-irv",
        "12x24",
        "10x20",
        "9x15bold",
        "-misc-fixed-bold-r-normal--20-200-75-75-c-100-iso10646-1",
        "fixed",
    ];

    for candidate in candidates {
        let name = CString::new(candidate).expect("font name");
        let font = unsafe { xlib::XLoadQueryFont(display, name.as_ptr()) };
        if !font.is_null() {
            return Some(font);
        }
    }
    None
}

fn format_state(state: &LinkState) -> &'static str {
    match state {
        LinkState::Down => "down",
        LinkState::Up => "up",
        LinkState::Unknown => "unknown",
    }
}

fn send_request(socket_path: &str, request: AgentRequest) -> io::Result<AgentResponse> {
    let mut stream = UnixStream::connect(socket_path)?;
    let mut request_text = kubuntu_lldp_core::encode_request(&request);
    request_text.push('\n');
    stream.write_all(request_text.as_bytes())?;
    stream.shutdown(std::net::Shutdown::Write)?;

    let mut response = String::new();
    BufReader::new(stream).read_to_string(&mut response)?;
    decode_response(&response).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

fn open_display() -> io::Result<*mut xlib::Display> {
    unsafe {
        let display = xlib::XOpenDisplay(ptr::null());
        if display.is_null() {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "unable to open X display",
            ))
        } else {
            Ok(display)
        }
    }
}
