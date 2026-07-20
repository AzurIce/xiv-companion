use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use serde_json::json;
use wasm_bindgen::{JsCast, closure::Closure};
use wasm_bindgen_futures::spawn_local;
use web_sys::{ErrorEvent, Event, MessageEvent, WebSocket};
use xiv_companion::inventory_bridge_protocol::{
    InventoryContainerAvailability, InventoryContainerDirectory, InventoryContainerSnapshot,
    ParsedInventoryBridgeMessage, parse_inventory_bridge_message,
};

#[derive(Clone, Debug)]
pub enum InventoryBridgeUpdate {
    Connected,
    RefreshStarted,
    RefreshComplete,
    Directory(InventoryContainerDirectory),
    Container(InventoryContainerSnapshot),
    WaitingForLogin,
    Disconnected,
    Error(String),
}

pub struct InventoryBridgeConnection {
    socket: WebSocket,
    _on_open: Closure<dyn FnMut(Event)>,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    _on_error: Closure<dyn FnMut(ErrorEvent)>,
    _on_close: Closure<dyn FnMut(Event)>,
}

impl InventoryBridgeConnection {
    pub fn connect(
        url: &str,
        on_update: impl FnMut(InventoryBridgeUpdate) + 'static,
    ) -> Result<Rc<Self>, String> {
        let url = url.trim();
        if !url.starts_with("ws://127.0.0.1:") && !url.starts_with("ws://localhost:") {
            return Err("桥接地址必须使用本机 ws://127.0.0.1 或 ws://localhost".to_string());
        }

        let socket = WebSocket::new(url).map_err(js_error)?;
        let callback: Rc<RefCell<dyn FnMut(InventoryBridgeUpdate)>> =
            Rc::new(RefCell::new(on_update));
        let request_sequence = Rc::new(Cell::new(1));
        let failed = Rc::new(Cell::new(false));
        let pending_containers = Rc::new(RefCell::new(HashSet::<String>::new()));

        let open_socket = socket.clone();
        let open_callback = callback.clone();
        let open_sequence = request_sequence.clone();
        let on_open = Closure::wrap(Box::new(move |_event: Event| {
            open_callback.borrow_mut()(InventoryBridgeUpdate::Connected);
            request_directory(&open_socket, &open_sequence, &open_callback);
        }) as Box<dyn FnMut(Event)>);
        socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));

        let message_socket = socket.clone();
        let message_callback = callback.clone();
        let message_sequence = request_sequence.clone();
        let message_pending = pending_containers;
        let on_message = Closure::wrap(Box::new(move |event: MessageEvent| {
            let Some(text) = event.data().as_string() else {
                return;
            };
            let Some(parsed) = parse_inventory_bridge_message(&text) else {
                return;
            };
            match parsed {
                ParsedInventoryBridgeMessage::Directory(directory) => {
                    message_callback.borrow_mut()(InventoryBridgeUpdate::RefreshStarted);
                    let requested = directory
                        .containers
                        .iter()
                        .filter(|container| {
                            container.availability != InventoryContainerAvailability::NotLoaded
                        })
                        .map(|container| container.container_id.clone())
                        .collect::<HashSet<_>>();
                    *message_pending.borrow_mut() = requested.clone();
                    for container_id in &requested {
                        request_container(
                            &message_socket,
                            &message_sequence,
                            &message_callback,
                            container_id,
                        );
                    }
                    message_callback.borrow_mut()(InventoryBridgeUpdate::Directory(directory));
                    if requested.is_empty() {
                        message_callback.borrow_mut()(InventoryBridgeUpdate::RefreshComplete);
                    }
                }
                ParsedInventoryBridgeMessage::Container(container) => {
                    let container_id = container.container_id.clone();
                    message_callback.borrow_mut()(InventoryBridgeUpdate::Container(container));
                    let removed = { message_pending.borrow_mut().remove(&container_id) };
                    let refresh_complete = removed && message_pending.borrow().is_empty();
                    if refresh_complete {
                        message_callback.borrow_mut()(InventoryBridgeUpdate::RefreshComplete);
                    }
                }
                ParsedInventoryBridgeMessage::SessionLogin => {
                    request_directory(&message_socket, &message_sequence, &message_callback);
                }
                ParsedInventoryBridgeMessage::SessionLogout => {
                    message_callback.borrow_mut()(InventoryBridgeUpdate::WaitingForLogin);
                }
                ParsedInventoryBridgeMessage::Error {
                    code, message: _, ..
                } if code == "not_logged_in" => {
                    message_callback.borrow_mut()(InventoryBridgeUpdate::WaitingForLogin);
                }
                ParsedInventoryBridgeMessage::Error {
                    code, message: _, ..
                } if code == "not_ready" => {
                    message_callback.borrow_mut()(InventoryBridgeUpdate::WaitingForLogin);
                    let retry_socket = message_socket.clone();
                    let retry_sequence = message_sequence.clone();
                    let retry_callback = message_callback.clone();
                    spawn_local(async move {
                        gloo_timers::future::TimeoutFuture::new(1_000).await;
                        if retry_socket.ready_state() == WebSocket::OPEN {
                            request_directory(&retry_socket, &retry_sequence, &retry_callback);
                        }
                    });
                }
                ParsedInventoryBridgeMessage::Error { code, .. }
                    if code == "container_not_found" =>
                {
                    request_directory(&message_socket, &message_sequence, &message_callback);
                }
                ParsedInventoryBridgeMessage::Error { message, .. } => {
                    message_callback.borrow_mut()(InventoryBridgeUpdate::Error(message));
                }
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

        let error_callback = callback.clone();
        let error_failed = failed.clone();
        let on_error = Closure::wrap(Box::new(move |event: ErrorEvent| {
            error_failed.set(true);
            let message = if event.message().is_empty() {
                "无法连接 API Bridge".to_string()
            } else {
                event.message()
            };
            error_callback.borrow_mut()(InventoryBridgeUpdate::Error(message));
        }) as Box<dyn FnMut(ErrorEvent)>);
        socket.set_onerror(Some(on_error.as_ref().unchecked_ref()));

        let close_callback = callback;
        let close_failed = failed;
        let on_close = Closure::wrap(Box::new(move |_event: Event| {
            if !close_failed.get() {
                close_callback.borrow_mut()(InventoryBridgeUpdate::Disconnected);
            }
        }) as Box<dyn FnMut(Event)>);
        socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));

        Ok(Rc::new(Self {
            socket,
            _on_open: on_open,
            _on_message: on_message,
            _on_error: on_error,
            _on_close: on_close,
        }))
    }
}

fn request_directory(
    socket: &WebSocket,
    sequence: &Cell<u64>,
    callback: &Rc<RefCell<dyn FnMut(InventoryBridgeUpdate)>>,
) {
    let id = next_request_id(sequence, "inventory-directory");
    send(
        socket,
        callback,
        json!({ "id": id, "method": "inventory.containers", "params": {} }),
    );
}

fn request_container(
    socket: &WebSocket,
    sequence: &Cell<u64>,
    callback: &Rc<RefCell<dyn FnMut(InventoryBridgeUpdate)>>,
    container_id: &str,
) {
    let id = next_request_id(sequence, "inventory-container");
    send(
        socket,
        callback,
        json!({
            "id": id,
            "method": "inventory.container",
            "params": { "containerId": container_id },
        }),
    );
}

fn next_request_id(sequence: &Cell<u64>, prefix: &str) -> String {
    let value = sequence.get();
    sequence.set(value.wrapping_add(1));
    format!("{prefix}-{value}")
}

fn send(
    socket: &WebSocket,
    callback: &Rc<RefCell<dyn FnMut(InventoryBridgeUpdate)>>,
    value: serde_json::Value,
) {
    if let Err(error) = socket.send_with_str(&value.to_string()) {
        callback.borrow_mut()(InventoryBridgeUpdate::Error(js_error(error)));
    }
}

impl Drop for InventoryBridgeConnection {
    fn drop(&mut self) {
        self.socket.set_onopen(None);
        self.socket.set_onmessage(None);
        self.socket.set_onerror(None);
        self.socket.set_onclose(None);
        let _ = self.socket.close();
    }
}

fn js_error(value: wasm_bindgen::JsValue) -> String {
    value
        .as_string()
        .unwrap_or_else(|| "浏览器 WebSocket 操作失败".to_string())
}
