#![allow(non_camel_case_types)]

use std::{
    ffi::CString,
    io::{self, BufReader, Read, Write},
    mem::MaybeUninit,
    os::raw::{c_char, c_int, c_long, c_uint, c_ulong, c_void},
    os::unix::net::UnixStream,
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

const MARGIN: i32 = 48;
const LINE_HEIGHT: i32 = 32;
const TOUCH_ROW_HEIGHT: i32 = 72;
const TOUCH_ROW_GAP: i32 = 8;
const BUTTON_HEIGHT: i32 = 72;
const BUTTON_Y: i32 = 250;
const INTERFACE_START_Y: i32 = 366;

struct UiState {
    snapshot: Option<RuntimeSnapshot>,
    error: Option<String>,
    last_refresh: Instant,
    selected_row: Option<usize>,
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
        let width = xlib::XDisplayWidth(display, screen) as u32;
        let height = xlib::XDisplayHeight(display, screen) as u32;
        let black = xlib::XBlackPixel(display, screen);
        let white = xlib::XWhitePixel(display, screen);

        let window = xlib::XCreateSimpleWindow(display, root, 0, 0, width, height, 0, black, black);
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
        xlib::XSetForeground(display, gc, white);
        xlib::XSetBackground(display, gc, black);
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
        draw(display, window, gc, width as i32, height as i32, &ui)?;

        loop {
            while xlib::XPending(display) > 0 {
                let mut event = MaybeUninit::<xlib::XEvent>::uninit();
                xlib::XNextEvent(display, event.as_mut_ptr());
                let event = event.assume_init();
                match event.type_ {
                    xlib::EXPOSE => {
                        draw(display, window, gc, width as i32, height as i32, &ui)?;
                    }
                    xlib::BUTTON_PRESS => {
                        let button = event.button;
                        if hit_retry(button.x, button.y, width as i32) {
                            let _ =
                                send_request(DEFAULT_SOCKET_PATH, AgentRequest::RetryProvisioning);
                            refresh_state(&mut ui);
                            draw(display, window, gc, width as i32, height as i32, &ui)?;
                        } else if hit_quit(button.x, button.y, width as i32) {
                            if let Some(font) = font {
                                xlib::XFreeFont(display, font);
                            }
                            xlib::XFreeGC(display, gc);
                            xlib::XDestroyWindow(display, window);
                            xlib::XCloseDisplay(display);
                            return Ok(());
                        } else if let Some(row) = hit_test(button.x, button.y, &ui) {
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
                                    draw(display, window, gc, width as i32, height as i32, &ui)?;
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
                                draw(display, window, gc, width as i32, height as i32, &ui)?;
                            }
                        }
                    }
                    _ => {}
                }
            }

            if ui.last_refresh.elapsed() >= Duration::from_secs(1) {
                refresh_state(&mut ui);
                draw(display, window, gc, width as i32, height as i32, &ui)?;
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

fn hit_test(_x: c_int, y: c_int, ui: &UiState) -> Option<usize> {
    let snapshot = ui.snapshot.as_ref()?;
    let mut row_top = INTERFACE_START_Y + 18;
    for row in 0..snapshot.interfaces.len() {
        if y >= row_top && y < row_top + TOUCH_ROW_HEIGHT {
            return Some(row);
        }
        row_top += TOUCH_ROW_HEIGHT + TOUCH_ROW_GAP;
    }
    None
}

fn hit_retry(x: c_int, y: c_int, width: i32) -> bool {
    let button_width = ((width - MARGIN * 2 - 24) / 2).max(160);
    x >= MARGIN && x <= MARGIN + button_width && y >= BUTTON_Y && y <= BUTTON_Y + BUTTON_HEIGHT
}

fn hit_quit(x: c_int, y: c_int, width: i32) -> bool {
    let button_width = ((width - MARGIN * 2 - 24) / 2).max(160);
    let x0 = MARGIN + button_width + 24;
    x >= x0 && x <= x0 + button_width && y >= BUTTON_Y && y <= BUTTON_Y + BUTTON_HEIGHT
}

fn draw(
    display: *mut xlib::Display,
    window: xlib::Window,
    gc: xlib::GC,
    width: i32,
    _height: i32,
    ui: &UiState,
) -> io::Result<()> {
    unsafe {
        xlib::XClearWindow(display, window);
        set_white(display, gc);
        draw_text(display, window, gc, MARGIN, 56, "KubuntuLLDP");
        draw_text(
            display,
            window,
            gc,
            MARGIN,
            94,
            "Interface picker and live state",
        );

        if let Some(error) = &ui.error {
            draw_text(display, window, gc, MARGIN, 134, &format!("Error: {error}"));
        }

        let Some(snapshot) = &ui.snapshot else {
            draw_button(
                display,
                window,
                gc,
                MARGIN,
                156,
                width - MARGIN * 2,
                BUTTON_HEIGHT,
                "Waiting for agent",
            );
            return Ok(());
        };

        let selected = snapshot.selected_interface.as_deref().unwrap_or("none");
        draw_text(
            display,
            window,
            gc,
            MARGIN,
            134,
            &format!("Selected interface: {selected}"),
        );
        draw_text(
            display,
            window,
            gc,
            MARGIN,
            166,
            &format!("Discovery: {}", snapshot.discovery_status),
        );
        draw_text(
            display,
            window,
            gc,
            MARGIN,
            198,
            &format!("DHCP: {}", snapshot.dhcp_status),
        );
        draw_text(
            display,
            window,
            gc,
            MARGIN,
            230,
            &format!(
                "Last error: {}",
                snapshot.last_error.as_deref().unwrap_or("none")
            ),
        );

        let button_width = ((width - MARGIN * 2 - 24) / 2).max(160);
        draw_button(
            display,
            window,
            gc,
            MARGIN,
            BUTTON_Y,
            button_width,
            BUTTON_HEIGHT,
            "Retry",
        );
        draw_button(
            display,
            window,
            gc,
            MARGIN + button_width + 24,
            BUTTON_Y,
            button_width,
            BUTTON_HEIGHT,
            "Quit",
        );

        let mut y = INTERFACE_START_Y;
        draw_text(display, window, gc, MARGIN, y, "Interfaces");
        y += 18;
        for iface in &snapshot.interfaces {
            let marker = if iface.is_selected { "*" } else { " " };
            let line = format!(
                "{marker} {:<16} {:<7} mac={} ip={}",
                iface.name,
                format_state(&iface.state),
                iface.mac_address.as_deref().unwrap_or("n/a"),
                iface.ip_address.as_deref().unwrap_or("n/a"),
            );

            draw_touch_row(
                display,
                window,
                gc,
                MARGIN,
                y,
                width - MARGIN * 2,
                TOUCH_ROW_HEIGHT,
                iface.is_selected,
            );
            draw_text(display, window, gc, MARGIN + 24, y + 44, &line);
            set_white(display, gc);
            y += TOUCH_ROW_HEIGHT + TOUCH_ROW_GAP;
        }

        y += 20;
        draw_text(display, window, gc, MARGIN, y, "Neighbors");
        y += LINE_HEIGHT;
        if snapshot.neighbors.is_empty() {
            draw_text(display, window, gc, MARGIN, y, "none yet");
            y += LINE_HEIGHT;
        } else {
            for neighbor in &snapshot.neighbors {
                let line = format!(
                    "{:?} chassis={} port={} system={}",
                    neighbor.protocol,
                    neighbor.chassis_id.as_deref().unwrap_or("n/a"),
                    neighbor.port_id.as_deref().unwrap_or("n/a"),
                    neighbor.system_name.as_deref().unwrap_or("n/a")
                );
                draw_text(display, window, gc, MARGIN, y, &line);
                y += LINE_HEIGHT;
            }
        }

        y += 20;
        draw_text(display, window, gc, MARGIN, y, "DHCP options");
        y += LINE_HEIGHT;
        if snapshot.dhcp_options.is_empty() {
            draw_text(display, window, gc, MARGIN, y, "none yet");
        } else {
            for option in &snapshot.dhcp_options {
                let code = option
                    .code
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string());
                draw_text(
                    display,
                    window,
                    gc,
                    MARGIN,
                    y,
                    &format!("option {code} {} = {}", option.name, option.value),
                );
                y += LINE_HEIGHT;
            }
        }
    }
    Ok(())
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
) {
    unsafe {
        xlib::XFillRectangle(display, window, gc, x, y, width as u32, height as u32);
        set_black(display, gc);
        draw_text(display, window, gc, x + 24, y + 40, label);
        set_white(display, gc);
    }
}

fn draw_touch_row(
    display: *mut xlib::Display,
    window: xlib::Window,
    gc: xlib::GC,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    selected: bool,
) {
    if !selected {
        return;
    }
    unsafe {
        xlib::XFillRectangle(display, window, gc, x, y, width as u32, height as u32);
        set_black(display, gc);
    }
}

fn set_black(display: *mut xlib::Display, gc: xlib::GC) {
    unsafe {
        xlib::XSetForeground(
            display,
            gc,
            xlib::XBlackPixel(display, xlib::XDefaultScreen(display)),
        );
    }
}

fn set_white(display: *mut xlib::Display, gc: xlib::GC) {
    unsafe {
        xlib::XSetForeground(
            display,
            gc,
            xlib::XWhitePixel(display, xlib::XDefaultScreen(display)),
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

fn load_large_font(display: *mut xlib::Display) -> Option<*mut xlib::XFontStruct> {
    let candidates = [
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
