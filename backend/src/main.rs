use bytes::Buf;
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::{HashMap, VecDeque},
    io::Cursor,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use tokio::{net::TcpListener, signal, sync::mpsc, time::interval};
use tokio_tungstenite::{accept_async, tungstenite::protocol::Message};
use tungstenite::Bytes;

type Tx = mpsc::UnboundedSender<Message>;
type Clients = Arc<RwLock<HashMap<usize, Tx>>>;
type SharedCanvas = Arc<RwLock<Canvas>>;
type RequestTracker = Arc<RwLock<VecDeque<(Instant, u32)>>>;

const ADDRESS: &str = "127.0.0.1:2325";

const CANVAS_SAVE_FILE: &str = "canvas.bin";
const CANVAS_WIDTH: usize = 1024;
const CANVAS_HEIGHT: usize = 1024;
const STARTING_COLOR: u32 = 0xFFFFFF; // white

const STATS_INTERVAL: Duration = Duration::from_secs(3); // interval for stats broadcast
const SAVE_INTERVAL: Duration = Duration::from_secs(30); // interval between saves
const STATS_WINDOW: f32 = 10.0; // rolling window time for stats

const STATS_WINDOW_INTERVAL: Duration = Duration::from_secs(STATS_WINDOW as u64);

struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<u32>, // RGB is stored on the lower 24 bits
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Canvas {
            width,
            height,
            pixels: vec![STARTING_COLOR; width * height],
        }
    }

    fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            let index = y * self.width + x;
            self.pixels[index] = color & 0x00FFFFFF;
        }
    }

    fn get_pixel(&self, x: usize, y: usize) -> Option<u32> {
        if x < self.width && y < self.height {
            Some(self.pixels[y * self.width + x])
        } else {
            None
        }
    }
}

enum MessageType {
    SetPixel = 1,
    GetPixel = 2,
    GetAllPixels = 3,
    GetStats = 4,
}

impl TryFrom<u8> for MessageType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(MessageType::SetPixel),
            2 => Ok(MessageType::GetPixel),
            3 => Ok(MessageType::GetAllPixels),
            4 => Ok(MessageType::GetStats),
            _ => Err(()),
        }
    }
}

enum ResponseType {
    Broadcast {
        x: u16,
        y: u16,
        color: u32,
        skip_sender: bool,
    },
    GetAllPixels(Vec<u32>),
    PixelColor(u32),
    Stats {
        client_count: u32,
        requests_per_second: f32,
    },
    Error(ErrorCode),
}

impl ResponseType {
    const PIXEL_COLOR: u8 = 10;
    const ERROR: u8 = 11;
    const GET_ALL_PIXELS: u8 = 12;
    const STATS: u8 = 13;
}

enum ErrorCode {
    InvalidMessageType = 1,
    OutOfBounds = 2,
}

fn save_canvas(canvas: &SharedCanvas) {
    let canvas_data = canvas.read().unwrap().pixels.clone();
    let packed_data: Vec<u8> = canvas_data
        .iter()
        .flat_map(|&pixel| pack_rgb(pixel))
        .collect();
    std::fs::write(CANVAS_SAVE_FILE, packed_data).expect("Unable to save canvas");
}

fn load_canvas(canvas: &SharedCanvas) {
    if let Ok(data) = std::fs::read(CANVAS_SAVE_FILE) {
        let mut pixels = vec![STARTING_COLOR; CANVAS_WIDTH * CANVAS_HEIGHT];
        for (i, chunk) in data.chunks(3).enumerate() {
            if chunk.len() == 3 {
                pixels[i] = unpack_rgb(chunk);
            }
        }
        canvas.write().unwrap().pixels = pixels;
        println!("Canvas loaded from file.");
    } else {
        println!("Canvas file not found, initializing with default color.");
        save_canvas(canvas);
    }
}

fn pack_coordinates(x: u16, y: u16) -> [u8; 3] {
    // Pack two 10-bit coordinates into 20 bits (3 bytes)
    // Layout: [x:10][y:10] = 20 bits
    let packed = ((x as u32) << 10) | (y as u32);
    [(packed >> 16) as u8, (packed >> 8) as u8, packed as u8]
}

fn unpack_coordinates(bytes: &[u8]) -> (u16, u16, bool) {
    // Unpack two 10-bit coordinates and a boolean flag into 21 bits (3 bytes)
    // Layout: [flag:1][x:10][y:10] = 21 bits
    let packed = ((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | (bytes[2] as u32);
    let flag = (packed & (1 << 20)) != 0;
    let x = ((packed >> 10) & 0x3FF) as u16;
    let y = (packed & 0x3FF) as u16;
    (x, y, flag)
}

fn pack_rgb(color: u32) -> [u8; 3] {
    [
        (color >> 16) as u8, // Red
        (color >> 8) as u8,  // Green
        color as u8,         // Blue
    ]
}

fn unpack_rgb(bytes: &[u8]) -> u32 {
    ((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | (bytes[2] as u32)
}

fn create_set_pixel_broadcast(x: u16, y: u16, color: u32) -> Vec<u8> {
    // Message format: [type:1][coords:3][rgb:3] = 7 bytes total
    let mut message = Vec::with_capacity(7);
    message.push(MessageType::SetPixel as u8);
    message.extend_from_slice(&pack_coordinates(x, y));
    message.extend_from_slice(&pack_rgb(color));
    message
}

fn create_pixel_response(color: u32) -> Vec<u8> {
    // Response format: [type:1][rgb:3] = 4 bytes total
    let mut message = Vec::with_capacity(4);
    message.push(ResponseType::PIXEL_COLOR);
    message.extend_from_slice(&pack_rgb(color));
    message
}

fn create_error_response(error: ErrorCode) -> Vec<u8> {
    vec![ResponseType::ERROR, error as u8]
}

fn create_stats_response(client_count: u32, requests_per_second: f32) -> Vec<u8> {
    // Response format: [type:1][client_count:4][rps:4] = 9 bytes total
    let mut message = Vec::with_capacity(9);
    message.push(ResponseType::STATS);
    message.extend_from_slice(&client_count.to_be_bytes());
    message.extend_from_slice(&requests_per_second.to_be_bytes());
    message
}

fn calculate_requests_per_second(request_tracker: &RequestTracker) -> f32 {
    let now = Instant::now();
    let interval_start = now - STATS_WINDOW_INTERVAL;

    // clean up old requests
    let mut tracker = request_tracker.write().unwrap();
    while let Some(&(front_time, _)) = tracker.front() {
        if front_time < interval_start {
            tracker.pop_front();
        } else {
            break;
        }
    }

    // calculate requests per second
    let total_requests: u32 = tracker.iter().map(|(_, count)| count).sum();
    (total_requests as f32) / STATS_WINDOW
}
fn parse_message(
    canvas: &SharedCanvas,
    clients: &Clients,
    request_tracker: &RequestTracker,
    data: &Bytes,
) -> ResponseType {
    if data.is_empty() {
        return ResponseType::Error(ErrorCode::InvalidMessageType);
    }

    // track this request
    request_tracker
        .write()
        .unwrap()
        .push_back((Instant::now(), 1));

    let mut cursor = Cursor::new(data);
    let msg_type = cursor.get_u8();

    match MessageType::try_from(msg_type) {
        Ok(MessageType::SetPixel) => {
            // Format: [type:1][coords:3][rgb:3] = 7 bytes
            if data.len() < 7 {
                return ResponseType::Error(ErrorCode::InvalidMessageType);
            }

            let coord_bytes = [cursor.get_u8(), cursor.get_u8(), cursor.get_u8()];
            let (x, y, skip_sender) = unpack_coordinates(&coord_bytes);

            let rgb_bytes = [cursor.get_u8(), cursor.get_u8(), cursor.get_u8()];
            let color = unpack_rgb(&rgb_bytes);

            canvas
                .write()
                .unwrap()
                .set_pixel(x as usize, y as usize, color);

            ResponseType::Broadcast {
                x,
                y,
                color,
                skip_sender,
            }
        }
        Ok(MessageType::GetPixel) => {
            // Format: [type:1][coords:3] = 4 bytes
            if data.len() < 4 {
                return ResponseType::Error(ErrorCode::InvalidMessageType);
            }

            let coord_bytes = [cursor.get_u8(), cursor.get_u8(), cursor.get_u8()];
            let (x, y, _) = unpack_coordinates(&coord_bytes);

            let pixel_color = canvas.read().unwrap().get_pixel(x as usize, y as usize);

            match pixel_color {
                Some(color) => ResponseType::PixelColor(color),
                None => ResponseType::Error(ErrorCode::OutOfBounds),
            }
        }
        Ok(MessageType::GetAllPixels) => {
            ResponseType::GetAllPixels(canvas.read().unwrap().pixels.clone())
        }
        Ok(MessageType::GetStats) => {
            let client_count = clients.read().unwrap().len() as u32;
            let requests_per_second = calculate_requests_per_second(request_tracker);
            ResponseType::Stats {
                client_count,
                requests_per_second,
            }
        }
        Err(_) => ResponseType::Error(ErrorCode::InvalidMessageType),
    }
}

#[tokio::main]
async fn main() {
    let canvas: SharedCanvas = Arc::new(RwLock::new(Canvas::new(CANVAS_WIDTH, CANVAS_HEIGHT)));
    let listener = TcpListener::bind(ADDRESS).await.unwrap();
    let clients: Clients = Arc::new(RwLock::new(HashMap::new()));
    let request_tracker: RequestTracker = Arc::new(RwLock::new(VecDeque::new()));
    let mut id_counter = 0;

    load_canvas(&canvas);

    // save task
    let canvas_for_save = canvas.clone();
    tokio::spawn(async move {
        let mut interval = interval(SAVE_INTERVAL);
        loop {
            interval.tick().await;
            save_canvas(&canvas_for_save);
        }
    });

    // stats broadcast task
    let clients_for_stats = clients.clone();
    let request_tracker_for_stats = request_tracker.clone();
    tokio::spawn(async move {
        let mut interval = interval(STATS_INTERVAL);
        loop {
            interval.tick().await;

            let client_count = clients_for_stats.read().unwrap().len() as u32;
            let requests_per_second = calculate_requests_per_second(&request_tracker_for_stats);
            let stats_msg = create_stats_response(client_count, requests_per_second);

            // broadcast stats to all clients
            for (_, tx) in clients_for_stats.read().unwrap().iter() {
                if !tx.is_closed() {
                    let _ = tx.send(Message::Binary(stats_msg.clone().into()));
                }
            }
        }
    });

    // shutdown handler
    let canvas_for_shutdown = canvas.clone();
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
        save_canvas(&canvas_for_shutdown);
        println!("\nCanvas saved! Exiting...");
        std::process::exit(0);
    });

    println!("Server running on ws://{ADDRESS}");

    // main loop
    while let Ok((stream, _)) = listener.accept().await {
        let ws_stream = accept_async(stream).await.unwrap();
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let id = id_counter;
        id_counter += 1;

        clients.write().unwrap().insert(id, tx.clone());

        let clients_ref = clients.clone();
        let canvas_ref = canvas.clone();
        let request_tracker_ref = request_tracker.clone();

        tokio::spawn(async move {
            while let Some(Ok(msg)) = ws_receiver.next().await {
                if let Message::Binary(data) = msg {
                    let action =
                        parse_message(&canvas_ref, &clients_ref, &request_tracker_ref, &data);

                    match action {
                        ResponseType::Broadcast {
                            x,
                            y,
                            color,
                            skip_sender,
                        } => {
                            // Broadcast message: [type:1][coords:3][rgb:3] = 7 bytes
                            let broadcast_msg = create_set_pixel_broadcast(x, y, color);

                            let mut broadcast_count = 0;
                            // send to all clients
                            for (client_id, other_tx) in clients_ref.read().unwrap().iter() {
                                if other_tx.is_closed() {
                                    // close the connection if the channel is closed
                                    clients_ref.write().unwrap().remove(client_id);
                                    continue; // skip it
                                }
                                if skip_sender && *client_id == id {
                                    // if flag is true, skip sending to the sender
                                    continue;
                                }
                                let _ =
                                    other_tx.send(Message::Binary(broadcast_msg.clone().into()));
                                broadcast_count += 1;
                            }

                            // track the broadcast
                            if broadcast_count > 0 {
                                request_tracker_ref
                                    .write()
                                    .unwrap()
                                    .push_back((Instant::now(), broadcast_count));
                            }
                        }
                        ResponseType::GetAllPixels(pixels) => {
                            // Send all pixels response: [type:1][pixels:3*n] = 1 + 3*n bytes
                            let mut response_msg = vec![ResponseType::GET_ALL_PIXELS];
                            for pixel in pixels {
                                response_msg.extend_from_slice(&pack_rgb(pixel));
                            }
                            let _ = tx.send(Message::Binary(response_msg.into()));
                        }
                        ResponseType::PixelColor(color) => {
                            // Send pixel color response: [type:1][rgb:3] = 4 bytes
                            let response_msg = create_pixel_response(color);
                            let _ = tx.send(Message::Binary(response_msg.into()));
                        }
                        ResponseType::Stats {
                            client_count,
                            requests_per_second,
                        } => {
                            // Send stats response: [type:1][client_count:4][rps:4] = 9 bytes
                            let stats_msg =
                                create_stats_response(client_count, requests_per_second);
                            let _ = tx.send(Message::Binary(stats_msg.into()));
                        }
                        ResponseType::Error(error_code) => {
                            // Send error response: [type:1][error:1] = 2 bytes
                            let error_msg = create_error_response(error_code);
                            let _ = tx.send(Message::Binary(error_msg.into()));
                        }
                    }
                }
            }
            clients_ref.write().unwrap().remove(&id);
        });

        // router
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if ws_sender.send(msg).await.is_err() {
                    break;
                }
            }
        });
    }
}
